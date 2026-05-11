"""
ROS2 Interactive Terminal — REPL shell for ROS2 developers.
Provides: run, pub, sub, echo, param, service, bag, graph, launch, info, and more.
"""

import os
import readline
import shutil
import subprocess
import time
from rich.console import Console
from rich.panel import Panel
from rich.table import Table
from rich.text import Text
from rich.rule import Rule
from rich import box
from rich.syntax import Syntax


# ── Command History ──────────────────────────────────────────────────────────
HISTORY_FILE = os.path.expanduser("~/.ros2_info_history")

def _setup_readline():
    """Configure readline for history and tab-completion."""
    try:
        readline.read_history_file(HISTORY_FILE)
    except FileNotFoundError:
        pass
    readline.set_history_length(500)
    readline.parse_and_bind("tab: complete")


def _save_history():
    try:
        readline.write_history_file(HISTORY_FILE)
    except Exception:
        pass


# ── Shell Helpers ────────────────────────────────────────────────────────────
def _run(cmd: list, timeout: int = 10, input_data: str = None) -> tuple[str, str, int]:
    """Run a command, return (stdout, stderr, returncode)."""
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            env={**os.environ},
            input=input_data,
        )
        return result.stdout.strip(), result.stderr.strip(), result.returncode
    except subprocess.TimeoutExpired:
        return "", "Command timed out", 1
    except FileNotFoundError:
        return "", f"Command not found: {cmd[0]}", 127
    except Exception as e:
        return "", str(e), 1


def _run_streaming(cmd: list, console: Console, theme: dict, timeout: int = 30):
    """Run a command with streaming output."""
    try:
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            env={**os.environ},
        )
        start = time.time()
        console.print(f"  [{theme['dim_style']}]PID {proc.pid} — Ctrl+C to stop[/]\n")
        try:
            for line in proc.stdout:
                console.print(f"  [{theme['value_style']}]{line.rstrip()}[/]")
                if time.time() - start > timeout:
                    proc.terminate()
                    break
        except KeyboardInterrupt:
            proc.terminate()
            console.print(f"\n  [{theme['warn_style']}]Interrupted.[/]")
        proc.wait()
    except Exception as e:
        console.print(f"  [{theme['error_style']}]Error: {e}[/]")


def _ros2_available() -> bool:
    return shutil.which("ros2") is not None


# ── ASCII RQT-like Graph ─────────────────────────────────────────────────────
def build_topic_graph(timeout: int = 5) -> dict:
    """Build a node→topic→node connection map."""
    graph = {"nodes": {}, "topics": {}}

    if not _ros2_available():
        return graph

    # Get nodes
    stdout, _, rc = _run(["ros2", "node", "list"], timeout=timeout)
    if rc != 0 or not stdout:
        return graph

    nodes = [n.strip() for n in stdout.split("\n") if n.strip()]

    for node in nodes:
        graph["nodes"][node] = {"pubs": [], "subs": []}

        # Publishers
        pub_out, _, _ = _run(["ros2", "node", "info", node], timeout=timeout)
        section = None
        for line in pub_out.split("\n"):
            line = line.strip()
            if "Publishers:" in line:
                section = "pubs"
            elif "Subscribers:" in line:
                section = "subs"
            elif "Service Servers:" in line or "Service Clients:" in line:
                section = None
            elif section and line.startswith("/"):
                topic = line.split(":")[0].strip()
                if topic not in graph["nodes"][node][section]:
                    graph["nodes"][node][section].append(topic)
                # Track topic → nodes
                if topic not in graph["topics"]:
                    graph["topics"][topic] = {"publishers": [], "subscribers": []}
                if section == "pubs" and node not in graph["topics"][topic]["publishers"]:
                    graph["topics"][topic]["publishers"].append(node)
                if section == "subs" and node not in graph["topics"][topic]["subscribers"]:
                    graph["topics"][topic]["subscribers"].append(node)

    return graph


