"""
ROS2 Information Collector
Gathers all ROS2 environment, packages, nodes, topics, services, actions
"""

import glob
import os
import shutil
import subprocess
from typing import Dict, List, Optional


# Known ROS2 distro release dates and EOL info
ROS2_DISTROS = {
    "foxy":    {"full": "Foxy Fitzroy",      "eol": "2023-05", "lts": False},
    "galactic":{"full": "Galactic Geochelone","eol": "2022-11", "lts": False},
    "humble":  {"full": "Humble Hawksbill",   "eol": "2027-05", "lts": True},
    "iron":    {"full": "Iron Irwini",         "eol": "2024-11", "lts": False},
    "jazzy":   {"full": "Jazzy Jalisco",       "eol": "2029-05", "lts": True},
    "kilted":  {"full": "Kilted Kaiju",        "eol": "2026-11", "lts": False},
    "rolling": {"full": "Rolling Ridley",      "eol": "N/A",     "lts": False},
}

def _run(cmd: list, timeout: int = 5) -> str:
    """Execute a ROS2 CLI command with timeout protection.
    
    Silently handles errors and timeouts to ensure robustness. This function
    is critical because ROS2 discovery can be slow on systems with many nodes.
    
    Args:
        cmd: Command and arguments as list (e.g., ["ros2", "node", "list"])
        timeout: Maximum seconds to wait for command completion
        
    Returns:
        Command stdout as string, or empty string if command times out or fails
    """
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            env={**os.environ}
        )
        return result.stdout.strip()
    except subprocess.TimeoutExpired:
        # ROS2 discovery timeout is expected on slow systems or with many nodes
        return ""
    except Exception:
        # Command not found, permission denied, etc. - fail gracefully
        return ""


def is_ros2_available() -> bool:
    return shutil.which("ros2") is not None


def get_distro() -> Optional[str]:
    """Get ROS2 distro name."""
    distro = os.environ.get("ROS_DISTRO")
    if distro:
        return distro

    # Try sourced setup files
    for path in ["/opt/ros/*/setup.bash"]:
        matches = glob.glob(path)
        if matches:
            # Extract distro from path like /opt/ros/humble/setup.bash
            for m in matches:
                parts = m.split("/")
                if "ros" in parts:
                    idx = parts.index("ros")
                    if idx + 1 < len(parts):
                        return parts[idx + 1]

    return None

def get_distro_info(distro: Optional[str]) -> dict:
    """Get metadata about a specific distro."""
    if not distro:
        return {}
    key = distro.lower()
    info = ROS2_DISTROS.get(key, {
        "full": distro.capitalize(),
        "eol": "Unknown",
        "lts": False
    })
    return {**info, "name": distro}


def get_dds_implementation() -> str:
    """Detect DDS middleware being used."""
    rmw = os.environ.get("RMW_IMPLEMENTATION", "")
    if rmw:
        return rmw

    # Try to detect from installed packages
    distro = get_distro()
    if distro:
        checks = [
            (f"/opt/ros/{distro}/lib/librmw_fastrtps_cpp.so", "rmw_fastrtps_cpp (Fast-DDS)"),
            (f"/opt/ros/{distro}/lib/librmw_cyclonedds_cpp.so", "rmw_cyclonedds_cpp (Cyclone DDS)"),
            (f"/opt/ros/{distro}/lib/librmw_connextdds.so", "rmw_connextdds (Connext DDS)"),
        ]
        for path, name in checks:
            if os.path.exists(path):
                return name

    return "Default (Fast-DDS)"


