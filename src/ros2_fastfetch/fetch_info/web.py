"""
ROS2 Info — Web Dashboard Server
Flask-based web server with graph API, terminal execution API, and SSE streaming.
"""

import os
import subprocess
import time
import urllib.request
import xml.etree.ElementTree as ET
from flask import Flask, abort, jsonify, render_template, request, send_file


def create_app():
    template_dir = os.path.join(os.path.dirname(__file__), 'templates')
    app = Flask(__name__, template_folder=template_dir)

    from fetch_info.collector import system, ros2, workspace

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
    def api_graph():
        """Return node/topic connection graph as JSON."""
        from fetch_info.terminal import build_topic_graph
        timeout = int(request.args.get("timeout", 5))
        graph = build_topic_graph(timeout=timeout)
        return jsonify(graph)

    @app.route("/api/exec", methods=["POST"])
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

        return jsonify({"output": out})

    @app.route("/api/status")
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

    return app


def run_web(host="0.0.0.0", port=8099):
    app = create_app()
    app.run(host=host, port=port, debug=False)
