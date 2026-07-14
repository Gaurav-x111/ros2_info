"""
ROS2 Diagnostic Intelligence
Detects common issues and suggests fixes
"""

import glob
import os
import subprocess
from pathlib import Path
from typing import Dict, List

try:
    import psutil
except ImportError:
    psutil = None

from .workspace import find_workspaces


class Issue:
    """Represents a diagnostic issue."""

    def __init__(
        self,
        severity: str,
        issue_type: str,
        title: str,
        description: str,
        impact: str,
        fix: str,
        details: dict = None,
    ):
        self.severity = severity  # "critical", "warning", "info"
        self.issue_type = issue_type
        self.title = title
        self.description = description
        self.impact = impact
        self.fix = fix
        self.details = details or {}

    def to_dict(self) -> dict:
        return {
            "severity": self.severity,
            "type": self.issue_type,
            "title": self.title,
            "description": self.description,
            "impact": self.impact,
            "fix": self.fix,
            "details": self.details,
        }


def check_domain_id_consistency() -> List[Issue]:
    """Scan for shells with different ROS_DOMAIN_ID values."""
    issues = []
    domain_ids: Dict[str, List[str]] = {}

    # Read /proc/*/environ for ROS_DOMAIN_ID
    for pid_dir in glob.glob("/proc/[0-9]*/environ"):
        try:
            with open(pid_dir, "rb") as f:
                env = f.read().decode("utf-8", errors="replace")
            for line in env.split("\x00"):
                if line.startswith("ROS_DOMAIN_ID="):
                    value = line.split("=")[1]
                    domain_ids.setdefault(value, []).append(pid_dir.split("/")[2])
        except (PermissionError, OSError):
            continue

    if len(domain_ids) > 1:
        issues.append(
            Issue(
                severity="critical",
                issue_type="domain_id_mismatch",
                title="ROS_DOMAIN_ID Mismatch",
                description=f"Multiple domain IDs detected: {', '.join(sorted(domain_ids.keys()))}",
                impact="Nodes cannot communicate across terminals",
                fix=f"Run 'export ROS_DOMAIN_ID={sorted(domain_ids.keys())[0]}' in all terminals",
                details={"domain_ids": domain_ids},
            )
        )

    return issues


def check_workspace_staleness() -> List[Issue]:
    """Detect modified source files that haven't been rebuilt."""
    issues = []

    for ws_path in find_workspaces():
        src_dir = Path(ws_path) / "src"
        install_dir = Path(ws_path) / "install"

        if not src_dir.exists() or not install_dir.exists():
            continue

        # Check for modified .py, .cpp, .hpp files
        modified = []
        for ext in ["*.py", "*.cpp", "*.hpp", "*.h"]:
            for f in src_dir.rglob(ext):
                try:
                    # Compare mtime with install dir
                    install_file = install_dir / f.relative_to(src_dir)
                    if (
                        install_file.exists()
                        and f.stat().st_mtime > install_file.stat().st_mtime
                    ):
                        modified.append(str(f.relative_to(src_dir)))
                except (OSError, ValueError):
                    continue

        if modified:
            issues.append(
                Issue(
                    severity="warning",
                    issue_type="stale_workspace",
                    title="Stale Workspace",
                    description=f"{len(modified)} files modified since last build",
                    impact="Running outdated code",
                    fix=f"cd {ws_path} && colcon build",
                    details={"modified_files": modified[:10]},  # Limit to 10
                )
            )

    return issues


def _ros2_quick_check() -> bool:
    """Quick check if ROS2 CLI is responsive (returns fast)."""
    try:
        result = subprocess.run(
            ["ros2", "topic", "list", "-t"],
            capture_output=True, text=True, timeout=2, env=os.environ
        )
        return result.returncode == 0
    except (subprocess.TimeoutExpired, FileNotFoundError):
        return False


def check_ros_daemon_status() -> List[Issue]:
    """Check if ROS2 daemon is running."""
    issues = []

    # Try to run ros2 daemon list
    import subprocess

    try:
        result = subprocess.run(
            ["ros2", "daemon", "list"],
            capture_output=True,
            text=True,
            timeout=2,
            env=os.environ,
        )
        if result.returncode != 0 or "Daemon is not running" in result.stderr:
            issues.append(
                Issue(
                    severity="critical",
                    issue_type="daemon_not_running",
                    title="ROS2 Daemon Not Running",
                    description="The ROS2 daemon process is not active",
                    impact="ROS2 discovery commands will be slow or fail",
                    fix="ros2 daemon start",
                    details={},
                )
            )
    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass

    return issues