def get_ros2_packages(distro: Optional[str]) -> dict:
    """Get installed ROS2 packages info."""
    result = {
        "total": 0,
        "categories": {},
        "notable": [],
    }

    if not distro:
        return result

    # Count packages in /opt/ros/{distro}
    ros_share = f"/opt/ros/{distro}/share"
    if os.path.isdir(ros_share):
        try:
            packages = [d for d in os.listdir(ros_share) if os.path.isdir(os.path.join(ros_share, d))]
            result["total"] = len(packages)

            # Categorize notable packages
            categories = {
                "Core": ["rclcpp", "rclpy", "rcl", "rmw", "rosidl"],
                "Navigation": ["nav2", "nav_msgs", "costmap_2d", "robot_localization"],
                "Perception": ["sensor_msgs", "image_transport", "cv_bridge", "pcl_ros"],
                "Simulation": ["gazebo_ros", "ros_gz", "ignition"],
                "MoveIt": ["moveit", "moveit_core", "moveit_ros"],
                "Control": ["ros2_control", "controller_manager", "joint_trajectory_controller"],
                "TF": ["tf2", "tf2_ros", "tf2_geometry_msgs"],
                "Visualization": ["rviz2", "rqt", "foxglove_bridge"],
                "Communication": ["rosbridge_server", "action_tutorials", "lifecycle"],
            }

            installed_cats = {}
            for cat, pkgs in categories.items():
                found = [p for p in pkgs if any(p in x for x in packages)]
                if found:
                    installed_cats[cat] = found

            result["categories"] = installed_cats
        except PermissionError:
            pass

    return result


def get_active_nodes(timeout: int = 3) -> List[str]:
    """Get list of currently active ROS2 nodes."""
    if not is_ros2_available():
        return []
    out = _run(["ros2", "node", "list"], timeout=timeout)
    if not out or "ERROR" in out.upper():
        return []
    return [n for n in out.split("\n") if n.strip()]


def get_active_topics(timeout: int = 3) -> List[Dict[str, str]]:
    """Get list of active ROS2 topics with types."""
    if not is_ros2_available():
        return []
    out = _run(["ros2", "topic", "list", "-t"], timeout=timeout)
    if not out:
        return []
    topics = []
    for line in out.split("\n"):
        line = line.strip()
        if not line:
            continue
        # Format: /topic_name [msg/type]
        if "[" in line:
            name = line.split("[")[0].strip()
            msg_type = line.split("[")[1].rstrip("]").strip()
            topics.append({"name": name, "type": msg_type})
        else:
            topics.append({"name": line, "type": "Unknown"})
    return topics


def get_active_services(timeout: int = 3) -> List[str]:
    """Get list of active ROS2 services."""
    if not is_ros2_available():
        return []
    out = _run(["ros2", "service", "list"], timeout=timeout)
    if not out:
        return []
    return [s for s in out.split("\n") if s.strip()]


def get_active_actions(timeout: int = 3) -> List[str]:
    """Get list of active ROS2 actions."""
    if not is_ros2_available():
        return []
    out = _run(["ros2", "action", "list"], timeout=timeout)
    if not out:
        return []
    return [a for a in out.split("\n") if a.strip()]


def get_ros2_environment() -> Dict[str, str]:
    """Get important ROS2 environment variables."""
    important_vars = [
        "ROS_DISTRO",
        "ROS_VERSION",
        "ROS_DOMAIN_ID",
        "ROS_LOCALHOST_ONLY",
        "RMW_IMPLEMENTATION",
        "AMENT_PREFIX_PATH",
        "COLCON_PREFIX_PATH",
        "ROS_PYTHON_VERSION",
        "CYCLONEDDS_URI",
        "FASTRTPS_DEFAULT_PROFILES_FILE",
        "ROS_AUTOMATIC_DISCOVERY_RANGE",
        "ROS_STATIC_PEERS",
    ]
    result = {}
    for var in important_vars:
        val = os.environ.get(var)
        if val:
            # Truncate very long values
            if len(val) > 80:
                val = val[:77] + "..."
            result[var] = val
    return result


def get_domain_id() -> str:
    return os.environ.get("ROS_DOMAIN_ID", "0 (default)")


def check_localhost_only() -> bool:
    return os.environ.get("ROS_LOCALHOST_ONLY", "0") == "1"


def get_workspace_source_path() -> Optional[str]:
    """Check if a workspace is currently sourced via COLCON_PREFIX_PATH."""
    colcon_path = os.environ.get("COLCON_PREFIX_PATH", "")
    if colcon_path:
        # First path is usually the local workspace install
        paths = colcon_path.split(":")
        for p in paths:
            if "/opt/ros" not in p and os.path.exists(p):
                return p
    return None