def render_ascii_graph(console: Console, theme: dict, timeout: int = 5):
    """Render an ASCII rqt-graph style visualization."""
    console.print(f"\n  [{theme['highlight']}]Building RQT-style graph...[/]")
    graph = build_topic_graph(timeout)

    if not graph["nodes"]:
        console.print(f"  [{theme['warn_style']}]No nodes found. Is ROS2 running?[/]")
        return

    nodes = list(graph["nodes"].keys())
    topics = graph["topics"]

    # Filter to only connected topics
    connected = {t: v for t, v in topics.items()
                 if v["publishers"] and v["subscribers"]}

    console.print()

    # Header
    title = Text()
    title.append("  ⬡ RQT GRAPH", style=f"bold {theme['logo_color1']}")
    title.append(f"  {len(nodes)} nodes", style=theme['dim_style'])
    title.append(f"  •  {len(topics)} topics", style=theme['dim_style'])
    title.append(f"  •  {len(connected)} connections", style=theme['dim_style'])
    console.print(title)
    console.print(Rule(style=theme["panel_border"]))
    console.print()

    if connected:
        console.print(f"  [{theme['section_title']}]Topic Connections:[/]\n")
        for topic, conns in list(connected.items())[:20]:
            # Publisher(s) → Topic → Subscriber(s)
            pubs = conns["publishers"]
            subs = conns["subscribers"]

            t = Text()
            # Publishers
            for i, pub in enumerate(pubs[:3]):
                short = pub.split("/")[-1]
                if i > 0:
                    t.append("\n" + " " * 6)
                t.append(f"[{short}]", style=f"bold {theme['ok_style']}")

            t.append(" ──► ", style=theme["dim_style"])
            # Topic
            t.append(topic, style=f"bold {theme['highlight']}")
            t.append(" ──► ", style=theme["dim_style"])
            # Subscribers
            for i, sub in enumerate(subs[:3]):
                short = sub.split("/")[-1]
                if i > 0:
                    t.append(", ")
                t.append(f"[{short}]", style=f"bold {theme['logo_color2']}")

            console.print("  ", t)

        if len(connected) > 20:
            console.print(f"\n  [{theme['dim_style']}]... and {len(connected)-20} more connections[/]")
    else:
        console.print(f"  [{theme['dim_style']}]No pub→sub connections found (topics may have no subscribers).[/]")

    console.print()

    # Node table
    console.print(f"  [{theme['section_title']}]Node Overview:[/]\n")
    tbl = Table(border_style=theme["panel_border"], box=box.MINIMAL_HEAVY_HEAD, show_lines=False)
    tbl.add_column("Node", style=f"bold {theme['key_style']}", no_wrap=True)
    tbl.add_column("Publishes", style=theme["ok_style"])
    tbl.add_column("Subscribes", style=theme["logo_color2"])

    for node in nodes[:25]:
        pubs = graph["nodes"][node]["pubs"]
        subs = graph["nodes"][node]["subs"]
        pub_str = "\n".join(p.split("/")[-1] for p in pubs[:4]) or "—"
        sub_str = "\n".join(s.split("/")[-1] for s in subs[:4]) or "—"
        if len(pubs) > 4:
            pub_str += f"\n+{len(pubs)-4} more"
        if len(subs) > 4:
            sub_str += f"\n+{len(subs)-4} more"
        tbl.add_row(node, pub_str, sub_str)

    console.print(tbl)
    console.print()


# ── Command Handlers ─────────────────────────────────────────────────────────
def cmd_echo(args: list, console: Console, theme: dict):
    """Echo a ROS2 topic: echo <topic> [--once]"""
    if not args:
        console.print(f"  [{theme['error_style']}]Usage: echo <topic> [--once][/]")
        return
    topic = args[0]
    once = "--once" in args
    ros_cmd = ["ros2", "topic", "echo"] + ([topic, "--once"] if once else [topic])
    _run_streaming(ros_cmd, console, theme, timeout=15)


def cmd_pub(args: list, console: Console, theme: dict):
    """Publish to a topic: pub <topic> <type> <yaml> [--once]"""
    if len(args) < 3:
        console.print(f"  [{theme['error_style']}]Usage: pub <topic> <msg_type> '{{data: value}}' [--once][/]")
        console.print(f"  [{theme['dim_style']}]Example: pub /chatter std_msgs/msg/String '{{data: hello}}' --once[/]")
        return
    topic, msg_type, data = args[0], args[1], args[2]
    ros_cmd = ["ros2", "topic", "pub"] + (["--once"] if "--once" in args else []) + [topic, msg_type, data]
    _run_streaming(ros_cmd, console, theme, timeout=10)