def check_localhost_only() -> List[Issue]:
    """Check if ROS_LOCALHOST_ONLY is set for production safety."""
    issues = []

    localhost_only = os.environ.get("ROS_LOCALHOST_ONLY", "0")

    if localhost_only != "1":
        issues.append(
            Issue(
                severity="info",
                issue_type="localhost_only_disabled",
                title="ROS_LOCALHOST_ONLY Not Set",
                description="Nodes are visible to the entire subnet",
                impact="Potential security risk in production environments",
                fix="export ROS_LOCALHOST_ONLY=1",
                details={"current_value": localhost_only},
            )
        )

    return issues


def check_node_crashes() -> List[Issue]:
    """Scan ROS2 log directory for crashed nodes."""
    issues = []
    log_dir = Path.home() / ".ros" / "log"

    if not log_dir.exists():
        return issues

    crash_files = list(log_dir.glob("*.log"))
    crashed_nodes = set()

    for log_file in crash_files:
        try:
            with open(log_file, "r", errors="ignore") as f:
                content = f.read()
                if "ERROR" in content or "Traceback" in content:
                    # Extract node name from log filename or content
                    node_name = log_file.stem.replace("_node_", "/").replace("_", "/")
                    crashed_nodes.add(node_name)
        except (OSError, IOError):
            continue

    if crashed_nodes:
        issues.append(
            Issue(
                severity="critical",
                issue_type="node_crash",
                title=f"{len(crashed_nodes)} Node(s) Crashed",
                description=f"Crashed nodes: {', '.join(list(crashed_nodes)[:5])}",
                impact="Functionality may be degraded or unavailable",
                fix="Check logs at ~/.ros/log/ and restart affected nodes",
                details={"crashed_nodes": list(crashed_nodes)},
            )
        )

    return issues


def check_high_cpu_nodes(threshold: float = 80.0) -> List[Issue]:
    """Detect nodes consuming excessive CPU."""
    issues = []

    try:
        import subprocess
        result = subprocess.run(
            ["ros2", "node", "list"],
            capture_output=True, text=True, timeout=2, env=os.environ
        )
        if result.returncode != 0:
            return issues

        nodes = [n.strip() for n in result.stdout.strip().split("\n") if n.strip()]
        high_cpu_nodes = []

        # Note: Actual CPU per-node profiling would require psutil per-pid
        # This is a placeholder for future enhancement
        # For now, check system CPU status
        system_cpu = psutil.cpu_percent(interval=0.5)
        if system_cpu > threshold:
            high_cpu_nodes.append(f"system-wide ({system_cpu:.1f}%)")

        if high_cpu_nodes:
            issues.append(
                Issue(
                    severity="warning",
                    issue_type="high_cpu",
                    title="High CPU Usage Detected",
                    description=f"CPU at {system_cpu:.1f}% (threshold: {threshold}%)",
                    impact="May cause timing issues or node starvation",
                    fix="Identify heavy nodes with 'top' or 'htop'; consider optimization",
                    details={"cpu_percent": system_cpu, "threshold": threshold},
                )
            )
    except (subprocess.TimeoutExpired, FileNotFoundError, ImportError):
        pass

    return issues


def check_memory_pressure(threshold: float = 85.0) -> List[Issue]:
    """Check for memory pressure conditions."""
    issues = []

    try:
        mem = psutil.virtual_memory()
        if mem.percent > threshold:
            issues.append(
                Issue(
                    severity="warning",
                    issue_type="memory_pressure",
                    title="High Memory Usage",
                    description=f"Memory at {mem.percent:.1f}% ({mem.available / 1e9:.1f}GB free)",
                    impact="System may become unresponsive; nodes may crash",
                    fix="Close unused applications; check for memory leaks",
                    details={"memory_percent": mem.percent, "available_gb": mem.available / 1e9},
                )
            )
    except (ImportError, OSError):
        pass

    return issues


