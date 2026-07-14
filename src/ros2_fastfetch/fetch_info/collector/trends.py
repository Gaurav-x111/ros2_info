"""
Historical Trends Module
Time-series storage and retrieval for system and ROS2 statistics
"""

import os
import sqlite3
from contextlib import contextmanager
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Tuple


DB_PATH = os.path.expanduser("~/.ros2_info/trends.db")

_SCHEMA = """
CREATE TABLE IF NOT EXISTS trends (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    cpu_percent REAL,
    memory_percent REAL,
    disk_percent REAL,
    battery_percent REAL,
    node_count INTEGER,
    topic_count INTEGER,
    service_count INTEGER
)
"""

_CREATE_INDEX = """
CREATE INDEX IF NOT EXISTS idx_trends_timestamp ON trends(timestamp)
"""


def _ensure_dir() -> None:
    os.makedirs(os.path.dirname(DB_PATH), exist_ok=True)


@contextmanager
def _get_db() -> sqlite3.Connection:
    _ensure_dir()
    conn = sqlite3.connect(DB_PATH)
    try:
        conn.execute(_SCHEMA)
        conn.execute(_CREATE_INDEX)
        yield conn
        conn.commit()
    finally:
        conn.close()


def record_snapshot(
    cpu_percent: float,
    memory_percent: float,
    disk_percent: float,
    battery_percent: Optional[float] = None,
    node_count: int = 0,
    topic_count: int = 0,
    service_count: int = 0,
) -> None:
    ts = datetime.now().isoformat()
    with _get_db() as conn:
        conn.execute(
            """INSERT INTO trends
               (timestamp, cpu_percent, memory_percent, disk_percent,
                battery_percent, node_count, topic_count, service_count)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
            (ts, cpu_percent, memory_percent, disk_percent,
             battery_percent, node_count, topic_count, service_count),
        )


def get_trend(duration_hours: int = 24) -> List[Dict]:
    cutoff = (datetime.now() - timedelta(hours=duration_hours)).isoformat()
    with _get_db() as conn:
        rows = conn.execute(
            """SELECT timestamp, cpu_percent, memory_percent, disk_percent,
                      battery_percent, node_count, topic_count, service_count
               FROM trends
               WHERE timestamp >= ?
               ORDER BY timestamp ASC""",
            (cutoff,),
        ).fetchall()
    return [
        {
            "timestamp": r[0],
            "cpu_percent": r[1],
            "memory_percent": r[2],
            "disk_percent": r[3],
            "battery_percent": r[4],
            "node_count": r[5],
            "topic_count": r[6],
            "service_count": r[7],
        }
        for r in rows
    ]


def get_summary() -> Dict:
    cutoff = (datetime.now() - timedelta(hours=24)).isoformat()
    with _get_db() as conn:
        row = conn.execute(
            """SELECT
                   MIN(cpu_percent), MAX(cpu_percent), AVG(cpu_percent),
                   MIN(memory_percent), MAX(memory_percent), AVG(memory_percent),
                   MIN(disk_percent), MAX(disk_percent), AVG(disk_percent),
                   MIN(battery_percent), MAX(battery_percent), AVG(battery_percent),
                   MIN(node_count), MAX(node_count), AVG(node_count),
                   MIN(topic_count), MAX(topic_count), AVG(topic_count),
                   MIN(service_count), MAX(service_count), AVG(service_count),
                   COUNT(*)
               FROM trends
               WHERE timestamp >= ?""",
            (cutoff,),
        ).fetchone()
    if not row or row[-1] == 0:
        return {
            "cpu": {}, "memory": {}, "disk": {}, "battery": {},
            "nodes": {}, "topics": {}, "services": {},
            "total_snapshots": 0,
        }
    return {
        "cpu": {
            "min": row[0], "max": row[1], "avg": round(row[2], 1) if row[2] is not None else None,
        },
        "memory": {
            "min": row[3], "max": row[4], "avg": round(row[5], 1) if row[5] is not None else None,
        },
        "disk": {
            "min": row[6], "max": row[7], "avg": round(row[8], 1) if row[8] is not None else None,
        },
        "battery": {
            "min": row[9], "max": row[10], "avg": round(row[11], 1) if row[11] is not None else None,
        },
        "nodes": {
            "min": row[12], "max": row[13], "avg": round(row[14], 1) if row[14] is not None else None,
        },
        "topics": {
            "min": row[15], "max": row[16], "avg": round(row[17], 1) if row[17] is not None else None,
        },
        "services": {
            "min": row[18], "max": row[19], "avg": round(row[20], 1) if row[20] is not None else None,
        },
        "total_snapshots": row[21],
    }


def prune_old_data(days: int = 30) -> int:
    cutoff = (datetime.now() - timedelta(days=days)).isoformat()
    with _get_db() as conn:
        cursor = conn.execute("DELETE FROM trends WHERE timestamp < ?", (cutoff,))
        return cursor.rowcount


def get_ascii_sparkline(values: List[float], height: int = 5, width: int = 40) -> List[str]:
    """
    Generate ASCII sparkline chart for a list of values.

    Args:
        values: List of numeric values to plot
        height: Chart height in characters
        width: Chart width in characters

    Returns:
        List of strings representing chart rows
    """
    if not values:
        return [" " * width] * height

    # Sample values to fit width
    sampled = []
    step = len(values) / width if len(values) > width else 1
    for i in range(min(width, len(values))):
        idx = int(i * step)
        sampled.append(values[idx] if idx < len(values) else min(values))

    # If we have fewer values than width, we'll use actual count
    actual_width = len(sampled)

    # Normalize values to 0-(height-1) range
    min_val = min(sampled)
    max_val = max(sampled)
    range_val = max_val - min_val if max_val != min_val else 1

    normalized = []
    for v in sampled:
        n = int(((v - min_val) / range_val) * (height - 0.01))
        normalized.append(min(n, height - 1))

    # Generate chart rows from top to bottom
    rows = []
    for row_idx in range(height - 1, -1, -1):
        row = ""
        for val in normalized:
            if val >= row_idx:
                row += "█"
            else:
                row += " "
        # Pad to width if needed
        row = row.ljust(width)
        rows.append(row)

    return rows


def _linear_trend(values):
    """Return slope (per hour) and projected hours until values reach 90."""
    if len(values) < 3:
        return 0.0, None
    n = len(values)
    xs = list(range(n))
    mean_x = sum(xs) / n
    mean_y = sum(values) / n
    num = sum((x - mean_x) * (y - mean_y) for x, y in zip(xs, values))
    den = sum((x - mean_x) ** 2 for x in xs)
    slope = num / den if den else 0.0
    if slope <= 0:
        return slope, None
    latest = values[-1]
    if latest >= 90:
        return slope, 0
    hours_to_90 = ((90 - latest) / slope) * 0.5
    return slope, round(hours_to_90, 1)


def predict_health(duration_hours: int = 24) -> Dict:
    data = get_trend(duration_hours)
    if not data:
        return {"predictions": [], "summary": "insufficient data"}

    mem_vals = [d["memory_percent"] for d in data if d["memory_percent"] is not None]
    cpu_vals = [d["cpu_percent"] for d in data if d["cpu_percent"] is not None]
    disk_vals = [d["disk_percent"] for d in data if d["disk_percent"] is not None]
    bat_vals = [d["battery_percent"] for d in data if d["battery_percent"] is not None]

    predictions = []

    mem_slope, mem_hours = _linear_trend(mem_vals) if mem_vals else (0, None)
    if mem_hours is not None and mem_hours < 48:
        predictions.append({
            "metric": "memory",
            "severity": "critical" if mem_hours < 6 else "warning",
            "message": f"Memory trending up ({mem_slope:.1f}%/hr). Projected 90% in ~{mem_hours}h",
            "hours_to_threshold": mem_hours,
        })

    disk_slope, disk_hours = _linear_trend(disk_vals) if disk_vals else (0, None)
    if disk_hours is not None and disk_hours < 72:
        predictions.append({
            "metric": "disk",
            "severity": "critical" if disk_hours < 12 else "warning",
            "message": f"Disk filling ({disk_slope:.1f}%/hr). Projected 90% in ~{disk_hours}h",
            "hours_to_threshold": disk_hours,
        })

    if cpu_vals and len(cpu_vals) > 3:
        avg_cpu = sum(cpu_vals) / len(cpu_vals)
        if avg_cpu > 80:
            predictions.append({
                "metric": "cpu",
                "severity": "warning",
                "message": f"Sustained high CPU: {avg_cpu:.0f}% average over {duration_hours}h",
                "hours_to_threshold": None,
            })

    if bat_vals and len(bat_vals) > 3:
        bat_slope, bat_hours = _linear_trend(bat_vals)
        if bat_hours is not None and bat_hours < 12:
            predictions.append({
                "metric": "battery",
                "severity": "critical" if bat_hours < 2 else "warning",
                "message": f"Battery draining ({bat_slope:.1f}%/hr). Estimated {bat_hours}h remaining",
                "hours_to_threshold": bat_hours,
            })

    return {
        "predictions": predictions,
        "summary": f"{len(predictions)} issue{'s' if len(predictions) != 1 else ''} detected" if predictions else "no issues detected",
        "data_points": len(data),
    }


def get_chart_data(duration_hours: int = 24, max_points: int = 50) -> Tuple[List[str], List[float], List[float], List[int]]:
    """
    Get trend data formatted for chart rendering.

    Returns:
        (timestamps, cpu_values, memory_values, node_counts)
    """
    data = get_trend(duration_hours)

    # Sample to max_points
    if len(data) > max_points:
        step = len(data) / max_points
        data = [data[int(i * step)] for i in range(max_points)]

    timestamps = [d["timestamp"] for d in data]
    cpu_values = [d["cpu_percent"] for d in data]
    memory_values = [d["memory_percent"] for d in data]
    node_counts = [d["node_count"] for d in data]

    return timestamps, cpu_values, memory_values, node_counts