def cmd_node_info(args: list, console: Console, theme: dict):
    """Show info for a node: node info <node_name>"""
    if not args:
        console.print(f"  [{theme['error_style']}]Usage: node info <node_name>[/]")
        return
    stdout, stderr, rc = _run(["ros2", "node", "info", args[0]], timeout=8)
    if rc != 0:
        console.print(f"  [{theme['error_style']}]{stderr or 'Node not found'}[/]")
        return
    syntax = Syntax(stdout, "yaml", theme="monokai", background_color="default")
    console.print(Panel(syntax, title=f"[bold]Node: {args[0]}[/]",
                        border_style=theme["panel_border"], box=box.ROUNDED))


def cmd_service_call(args: list, console: Console, theme: dict):
    """Call a ROS2 service: service call <srv> <type> [<yaml>]"""
    if len(args) < 2:
        console.print(f"  [{theme['error_style']}]Usage: service call <service> <type> ['{{}}'][/]")
        return
    srv, srv_type = args[0], args[1]
    data = args[2] if len(args) > 2 else "{}"
    ros_cmd = ["ros2", "service", "call", srv, srv_type, data]
    _run_streaming(ros_cmd, console, theme, timeout=10)


def cmd_param(args: list, console: Console, theme: dict):
    """ROS2 param: param get/set/list <node> [<param>] [<value>]"""
    if not args:
        console.print(f"  [{theme['error_style']}]Usage: param get|set|list <node> [<param>] [<value>][/]")
        return
    subcmd = args[0]
    if subcmd == "list":
        node = args[1] if len(args) > 1 else None
        ros_cmd = ["ros2", "param", "list"] + ([node] if node else [])
        stdout, stderr, rc = _run(ros_cmd, timeout=8)
        if rc != 0:
            console.print(f"  [{theme['error_style']}]{stderr}[/]")
            return
        for line in stdout.split("\n"):
            if line.strip():
                console.print(f"  [{theme['value_style']}]{line}[/]")
    elif subcmd == "get" and len(args) >= 3:
        stdout, stderr, rc = _run(["ros2", "param", "get", args[1], args[2]], timeout=8)
        if rc != 0:
            console.print(f"  [{theme['error_style']}]{stderr}[/]")
        else:
            console.print(f"  [{theme['ok_style']}]{stdout}[/]")
    elif subcmd == "set" and len(args) >= 4:
        stdout, stderr, rc = _run(["ros2", "param", "set", args[1], args[2], args[3]], timeout=8)
        if rc != 0:
            console.print(f"  [{theme['error_style']}]{stderr}[/]")
        else:
            console.print(f"  [{theme['ok_style']}]✓ {stdout}[/]")
    else:
        console.print(f"  [{theme['error_style']}]Usage: param get|set|list <node> [<param>] [<value>][/]")


def cmd_bag(args: list, console: Console, theme: dict):
    """ROS2 bag: bag record [topics...] | bag play <file> | bag info <file>"""
    if not args:
        console.print(f"  [{theme['error_style']}]Usage: bag record|play|info [...][/]")
        return
    subcmd = args[0]
    if subcmd == "record":
        topics = args[1:] if len(args) > 1 else ["-a"]
        ts = time.strftime("%Y%m%d_%H%M%S")
        bag_name = f"ros2_bag_{ts}"
        ros_cmd = ["ros2", "bag", "record", "-o", bag_name] + topics
        console.print(f"  [{theme['ok_style']}]Recording to: {bag_name}/  (Ctrl+C to stop)[/]")
        _run_streaming(ros_cmd, console, theme, timeout=3600)
    elif subcmd == "play" and len(args) >= 2:
        ros_cmd = ["ros2", "bag", "play"] + args[1:]
        _run_streaming(ros_cmd, console, theme, timeout=3600)
    elif subcmd == "info" and len(args) >= 2:
        stdout, stderr, rc = _run(["ros2", "bag", "info", args[1]], timeout=10)
        if rc != 0:
            console.print(f"  [{theme['error_style']}]{stderr}[/]")
        else:
            syntax = Syntax(stdout, "yaml", theme="monokai", background_color="default")
            console.print(Panel(syntax, title="[bold]Bag Info[/]",
                                border_style=theme["panel_border"], box=box.ROUNDED))
    else:
        console.print(f"  [{theme['error_style']}]Unknown bag subcommand. Use: record | play | info[/]")