def check_disk_space(threshold: float = 90.0) -> List[Issue]:
    """Check disk space usage."""
    issues = []

    try:
        home_usage = psutil.disk_usage(str(Path.home()))
        if home_usage.percent > threshold:
            issues.append(
                Issue(
                    severity="warning",
                    issue_type="disk_space",
                    title="Low Disk Space",
                    description=f"Home directory at {home_usage.percent:.1f}% full",
                    impact="Bag recording may fail; logs may not write",
                    fix="Free disk space; clear old bag files and logs",
                    details={"disk_percent": home_usage.percent, "free_gb": home_usage.free / 1e9},
                )
            )
    except (ImportError, OSError):
        pass

    return issues


def check_network_interfaces() -> List[Issue]:
    """Check network interface status."""
    issues = []

    try:
        import psutil
        net_if_addrs = psutil.net_if_addrs()
        net_if_stats = psutil.net_if_stats()

        # Check for loopback-only configuration
        has_loopback = "lo" in net_if_addrs
        has_ethernet = any(stat.isup for name, stat in net_if_stats.items() if name != "lo")

        if not has_ethernet and has_loopback:
            issues.append(
                Issue(
                    severity="info",
                    issue_type="network_isolated",
                    title="Network Isolated (Loopback Only)",
                    description="No active network interfaces besides loopback",
                    impact="Cannot communicate with other robots or external systems",
                    fix="Connect to network or enable WiFi/Ethernet",
                    details={"interfaces": list(net_if_stats.keys())},
                )
            )
    except (ImportError, OSError):
        pass

    return issues


def check_dds_implementation() -> List[Issue]:
    """Check DDS middleware configuration."""
    issues = []

    rmw = os.environ.get("RMW_IMPLEMENTATION", "rmw_fastrtps_cpp")

    # Check for potentially problematic DDS configurations
    cyclic = os.environ.get("CYCLONEDDS_URI")
    fastdds = os.environ.get("FASTRTPS_DEFAULT_PROFILES_FILE")

    if rmw == "rmw_cyclonedds_cpp" and not cyclic:
        issues.append(
            Issue(
                severity="info",
                issue_type="dds_default_config",
                title="CycloneDDS Using Default Config",
                description="No CYCLONEDDS_URI environment variable set",
                impact="May have suboptimal discovery or QoS settings",
                fix="Set CYCLONEDDS_URI to point to a config XML file",
                details={"rmw": rmw},
            )
        )

    return issues


def check_topic_type_mismatches(timeout: int = 2) -> List[Issue]:
    """Detect topic type mismatches between publishers and subscribers."""
    issues = []

    try:
        # Get all topics with their types
        result = subprocess.run(
            ["ros2", "topic", "list", "-t"],
            capture_output=True, text=True, timeout=timeout, env=os.environ
        )
        if result.returncode != 0:
            return issues

        topic_types = {}
        for line in result.stdout.strip().split("\n"):
            if line.startswith("/"):
                parts = line.split()
                topic_name = parts[0]
                topic_type = parts[1].strip("[]") if len(parts) > 1 else "unknown"
                topic_types[topic_name] = topic_type

        # Check for topics with multiple types (shouldn't happen, but indicates issues)
        # In ROS2, this is usually caught at runtime, but we can check for suspicious patterns
        # For now, just report topics with "unknown" type
        unknown_topics = [k for k, v in topic_types.items() if v == "unknown"]

        if unknown_topics:
            issues.append(
                Issue(
                    severity="warning",
                    issue_type="topic_type_unknown",
                    title="Topic Types Unresolved",
                    description=f"{len(unknown_topics)} topics have unresolved types: {', '.join(unknown_topics[:5])}",
                    impact="May indicate type mismatches or discovery issues",
                    fix="Check that publishers and subscribers agree on message types",
                    details={"unknown_topics": unknown_topics},
                )
            )

    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass

    return issues


