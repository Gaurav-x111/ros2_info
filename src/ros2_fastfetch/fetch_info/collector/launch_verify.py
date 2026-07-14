"""
ROS2 Launch File Verification Module
Analyzes launch files for common issues
"""

import os
import re
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any, Dict, List, Optional

from .workspace import get_workspace_packages


COMMON_PORTS: List[str] = ["8080", "9090", "11311", "9091", "9092"]


def _check_exists(launch_file_path: str) -> List[Dict[str, Any]]:
    results: List[Dict[str, Any]] = []
    path = Path(launch_file_path)
    if not path.exists():
        results.append({
            "severity": "error",
            "check": "file_exists",
            "message": f"Launch file not found: {launch_file_path}",
            "fix": f"Create {launch_file_path} or correct the path",
            "line": 0,
        })
    return results


def _check_file_type(launch_file_path: str) -> Optional[str]:
    path = Path(launch_file_path)
    name = path.name
    if name.endswith(".launch.py"):
        return "python"
    if name.endswith(".launch.xml") or name.endswith(".launch"):
        return "xml"
    return None


def _check_xml_launch(root: ET.Element, results: List[Dict[str, Any]]) -> None:
    for elem in root.iter("node"):
        pkg = elem.get("pkg", "")
        exec_name = elem.get("exec", elem.get("type", ""))
        line_num = 0

        if not pkg:
            results.append({
                "severity": "warning",
                "check": "xml_node_pkg_missing",
                "message": "XML node element missing 'pkg' attribute",
                "fix": "Add pkg=\"your_package\" to the node element",
                "line": line_num,
            })
        if not exec_name:
            results.append({
                "severity": "warning",
                "check": "xml_node_exec_missing",
                "message": "XML node element missing 'exec' or 'type' attribute",
                "fix": "Add exec=\"your_executable\" to the node element",
                "line": line_num,
            })

    for elem in root.iter("include"):
        file_attr = elem.get("file", "")
        if not file_attr:
            results.append({
                "severity": "warning",
                "check": "xml_include_file_missing",
                "message": "XML include element missing 'file' attribute",
                "fix": "Add file=\"path/to/launch_file\" to the include element",
                "line": 0,
            })

    for elem in root.iter("remap"):
        from_attr = elem.get("from", "")
        to_attr = elem.get("to", "")
        if not from_attr and not to_attr:
            results.append({
                "severity": "warning",
                "check": "xml_remap_incomplete",
                "message": "XML remap element missing both 'from' and 'to' attributes",
                "fix": "Add from=\"original_topic\" and to=\"new_topic\"",
                "line": 0,
            })


_NODE_PATTERN = re.compile(
    r'Node\s*\(\s*(?:\s*\w+\s*[=:]\s*.*?)?'
)


def _find_node_packages_python(content: str) -> List[Dict[str, Any]]:
    nodes: List[Dict[str, Any]] = []
    lines = content.split("\n")
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        if "Node(" in stripped or "Node (" in stripped:
            start_idx = max(stripped.find("Node("), stripped.find("Node ("))
            brace_count = 0
            in_brace = False
            snippet = stripped[start_idx:]
            for ch in snippet:
                if ch == "(":
                    brace_count += 1
                    in_brace = True
                elif ch == ")":
                    brace_count -= 1
                    if in_brace and brace_count == 0:
                        break
            package = ""
            executable = ""
            for match in re.finditer(r'(package|executable|node_name)\s*[=:]\s*["\']([^"\']+)["\']', snippet):
                key = match.group(1)
                value = match.group(2)
                if key == "package":
                    package = value
                elif key == "executable":
                    executable = value
            nodes.append({
                "package": package,
                "executable": executable,
                "line": i + 1,
            })
        i += 1
    return nodes


def _find_node_packages_xml(root: ET.Element) -> List[Dict[str, Any]]:
    nodes: List[Dict[str, Any]] = []
    for elem in root.iter("node"):
        nodes.append({
            "package": elem.get("pkg", ""),
            "executable": elem.get("exec", elem.get("type", "")),
            "line": 0,
        })
    return nodes


def _find_node_packages(launch_file_path: str) -> List[Dict[str, Any]]:
    path = Path(launch_file_path)
    if not path.exists():
        return []
    file_type = _check_file_type(launch_file_path)
    if file_type is None:
        return []
    try:
        content = path.read_text(encoding="utf-8", errors="replace")
    except Exception:
        return []
    if file_type == "python":
        return _find_node_packages_python(content)
    elif file_type == "xml":
        try:
            root = ET.fromstring(content)
        except ET.ParseError:
            return []
        return _find_node_packages_xml(root)
    return []