def cmd_launch(args: list, console: Console, theme: dict):
    """Launch a ROS2 file: launch <pkg> <file.launch.py> [args...]"""
    if len(args) < 2:
        console.print(f"  [{theme['error_style']}]Usage: launch <package> <launch_file> [key:=val ...][/]")
        return
    ros_cmd = ["ros2", "launch"] + args
    _run_streaming(ros_cmd, console, theme, timeout=120)


def cmd_run_node(args: list, console: Console, theme: dict):
    """Run a ROS2 node: run <package> <executable> [args...]"""
    if len(args) < 2:
        console.print(f"  [{theme['error_style']}]Usage: run <package> <executable> [args...][/]")
        return
    ros_cmd = ["ros2", "run"] + args
    _run_streaming(ros_cmd, console, theme, timeout=120)


def cmd_topic_hz(args: list, console: Console, theme: dict):
    """Check topic publish rate: hz <topic>"""
    if not args:
        console.print(f"  [{theme['error_style']}]Usage: hz <topic>[/]")
        return
    ros_cmd = ["ros2", "topic", "hz", args[0]]
    _run_streaming(ros_cmd, console, theme, timeout=10)


def cmd_topic_bw(args: list, console: Console, theme: dict):
    """Check topic bandwidth: bw <topic>"""
    if not args:
        console.print(f"  [{theme['error_style']}]Usage: bw <topic>[/]")
        return
    ros_cmd = ["ros2", "topic", "bw", args[0]]
    _run_streaming(ros_cmd, console, theme, timeout=10)


def cmd_shell(args: list, console: Console, theme: dict):
    """Run arbitrary shell command: shell <cmd> [args...]"""
    if not args:
        console.print(f"  [{theme['error_style']}]Usage: shell <command> [args...][/]")
        return
    _run_streaming(args, console, theme, timeout=30)


