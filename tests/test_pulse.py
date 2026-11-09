import socket
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pytest

from fetch_info.collector import pulse

# Shaped like pulse-export's /metrics output.
METRICS = """\
# HELP ros2_pulse_topic_publish_rate_hertz Publish rate in the process's latest window.
# TYPE ros2_pulse_topic_publish_rate_hertz gauge
ros2_pulse_topic_publish_rate_hertz{pid="4242",topic="/scan",path="inter"} 10
ros2_pulse_topic_publish_rate_hertz{pid="4242",topic="/cmd_vel",path="inter"} 19.8
ros2_pulse_topic_publish_rate_hertz{pid="4242",topic="/cmd_vel",path="intra"} 20.2
ros2_pulse_topic_publish_rate_hertz{pid="5151",topic="/scan",path="inter"} 5
ros2_pulse_topic_publish_rate_hertz{pid="5151",topic="/odd\\"name",path="inter"} 1.5
# HELP ros2_pulse_topic_receive_rate_hertz Subscription callback rate.
# TYPE ros2_pulse_topic_receive_rate_hertz gauge
ros2_pulse_topic_receive_rate_hertz{pid="6000",topic="/only_received",path="inter"} 30
ros2_pulse_process_window_seconds{pid="4242"} 5
ros2_pulse_lines_skipped_total 0
"""


def test_parse_takes_busier_path_and_sums_processes():
    rates = pulse.parse_publish_rates(METRICS)
    assert rates == {"/scan": 15.0, "/cmd_vel": 20.2, '/odd"name': 1.5}


def test_parse_ignores_garbage():
    assert pulse.parse_publish_rates("") == {}
    assert pulse.parse_publish_rates("not prometheus\n{}\n") == {}
    bad = 'ros2_pulse_topic_publish_rate_hertz{pid="1",topic="/a",path="inter"} NaN\n'
    assert pulse.parse_publish_rates(bad) == {}


@pytest.fixture
def exporter():
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            body = METRICS.encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; version=0.0.4")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, format, *args):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    yield f"http://127.0.0.1:{server.server_address[1]}/metrics"
    server.shutdown()
    server.server_close()


def test_get_rates_from_exporter(exporter):
    assert pulse.get_pulse_rates(exporter)["/scan"] == 15.0


def test_url_from_env(exporter, monkeypatch):
    monkeypatch.setenv(pulse.URL_ENV, exporter)
    assert pulse.get_pulse_rates()["/cmd_vel"] == 20.2


def test_exporter_down_returns_empty():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
    assert pulse.get_pulse_rates(f"http://127.0.0.1:{port}/metrics") == {}


def _run_benchmark(monkeypatch, pulse_rates, topics):
    pytest.importorskip("click")
    pytest.importorskip("rich")
    import json
    import subprocess
    from click.testing import CliRunner
    from fetch_info.commands import monitoring

    hz_calls = []

    def fake_run(cmd, **kwargs):
        hz_calls.append(cmd[3])
        return subprocess.CompletedProcess(cmd, 0, stdout="average rate: 1.000\n", stderr="")

    monkeypatch.setattr(monitoring.pulse, "get_pulse_rates", lambda: pulse_rates)
    monkeypatch.setattr(monitoring.ros2, "get_active_topics", lambda timeout=3: [{"name": n} for n in topics])
    monkeypatch.setattr(monitoring.ros2, "get_active_nodes", lambda timeout=3: [])
    monkeypatch.setattr(monitoring.system, "collect_all", lambda: {})
    monkeypatch.setattr(monitoring.subprocess, "run", fake_run)
    result = CliRunner().invoke(monitoring.cmd_benchmark, ["--json"])
    assert result.exit_code == 0, result.output
    out = json.loads(result.output[result.output.index("{"):])
    return out, hz_calls


def test_benchmark_uses_pulse_and_falls_back_per_topic(monkeypatch):
    out, hz_calls = _run_benchmark(monkeypatch, {"/scan": 15.0}, ["/scan", "/tf"])
    assert hz_calls == ["/tf"]
    assert out["topic_rates"] == {"/scan": "15.00 Hz", "/tf": "average rate: 1.000"}
    assert out["topic_rate_source"] == {"/scan": "ros2_pulse", "/tf": "topic hz"}


def test_benchmark_without_pulse_keeps_topic_hz_for_first_ten(monkeypatch):
    topics = [f"/t{i}" for i in range(12)]
    out, hz_calls = _run_benchmark(monkeypatch, {}, topics)
    assert hz_calls == topics[:10]
    assert set(out["topic_rate_source"].values()) == {"topic hz"}
