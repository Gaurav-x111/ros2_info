"""
ROS2 Info — Web Dashboard Server
Flask-based web server with graph API, terminal execution API, and SSE streaming.
"""

import os
import subprocess
import time
import urllib.request
import xml.etree.ElementTree as ET
from functools import wraps
from flask import Flask, Response, Response, abort, jsonify, render_template, request, send_file


def create_app():
    template_dir = os.path.join(os.path.dirname(__file__), 'templates')
    app = Flask(__name__, template_folder=template_dir)

    from fetch_info.collector import system, ros2, workspace

    _auth_username = os.environ.get('ROS2_INFO_USERNAME')
    _auth_password = os.environ.get('ROS2_INFO_PASSWORD')

    # Rate limiting: track request counts per IP
    _rate_limit_window = 60  # seconds
    _rate_limit_max_requests = 100  # max requests per window
    _request_counts = {}  # ip -> [(timestamp, count)]

    def _get_client_ip():
        return request.remote_addr or "127.0.0.1"

    def _check_rate_limit():
        """Check if client IP exceeds rate limit. Returns True if allowed."""
        ip = _get_client_ip()
        now = time.time()

        if ip not in _request_counts:
            _request_counts[ip] = []

        # Remove old entries outside window
        _request_counts[ip] = [(t, c) for t, c in _request_counts[ip] if now - t < _rate_limit_window]

        # Count total requests in window
        total = sum(c for _, c in _request_counts[ip])

        if total >= _rate_limit_max_requests:
            return False

        # Add this request
        if not _request_counts[ip] or now - _request_counts[ip][-1][0] > 1:
            _request_counts[ip].append((now, 1))
        else:
            t, c = _request_counts[ip][-1]
            _request_counts[ip][-1] = (t, c + 1)

        return True

    def rate_limit(f):
        """Decorator to enforce rate limiting."""
        @wraps(f)
        def decorated(*args, **kwargs):
            if not _check_rate_limit():
                return Response('Rate limit exceeded. Try again later.', 429)
            return f(*args, **kwargs)
        return decorated

    # Command audit logging
    _audit_log = []  # List of {timestamp, ip, cmd, result}
    _audit_log_file = os.path.expanduser("~/.ros2_info_web_audit.log")

    def _audit_log_command(ip, cmd, result):
        """Log command execution for audit trail."""
        entry = {
            "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
            "ip": ip,
            "cmd": cmd,
            "result": result[:200] if result else "",
        }
        _audit_log.append(entry)
        # Keep last 1000 entries in memory
        if len(_audit_log) > 1000:
            _audit_log.pop(0)
        # Also write to file
        try:
            with open(_audit_log_file, "a") as f:
                f.write(f"{entry['timestamp']} | {entry['ip']} | {entry['cmd']} | {entry['result']}\n")
        except Exception:
            pass

    if _auth_username and _auth_password:
        @app.before_request
        def require_auth():
            auth = request.authorization
            if not auth or auth.username != _auth_username or auth.password != _auth_password:
                return Response('Unauthorized', 401, {'WWW-Authenticate': 'Basic realm="ROS2 Info"'})

    def _collect_all():
        data = {}
        data["system"] = system.collect_all()
        data["ros2"] = ros2.collect_all(check_live=True, live_timeout=3, check_updates=True)
        data["workspace"] = workspace.collect_all()
        return data

    @app.route("/")
    def index():
        return render_template("index.html")

    @app.route("/api/info")
    @rate_limit
    def api_info():
        data = _collect_all()
        return jsonify(data)

    @app.route("/api/logo/<distro>.png")
    def api_logo(distro):
        """Serve high-resolution distro logo assets for the dashboard."""
        from fetch_info.display.fastfetch import resolve_logo_image_path

        asset_path = resolve_logo_image_path(distro)
        if not asset_path:
            abort(404)
        return send_file(asset_path, mimetype="image/png", max_age=3600)

    @app.route("/api/graph")
    @rate_limit
    def api_graph():
        """Return node/topic connection graph as JSON."""
        from fetch_info.terminal import build_topic_graph
        timeout = int(request.args.get("timeout", 5))
        graph = build_topic_graph(timeout=timeout)
        return jsonify(graph)

    @app.route("/api/exec", methods=["POST"])
    @rate_limit
    def api_exec():
        """
        Execute a safe subset of ros2 commands from the web terminal.
        Body JSON: { "cmd": "nodes" | "topics" | "services" | "actions" | "env" | "node info <name>" | "param list [node]" | "bag info <file>" }
        Returns: { "output": "<text>" }
        """
        body = request.get_json(silent=True) or {}
        cmd_str = (body.get("cmd") or "").strip()
        if not cmd_str:
            return jsonify({"output": "Error: empty command"}), 400

        # Audit log the command
        client_ip = _get_client_ip()
        _audit_log_command(client_ip, cmd_str, "")

        tokens = cmd_str.split()
        verb = tokens[0].lower()

        ALLOWED = {
            "nodes": ["ros2", "node", "list"],
            "topics": ["ros2", "topic", "list", "-t"],
            "services": ["ros2", "service", "list"],
            "actions": ["ros2", "action", "list"],
            "env": None,   # handled specially
        }

        ros2_cmd = None

        if verb == "env":
            from fetch_info.collector.ros2 import get_ros2_environment
            env = get_ros2_environment()
            lines = [f"{k}={v}" for k, v in env.items()]
            return jsonify({"output": "\n".join(lines)})

        elif verb in ALLOWED and ALLOWED[verb]:
            ros2_cmd = ALLOWED[verb]

        elif verb == "node" and len(tokens) >= 3 and tokens[1] == "info":
            node_name = tokens[2]
            if node_name.startswith("/"):
                ros2_cmd = ["ros2", "node", "info", node_name]

        elif verb == "param" and len(tokens) >= 2 and tokens[1] == "list":
            ros2_cmd = ["ros2", "param", "list"] + (tokens[2:] if len(tokens) > 2 else [])

        elif verb == "bag" and len(tokens) >= 3 and tokens[1] == "info":
            ros2_cmd = ["ros2", "bag", "info", tokens[2]]

        elif verb == "interface" and len(tokens) >= 3 and tokens[1] == "show":
            ros2_cmd = ["ros2", "interface", "show", tokens[2]]

        elif verb == "hz" and len(tokens) >= 2:
            return jsonify({"output": "hz runs streaming — use CLI: ros2_info terminal"})

        elif verb == "echo" and len(tokens) >= 2:
            return jsonify({"output": "echo runs streaming — use CLI: ros2_info terminal"})

        else:
            return jsonify({"output": f"Command '{verb}' not supported in web terminal.\n"
                                       "Supported: nodes, topics, services, actions, env, "
                                       "node info <n>, param list [n], bag info <f>, interface show <t>"}), 400

        try:
            result = subprocess.run(
                ros2_cmd, capture_output=True, text=True,
                timeout=5, env={**os.environ}
            )
            out = result.stdout.strip() or result.stderr.strip() or "(no output)"
        except subprocess.TimeoutExpired:
            out = "Timed out (5s). ROS2 daemon may not be running — try: ros2 daemon start"
        except Exception as e:
            out = f"Error: {e}"

        # Update audit log with result
        _audit_log_command(client_ip, cmd_str, out)

        return jsonify({"output": out})

    @app.route("/api/status")
    @rate_limit
    def api_status():
        """Lightweight system status for quick polling (no ROS2 calls)."""
        stats = {"timestamp": time.time()}
        try:
            import psutil
            stats["cpu_percent"] = psutil.cpu_percent(interval=0.1)
            vm = psutil.virtual_memory()
            stats["mem_percent"] = vm.percent
            stats["mem_used_gb"] = round(vm.used / 1e9, 1)
            stats["mem_total_gb"] = round(vm.total / 1e9, 1)
            disk = psutil.disk_usage("/")
            stats["disk_percent"] = disk.percent
            bat = psutil.sensors_battery()
            if bat:
                stats["battery"] = {"percent": round(bat.percent, 1), "plugged": bat.power_plugged}
            temps = psutil.sensors_temperatures()
            if temps:
                for name, entries in list(temps.items())[:2]:
                    for entry in entries[:1]:
                        if entry.current:
                            stats.setdefault("temps", {})[name] = round(entry.current, 1)
            net = psutil.net_io_counters()
            stats["net_sent_mb"] = round(net.bytes_sent / 1e6, 1)
            stats["net_recv_mb"] = round(net.bytes_recv / 1e6, 1)
        except ImportError:
            pass
        except Exception:
            pass
        return jsonify(stats)

    @app.route("/api/blog")
    @rate_limit
    def api_blog():
        """Fetch and parse the official ROS2 blog RSS feed."""
        entries = []
        try:
            urls = [
                "https://planet.ros.org/rss20.xml",
                "https://discourse.ros.org/latest.rss",
            ]
            for url in urls:
                try:
                    req = urllib.request.Request(url, headers={"User-Agent": "ROS2Info/2.0"})
                    with urllib.request.urlopen(req, timeout=8) as resp:
                        xml_data = resp.read().decode("utf-8", errors="replace")

                    root = ET.fromstring(xml_data)

                    for item in root.findall(".//item")[:15]:
                        title = item.findtext("title", "")
                        link = item.findtext("link", "")
                        pub_date = item.findtext("pubDate", "")
                        desc = item.findtext("description", "")
                        if desc:
                            import re
                            desc = re.sub(r'<[^>]+>', '', desc)
                            desc = desc.strip()[:200]
                            if len(desc) == 200:
                                desc += "..."
                        entries.append({
                            "title": title,
                            "link": link,
                            "date": pub_date,
                            "summary": desc,
                            "source": "Planet ROS" if "planet" in url else "ROS Discourse",
                        })
                    if entries:
                        break
                except Exception:
                    continue
        except Exception as e:
            entries = [{"title": "Could not fetch blog feed", "link": "", "date": "", "summary": str(e), "source": ""}]

        return jsonify(entries[:12])

    @app.route("/api/audit")
    @rate_limit
    def api_audit():
        """Return recent command audit log (for administrators)."""
        # Only return last 100 entries
        recent = _audit_log[-100:] if _audit_log else []
        return jsonify(recent)

    @app.route("/api/health")
    def api_health():
        """Health check endpoint for monitoring."""
        return jsonify({
            "status": "healthy",
            "timestamp": time.time(),
            "version": "2.4.0"
        })

    @app.route("/api/health/predict")
    @rate_limit
    def api_health_predict():
        """Predictive health analysis from trend data."""
        from fetch_info.collector.trends import predict_health
        hours = int(request.args.get("hours", 24))
        return jsonify(predict_health(duration_hours=hours))

    @app.route("/api/fleet/check", methods=["POST"])
    @rate_limit
    def api_fleet_check():
        """Check fleet hosts and return alerts."""
        from fetch_info.collector.fleet import FleetHost, check_alerts, collect_fleet
        body = request.get_json(silent=True) or {}
        hosts = body.get("hosts", [])
        if not hosts:
            return jsonify({"alerts": [], "message": "no hosts provided"}), 400
        fleet_hosts = [FleetHost(**h) for h in hosts]
        results = collect_fleet(fleet_hosts)
        alerts = check_alerts(results)
        return jsonify({"results": results, "alerts": alerts})

    return app


def _ssl_context(cert=None, key=None):
    if cert and key:
        return (cert, key)
    if cert:
        return cert
    cert_dir = os.path.expanduser("~/.ros2_info/certs")
    auto_cert = os.path.join(cert_dir, "cert.pem")
    auto_key = os.path.join(cert_dir, "key.pem")
    if os.path.exists(auto_cert) and os.path.exists(auto_key):
        return (auto_cert, auto_key)
    return "adhoc"


def run_web(host="0.0.0.0", port=8099, ssl=False, cert=None, key=None):
    app = create_app()
    ctx = _ssl_context(cert, key) if ssl else None
    protocol = "https" if ctx else "http"
    print(f"Starting on {protocol}://{host}:{port}")
    app.run(host=host, port=port, debug=False, ssl_context=ctx)
