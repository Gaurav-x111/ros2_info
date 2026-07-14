"""
Bag Forensics Module
Analyzes ROS2 bag files for forensic and diagnostic purposes
"""

import os
import re
import subprocess
from typing import Any, Dict, List, Optional


def _run_ros2_bag_info(bag_path: str) -> Optional[str]:
    try:
        result = subprocess.run(
            ["ros2", "bag", "info", bag_path],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode != 0:
            return None
        return result.stdout
    except (subprocess.TimeoutExpired, FileNotFoundError, Exception):
        return None


def _parse_duration_seconds(duration_str: str) -> Optional[float]:
    match = re.match(r'([\d.]+)\s*s', duration_str.strip())
    if match:
        return float(match.group(1))
    return None


def _parse_timestamp(ts_str: str) -> Optional[str]:
    match = re.search(r'\((\d+\.\d+)\)', ts_str)
    if match:
        return match.group(1)
    return None


def _parse_flat_format(text: str) -> Dict[str, Any]:
    result: Dict[str, Any] = {
        "duration": None,
        "size": None,
        "messages": 0,
        "topics": [],
        "start_time": None,
        "end_time": None,
        "dropped": None,
        "compression": None,
    }

    for line in text.split("\n"):
        stripped = line.strip()

        if not stripped:
            continue

        if "Topic:" in stripped and "|" in stripped:
            topic_data: Dict[str, Any] = {}
            for segment in stripped.split("|"):
                segment = segment.strip()
                if segment.startswith("Topic information:"):
                    segment = segment[len("Topic information:"):].strip()
                if segment.startswith("Topic:"):
                    topic_data["name"] = segment.split(":", 1)[1].strip()
                elif segment.startswith("Type:"):
                    topic_data["type"] = segment.split(":", 1)[1].strip()
                elif segment.startswith("Count:"):
                    try:
                        topic_data["count"] = int(segment.split(":", 1)[1].strip())
                    except ValueError:
                        pass
            if "name" in topic_data and "type" in topic_data:
                result["topics"].append(topic_data)
                continue

        if ":" not in stripped:
            continue

        key, _, value = stripped.partition(":")
        key = key.strip().lower()
        value = value.strip()

        if key == "duration":
            result["duration"] = _parse_duration_seconds(value)
        elif key in ("bag size", "bag_size", "size"):
            result["size"] = value
        elif key in ("messages", "message count", "message_count"):
            try:
                result["messages"] = int(value)
            except ValueError:
                pass
        elif key in ("start", "starting_time"):
            result["start_time"] = _parse_timestamp(value)
        elif key == "end":
            result["end_time"] = _parse_timestamp(value)
        elif key == "dropped":
            try:
                result["dropped"] = int(value)
            except ValueError:
                pass
        elif key == "compression":
            result["compression"] = value
        elif key in ("compression format", "compression_format"):
            if value:
                result["compression"] = value

    if result["messages"] == 0 and result["topics"]:
        result["messages"] = sum(t.get("count", 0) for t in result["topics"])

    return result


def _parse_yaml_format(data: dict) -> Dict[str, Any]:
    bag_info = data.get("rosbag2_bag_info", data)

    result: Dict[str, Any] = {
        "duration": None,
        "size": None,
        "messages": 0,
        "topics": [],
        "start_time": None,
        "end_time": None,
        "dropped": None,
        "compression": None,
    }

    duration = bag_info.get("duration", {})
    if isinstance(duration, dict):
        ns = duration.get("nanoseconds", 0)
        result["duration"] = ns / 1e9
    elif isinstance(duration, (int, float)):
        result["duration"] = float(duration)

    bag_size = bag_info.get("bag_size", bag_info.get("size"))
    if bag_size is not None:
        result["size"] = str(bag_size)

    message_count = bag_info.get("message_count", bag_info.get("messages", 0))
    result["messages"] = int(message_count)

    starting_time = bag_info.get("starting_time", {})
    if isinstance(starting_time, dict):
        ns = starting_time.get("nanoseconds", 0)
        if ns:
            result["start_time"] = f"{ns / 1e9:.3f}"
    elif isinstance(starting_time, str):
        result["start_time"] = _parse_timestamp(starting_time)

    ending_time = bag_info.get("ending_time", {})
    if isinstance(ending_time, dict):
        ns = ending_time.get("nanoseconds", 0)
        if ns:
            result["end_time"] = f"{ns / 1e9:.3f}"
    elif isinstance(ending_time, str):
        result["end_time"] = _parse_timestamp(ending_time)

    topics = bag_info.get("topics", [])
    for topic_entry in topics:
        meta = topic_entry.get("topic_metadata", topic_entry)
        if isinstance(meta, dict):
            result["topics"].append({
                "name": meta.get("name", "Unknown"),
                "type": meta.get("type", "Unknown"),
                "count": int(meta.get("count", 0)),
            })

    compression = bag_info.get("compression_format", "")
    result["compression"] = compression if compression else "none"

    return result


def _parse_bag_info_output(text: str) -> Optional[Dict[str, Any]]:
    if not text:
        return None

    try:
        import yaml
        data = yaml.safe_load(text)
        if isinstance(data, dict):
            bag_info = data.get("rosbag2_bag_info", data)
            if isinstance(bag_info.get("topics"), list):
                return _parse_yaml_format(data)
    except Exception:
        pass

    return _parse_flat_format(text)


def analyze_bag(bag_path: str) -> Dict[str, Any]:
    """Parse ros2 bag info output into structured data.

    Args:
        bag_path: Path to the ROS2 bag directory or file

    Returns:
        Dictionary with duration, size, messages per topic, topic types, start/end time
    """
    if not os.path.exists(bag_path):
        return {"error": f"Bag path does not exist: {bag_path}"}

    info_text = _run_ros2_bag_info(bag_path)
    if info_text is None:
        return {"error": f"Failed to run ros2 bag info on {bag_path}"}

    parsed = _parse_bag_info_output(info_text)
    if parsed is None:
        return {"error": f"Failed to parse ros2 bag info output for {bag_path}"}

    return parsed


def check_bag_health(bag_path: str) -> Dict[str, Any]:
    """Check bag file health and return any issues found.

    Args:
        bag_path: Path to the ROS2 bag directory or file

    Returns:
        Dictionary with health status and list of issues
    """
    issues: List[str] = []
    health: Dict[str, Any] = {
        "healthy": True,
        "issues": issues,
        "bag_path": bag_path,
    }

    if not os.path.exists(bag_path):
        issues.append("Bag path does not exist")
        health["healthy"] = False
        return health

    if os.path.isdir(bag_path):
        metadata_yaml = os.path.join(bag_path, "metadata.yaml")
        if not os.path.exists(metadata_yaml):
            issues.append("metadata.yaml not found in bag directory")

    info_text = _run_ros2_bag_info(bag_path)
    if info_text is None:
        issues.append("Failed to run ros2 bag info")
        health["healthy"] = False
        return health

    parsed = _parse_bag_info_output(info_text)
    if parsed is None:
        issues.append("Failed to parse bag info output")
        health["healthy"] = False
        return health

    if parsed.get("dropped") is not None and parsed["dropped"] > 0:
        issues.append(f"Dropped messages detected: {parsed['dropped']}")
        health["healthy"] = False

    compression = parsed.get("compression", "none")
    if compression and compression.lower() != "none":
        health["compression"] = compression

    return health


def get_topic_timeline(bag_path: str) -> Dict[str, Any]:
    """Get publish timeline for each topic.

    Args:
        bag_path: Path to the ROS2 bag directory or file

    Returns:
        Dictionary mapping topic names to message count and rate estimate
    """
    if not os.path.exists(bag_path):
        return {"error": f"Bag path does not exist: {bag_path}"}

    info_text = _run_ros2_bag_info(bag_path)
    if info_text is None:
        return {"error": f"Failed to run ros2 bag info on {bag_path}"}

    parsed = _parse_bag_info_output(info_text)
    if parsed is None:
        return {"error": f"Failed to parse ros2 bag info output for {bag_path}"}

    duration = parsed.get("duration")
    if duration is None or duration == 0:
        return {"error": "Cannot compute timeline without duration data"}

    timeline: Dict[str, Dict[str, Any]] = {}
    for topic in parsed.get("topics", []):
        name = topic.get("name", "Unknown")
        count = topic.get("count", 0)
        rate = count / duration if duration > 0 else 0.0
        timeline[name] = {
            "message_count": count,
            "rate_hz": round(rate, 2),
        }

    return timeline


def compare_bags(bag_path_1: str, bag_path_2: str) -> Dict[str, Any]:
    """Compare two ROS2 bag files.

    Args:
        bag_path_1: Path to the first bag
        bag_path_2: Path to the second bag

    Returns:
        Dictionary with comparative metrics between the two bags
    """
    info_1 = analyze_bag(bag_path_1)
    info_2 = analyze_bag(bag_path_2)

    if "error" in info_1 and "error" in info_2:
        return {"error": f"Both bags failed: {info_1['error']}; {info_2['error']}"}

    result: Dict[str, Any] = {}

    dur_1 = info_1.get("duration")
    dur_2 = info_2.get("duration")
    if dur_1 is not None and dur_2 is not None and dur_2 != 0:
        result["duration_difference"] = round(abs(dur_1 - dur_2), 3)
        result["duration_ratio"] = round(dur_1 / dur_2, 3)
    result["bag_1_duration"] = dur_1
    result["bag_2_duration"] = dur_2

    count_1 = info_1.get("messages", 0)
    count_2 = info_2.get("messages", 0)
    result["bag_1_message_count"] = count_1
    result["bag_2_message_count"] = count_2
    result["message_count_difference"] = abs(count_1 - count_2)

    dropped_1 = info_1.get("dropped", 0)
    dropped_2 = info_2.get("dropped", 0)
    total_1 = count_1 + dropped_1
    total_2 = count_2 + dropped_2
    result["bag_1_dropped_pct"] = round(dropped_1 / total_1 * 100, 2) if total_1 > 0 else 0.0
    result["bag_2_dropped_pct"] = round(dropped_2 / total_2 * 100, 2) if total_2 > 0 else 0.0

    topics_1 = {t["name"]: t for t in info_1.get("topics", [])}
    topics_2 = {t["name"]: t for t in info_2.get("topics", [])}
    names_1 = set(topics_1.keys())
    names_2 = set(topics_2.keys())

    result["shared_topics"] = sorted(names_1 & names_2)
    result["bag_1_only_topics"] = sorted(names_1 - names_2)
    result["bag_2_only_topics"] = sorted(names_2 - names_1)

    return result
