"""Topic rates from a running ros2_pulse exporter (https://github.com/TanayK07/ros2_pulse).

ros2_pulse counts publishes inside each process (LD_PRELOAD probe) and
`pulse-export` serves the counts as Prometheus text. One HTTP read gives the
rate of every probed topic, with no subscriber and no per-topic wait.
"""

import os
import re
import urllib.request
from typing import Dict, Optional

DEFAULT_URL = "http://127.0.0.1:10061/metrics"
URL_ENV = "ROS2_PULSE_METRICS_URL"
PUBLISH_RATE = "ros2_pulse_topic_publish_rate_hertz"

_SAMPLE = re.compile(r"^([a-zA-Z_:][a-zA-Z0-9_:]*)(?:\{(.*)\})?\s+(\S+)")
_LABEL = re.compile(r'([a-zA-Z_][a-zA-Z0-9_]*)="((?:[^"\\]|\\.)*)"')
_UNESCAPE = {"\\\\": "\\", '\\"': '"', "\\n": "\n"}


def _unescape(value: str) -> str:
    return re.sub(r'\\[\\"n]', lambda m: _UNESCAPE[m.group(0)], value)


def parse_publish_rates(text: str) -> Dict[str, float]:
    """Per-topic publish rate (Hz) from a pulse-export /metrics body.

    The probe reports each process's inter and intra process paths separately
    and one publish can go out on both, so the busier path is the process's
    rate. Different processes publishing on one topic add up, which is what a
    subscriber such as `ros2 topic hz` would see.
    """
    per_process: Dict[tuple, float] = {}
    for line in text.splitlines():
        m = _SAMPLE.match(line)
        if not m or m.group(1) != PUBLISH_RATE:
            continue
        labels = {k: _unescape(v) for k, v in _LABEL.findall(m.group(2) or "")}
        topic = labels.get("topic")
        if not topic:
            continue
        try:
            hz = float(m.group(3))
        except ValueError:
            continue
        if hz != hz:  # NaN
            continue
        key = (labels.get("pid", ""), topic)
        per_process[key] = max(hz, per_process.get(key, 0.0))

    rates: Dict[str, float] = {}
    for (_, topic), hz in per_process.items():
        rates[topic] = rates.get(topic, 0.0) + hz
    return rates


def get_pulse_rates(url: Optional[str] = None, timeout: float = 0.5) -> Dict[str, float]:
    """Topic rates from pulse-export, or {} when it is not running or unreadable."""
    url = url or os.environ.get(URL_ENV) or DEFAULT_URL
    try:
        with urllib.request.urlopen(url, timeout=timeout) as resp:
            body = resp.read().decode("utf-8", errors="replace")
    except Exception:
        return {}
    return parse_publish_rates(body)
