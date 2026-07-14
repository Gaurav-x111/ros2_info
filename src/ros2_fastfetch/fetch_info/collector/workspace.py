"""
ROS2 Workspace Information Collector
Scans for colcon workspaces, local packages, build info
"""

from pathlib import Path
from typing import Any, Dict, List


def find_workspaces(max_depth: int = 4) -> List[str]:
    """
    Search common locations for colcon workspaces.
    A workspace is identified by having: src/, install/, build/, or .colcon/ directories.
    """
    home = Path.home()
    search_roots = [home, Path("/opt"), Path("/workspace"), Path("/ws")]
    found = []

    workspace_indicators = {"src", "install", "build", ".colcon"}

    def is_workspace(path: Path) -> bool:
        try:
            children = {p.name for p in path.iterdir() if p.is_dir()}
            # Require at least 2 colcon indicators AND a package.xml in src/
            has_indicators = len(children & workspace_indicators) >= 2
            if not has_indicators:
                return False
            # Must have at least one package.xml under src/ to be a real ROS2 workspace
            src = path / "src"
            if src.exists():
                try:
                    next(src.rglob("package.xml"))
                    return True
                except StopIteration:
                    return False
            # If no src/, fall back to just indicators (for install-only workspaces)
            return True
        except PermissionError:
            return False

    def search(path: Path, depth: int):
        if depth > max_depth:
            return
        try:
            if is_workspace(path):
                found.append(str(path))
                return  # Don't recurse into workspaces
            for child in path.iterdir():
                if child.is_dir() and not child.name.startswith('.'):
                    search(child, depth + 1)
        except (PermissionError, OSError):
            pass

    for root in search_roots:
        if root.exists():
            search(root, 0)

    # Also check common workspace paths explicitly
    common = [
        home / "ros2_ws",
        home / "dev_ws",
        home / "colcon_ws",
        home / "robot_ws",
        Path("/opt/ros2_ws"),
        Path("/workspace"),
        Path("/ws"),
    ]
    for ws in common:
        if ws.exists() and str(ws) not in found and is_workspace(ws):
            found.append(str(ws))

    return list(set(found))


def get_workspace_packages(workspace_path: str) -> Dict[str, Any]:
    """
    Get packages in a workspace's src/ directory.
    Returns package names with their build status.
    """
    ws = Path(workspace_path)
    src_dir = ws / "src"
    install_dir = ws / "install"
    build_dir = ws / "build"

    packages = []

    if src_dir.exists():
        # Find package.xml files to identify packages
        for pkg_xml in src_dir.rglob("package.xml"):
            pkg_dir = pkg_xml.parent
            pkg_name = pkg_dir.name

            # Try to get name from package.xml
            try:
                import xml.etree.ElementTree as ET
                tree = ET.parse(pkg_xml)
                root = tree.getroot()
                name_el = root.find("name")
                if name_el is not None:
                    pkg_name = name_el.text.strip()
            except Exception:
                pass

            # Check build status
            built = (install_dir / pkg_name).exists() or (build_dir / pkg_name).exists()
            packages.append({
                "name": pkg_name,
                "path": str(pkg_dir.relative_to(src_dir)),
                "built": built,
            })

    return {
        "path": workspace_path,
        "packages": packages,
        "package_count": len(packages),
        "has_install": install_dir.exists(),
        "has_build": build_dir.exists(),
        "has_src": src_dir.exists(),
    }

def get_recently_modified_packages(workspace_path: str, limit: int = 5) -> List[str]:
    """Get recently modified packages in the workspace."""
    src = Path(workspace_path) / "src"
    if not src.exists():
        return []

    packages_with_mtime = []
    for pkg_xml in src.rglob("package.xml"):
        try:
            mtime = pkg_xml.stat().st_mtime
            packages_with_mtime.append((pkg_xml.parent.name, mtime))
        except Exception:
            pass

    packages_with_mtime.sort(key=lambda x: x[1], reverse=True)
    return [name for name, _ in packages_with_mtime[:limit]]


def get_launch_count(workspace_path: str) -> int:
    """Count launch files in the workspace."""
    src = Path(workspace_path) / "src"
    if not src.exists():
        return 0
    count = 0
    for p in src.rglob("*.launch.py"):
        count += 1
    for p in src.rglob("*.launch.xml"):
        count += 1
    return count


def collect_all() -> dict:
    """Collect all workspace information."""
    workspaces = find_workspaces()
    workspace_details = []

    for ws_path in workspaces[:5]:  # Limit to 5 workspaces
        detail = get_workspace_packages(ws_path)
        detail["launches"] = get_launch_count(ws_path)
        detail["recent"] = get_recently_modified_packages(ws_path, 3)
        workspace_details.append(detail)

    return {
        "workspaces": workspace_details,
        "count": len(workspaces),
    }
