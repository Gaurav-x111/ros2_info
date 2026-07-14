import json
import os
import re
import subprocess
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field
from typing import Dict, List, Optional


@dataclass
class FleetHost:
    hostname: str
    ip: str
    port: int = 22
    username: str = "root"
    key_path: Optional[str] = None


def check_host(host: FleetHost) -> dict:
    result = {"hostname": host.hostname, "ip": host.ip, "online": False, "latency_ms": None}
    try:
        p = subprocess.run(
            ["ping", "-c", "1", "-W", "2", host.ip],
            capture_output=True,
            text=True,
            timeout=5,
        )
        if p.returncode == 0:
            result["online"] = True
            m = re.search(r"time=([0-9.]+)\s*ms", p.stdout)
            if m:
                result["latency_ms"] = round(float(m.group(1)), 1)
            else:
                result["latency_ms"] = 0.0
    except (subprocess.TimeoutExpired, subprocess.SubprocessError, OSError):
        pass
    return result


def _build_ssh_cmd(host: FleetHost, remote_cmd: str) -> List[str]:
    cmd = [
        "ssh",
        "-o", "ConnectTimeout=5",
        "-o", "StrictHostKeyChecking=no",
        "-o", "UserKnownHostsFile=/dev/null",
        "-p", str(host.port),
    ]
    if host.key_path:
        cmd.extend(["-i", host.key_path])
    cmd.append(f"{host.username}@{host.ip}")
    cmd.append(remote_cmd)
    return cmd


_REMOTE_COMMANDS = {
    "uptime": "uptime",
    "memory": 'free -h | grep Mem',
    "disk": 'df -h / | tail -1',
    "ros2_nodes": 'ros2 node list 2>/dev/null || echo "ROS2_NOT_AVAILABLE"',
    "ros_distro": 'echo $ROS_DISTRO',
}


def collect_remote_info(host: FleetHost) -> dict:
    result = {
        "hostname": host.hostname,
        "ip": host.ip,
        "reachable": False,
        "uptime": None,
        "memory": None,
        "disk": None,
        "ros2_nodes": None,
        "ros_distro": None,
        "error": None,
    }
    try:
        ping_ok = subprocess.run(
            ["ping", "-c", "1", "-W", "2", host.ip],
            capture_output=True,
            timeout=5,
        ).returncode == 0
        if not ping_ok:
            result["error"] = "Host unreachable"
            return result

        result["reachable"] = True

        for key, remote_cmd in _REMOTE_COMMANDS.items():
            try:
                cmd = _build_ssh_cmd(host, remote_cmd)
                p = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
                if p.returncode == 0:
                    output = p.stdout.strip()
                    if key == "memory":
                        result[key] = output
                    elif key == "disk":
                        result[key] = output
                    elif key == "ros2_nodes":
                        result[key] = output.split("\n") if output != "ROS2_NOT_AVAILABLE" else []
                    elif key == "ros_distro":
                        result[key] = output if output else None
                    else:
                        result[key] = output
            except (subprocess.TimeoutExpired, subprocess.SubprocessError, OSError):
                result[key] = None
    except (subprocess.TimeoutExpired, subprocess.SubprocessError, OSError) as e:
        result["error"] = str(e)
    return result


def collect_fleet(hosts: List[FleetHost], max_workers: int = 10) -> List[dict]:
    results = []
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        futures = {executor.submit(collect_remote_info, h): h for h in hosts}
        for future in as_completed(futures):
            try:
                results.append(future.result())
            except Exception as e:
                h = futures[future]
                results.append({
                    "hostname": h.hostname,
                    "ip": h.ip,
                    "reachable": False,
                    "error": str(e),
                })
    return results


def _ping_ip(ip: str) -> Optional[str]:
    try:
        p = subprocess.run(
            ["ping", "-c", "1", "-W", "1", ip],
            capture_output=True,
            timeout=3,
        )
        if p.returncode == 0:
            return ip
    except (subprocess.TimeoutExpired, subprocess.SubprocessError, OSError):
        pass
    return None


_WEBHOOK_URL = os.environ.get("ROS2_INFO_WEBHOOK_URL", "")


def _send_webhook(payload: Dict) -> None:
    if not _WEBHOOK_URL:
        return
    try:
        data = json.dumps(payload).encode()
        req = urllib.request.Request(
            _WEBHOOK_URL,
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        urllib.request.urlopen(req, timeout=10)
    except Exception:
        pass


_ALERT_THRESHOLDS = {
    "memory_percent": 85,
    "disk_percent": 85,
    "cpu_percent": 80,
}


def check_alerts(hosts_info: List[Dict]) -> List[Dict]:
    """Check fleet host data against thresholds and send webhook alerts."""
    alerts = []
    for h in hosts_info:
        if not h.get("reachable"):
            payload = {
                "type": "host_down",
                "hostname": h["hostname"],
                "ip": h["ip"],
                "timestamp": __import__("time").time(),
                "message": f"Host {h['hostname']} ({h['ip']}) is unreachable",
            }
            alerts.append(payload)
            _send_webhook(payload)
            continue

        mem_str = h.get("memory", "")
        if mem_str:
            m = re.search(r"(\d+(?:\.\d+)?)%", mem_str)
            if m and float(m.group(1)) >= _ALERT_THRESHOLDS["memory_percent"]:
                payload = {
                    "type": "high_memory",
                    "hostname": h["hostname"],
                    "ip": h["ip"],
                    "value": float(m.group(1)),
                    "threshold": _ALERT_THRESHOLDS["memory_percent"],
                    "timestamp": __import__("time").time(),
                    "message": f"{h['hostname']}: memory at {m.group(1)}%",
                }
                alerts.append(payload)
                _send_webhook(payload)

        disk_str = h.get("disk", "")
        if disk_str:
            parts = disk_str.split()
            if parts:
                m = re.search(r"(\d+(?:\.\d+)?)%", parts[-1]) if parts else None
                if m and float(m.group(1)) >= _ALERT_THRESHOLDS["disk_percent"]:
                    payload = {
                        "type": "high_disk",
                        "hostname": h["hostname"],
                        "ip": h["ip"],
                        "value": float(m.group(1)),
                        "threshold": _ALERT_THRESHOLDS["disk_percent"],
                        "timestamp": __import__("time").time(),
                        "message": f"{h['hostname']}: disk at {m.group(1)}%",
                    }
                    alerts.append(payload)
                    _send_webhook(payload)

    return alerts


def discover_hosts(subnet_prefix: str = "192.168.1.") -> List[str]:
    ips = [f"{subnet_prefix}{i}" for i in range(1, 255)]
    responsive = []
    with ThreadPoolExecutor(max_workers=20) as executor:
        futures = {executor.submit(_ping_ip, ip): ip for ip in ips}
        for future in as_completed(futures):
            result = future.result()
            if result:
                responsive.append(result)
    return sorted(responsive, key=lambda x: int(x.rsplit(".", 1)[1]))
