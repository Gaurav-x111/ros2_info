"""
ROS2 Info — Bootstrap
Auto-sources ROS2, installs missing packages, and prepares the environment.
"""

import glob
import os
import shutil
import subprocess
import sys


# ── Auto-source ROS2 ──────────────────────────────────────────────────────────

def find_ros2_setup() -> str | None:
    """Find the best available ROS2 setup.bash."""
    # Already sourced
    if os.environ.get("ROS_DISTRO"):
        return None

    # Prefer LTS distros first, then others
    preferred = ["jazzy", "humble", "kilted", "iron", "rolling", "foxy", "galactic"]
    for distro in preferred:
        path = f"/opt/ros/{distro}/setup.bash"
        if os.path.exists(path):
            return path

    # Wildcard fallback
    matches = glob.glob("/opt/ros/*/setup.bash")
    if matches:
        return sorted(matches)[-1]   # pick latest alphabetically

    return None


def source_ros2(setup_bash: str) -> bool:
    """
    Source a ROS2 setup.bash and inject the env vars into the current process.
    Returns True if successful.
    """
    try:
        cmd = f"bash -c 'source {setup_bash} && env'"
        result = subprocess.run(
            cmd, shell=True, capture_output=True, text=True, timeout=15
        )
        if result.returncode != 0:
            return False
        new_env = {}
        for line in result.stdout.split("\n"):
            if "=" in line:
                k, _, v = line.partition("=")
                new_env[k] = v
        os.environ.update(new_env)
        return True
    except Exception:
        return False


def ensure_ros2_sourced(console=None) -> bool:
    """Ensure ROS2 is sourced in the current process. Returns True if sourced."""
    if os.environ.get("ROS_DISTRO"):
        return True  # already sourced

    setup = find_ros2_setup()
    if not setup:
        if console:
            console.print(
                "  [bold yellow]⚠  ROS2 not found at /opt/ros/*[/]  "
                "[dim]Install ROS2 first.[/]"
            )
        return False

    ok = source_ros2(setup)
    distro = os.environ.get("ROS_DISTRO", "unknown")
    if ok and console:
        console.print(
            f"  [bold green]✓ Auto-sourced:[/] [cyan]{setup}[/]  "
            f"[dim](ROS2 {distro})[/]"
        )
    return ok


# ── Auto-source workspace ─────────────────────────────────────────────────────

def find_workspace_setup() -> str | None:
    """Find a local workspace install/setup.bash to auto-source."""
    if os.environ.get("COLCON_PREFIX_PATH"):
        return None   # already sourced

    home = os.path.expanduser("~")
    candidates = [
        os.path.join(home, "ros2_ws", "install", "setup.bash"),
        os.path.join(home, "dev_ws",  "install", "setup.bash"),
        os.path.join(home, "colcon_ws", "install", "setup.bash"),
        os.path.join(home, "robot_ws", "install", "setup.bash"),
        "/workspace/install/setup.bash",
        "/ws/install/setup.bash",
    ]
    # Also check current directory and parent
    cwd = os.getcwd()
    for _ in range(3):
        candidate = os.path.join(cwd, "install", "setup.bash")
        if os.path.exists(candidate):
            return candidate
        cwd = os.path.dirname(cwd)

    for c in candidates:
        if os.path.exists(c):
            return c
    return None


def source_workspace(setup_bash: str, console=None) -> bool:
    """Source local workspace setup."""
    ok = source_ros2(setup_bash)   # same mechanism works for workspace
    if ok and console:
        console.print(
            f"  [bold green]✓ Workspace sourced:[/] [cyan]{setup_bash}[/]"
        )
    return ok


# ── Dependency check ──────────────────────────────────────────────────────────

REQUIRED_PYTHON = ["click", "rich", "psutil", "flask", "climage"]


def auto_install_missing(console=None):
    """Install any missing Python packages via uv or pip."""
    try:
        import importlib
        missing = []
        for pkg in REQUIRED_PYTHON:
            try:
                importlib.import_module(pkg)
            except ImportError:
                missing.append(pkg)

        if not missing:
            return

        if console:
            console.print(f"  [yellow]Installing missing packages: {', '.join(missing)}[/]")

        installer = shutil.which("uv")
        if installer:
            cmd = [installer, "pip", "install"] + missing
        else:
            cmd = [sys.executable, "-m", "pip", "install"] + missing

        # Add timeout to prevent hanging on slow/unresponsive installers
        subprocess.run(cmd, check=True, capture_output=True, timeout=300)
        if console:
            console.print(f"  [green]✓ Installed: {', '.join(missing)}[/]")
    except Exception as e:
        if console:
            console.print(f"  [red]Auto-install failed: {e}[/]")


# ── Full bootstrap ─────────────────────────────────────────────────────────────

def bootstrap(console=None) -> dict:
    """
    Run full bootstrap:
    1. Auto-install missing Python deps
    2. Auto-source ROS2 if not sourced
    3. Auto-source local workspace if found
    Returns a dict with sourced info.
    """
    result = {"ros2_sourced": False, "workspace_sourced": False, "distro": None}

    auto_install_missing(console)

    # ROS2
    if ensure_ros2_sourced(console):
        result["ros2_sourced"] = True
        result["distro"] = os.environ.get("ROS_DISTRO")

    # Workspace
    ws_setup = find_workspace_setup()
    if ws_setup:
        if source_workspace(ws_setup, console):
            result["workspace_sourced"] = True

    return result