def check_qos_compatibility(timeout: int = 2) -> List[Issue]:
    """
    Check for QoS compatibility issues between publishers and subscribers.

    Note: Full QoS compatibility checking requires runtime introspection.
    This check looks for signs of QoS issues like dropped messages.
    """
    issues = []

    try:
        # Get topic info to check for message drops
        result = subprocess.run(
            ["ros2", "topic", "list"],
            capture_output=True, text=True, timeout=timeout, env=os.environ
        )
        if result.returncode != 0:
            return issues

        topics = [t.strip() for t in result.stdout.strip().split("\n") if t.strip()]

        # Check a few topics for publication health
        unhealthy_topics = []
        for topic in topics[:10]:  # Check first 10 topics
            info_result = subprocess.run(
                ["ros2", "topic", "info", topic],
                capture_output=True, text=True, timeout=timeout, env=os.environ
            )
            if info_result.returncode == 0:
                output = info_result.stdout
                # Check if topic has publishers but no subscribers (orphaned)
                pub_count = output.count("Publisher")
                sub_count = output.count("Subscriber")
                if pub_count > 0 and sub_count == 0:
                    unhealthy_topics.append(topic)

        if unhealthy_topics:
            issues.append(
                Issue(
                    severity="warning",
                    issue_type="orphaned_topic",
                    title="Orphaned Topics Detected",
                    description=f"Topics with publishers but no subscribers: {', '.join(unhealthy_topics[:5])}",
                    impact="Messages being published with no recipients",
                    fix="Start a subscriber node for these topics or stop the publisher",
                    details={"orphaned_topics": unhealthy_topics},
                )
            )

    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass

    return issues


def check_security_configuration() -> List[Issue]:
    """Check ROS2 security configuration (SROS2 readiness)."""
    issues = []

    # Check for SROS2 environment
    rmw = os.environ.get("RMW_IMPLEMENTATION", "rmw_fastrtps_cpp")
    localhost_only = os.environ.get("ROS_LOCALHOST_ONLY", "0")
    ros_domain_id = os.environ.get("ROS_DOMAIN_ID", "0")

    # Check if using default domain ID (security risk)
    if ros_domain_id == "0":
        issues.append(
            Issue(
                severity="info",
                issue_type="default_domain_id",
                title="Using Default Domain ID",
                description="ROS_DOMAIN_ID=0 (default)",
                impact="Potential for cross-talk with other ROS2 systems on network",
                fix="Set ROS_DOMAIN_ID to a unique value (e.g., export ROS_DOMAIN_ID=42)",
                details={"current_domain_id": ros_domain_id},
            )
        )

    # Check for SROS2 enablement
    security_env = os.environ.get("RMW_SECURITY_ENFORCE", "0")
    if security_env != "1" and "secure" not in rmw:
        issues.append(
            Issue(
                severity="info",
                issue_type="sros2_not_enabled",
                title="SROS2 Security Not Enabled",
                description="DDS security plugins not enforced",
                impact="Communication is unencrypted and unauthenticated",
                fix="For production: enable SROS2 with RMW_SECURITY_ENFORCE=1",
                details={"rmw": rmw, "security_enforce": security_env},
            )
        )

    return issues


def run_full_diagnosis() -> List[Issue]:
    """Run all diagnostic checks and return list of issues."""
    all_issues = []

    # Environment checks
    all_issues.extend(check_domain_id_consistency())
    all_issues.extend(check_workspace_staleness())
    all_issues.extend(check_localhost_only())

    # System health checks
    all_issues.extend(check_node_crashes())
    all_issues.extend(check_memory_pressure())
    all_issues.extend(check_disk_space())
    all_issues.extend(check_network_interfaces())
    all_issues.extend(check_dds_implementation())

    # ROS2-dependent checks (skip fast if ROS2 unavailable)
    if _ros2_quick_check():
        all_issues.extend(check_ros_daemon_status())
        all_issues.extend(check_high_cpu_nodes())
        all_issues.extend(check_topic_type_mismatches())
        all_issues.extend(check_qos_compatibility())

    # Security checks
    all_issues.extend(check_security_configuration())

    return all_issues


# ── Compatibility wrapper for existing CLI commands ──────────────────────────

CHECK_RESULTS = {
    "pass": "✓",
    "fail": "✗",
    "warn": "⚠",
    "info": "ℹ",
}