def get_ros2_log_dir() -> str:
    """Get the ROS2 log directory."""
    home = os.path.expanduser("~")
    ros_log = os.path.join(home, ".ros", "log")
    if os.path.exists(ros_log):
        return ros_log
    return os.path.join(home, ".ros", "log")


def check_ros2_update_available(distro: Optional[str]) -> Optional[str]:
    """Check if there are pending apt updates for ROS2 packages."""
    if not shutil.which("apt"):
        return None
    try:
        out = subprocess.run(
            ["apt", "list", "--upgradable"],
            capture_output=True, text=True, timeout=10
        ).stdout
        ros_updates = [
            line.split("/")[0]
            for line in out.split("\n")
            if "ros" in line.lower() and "upgradable" in line.lower()
        ]
        if ros_updates:
            return f"{len(ros_updates)} package(s) upgradable"
        return "Up to date"
    except Exception:
        return None


def get_last_ros2_update(distro: Optional[str]) -> Optional[str]:
    """Determine when ROS2 packages were last updated/installed."""
    from datetime import datetime

    # Method 1: check apt history log
    try:
        history = "/var/log/apt/history.log"
        if os.path.exists(history):
            with open(history) as f:
                content = f.read()
            # Look for ros-related entries
            last_date = None
            for block in content.split("\n\n"):
                if "ros-" in block.lower() or (distro and distro in block.lower()):
                    for line in block.split("\n"):
                        if line.startswith("Start-Date:"):
                            date_str = line.replace("Start-Date:", "").strip()
                            try:
                                last_date = datetime.strptime(date_str, "%Y-%m-%d  %H:%M:%S")
                            except Exception:
                                pass
            if last_date:
                delta = datetime.now() - last_date
                if delta.days == 0:
                    return f"Today ({last_date.strftime('%H:%M')})"
                elif delta.days == 1:
                    return f"Yesterday ({last_date.strftime('%Y-%m-%d')})"
                elif delta.days < 30:
                    return f"{delta.days} days ago ({last_date.strftime('%Y-%m-%d')})"
                else:
                    return f"{last_date.strftime('%Y-%m-%d')} ({delta.days} days ago)"
    except Exception:
        pass

    # Method 2: check modification time of /opt/ros/<distro>
    if distro:
        ros_path = f"/opt/ros/{distro}"
        if os.path.exists(ros_path):
            try:
                mtime = os.path.getmtime(ros_path)
                dt = datetime.fromtimestamp(mtime)
                delta = datetime.now() - dt
                if delta.days < 30:
                    return f"{delta.days} days ago ({dt.strftime('%Y-%m-%d')})"
                return dt.strftime("%Y-%m-%d")
            except Exception:
                pass

    return None


def collect_all(
    check_live: bool = True,
    check_updates: bool = True,
    live_timeout: int = 3,
) -> dict:
    """Collect all ROS2 information."""
    distro = get_distro()
    distro_info = get_distro_info(distro)

    data = {
        "available": is_ros2_available(),
        "distro": distro,
        "distro_info": distro_info,
        "dds": get_dds_implementation(),
        "domain_id": get_domain_id(),
        "localhost_only": check_localhost_only(),
        "packages": get_ros2_packages(distro),
        "environment": get_ros2_environment(),
        "workspace_source": get_workspace_source_path(),
        "log_dir": get_ros2_log_dir(),
        "last_updated": get_last_ros2_update(distro),
        "nodes": [],
        "topics": [],
        "services": [],
        "actions": [],
        "updates": None,
    }

    if check_live:
        data["nodes"] = get_active_nodes(live_timeout)
        data["topics"] = get_active_topics(live_timeout)
        data["services"] = get_active_services(live_timeout)
        data["actions"] = get_active_actions(live_timeout)

    if check_updates:
        data["updates"] = check_ros2_update_available(distro)

    return data
