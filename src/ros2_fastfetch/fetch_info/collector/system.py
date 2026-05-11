"""
System Information Collector
Gathers OS, CPU, RAM, disk and uptime info
"""

import os
import platform
import subprocess
import shutil
from datetime import datetime, timedelta
from typing import Optional


def _run(cmd: list[str]) -> str:
    """Execute a shell command and return stdout, silently failing on errors.
    
    Args:
        cmd: Command and arguments as a list
        
    Returns:
        Command stdout as string, or empty string on failure
    """
    try:
        return subprocess.check_output(cmd, stderr=subprocess.DEVNULL, text=True).strip()
    except Exception:
        return ""


def get_hostname() -> str:
    return platform.node()


def get_os_info() -> dict:
    info = {
        "name": "Unknown",
        "version": "",
        "arch": platform.machine(),
        "kernel": platform.release(),
    }

    # Try /etc/os-release first (most Linux distros)
    if os.path.exists("/etc/os-release"):
        with open("/etc/os-release") as f:
            data = {}
            for line in f:
                line = line.strip()
                if "=" in line:
                    k, _, v = line.partition("=")
                    data[k] = v.strip('"')
        info["name"] = data.get("NAME", "Linux")
        info["version"] = data.get("VERSION_ID", data.get("VERSION", ""))
        info["codename"] = data.get("VERSION_CODENAME", "")
    elif platform.system() == "Darwin":
        info["name"] = "macOS"
        info["version"] = platform.mac_ver()[0]
    elif platform.system() == "Windows":
        info["name"] = "Windows"
        info["version"] = platform.version()

    return info


def get_uptime() -> str:
    try:
        import psutil
        boot_time = psutil.boot_time()
        uptime_seconds = (datetime.now() - datetime.fromtimestamp(boot_time)).total_seconds()
        td = timedelta(seconds=int(uptime_seconds))
        hours, remainder = divmod(td.seconds, 3600)
        minutes = remainder // 60
        days = td.days
        parts = []
        if days > 0:
            parts.append(f"{days}d")
        if hours > 0:
            parts.append(f"{hours}h")
        parts.append(f"{minutes}m")
        return " ".join(parts)
    except Exception:
        # Fallback: read /proc/uptime
        try:
            with open("/proc/uptime") as f:
                seconds = float(f.read().split()[0])
            td = timedelta(seconds=int(seconds))
            return str(td)
        except Exception:
            return "Unknown"


def get_cpu_info() -> dict:
    info = {"model": "Unknown", "cores": 1, "threads": 1, "freq_mhz": 0}
    try:
        import psutil
        info["cores"] = psutil.cpu_count(logical=False) or 1
        info["threads"] = psutil.cpu_count(logical=True) or 1
        freq = psutil.cpu_freq()
        if freq:
            info["freq_mhz"] = round(freq.current)
    except Exception:
        pass

    # CPU model name
    if os.path.exists("/proc/cpuinfo"):
        with open("/proc/cpuinfo") as f:
            for line in f:
                if "model name" in line:
                    info["model"] = line.split(":")[1].strip()
                    break
    elif platform.system() == "Darwin":
        info["model"] = _run(["sysctl", "-n", "machdep.cpu.brand_string"]) or "Apple Silicon"
    return info


def get_memory_info() -> dict:
    try:
        import psutil
        vm = psutil.virtual_memory()
        return {
            "total_gb": round(vm.total / 1e9, 1),
            "used_gb": round(vm.used / 1e9, 1),
            "available_gb": round(vm.available / 1e9, 1),
            "percent": vm.percent,
        }
    except Exception:
        return {"total_gb": 0, "used_gb": 0, "available_gb": 0, "percent": 0}


def get_disk_info(path: str = "/") -> dict:
    try:
        import psutil
        disk = psutil.disk_usage(path)
        return {
            "total_gb": round(disk.total / 1e9, 1),
            "used_gb": round(disk.used / 1e9, 1),
            "free_gb": round(disk.free / 1e9, 1),
            "percent": disk.percent,
        }
    except Exception:
        return {"total_gb": 0, "used_gb": 0, "free_gb": 0, "percent": 0}


def get_gpu_info() -> Optional[str]:
    # Try nvidia-smi first
    if shutil.which("nvidia-smi"):
        out = _run(["nvidia-smi", "--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
        if out:
            parts = out.split(",")
            if len(parts) >= 2:
                return f"{parts[0].strip()} ({int(float(parts[1].strip()))} MiB)"
            return out

    # Try lspci
    out = _run(["lspci"])
    for line in out.split("\n"):
        if any(k in line.upper() for k in ["VGA", "3D", "DISPLAY"]):
            # Extract just the device name
            if ":" in line:
                return line.split(":", 2)[-1].strip()

    return None


def get_shell() -> str:
    shell = os.environ.get("SHELL", "")
    if shell:
        return os.path.basename(shell)
    return "Unknown"


def get_terminal() -> str:
    for var in ["TERM_PROGRAM", "TERM", "COLORTERM"]:
        val = os.environ.get(var)
        if val and val not in ("xterm", "xterm-256color"):
            return val
    return os.environ.get("TERM", "Unknown")


def get_python_version() -> str:
    import sys
    return f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}"


def get_temperatures() -> dict:
    try:
        import psutil
        temps = psutil.sensors_temperatures()
        if not temps:
            return {}
        result = {}
        for name, entries in temps.items():
            for entry in entries:
                if entry.current:
                    key = name
                    if entry.label:
                        key = f"{name} ({entry.label})"
                    result[key] = round(entry.current, 1)
        return result
    except Exception:
        return {}


def get_battery() -> dict:
    try:
        import psutil
        bat = psutil.sensors_battery()
        if not bat:
            return {}
        return {
            "percent": round(bat.percent, 1),
            "plugged": bat.power_plugged,
            "time_left": bat.secsleft if bat.secsleft > 0 else 0
        }
    except Exception:
        return {}


def get_network() -> dict:
    try:
        import psutil
        net = psutil.net_io_counters()
        return {
            "sent_mb": round(net.bytes_sent / 1e6, 2),
            "recv_mb": round(net.bytes_recv / 1e6, 2)
        }
    except Exception:
        return {}


def collect_all() -> dict:
    return {
        "hostname": get_hostname(),
        "os": get_os_info(),
        "uptime": get_uptime(),
        "cpu": get_cpu_info(),
        "memory": get_memory_info(),
        "disk": get_disk_info(),
        "gpu": get_gpu_info(),
        "shell": get_shell(),
        "terminal": get_terminal(),
        "python": get_python_version(),
        "temperatures": get_temperatures(),
        "battery": get_battery(),
        "network": get_network(),
    }
