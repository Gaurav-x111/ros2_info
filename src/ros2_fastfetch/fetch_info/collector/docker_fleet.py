"""
Docker / docker-compose fleet collector.

Aggregates the ROS2 node/topic/service/action graph across all running
Docker containers on the local host, regardless of DDS domain ID or Docker
network isolation. This is the complement to the SSH-based ``fleet`` collector:
where ``fleet`` reaches remote *hosts* over the network, this reaches local
*containers* via ``docker exec``.

Use case: docker-compose stacks where each subsystem runs in its own container
on a private network. Standard DDS discovery only sees one domain per process,
so a single ``ros2 node list`` never shows the whole system. This collector
runs ``ros2 node list`` *inside each container* and merges the results, tagging
every entity with the container it lives in.
"""

import shutil
import subprocess
from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import Dict, List, Optional


def docker_available() -> bool:
    """Return True if the ``docker`` CLI is present and the daemon responds."""
    if shutil.which("docker") is None:
        return False
    try:
        p = subprocess.run(
            ["docker", "info"],
            capture_output=True,
            text=True,
            timeout=8,
        )
        return p.returncode == 0
    except (subprocess.TimeoutExpired, subprocess.SubprocessError, OSError):
        return []


def list_containers() -> List[Dict[str, str]]:
    """List running Docker containers.

    Returns a list of dicts with keys: id, name, image, state.
    """
    if not docker_available():
        return []
    try:
        p = subprocess.run(
            [
                "docker",
                "ps",
                "--filter",
                "status=running",
                "--format",
                "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.State}}",
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if p.returncode != 0:
            return []
        containers = []
        for line in p.stdout.splitlines():
            line = line.strip()
            if not line:
                continue
            parts = line.split("\t")
            while len(parts) < 4:
                parts.append("")
            containers.append(
                {
                    "id": parts[0],
                    "name": parts[1],
                    "image": parts[2],
                    "state": parts[3],
                }
            )
        return containers
    except (subprocess.TimeoutExpired, subprocess.SubprocessError, OSError):
        return []


def _exec_in_container(container_id: str, cmd: str, timeout: int = 5) -> Optional[str]:
    """Run a shell command inside a container via ``docker exec``.

    Returns the stripped stdout, ``None`` on any failure (container has no
    ros2 on PATH, not running, command errored, etc.).
    """
    try:
        full = (
            "if [ -f /opt/ros/$ROS_DISTRO/setup.bash ]; then "
            "source /opt/ros/$ROS_DISTRO/setup.bash 2>/dev/null; fi; "
            "if [ -f /usr/local/setup.bash ]; then "
            "source /usr/local/setup.bash 2>/dev/null; fi; " + cmd
        )
        p = subprocess.run(
            ["docker", "exec", "-i", container_id, "bash", "-c", full],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        if p.returncode != 0:
            return None
        return p.stdout.strip()
    except (subprocess.TimeoutExpired, subprocess.SubprocessError, OSError):
        return None


def collect_container_graph(container: Dict[str, str], timeout: int = 5) -> Dict:
    """Collect the ROS2 graph from a single container.

    Returns a dict describing the container and the ROS2 entities discovered
    inside it (empty lists if the container is not a ROS2 environment).
    """
    cid = container["id"]
    ros_distro = _exec_in_container(cid, "echo $ROS_DISTRO", timeout=timeout)
    ros_domain = _exec_in_container(cid, "echo $ROS_DOMAIN_ID", timeout=timeout)

    is_ros = ros_distro is not None and ros_distro != ""

    nodes_raw = _exec_in_container(cid, "ros2 node list 2>/dev/null", timeout=timeout) if is_ros else None
    topics_raw = _exec_in_container(cid, "ros2 topic list -t 2>/dev/null", timeout=timeout) if is_ros else None
    services_raw = _exec_in_container(cid, "ros2 service list 2>/dev/null", timeout=timeout) if is_ros else None
    actions_raw = _exec_in_container(cid, "ros2 action list 2>/dev/null", timeout=timeout) if is_ros else None

    nodes = [n.strip() for n in (nodes_raw or "").splitlines() if n.strip()]
    topics = []
    for line in (topics_raw or "").splitlines():
        line = line.strip()
        if not line:
            continue
        if "[" in line:
            name = line.split("[")[0].strip()
            msg_type = line.split("[")[1].rstrip("]").strip()
            topics.append({"name": name, "type": msg_type})
        else:
            topics.append({"name": line, "type": "Unknown"})
    services = [s.strip() for s in (services_raw or "").splitlines() if s.strip()]
    actions = [a.strip() for a in (actions_raw or "").splitlines() if a.strip()]

    return {
        "id": cid,
        "name": container["name"],
        "image": container["image"],
        "state": container["state"],
        "is_ros": is_ros,
        "ros_distro": ros_distro or None,
        "ros_domain_id": ros_domain or "0",
        "nodes": [{"name": n, "container": container["name"]} for n in nodes],
        "topics": topics,
        "services": services,
        "actions": actions,
    }


def collect_docker_fleet(timeout: int = 5, max_workers: int = 16) -> Dict:
    """Aggregate the ROS2 graph across all running containers.

    Returns a dict with per-container details plus the merged, de-duplicated
    system-wide view (unique topics keyed by name, total node count, etc.).
    """
    containers = list_containers()
    per_container: List[Dict] = []

    if containers:
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = {
                executor.submit(collect_container_graph, c, timeout): c
                for c in containers
            }
            for future in as_completed(futures):
                try:
                    per_container.append(future.result())
                except Exception:
                    c = futures[future]
                    per_container.append({**c, "is_ros": False, "nodes": [], "topics": []})

    per_container.sort(key=lambda x: x.get("name", ""))

    # Merge unique topics across containers (by name + type).
    merged_topics: Dict[str, Dict] = {}
    total_nodes = 0
    ros_containers = 0
    for c in per_container:
        if c.get("is_ros"):
            ros_containers += 1
            total_nodes += len(c.get("nodes", []))
        for t in c.get("topics", []):
            key = f"{t['name']}|{t['type']}"
            if key not in merged_topics:
                merged_topics[key] = {**t, "containers": [c["name"]]}
            else:
                if c["name"] not in merged_topics[key]["containers"]:
                    merged_topics[key]["containers"].append(c["name"])

    domain_ids = sorted(
        {c.get("ros_domain_id", "0") for c in per_container if c.get("is_ros")}
    )

    return {
        "docker_available": docker_available(),
        "container_count": len(containers),
        "ros_container_count": ros_containers,
        "total_nodes": total_nodes,
        "unique_topics": len(merged_topics),
        "domain_ids": domain_ids,
        "containers": per_container,
        "topics": sorted(merged_topics.values(), key=lambda x: x["name"]),
    }