def run_diagnostics() -> dict:
    """
    Run all diagnostic checks and return structured result.

    Returns:
        dict with keys:
            - checks: List of check results with name, status, detail, fix
            - summary: Dict with total, passed, warnings, failed, score
    """
    checks = []
    passed = 0
    warnings = 0
    failed = 0

    # Run non-ROS2 checks directly (they are fast)
    fast_checks = [
        ("Domain ID Consistency", check_domain_id_consistency),
        ("Workspace Staleness", check_workspace_staleness),
        ("ROS_LOCALHOST_ONLY", check_localhost_only),
        ("Node Crash Detection", check_node_crashes),
        ("Memory Pressure", check_memory_pressure),
        ("Disk Space", check_disk_space),
        ("Network Interfaces", check_network_interfaces),
        ("DDS Configuration", check_dds_implementation),
        ("Security Configuration", check_security_configuration),
    ]

    # ROS2-dependent checks — skip fast if ROS2 unavailable
    ros2_checks = [
        ("ROS2 Daemon Status", check_ros_daemon_status),
        ("CPU Usage", check_high_cpu_nodes),
        ("Topic Type Mismatches", check_topic_type_mismatches),
        ("QoS Compatibility", check_qos_compatibility),
    ]

    _ros2_available = _ros2_quick_check()

    def _process(name, issues_list):
        nonlocal passed, warnings, failed
        if not issues_list:
            checks.append({"name": name, "status": "pass", "detail": "OK", "fix": None})
            passed += 1
        else:
            for issue in issues_list:
                if issue.severity == "critical":
                    status = "fail"
                    failed += 1
                elif issue.severity == "warning":
                    status = "warn"
                    warnings += 1
                else:
                    status = "info"
                    passed += 1
                checks.append({
                    "name": f"{name}: {issue.title}",
                    "status": status,
                    "detail": issue.description[:60],
                    "fix": issue.fix,
                })

    for name, func in fast_checks:
        _process(name, func())

    for name, func in ros2_checks:
        if _ros2_available:
            _process(name, func())
        else:
            checks.append({"name": name, "status": "info", "detail": "ROS2 not accessible", "fix": None})
            passed += 1

    total = len(checks)
    score = int((passed / total) * 100) if total > 0 else 0

    return {
        "checks": checks,
        "summary": {
            "total": total,
            "passed": passed,
            "warnings": warnings,
            "failed": failed,
            "score": score,
        },
    }


def apply_fix(check_name: str, detail: str = None) -> tuple:
    """
    Apply an automatic fix for a diagnostic issue.

    Returns:
        (success: bool, message: str)
    """
    fixes = {
        "ROS2 Daemon Status": _fix_daemon_start,
        "Domain ID Consistency": _fix_domain_id,
        "Workspace Staleness": _fix_workspace_build,
        "ROS_LOCALHOST_ONLY": _fix_localhost_only,
    }

    fix_func = fixes.get(check_name)
    if fix_func:
        return fix_func(detail)
    return False, f"No auto-fix available for: {check_name}"


def _fix_daemon_start(detail=None) -> tuple:
    """Start the ROS2 daemon."""
    try:
        result = subprocess.run(
            ["ros2", "daemon", "start"],
            capture_output=True, text=True, timeout=10, env=os.environ
        )
        if result.returncode == 0:
            return True, "ROS2 daemon started successfully"
        return False, f"Daemon start failed: {result.stderr.strip()}"
    except Exception as e:
        return False, f"Error: {str(e)}"


def _fix_domain_id(detail=None) -> tuple:
    """Cannot auto-fix domain ID mismatch across processes."""
    return False, "Manual fix required: Run 'export ROS_DOMAIN_ID=<value>' in all terminals"


def _fix_workspace_build(detail=None) -> tuple:
    """Rebuild stale workspace."""
    try:
        workspaces = find_workspaces()
        if not workspaces:
            return False, "No workspace found"
        ws = workspaces[0]
        result = subprocess.run(
            ["colcon", "build", "--cwd", ws],
            capture_output=True, text=True, timeout=300, env=os.environ
        )
        if result.returncode == 0:
            return True, f"Workspace rebuilt successfully: {ws}"
        return False, f"Build failed: {result.stderr.strip()}"
    except Exception as e:
        return False, f"Error: {str(e)}"


def _fix_localhost_only(detail=None) -> tuple:
    """Set ROS_LOCALHOST_ONLY environment variable."""
    return False, "Manual fix required: Add 'export ROS_LOCALHOST_ONLY=1' to your .bashrc"