def _check_port_conflicts(launch_file_path: str) -> List[Dict[str, Any]]:
    results: List[Dict[str, Any]] = []
    path = Path(launch_file_path)
    if not path.exists():
        return results
    try:
        content = path.read_text(encoding="utf-8", errors="replace")
    except Exception:
        return results

    lines = content.split("\n")
    for port in COMMON_PORTS:
        for line_num, line in enumerate(lines, 1):
            stripped = line.strip()
            if stripped.startswith("#") or stripped.startswith("<!--"):
                continue
            if port in stripped:
                results.append({
                    "severity": "warning",
                    "check": "port_conflict",
                    "message": f"Port {port} is hardcoded in the launch file and may conflict with other services",
                    "fix": f"Use $(var port) or a parameter instead of hardcoding port {port}",
                    "line": line_num,
                })
    return results


def _check_resource_constraints() -> List[Dict[str, Any]]:
    results: List[Dict[str, Any]] = []
    try:
        import psutil
        vm = psutil.virtual_memory()
        total_gb = vm.total / 1e9
        available_gb = vm.available / 1e9

        if total_gb < 4.0:
            results.append({
                "severity": "error",
                "check": "system_memory",
                "message": f"System has only {total_gb:.1f} GB RAM, may not be enough for ROS2 nodes",
                "fix": "Upgrade system memory or limit simultaneous node launches",
                "line": 0,
            })
        elif total_gb < 8.0:
            results.append({
                "severity": "warning",
                "check": "system_memory",
                "message": f"System has {total_gb:.1f} GB RAM, consider monitoring resource usage",
                "fix": "Consider upgrading memory for complex ROS2 setups",
                "line": 0,
            })

        if available_gb < 1.0:
            results.append({
                "severity": "error",
                "check": "available_memory",
                "message": f"Only {available_gb:.1f} GB RAM available, launch may fail",
                "fix": "Free up memory or close other applications before launching",
                "line": 0,
            })
        elif available_gb < 2.0:
            results.append({
                "severity": "warning",
                "check": "available_memory",
                "message": f"Only {available_gb:.1f} GB RAM available, may cause performance issues",
                "fix": "Close unused applications to free memory",
                "line": 0,
            })
    except ImportError:
        pass
    except Exception:
        pass
    return results


def _parse_launch_file(launch_file_path: str) -> List[Dict[str, Any]]:
    results: List[Dict[str, Any]] = []
    path = Path(launch_file_path)

    if not path.exists():
        return results

    file_type = _check_file_type(launch_file_path)
    if file_type is None:
        results.append({
            "severity": "warning",
            "check": "file_type",
            "message": f"Unknown launch file type: {launch_file_path}",
            "fix": "Use .launch.py or .launch.xml extension",
            "line": 0,
        })
        return results

    try:
        content = path.read_text(encoding="utf-8", errors="replace")
    except Exception as e:
        results.append({
            "severity": "error",
            "check": "file_readable",
            "message": f"Cannot read launch file: {e}",
            "fix": f"Check permissions on {launch_file_path}",
            "line": 0,
        })
        return results

    if file_type == "xml":
        try:
            root = ET.fromstring(content)
        except ET.ParseError as e:
            line_num = 0
            if hasattr(e, "position") and e.position:
                line_num = e.position[0]
            results.append({
                "severity": "error",
                "check": "xml_parse",
                "message": f"XML parse error: {e}",
                "fix": "Fix XML syntax errors",
                "line": line_num,
            })
            return results
        _check_xml_launch(root, results)
    elif file_type == "python":
        try:
            compile(content, launch_file_path, "exec")
        except SyntaxError as e:
            results.append({
                "severity": "error",
                "check": "python_syntax",
                "message": f"Python syntax error: {e.msg}",
                "fix": "Fix Python syntax errors",
                "line": e.lineno or 0,
            })
            return results

    return results


def verify_launch_file(launch_file_path: str) -> Dict[str, Any]:
    """
    Analyze a single ROS2 launch file for common issues.

    Args:
        launch_file_path: Path to the launch file.

    Returns:
        Dict with file info and list of check results.
    """
    checks: List[Dict[str, Any]] = []

    checks += _check_exists(launch_file_path)

    if not Path(launch_file_path).exists():
        return {
            "file": launch_file_path,
            "exists": False,
            "type": None,
            "checks": checks,
        }

    file_type = _check_file_type(launch_file_path)

    checks += _parse_launch_file(launch_file_path)
    checks += _check_port_conflicts(launch_file_path)
    checks += _check_resource_constraints()

    return {
        "file": launch_file_path,
        "type": file_type,
        "exists": True,
        "checks": checks,
    }