def cmd_rqt_graph(console: Console, theme: dict):
    """Launch rqt_graph GUI in background."""
    if not shutil.which("rqt_graph"):
        distro = os.environ.get("ROS_DISTRO", "humble")
        console.print(
            f"  [{theme['warn_style']}]rqt_graph not found.[/]\n"
            f"  [{theme['dim_style']}]Install: sudo apt install ros-{distro}-rqt-graph[/]"
        )
        return
    console.print(f"  [{theme['ok_style']}]Launching rqt_graph GUI...[/]")
    try:
        proc = subprocess.Popen(
            ["rqt_graph"],
            env={**os.environ},
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        console.print(f"  [{theme['dim_style']}]PID {proc.pid} — running in background. Close the window to stop.[/]")
    except Exception as e:
        console.print(f"  [{theme['error_style']}]Failed: {e}[/]")


def cmd_tmux(args: list, console: Console, theme: dict):
    """Tmux session management."""
    if not shutil.which("tmux"):
        console.print(
            f"  [{theme['warn_style']}]tmux not found.[/]\n"
            f"  [{theme['dim_style']}]Install: sudo apt install tmux[/]"
        )
        return
    subcmd = args[0].lower() if args else "list"
    if subcmd == "new":
        name = args[1] if len(args) > 1 else f"ros2_{time.strftime('%H%M%S')}"
        rc = subprocess.run(["tmux", "new-session", "-d", "-s", name]).returncode
        if rc == 0:
            console.print(f"  [{theme['ok_style']}]Created session: [bold]{name}[/][/]\n"
                          f"  [{theme['dim_style']}]Attach: tmux attach {name}[/]")
        else:
            console.print(f"  [{theme['error_style']}]Failed (session may already exist)[/]")
    elif subcmd == "list":
        stdout, _, rc = _run(["tmux", "list-sessions"], timeout=5)
        if rc != 0 or not stdout:
            console.print(f"  [{theme['dim_style']}]No tmux sessions active.[/]")
        else:
            tbl = Table(title="Tmux Sessions", border_style=theme["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
            tbl.add_column("Session", style=f"bold {theme['key_style']}")
            tbl.add_column("Info", style=theme["value_style"])
            for line in stdout.strip().split("\n"):
                parts = line.split(":", 1)
                tbl.add_row(parts[0], parts[1].strip() if len(parts) > 1 else "")
            console.print(tbl)
    elif subcmd == "attach":
        name = args[1] if len(args) > 1 else ""
        cmd_args = f"tmux attach-session" + (f" -t {name}" if name else "")
        console.print(f"  [{theme['dim_style']}]Attaching... (Ctrl+B D to detach)[/]")
        os.system(cmd_args)
    elif subcmd == "kill":
        name = args[1] if len(args) > 1 else ""
        if not name:
            console.print(f"  [{theme['error_style']}]Usage: tmux kill <name>[/]")
            return
        _, stderr, rc = _run(["tmux", "kill-session", "-t", name], timeout=5)
        if rc == 0:
            console.print(f"  [{theme['ok_style']}]Killed: {name}[/]")
        else:
            console.print(f"  [{theme['error_style']}]{stderr}[/]")
    elif subcmd in ("split", "hsplit"):
        subprocess.run(["tmux", "split-window", "-v"])
        console.print(f"  [{theme['ok_style']}]Horizontal split created.[/]")
    elif subcmd == "vsplit":
        subprocess.run(["tmux", "split-window", "-h"])
        console.print(f"  [{theme['ok_style']}]Vertical split created.[/]")
    else:
        console.print(f"  [{theme['error_style']}]Unknown: {subcmd}  (new|list|attach|kill|split|vsplit)[/]")


def cmd_colcon(args: list, console: Console, theme: dict):
    """Run colcon commands."""
    if not shutil.which("colcon"):
        console.print(f"  [{theme['warn_style']}]colcon not found.[/]\n"
                      f"  [{theme['dim_style']}]Install: sudo apt install python3-colcon-common-extensions[/]")
        return
    _run_streaming(["colcon"] + args, console, theme, timeout=300)


def cmd_source(args: list, console: Console, theme: dict):
    """Source a bash file into this session's env."""
    if not args:
        console.print(f"  [{theme['error_style']}]Usage: source <file.bash>[/]")
        return
    path = os.path.expanduser(args[0])
    if not os.path.exists(path):
        console.print(f"  [{theme['error_style']}]File not found: {path}[/]")
        return
    try:
        result = subprocess.run(
            f"bash -c 'source {path} && env'",
            shell=True, capture_output=True, text=True, timeout=10
        )
        count = 0
        for line in result.stdout.split("\n"):
            if "=" in line:
                k, _, v = line.partition("=")
                os.environ[k] = v
                count += 1
        console.print(f"  [{theme['ok_style']}]Sourced {path} — {count} env vars loaded.[/]")
    except Exception as e:
        console.print(f"  [{theme['error_style']}]Source failed: {e}[/]")


def cmd_info(console: Console, theme: dict):
    from fetch_info.collector import system, ros2
    from fetch_info.display.fastfetch import render_fastfetch
    try:
        with console.status("[cyan]Collecting ROS2 info...", spinner="dots2"):
            data = {
                "system": system.collect_all(),
                "ros2": ros2.collect_all(check_live=True, live_timeout=2)
            }
        render_fastfetch(console, data, theme)
    except Exception as e:
        import traceback
        console.print(f"  [{theme['error_style']}]Rendering Error: {str(e)}[/]")
        # Hidden debug log for developer
        with open("/tmp/ros2_info_debug.log", "a") as f:
            f.write(f"\n--- {time.ctime()} ---\n")
            traceback.print_exc(file=f)


def cmd_env(console: Console, theme: dict):
    """Show all ROS2 environment variables."""
    from fetch_info.collector.ros2 import get_ros2_environment
    env = get_ros2_environment()
    tbl = Table(border_style=theme["panel_border"], box=box.MINIMAL_HEAVY_HEAD, show_lines=True)
    tbl.add_column("Variable", style=f"bold {theme['key_style']}", no_wrap=True)
    tbl.add_column("Value", style=theme["value_style"])
    for k, v in env.items():
        tbl.add_row(k, v)
    console.print(tbl)


def cmd_nodes_list(console: Console, theme: dict):
    """List all active nodes."""
    from fetch_info.collector.ros2 import get_active_nodes
    nodes = get_active_nodes(timeout=5)
    if not nodes:
        console.print(f"  [{theme['warn_style']}]No active nodes.[/]")
        return
    tbl = Table(title=f"Active Nodes ({len(nodes)})",
                border_style=theme["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
    tbl.add_column("Node", style=f"bold {theme['key_style']}")
    tbl.add_column("Status", style=theme["ok_style"])
    for n in nodes:
        tbl.add_row(n, "● Running")
    console.print(tbl)


def cmd_topics_list(console: Console, theme: dict):
    """List all active topics."""
    from fetch_info.collector.ros2 import get_active_topics
    topics = get_active_topics(timeout=5)
    if not topics:
        console.print(f"  [{theme['warn_style']}]No active topics.[/]")
        return
    tbl = Table(title=f"Active Topics ({len(topics)})",
                border_style=theme["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
    tbl.add_column("Topic", style=f"bold {theme['key_style']}")
    tbl.add_column("Type", style=theme["value_style"])
    for t in topics:
        tbl.add_row(t["name"], t.get("type", "Unknown"))
    console.print(tbl)


def cmd_services_list(console: Console, theme: dict):
    """List all active services."""
    from fetch_info.collector.ros2 import get_active_services
    services = get_active_services(timeout=5)
    if not services:
        console.print(f"  [{theme['warn_style']}]No active services.[/]")
        return
    tbl = Table(title=f"Active Services ({len(services)})",
                border_style=theme["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
    tbl.add_column("Service", style=f"bold {theme['key_style']}")
    for s in services:
        tbl.add_row(s)
    console.print(tbl)


def cmd_actions_list(console: Console, theme: dict):
    """List all active actions."""
    from fetch_info.collector.ros2 import get_active_actions
    actions = get_active_actions(timeout=5)
    if not actions:
        console.print(f"  [{theme['warn_style']}]No active actions.[/]")
        return
    tbl = Table(title=f"Active Actions ({len(actions)})",
                border_style=theme["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
    tbl.add_column("Action", style=f"bold {theme['key_style']}")
    for a in actions:
        tbl.add_row(a)
    console.print(tbl)


def cmd_watch_nodes(console: Console, theme: dict, interval: float = 2.0):
    """Watch nodes in real-time."""
    console.print(f"  [{theme['dim_style']}]Watching nodes — Ctrl+C to stop[/]")
    try:
        while True:
            console.clear()
            cmd_nodes_list(console, theme)
            console.print(f"  [{theme['dim_style']}]Refreshing every {interval}s...[/]")
            time.sleep(interval)
    except KeyboardInterrupt:
        console.print(f"\n  [{theme['dim_style']}]Watch stopped.[/]")


def cmd_ping_node(args: list, console: Console, theme: dict):
    """Ping a node to check if it's alive: ping <node>"""
    if not args:
        console.print(f"  [{theme['error_style']}]Usage: ping <node_name>[/]")
        return
    node = args[0]
    stdout, stderr, rc = _run(["ros2", "node", "info", node], timeout=5)
    if rc == 0 and stdout:
        console.print(f"  [{theme['ok_style']}]✓ Node {node} is alive.[/]")
    else:
        console.print(f"  [{theme['error_style']}]✗ Node {node} not found or unreachable.[/]")


def cmd_interface_show(args: list, console: Console, theme: dict):
    """Show a message/service/action interface: interface show <type>"""
    if not args:
        console.print(f"  [{theme['error_style']}]Usage: interface show <type>[/]")
        console.print(f"  [{theme['dim_style']}]Example: interface show std_msgs/msg/String[/]")
        return
    stdout, stderr, rc = _run(["ros2", "interface", "show", args[0]], timeout=8)
    if rc != 0:
        console.print(f"  [{theme['error_style']}]{stderr}[/]")
    else:
        syntax = Syntax(stdout, "yaml", theme="monokai", background_color="default")
        console.print(Panel(syntax, title=f"[bold]Interface: {args[0]}[/]",
                            border_style=theme["panel_border"], box=box.ROUNDED))


HELP_TEXT = """
╔═══════════════════════════════════════════════════════════════════════════╗
║               ROS2 Info — Interactive Terminal  v2.0                     ║
╚═══════════════════════════════════════════════════════════════════════════╝

  DISCOVERY
    info                System and ROS2 fastfetch overview
    nodes               List active nodes
    topics              List active topics
    services            List active services
    actions             List active actions
    env                 Show ROS2 env variables
    node info <name>    Show pub/sub/service info
    interface show <t>  Show message/srv definition

  MONITORING
    echo <topic> [--once]   Stream topic messages
    hz <topic>              Publish rate
    bw <topic>              Bandwidth
    watch [interval]        Live node refresh (2s)
    ping <node>             Liveness check
    graph [timeout]         ASCII pub→sub graph
    rqt                     Launch rqt_graph GUI (bg)

  ACTIONS
    pub <topic> <type> <yaml> [--once]
    service call <srv> <type> [<yaml>]
    param list [/node]
    param get <node> <param>
    param set <node> <param> <value>
    bag record [-a | topics...]
    bag play <file>
    bag info <file>
    launch <pkg> <file> [key:=val ...]
    run <pkg> <exe> [args...]
    colcon build [--packages-select pkg]
    colcon test

  TMUX
    tmux new [name]     New session
    tmux list           List sessions
    tmux attach [name]  Attach session
    tmux kill <name>    Kill session
    tmux split          Horizontal split
    tmux vsplit         Vertical split
    web [port]          Launch web dashboard (bg)

  SYSTEM
    cd <path>           Change directory
    ls [path]           List files
    pwd                 Print working directory
    shell <cmd>         Run any shell command
    source <file>       Source bash file → env
    history             Show command history
    help                Show this help
    clear               Clear screen
    quit / exit / q     Exit terminal
"""


# ── Tab Completer ─────────────────────────────────────────────────────────────
COMMANDS = [
    "info", "nodes", "topics", "services", "actions", "env", "node", "interface",
    "echo", "hz", "bw", "watch", "ping", "graph", "rqt",
    "pub", "service", "param", "bag", "launch", "run",
    "tmux", "colcon", "cd", "ls", "pwd", "source", "web",
    "shell", "clear", "history", "help", "quit", "exit", "q",
]

class ROS2Completer:
    def __init__(self):
        self._topics = []
        self._nodes = []
        self._last_fetch = 0

    def _refresh(self):
        now = time.time()
        if now - self._last_fetch < 5:
            return
        self._last_fetch = now
        try:
            out = subprocess.run(
                ["ros2", "topic", "list"], capture_output=True, text=True, timeout=2
            ).stdout
            self._topics = [l.strip() for l in out.split("\n") if l.strip()]
            out2 = subprocess.run(
                ["ros2", "node", "list"], capture_output=True, text=True, timeout=2
            ).stdout
            self._nodes = [l.strip() for l in out2.split("\n") if l.strip()]
        except Exception:
            pass

    def complete(self, text, state):
        self._refresh()
        options = COMMANDS + self._topics + self._nodes
        matches = [c for c in options if c.startswith(text)]
        if state < len(matches):
            return matches[state]
        return None


# ── Main Interactive Terminal ─────────────────────────────────────────────────
def run_interactive_terminal(theme_name: str = "default"):
    """Launch the full interactive ROS2 terminal."""
    from fetch_info.display.themes import get_theme
    from fetch_info.display.logo import get_main_banner

    theme = get_theme(theme_name)
    console = Console(width=shutil.get_terminal_size((120, 40)).columns)

    _setup_readline()
    completer = ROS2Completer()
    readline.set_completer(completer.complete)
    readline.parse_and_bind("tab: complete")

    # Splash
    console.clear()
    console.print()
    console.print(get_main_banner(theme))
    console.print()
    panel_text = Text()
    panel_text.append("  ROS2 Interactive Terminal\n", style=f"bold {theme['logo_color1']}")
    panel_text.append("  Type ", style=theme['dim_style'])
    panel_text.append(" help", style=f"bold {theme['highlight']}")
    panel_text.append(" for commands, ", style=theme['dim_style'])
    panel_text.append("web", style=f"bold {theme['highlight']}")
    panel_text.append(" for dashboard. ", style=theme['dim_style'])
    panel_text.append("Tab", style=f"bold {theme['highlight']}")
    panel_text.append(" to autocomplete. ", style=theme['dim_style'])
    panel_text.append("Ctrl+C", style=f"bold {theme['highlight']}")
    panel_text.append(" to interrupt.\n", style=theme['dim_style'])
    panel_text.append("  History saved to: ", style=theme['dim_style'])
    panel_text.append(HISTORY_FILE, style=theme['value_style'])
    console.print(Panel(panel_text, border_style=theme["panel_border"], box=box.ROUNDED))
    console.print()

    prompt = f"\x1b[1;36mros2 ›\x1b[0m "

    while True:
        try:
            line = input(prompt).strip()
        except (EOFError, KeyboardInterrupt):
            _save_history()
            console.print(f"\n  [{theme['dim_style']}]Goodbye! 👋[/]")
            break

        if not line:
            continue

        readline.add_history(line)
        tokens = line.split()
        cmd = tokens[0].lower()
        args = tokens[1:]

        try:
            if cmd in ("quit", "exit", "q"):
                _save_history()
                console.print(f"  [{theme['dim_style']}]Goodbye! 👋[/]")
                break

            elif cmd == "help":
                console.print(Text(HELP_TEXT, style=theme["value_style"]))

            elif cmd == "clear":
                os.system("clear")

            elif cmd == "history":
                count = readline.get_current_history_length()
                for i in range(max(1, count - 25), count + 1):
                    try:
                        item = readline.get_history_item(i)
                        if item:
                            console.print(f"  [{theme['dim_style']}]{i:4d}[/]  [{theme['value_style']}]{item}[/]")
                    except Exception:
                        pass

            elif cmd == "graph":
                t = int(args[0]) if args and args[0].isdigit() else 5
                render_ascii_graph(console, theme, timeout=t)

            elif cmd == "info":
                cmd_info(console, theme)

            elif cmd == "nodes":
                with console.status("[cyan]Querying nodes...", spinner="dots"):
                    cmd_nodes_list(console, theme)

            elif cmd == "topics":
                with console.status("[cyan]Querying topics...", spinner="dots"):
                    cmd_topics_list(console, theme)

            elif cmd == "services":
                with console.status("[cyan]Querying services...", spinner="dots"):
                    cmd_services_list(console, theme)

            elif cmd == "actions":
                with console.status("[cyan]Querying actions...", spinner="dots"):
                    cmd_actions_list(console, theme)

            elif cmd == "env":
                cmd_env(console, theme)

            elif cmd == "node" and args and args[0] == "info":
                cmd_node_info(args[1:], console, theme)

            elif cmd == "interface" and args and args[0] == "show":
                cmd_interface_show(args[1:], console, theme)

            elif cmd == "echo":
                cmd_echo(args, console, theme)

            elif cmd == "hz":
                cmd_topic_hz(args, console, theme)

            elif cmd == "bw":
                cmd_topic_bw(args, console, theme)

            elif cmd == "watch":
                interval = float(args[0]) if args else 2.0
                cmd_watch_nodes(console, theme, interval=interval)

            elif cmd == "ping":
                cmd_ping_node(args, console, theme)

            elif cmd == "pub":
                cmd_pub(args, console, theme)

            elif cmd == "service" and args and args[0] == "call":
                cmd_service_call(args[1:], console, theme)

            elif cmd == "param":
                cmd_param(args, console, theme)

            elif cmd == "bag":
                cmd_bag(args, console, theme)

            elif cmd == "launch":
                cmd_launch(args, console, theme)

            elif cmd == "run":
                cmd_run_node(args, console, theme)

            elif cmd == "rqt":
                cmd_rqt_graph(console, theme)

            elif cmd == "web":
                port = int(args[0]) if args else 8099
                console.print(f"  [bold cyan]🌐 Starting Web UI on http://localhost:{port}[/]")
                console.print(f"  [{theme['dim_style']}]Press Ctrl+C in this terminal to stop the server when done.[/]")
                from fetch_info.web import run_web
                try:
                    run_web(port=port)
                except Exception as e:
                    console.print(f"  [{theme['error_style']}]Web UI Error: {e}[/]")

            elif cmd == "tmux":
                cmd_tmux(args, console, theme)

            elif cmd == "colcon":
                cmd_colcon(args, console, theme)

            elif cmd == "source":
                cmd_source(args, console, theme)

            elif cmd == "cd":
                path = os.path.expanduser(args[0]) if args else os.path.expanduser("~")
                try:
                    os.chdir(path)
                    console.print(f"  [{theme['dim_style']}]{os.getcwd()}[/]")
                except Exception as e:
                    console.print(f"  [{theme['error_style']}]{e}[/]")

            elif cmd == "pwd":
                console.print(f"  [{theme['value_style']}]{os.getcwd()}[/]")

            elif cmd == "ls":
                target = args[0] if args else "."
                _run_streaming(["ls", "--color=always", "-lh", target], console, theme, timeout=5)

            elif cmd == "shell":
                cmd_shell(args, console, theme)

            else:
                console.print(
                    f"  [{theme['error_style']}]Unknown command: {cmd}[/]  "
                    f"[{theme['dim_style']}]Type 'help' to see all commands.[/]"
                )

        except KeyboardInterrupt:
            console.print(f"\n  [{theme['warn_style']}]Command interrupted.[/]")
        except Exception as e:
            console.print(f"  [{theme['error_style']}]Error: {e}[/]")

        console.print()