def _find_launch_files(workspace_path: str) -> List[str]:
    src = Path(workspace_path) / "src"
    if not src.exists():
        return []
    files: List[str] = []
    for pattern in ["*.launch.py", "*.launch.xml", "*.launch"]:
        for p in src.rglob(pattern):
            files.append(str(p))
    return sorted(files)


def verify_workspace_launch_files(workspace_path: str) -> Dict[str, Any]:
    """
    Verify all launch files in a workspace.

    Args:
        workspace_path: Path to the ROS2 workspace.

    Returns:
        Dict with overall summary and per-file results.
    """
    launch_files = _find_launch_files(workspace_path)
    results: List[Dict[str, Any]] = []
    total_errors = 0
    total_warnings = 0
    total_info = 0

    for lf in launch_files:
        result = verify_launch_file(lf)
        results.append(result)
        for c in result.get("checks", []):
            if c["severity"] == "error":
                total_errors += 1
            elif c["severity"] == "warning":
                total_warnings += 1
            else:
                total_info += 1

    return {
        "workspace": workspace_path,
        "total_launch_files": len(launch_files),
        "total_errors": total_errors,
        "total_warnings": total_warnings,
        "total_info": total_info,
        "results": results,
    }


def _find_owning_package(launch_file_path: str) -> Optional[str]:
    path = Path(launch_file_path).resolve()
    for parent in path.parents:
        pkg_xml = parent / "package.xml"
        if pkg_xml.exists():
            try:
                tree = ET.parse(pkg_xml)
                root = tree.getroot()
                name_el = root.find("name")
                if name_el is not None and name_el.text:
                    return name_el.text.strip()
            except Exception:
                pass
    return None


def _get_exec_depends(launch_file_path: str) -> List[str]:
    path = Path(launch_file_path).resolve()
    for parent in path.parents:
        pkg_xml = parent / "package.xml"
        if pkg_xml.exists():
            try:
                tree = ET.parse(pkg_xml)
                root = tree.getroot()
                deps: List[str] = []
                for dep in root.iter("exec_depend"):
                    if dep.text:
                        deps.append(dep.text.strip())
                for dep in root.iter("depend"):
                    if dep.text:
                        deps.append(dep.text.strip())
                return deps
            except Exception:
                break
    return []


def find_missing_dependencies(
    launch_file_path: str, workspace_path: str
) -> List[Dict[str, Any]]:
    """
    Cross-reference launched nodes with workspace packages.

    Args:
        launch_file_path: Path to the launch file.
        workspace_path: Path to the ROS2 workspace.

    Returns:
        List of check results for missing dependencies.
    """
    results: List[Dict[str, Any]] = []

    if not Path(launch_file_path).exists():
        results.append({
            "severity": "error",
            "check": "file_exists",
            "message": f"Launch file not found: {launch_file_path}",
            "fix": f"Create {launch_file_path} or correct the path",
            "line": 0,
        })
        return results

    nodes = _find_node_packages(launch_file_path)
    if not nodes:
        results.append({
            "severity": "info",
            "check": "no_nodes_found",
            "message": "No ROS2 nodes found in launch file",
            "fix": "",
            "line": 0,
        })
        return results

    ws_packages = get_workspace_packages(workspace_path)
    ws_pkg_names = [p["name"] for p in ws_packages["packages"]]

    exec_depends = _get_exec_depends(launch_file_path)
    owning_pkg = _find_owning_package(launch_file_path)

    for node in nodes:
        pkg = node["package"]
        if not pkg:
            continue

        if pkg == owning_pkg:
            continue

        if pkg not in ws_pkg_names:
            results.append({
                "severity": "error",
                "check": "missing_package",
                "message": f"Node references package '{pkg}' which does not exist in workspace",
                "fix": f"Create package '{pkg}' in workspace or install it: rosdep install {pkg}",
                "line": node["line"],
            })
        elif pkg not in exec_depends and owning_pkg is not None:
            results.append({
                "severity": "warning",
                "check": "missing_exec_depend",
                "message": f"Package '{pkg}' is launched but not listed in exec_depend of '{owning_pkg}'",
                "fix": f"Add <exec_depend>{pkg}</exec_depend> to the package.xml of '{owning_pkg}'",
                "line": node["line"],
            })

    if not results:
        results.append({
            "severity": "info",
            "check": "dependencies_ok",
            "message": "All launched packages have matching dependencies",
            "fix": "",
            "line": 0,
        })

    return results
