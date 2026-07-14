import sys
import click
import json
import os
import shutil
import subprocess
import time
from concurrent.futures import ThreadPoolExecutor
from rich.console import Console
from rich.table import Table
from rich.text import Text
from fetch_info.display.panels import render_all
from fetch_info.display.themes import get_theme, list_themes
from fetch_info.collector import system, ros2, workspace


def collect(live: bool = False, skip_ws: bool = False, skip_sys: bool = False, timeout: int = 3, updates: bool = True, use_cache: bool = True) -> dict:
    """Collect system, ROS2, and workspace information in parallel.

    Args:
        live: Whether to collect live ROS2 runtime data (nodes/topics/services)
        skip_ws: Skip workspace collection
        skip_sys: Skip system information collection
        timeout: Timeout in seconds for ROS2 graph discovery
        updates: Whether to check for ROS2 package updates
        use_cache: If True, use cached ROS2 data if < 5 seconds old

    Returns:
        Dictionary with keys: 'system', 'ros2', 'workspace' (conditionally included)
    """
    data: dict = {}

    # Run system and workspace collection in parallel (ROS2 blocks on nothing)
    with ThreadPoolExecutor(max_workers=2) as executor:
        futures = {}
        if not skip_sys:
            futures["system"] = executor.submit(system.collect_all)
        if not skip_ws:
            futures["workspace"] = executor.submit(workspace.collect_all)

        # ROS2 always runs (doesn't block on others)
        data["ros2"] = ros2.collect_all(check_live=live, live_timeout=timeout, check_updates=updates, use_cache=use_cache)

        # Collect parallel results
        if "system" in futures:
            data["system"] = futures["system"].result()
        if "workspace" in futures:
            data["workspace"] = futures["workspace"].result()

    return data


def interactive_mode(theme_name):
    """Interactive TUI — let the user pick what to view."""
    console = Console()
    theme = get_theme(theme_name)

    menu_items = [
        ("1", "Full System Overview",             "full"),
        ("2", "ROS2 Environment Only",            "ros2"),
        ("3", "System Info Only",                 "system"),
        ("4", "Live Nodes/Topics/Services",       "live"),
        ("5", "Workspace Info",                   "workspace"),
        ("6", "Environment Variables",            "env"),
        ("7", "RQT-like ASCII Graph",             "graph"),
        ("8", "Interactive Terminal (REPL)",       "terminal"),
        ("9", "Export as JSON",                   "json"),
        ("W", "Start Web UI",                     "web"),
        ("q", "Quit",                             "quit"),
    ]

    from fetch_info.display.logo import get_main_banner

    while True:
        console.clear()
        console.print()
        console.print(get_main_banner(theme))
        console.print(f"  [{theme['highlight']}]Interactive Menu[/]\n")

        for key, label, _ in menu_items:
            style = theme["error_style"] if key == "q" else theme["value_style"]
            console.print(f"   [{theme['highlight']}]{key}[/]  [{style}]{label}[/]")
        console.print()

        choice = console.input(f"  [{theme['logo_color1']}]Select an option: [/]").strip().lower()

        action = None
        for key, _, act in menu_items:
            if choice == key.lower():
                action = act
                break

        if action is None:
            console.print(f"  [{theme['error_style']}]Invalid choice. Try again.[/]")
            time.sleep(0.8)
            continue

        if action == "quit":
            console.print(f"  [{theme['dim_style']}]Goodbye! 👋[/]")
            break

        if action == "web":
            console.print(f"  [{theme['ok_style']}]Starting web UI...[/]")
            from fetch_info.web import run_web
            run_web(port=8099)
            break

        if action == "terminal":
            from fetch_info.terminal import run_interactive_terminal
            run_interactive_terminal(theme_name)
            continue

        if action == "graph":
            from fetch_info.terminal import render_ascii_graph
            render_ascii_graph(console, theme, timeout=5)
            console.print(f"\n  [{theme['dim_style']}]Press Enter to continue...[/]")
            input()
            continue

        if action == "json":
            with console.status("[cyan]Collecting...", spinner="dots"):
                data = collect(live=True)
            click.echo(json.dumps(data, indent=2, default=str))
            console.print(f"\n  [{theme['dim_style']}]Press Enter to continue...[/]")
            input()
            continue

        with console.status("[cyan]Collecting...", spinner="dots2"):
            if action == "full":
                data = collect(live=True)
                render_all(console, data, theme_name, show_live=True, show_env=True)
            elif action == "ros2":
                data = collect(skip_ws=True, skip_sys=True)
                from fetch_info.display.panels import render_ros2_panel
                render_ros2_panel(console, data, theme)
            elif action == "system":
                data = collect(skip_ws=True, live=False)
                from fetch_info.display.panels import render_system_panel
                render_system_panel(console, data, theme)
            elif action == "live":
                data = collect(live=True, skip_ws=True, skip_sys=True)
                from fetch_info.display.panels import render_live_panel
                render_live_panel(console, data, theme)
            elif action == "workspace":
                data = collect(skip_sys=True, live=False)
                from fetch_info.display.panels import render_workspace_panel
                render_workspace_panel(console, data, theme)
            elif action == "env":
                data = collect(skip_ws=True, skip_sys=True, live=False)
                from fetch_info.display.panels import render_env_panel
                render_env_panel(console, data, theme)

        console.print(f"\n  [{theme['dim_style']}]Press Enter to continue...[/]")
        input()


# ── Main Command Group ────────────────────────────────────────────────────────
@click.group(invoke_without_command=True)
@click.option("--theme",    "-t", default="default", type=click.Choice(list_themes()))
@click.option("--live",     "-l", is_flag=True,  help="Show live nodes/topics/services")
@click.option("--watch",    "-w", default=0,     help="Refresh every N seconds")
@click.option("--json",     "out_json", is_flag=True, help="Output raw JSON")
@click.option("--env",      "-e", is_flag=True,  help="Show env vars panel")
@click.option("--interactive", "-i", is_flag=True, help="Interactive TUI menu")
@click.option("--info",         is_flag=True, help="Show system info snapshot (old default)")
@click.option("--verbose",      is_flag=True, help="Expanded output with full lists")
@click.option("--no-logo",      is_flag=True, help="Skip logo rendering, info only")
@click.option("--ascii",        is_flag=True, help="Force ASCII logo")
@click.option("--image",        is_flag=True, help="Force image rendering")
@click.option("--width",        default=None, type=int, help="Override terminal width (columns)")
@click.option("--no-system",    is_flag=True)
@click.option("--no-workspace", is_flag=True)
@click.option("--no-updates",   is_flag=True)
@click.option("--logo",         is_flag=True, help="Print ASCII logo only")
@click.option("--timeout",  default=3)
@click.option("--no-boot",  is_flag=True, help="Skip auto-source / bootstrap")
@click.option("--web",      is_flag=True, help="Launch web dashboard")
@click.pass_context
def main(ctx, theme, live, watch, out_json, env, interactive, info, verbose, no_logo, ascii, image, width, no_system, no_workspace, no_updates, logo, timeout, no_boot, web):
    """ROS2 Info v2 — type ros2_info to enter the interactive terminal.

    \b
    No flags      → Interactive terminal (REPL)
    --info        → System info snapshot
    -i            → TUI menu
    --watch N     → Live refresh every N seconds
    --json        → JSON dump
    terminal      → Full REPL (subcommand)
    web           → Web dashboard
    """
    # ── Bootstrap: auto-source ROS2 & workspace, install deps ─────────────
    # Apply terminal width override if specified
    terminal_width = width if width else shutil.get_terminal_size((120, 40)).columns
    console = Console(width=terminal_width)
    if not no_boot and not ctx.invoked_subcommand:
        from fetch_info.bootstrap import bootstrap
        # ponytail: bootstrap status goes to stderr so --json / pipe stay clean
        boot = bootstrap(console=Console(width=terminal_width, stderr=True))

    if ctx.invoked_subcommand:
        # silently bootstrap for subcommands too (no output)
        if not no_boot:
            from fetch_info.bootstrap import ensure_ros2_sourced, find_workspace_setup, source_workspace
            ensure_ros2_sourced()
            ws = find_workspace_setup()
            if ws:
                source_workspace(ws)
        return

    # ── No flags → launch terminal directly ───────────────────────────────
    if not any([interactive, info, out_json, watch, logo, live, web]):
        from fetch_info.terminal import run_interactive_terminal
        run_interactive_terminal(theme)
        return

    if web:
        console.print(f"\n  [bold cyan]🌐 Launching Web UI...[/]")
        from fetch_info.web import run_web
        run_web(port=8099)
        return

    if interactive:
        interactive_mode(theme)
        return

    if logo:
        from fetch_info.display.logo import get_main_banner
        console.print()
        console.print(get_main_banner(get_theme(theme)))
        console.print()
        return

    if out_json:
        with console.status("[cyan]Collecting...", spinner="dots"):
            data = collect(live=True, skip_ws=no_workspace, skip_sys=no_system,
                           timeout=timeout, updates=not no_updates)
        click.echo(json.dumps(data, indent=2, default=str))
        return

    if watch > 0:
        try:
            while True:
                console.clear()
                with console.status("[cyan]Collecting...", spinner="dots2"):
                    data = collect(live=True, skip_ws=no_workspace, skip_sys=no_system,
                                   timeout=timeout, updates=not no_updates)
                render_all(console, data, theme, show_live=True, show_env=env)
                console.print(f"  [dim]Refreshing every {watch}s — Ctrl+C to exit[/]")
                time.sleep(watch)
        except KeyboardInterrupt:
            console.print("\n[dim]Exiting watch mode.[/]")
            return

    # --info or --live flag: show static snapshot
    with console.status("[cyan]Collecting ROS2 info...", spinner="dots2"):
        data = collect(live=live or info, skip_ws=no_workspace, skip_sys=no_system,
                       timeout=timeout, updates=not no_updates)
    
    if info:
        from fetch_info.display.fastfetch import render_fastfetch
        render_fastfetch(console, data, get_theme(theme), show_logo=not no_logo, 
                        force_ascii=ascii, force_image=image, verbose=verbose)
    else:
        render_all(console, data, theme, show_live=live, show_env=env)


# ── Subcommands ───────────────────────────────────────────────────────────────

@main.command("terminal")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
def cmd_terminal(theme):
    """🖥  Launch the full interactive ROS2 terminal (REPL).

    Commands available inside the terminal:
      nodes, topics, services, actions, env, echo, hz, bw,
      pub, service call, param get/set/list, bag record/play/info,
      launch, run, graph, shell, watch, ping, interface show, ...
    """
    from fetch_info.terminal import run_interactive_terminal
    run_interactive_terminal(theme)


@main.command("graph")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--timeout", default=5, help="Discovery timeout (seconds)")
def cmd_graph(theme, timeout):
    """📊 Show ASCII RQT-like node/topic graph."""
    from fetch_info.terminal import render_ascii_graph
    console = Console()
    t = get_theme(theme)
    render_ascii_graph(console, t, timeout=timeout)


@main.command("nodes")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--timeout", default=5)
@click.option("--info", "-I", "show_info", is_flag=True, help="Show pub/sub info for each node")
def cmd_nodes(theme, timeout, show_info):
    """List all active ROS2 nodes."""
    console = Console()
    t = get_theme(theme)
    with console.status("[cyan]Querying nodes...", spinner="dots"):
        nodes = ros2.get_active_nodes(timeout)
    if not nodes:
        console.print("[yellow]No active nodes found. Is ROS2 running?[/]")
        return
    table = Table(title="Active ROS2 Nodes", border_style=t["panel_border"])
    table.add_column("Node", style=t["key_style"])
    table.add_column("Status", style=t["ok_style"])
    for n in nodes:
        table.add_row(n, "● Running")
    console.print(table)

    if show_info:
        for n in nodes:
            from fetch_info.terminal import _run
            stdout, _, rc = _run(["ros2", "node", "info", n], timeout=timeout)
            if rc == 0:
                from rich.panel import Panel
                from rich import box
                from rich.syntax import Syntax
                syntax = Syntax(stdout, "yaml", theme="monokai", background_color="default")
                console.print(Panel(syntax, title=f"[bold]{n}[/]",
                                    border_style=t["panel_border"], box=box.ROUNDED))


@main.command("topics")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--verbose", "-v", is_flag=True)
@click.option("--timeout", default=5)
@click.option("--hz", is_flag=True, help="Show publish rate for each topic (slow)")
def cmd_topics(theme, verbose, timeout, hz):
    """List all active ROS2 topics."""
    console = Console()
    t = get_theme(theme)
    with console.status("[cyan]Querying topics...", spinner="dots"):
        topics = ros2.get_active_topics(timeout)
    if not topics:
        console.print("[yellow]No active topics found.[/]")
        return
    table = Table(title=f"Active Topics ({len(topics)})", border_style=t["panel_border"])
    table.add_column("Topic", style=t["key_style"])
    if verbose:
        table.add_column("Type", style=t["value_style"])
    for tp in topics:
        table.add_row(tp["name"], tp["type"]) if verbose else table.add_row(tp["name"])
    console.print(table)


@main.command("services")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--timeout", default=5)
def cmd_services(theme, timeout):
    """List all active ROS2 services."""
    console = Console()
    t = get_theme(theme)
    with console.status("[cyan]Querying services...", spinner="dots"):
        services = ros2.get_active_services(timeout)
    if not services:
        console.print("[yellow]No active services found.[/]")
        return
    table = Table(title=f"Active Services ({len(services)})", border_style=t["panel_border"])
    table.add_column("Service", style=t["key_style"])
    for s in services:
        table.add_row(s)
    console.print(table)


@main.command("actions")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--timeout", default=5)
def cmd_actions(theme, timeout):
    """List all active ROS2 actions."""
    console = Console()
    t = get_theme(theme)
    with console.status("[cyan]Querying actions...", spinner="dots"):
        actions = ros2.get_active_actions(timeout)
    if not actions:
        console.print("[yellow]No active actions found.[/]")
        return
    table = Table(title=f"Active Actions ({len(actions)})", border_style=t["panel_border"])
    table.add_column("Action", style=t["key_style"])
    for a in actions:
        table.add_row(a)
    console.print(table)


@main.command("packages")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--filter", "-f", "pkg_filter", default="")
def cmd_packages(theme, pkg_filter):
    """List all installed ROS2 packages."""
    import os
    from rich.columns import Columns
    console = Console()
    t = get_theme(theme)
    distro = ros2.get_distro()
    if not distro:
        console.print("[red]ROS2 not sourced.[/]")
        return
    ros_share = f"/opt/ros/{distro}/share"
    pkgs = sorted([d for d in os.listdir(ros_share) if os.path.isdir(f"{ros_share}/{d}")])
    if pkg_filter:
        pkgs = [p for p in pkgs if pkg_filter.lower() in p.lower()]
    console.print(f"\n[bold cyan]ROS2 Packages — {distro} ({len(pkgs)} shown)[/]\n")
    console.print(Columns([Text(f"  {p}", style=t["value_style"]) for p in pkgs], equal=True))


@main.command("workspace")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
def cmd_workspace(theme):
    """Scan for colcon workspaces."""
    from fetch_info.display.panels import render_workspace_panel
    console = Console()
    t = get_theme(theme)
    with console.status("[cyan]Scanning...", spinner="dots"):
        data = {"workspace": workspace.collect_all()}
    render_workspace_panel(console, data, t)


@main.command("env")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
def cmd_env(theme):
    """Show all ROS2 environment variables."""
    from rich.table import Table
    console = Console()
    t = get_theme(theme)
    env = ros2.get_ros2_environment()
    if not env:
        console.print("[yellow]No ROS2 env vars found. Source ROS2 first.[/]")
        return
    table = Table(title="ROS2 Environment", border_style=t["panel_border"], show_lines=True)
    table.add_column("Variable", style=t["key_style"])
    table.add_column("Value", style=t["value_style"])
    for k, v in env.items():
        table.add_row(k, v)
    console.print(table)


@main.command("themes")
def cmd_themes():
    """Preview all available themes."""
    console = Console()
    console.print("\n[bold cyan]Available Themes:[/]\n")
    for name in list_themes():
        t = get_theme(name)
        row = Text()
        row.append(f"  {name:<12}", style=f"bold {t['logo_color1']}")
        for key in ["logo_color1", "logo_color2", "logo_color3", "highlight"]:
            c = t.get(key, "#fff")
            if c.startswith("#"):
                row.append("██", style=f"bold {c}")
        row.append(f"  ros2_info --theme {name}", style="dim")
        console.print(row)
    console.print()


@main.command("web")
@click.option("--port", "-p", default=8099, help="Port for web UI")
@click.option("--host", default="0.0.0.0", help="Host to bind to")
@click.option("--ssl", is_flag=True, help="Enable HTTPS (auto-generates self-signed cert)")
@click.option("--cert", default=None, help="Path to SSL certificate file (requires --key)")
@click.option("--key", default=None, help="Path to SSL key file (requires --cert)")
@click.option("--flask", is_flag=True, help="Use legacy Flask backend instead of default Rust real-time backend")
@click.option("--auth", is_flag=True, help="Require HTTP basic auth (prompts for credentials)")
def cmd_web(port, host, flask, auth, ssl, cert, key):
    """🌐 Launch the ROS2 Info web dashboard."""
    console = Console()
    if auth:
        if not os.environ.get('ROS2_INFO_USERNAME'):
            os.environ['ROS2_INFO_USERNAME'] = click.prompt('Username')
        if not os.environ.get('ROS2_INFO_PASSWORD'):
            os.environ['ROS2_INFO_PASSWORD'] = click.prompt('Password', hide_input=True)

    if flask:
        protocol = "https" if ssl else "http"
        console.print(f"\n  [bold cyan]🌐 ROS2 Info Web UI (legacy Flask backend)[/]")
        console.print(f"  [dim]Starting on {protocol}://localhost:{port}[/]\n")
        from fetch_info.web import run_web
        run_web(host=host, port=port, ssl=ssl, cert=cert, key=key)
        return

    # Rust real-time backend is the default
    if ssl:
        console.print(f"  [bold yellow]⚠  SSL not supported in Rust backend yet — use --flask for HTTPS[/]")
    import subprocess
    rust_bin = os.path.join(os.path.dirname(__file__), "..", "..", "backend", "target", "release", "ros2-info-rt")
    template_dir = os.path.join(os.path.dirname(__file__), "templates")
    if not os.path.exists(rust_bin):
        rust_bin = os.path.join(os.path.dirname(__file__), "..", "..", "backend", "target", "debug", "ros2-info-rt")
    if os.path.exists(rust_bin):
        console.print(f"\n  [bold green]🚀 ROS2 Info Rust Backend[/]")
        console.print(f"  [dim]Starting on http://localhost:{port}[/]\n")
        subprocess.run([rust_bin, "--port", str(port), "--templates", template_dir])
    else:
        console.print(f"  [bold yellow]⚠  Rust backend not built. Building now...[/]")
        build_dir = os.path.join(os.path.dirname(__file__), "..", "..", "backend")
        subprocess.run(["cargo", "build", "--release"], cwd=build_dir)
        subprocess.run([rust_bin, "--port", str(port), "--templates", template_dir])


@main.command("bag")
@click.argument("subcmd", type=click.Choice(["record", "play", "info"]))
@click.argument("args", nargs=-1)
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
def cmd_bag(subcmd, args, theme):
    """📦 ROS2 bag operations: record | play | info.

    Examples:\n
      ros2_info bag record -a\n
      ros2_info bag record /topic1 /topic2\n
      ros2_info bag play my_bag/\n
      ros2_info bag info my_bag/\n
    """
    console = Console()
    t = get_theme(theme)
    from fetch_info.terminal import cmd_bag as _cmd_bag
    _cmd_bag([subcmd] + list(args), console, t)


@main.command("param")
@click.argument("subcmd", type=click.Choice(["get", "set", "list"]))
@click.argument("args", nargs=-1)
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
def cmd_param(subcmd, args, theme):
    """⚙️  ROS2 parameter operations: get | set | list.

    Examples:\n
      ros2_info param list /my_node\n
      ros2_info param get /my_node use_sim_time\n
      ros2_info param set /my_node use_sim_time true\n
    """
    console = Console()
    t = get_theme(theme)
    from fetch_info.terminal import cmd_param as _cmd_param
    _cmd_param([subcmd] + list(args), console, t)


@main.command("launch")
@click.argument("package")
@click.argument("launch_file")
@click.argument("args", nargs=-1)
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
def cmd_launch(package, launch_file, args, theme):
    """🚀 Launch a ROS2 launch file.

    Example: ros2_info launch my_pkg my_file.launch.py key:=value
    """
    console = Console()
    t = get_theme(theme)
    from fetch_info.terminal import cmd_launch as _cmd_launch
    _cmd_launch([package, launch_file] + list(args), console, t)


@main.command("run")
@click.argument("package")
@click.argument("executable")
@click.argument("args", nargs=-1)
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
def cmd_run(package, executable, args, theme):
    """▶  Run a ROS2 node executable.

    Example: ros2_info run demo_nodes_cpp talker
    """
    console = Console()
    t = get_theme(theme)
    from fetch_info.terminal import cmd_run_node as _cmd_run
    _cmd_run([package, executable] + list(args), console, t)


@main.command("interface")
@click.argument("msg_type")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
def cmd_interface(msg_type, theme):
    """🔎 Show a ROS2 message/service/action interface definition.

    Example: ros2_info interface std_msgs/msg/String
    """
    console = Console()
    t = get_theme(theme)
    from fetch_info.terminal import cmd_interface_show
    cmd_interface_show([msg_type], console, t)


# ── Doctor / Diagnose / Matrix / Benchmark ─────────────────────────────────────

@main.command("doctor")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--json", "out_json", is_flag=True, help="Output raw JSON")
@click.option("--fix", is_flag=True, help="Apply safe auto-fixes interactively")
def cmd_doctor(theme, out_json, fix):
    """🩺 Run comprehensive ROS2 health check (10+ diagnostics)."""
    from fetch_info.collector.diagnostics import run_diagnostics, apply_fix
    from rich.table import Table
    from rich.panel import Panel
    from rich.text import Text
    from rich import box

    console = Console()
    t = get_theme(theme)

    if out_json:
        with console.status("[cyan]Running diagnostics...", spinner="dots"):
            result = run_diagnostics()
        click.echo(json.dumps(result, indent=2))
        return

    console.print()
    banner = Text("  ROS2 DOCTOR", style=f"bold {t['logo_color1']}")
    banner.append(f"  Running diagnostic checks...", style=t['dim_style'])
    console.print(banner)
    console.print()

    with console.status("[cyan]Running diagnostics...", spinner="dots2"):
        result = run_diagnostics()
        checks = result["checks"]
        summary = result["summary"]

    issues_to_fix = []

    for check in checks:
        icon = CHECK_RESULTS.get(check["status"], "?")
        label = f"  {icon}  {check['name']:<35}"
        detail = check['detail'][:60]

        if check["status"] == "pass":
            style = t["ok_style"]
        elif check["status"] == "fail":
            style = t["error_style"]
        elif check["status"] == "warn":
            style = t["warn_style"]
        else:
            style = t["dim_style"]

        console.print(f"{label} [{style}]{detail}[/]")

        if check.get("fix") and check["status"] in ("warn", "fail"):
            issues_to_fix.append(check)
            if fix:
                console.print(f"  {'':>40} [{t['highlight']}]→ Fix: {check['fix']}[/]")

    console.print()
    summary_table = Table(border_style=t["panel_border"], box=box.ROUNDED)
    summary_table.add_column("Result", style=t["key_style"])
    summary_table.add_column("Count", style=t["value_style"])
    summary_table.add_row("Total Checks", str(summary["total"]))
    summary_table.add_row(f"✅ Passed", str(summary["passed"]))
    summary_table.add_row(f"⚠️  Warnings", str(summary["warnings"]))
    summary_table.add_row(f"❌ Failed", str(summary["failed"]))
    summary_table.add_row("Health Score", f"{summary['score']}%")
    console.print(Panel(summary_table, title="[bold]Summary[/]",
                         border_style=t["panel_border"]))

    # Interactive fix mode
    if fix and issues_to_fix:
        interactive = hasattr(sys.stdin, 'isatty') and sys.stdin.isatty()
        if not interactive:
            console.print(f"\n  [{t['dim_style']}]Non-interactive mode. Use: ros2_info doctor to view issues.[/]")
        else:
            console.print(f"\n  [{t['highlight']}]{len(issues_to_fix)} issue(s) can be addressed[/]")

            for i, issue in enumerate(issues_to_fix):
                check_name = issue["name"].split(":")[0].strip()
                console.print(f"\n  [{t['key_style']}]{i+1}. {issue['name']}[/]")
                console.print(f"     [{t['dim_style']}]Fix: {issue.get('fix', 'N/A')}[/]")

                try:
                    choice = click.prompt(
                        f"  Apply fix?",
                        type=click.Choice(["y", "n", "s"], case_sensitive=False),
                        default="y"
                    )
                except (click.Abort, SystemExit):
                    console.print(f"  [{t['warn_style']}]Fix prompts aborted (non-interactive).[/]")
                    break

                if choice == "y":
                    with console.status("[cyan]Applying fix...", spinner="dots"):
                        success, msg = apply_fix(check_name, issue["detail"])

                    if success:
                        console.print(f"  [{t['ok_style']}]✓ {msg}[/]")
                    else:
                        console.print(f"  [{t['warn_style']}]{msg}[/]")
                elif choice == "s":
                    break

            console.print(f"\n  [{t['dim_style']}]Fix session complete.[/]")

    console.print()


@main.command("diagnose")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--json", "out_json", is_flag=True, help="Output raw JSON")
@click.option("--fix", is_flag=True, help="Apply safe auto-fixes")
def cmd_diagnose(theme, out_json, fix):
    """🔍 Deep-dive diagnostics with root-cause analysis."""
    from fetch_info.collector.diagnostics import run_diagnostics
    from rich.panel import Panel
    from rich.text import Text
    from rich.table import Table
    from rich import box

    # JSON output first (no console output)
    if out_json:
        result = run_diagnostics()
        click.echo(json.dumps(result, indent=2))
        return

    console = Console()
    t = get_theme(theme)

    with console.status("[cyan]Analyzing ROS2 environment...", spinner="dots"):
        result = run_diagnostics()
        checks = result["checks"]
        summary = result["summary"]

    console.print()
    header = Text("  ROS2 DIAGNOSE", style=f"bold {t['logo_color1']}")
    header.append(f"  Found {summary['failed']} issues, {summary['warnings']} warnings", style=t['dim_style'])
    console.print(header)
    console.print()

    for check in checks:
        if check["status"] in ("pass", "info"):
            continue

        icon = CHECK_RESULTS.get(check["status"], "?")
        label = f"  {icon}  {check['name']}"
        detail = check['detail']

        if check["status"] == "fail":
            style = t["error_style"]
        else:
            style = t["warn_style"]

        content = Text()
        content.append(f"Issue: ", style=t["key_style"])
        content.append(f"{detail}\n", style=style)
        if check.get("fix"):
            content.append(f"Fix:   ", style=t["key_style"])
            content.append(f"{check['fix']}\n", style=t["highlight"])

        console.print(Panel(content, title=label,
                            border_style=style, box=box.ROUNDED))

    console.print()
    console.print(f"  [{t['dim_style']}]Run with --fix to auto-apply safe fixes[/]")
    console.print()


@main.command("matrix")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--timeout", default=5)
@click.option("--json", "out_json", is_flag=True, help="Output raw JSON")
def cmd_matrix(theme, timeout, out_json):
    """📊 Show topic communication matrix (who talks to whom)."""
    from fetch_info.terminal import build_topic_graph
    from rich.table import Table
    from rich.text import Text
    from rich import box

    console = Console()
    t = get_theme(theme)

    with console.status("[cyan]Building communication matrix...", spinner="dots"):
        graph = build_topic_graph(timeout=timeout)

    if out_json:
        click.echo(json.dumps(graph, indent=2))
        return

    console.print()
    console.print(f"  [bold {t['logo_color1']}]Topic Communication Matrix[/]")
    console.print()

    if not graph["nodes"]:
        console.print(f"  [{t['warn_style']}]No nodes found. Is ROS2 running?[/]")
        return

    nodes = list(graph["nodes"].keys())
    topics = graph["topics"]

    tbl = Table(border_style=t["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
    tbl.add_column("Node", style=f"bold {t['key_style']}", no_wrap=True)
    tbl.add_column("Publishes →", style=t["ok_style"])
    tbl.add_column("→ Subscribes", style=t["logo_color2"])

    for node in nodes:
        pubs = graph["nodes"][node].get("pubs", [])
        subs = graph["nodes"][node].get("subs", [])
        pub_names = "\n".join(p.split("/")[-1] for p in pubs[:5]) if pubs else "—"
        sub_names = "\n".join(s.split("/")[-1] for s in subs[:5]) if subs else "—"
        tbl.add_row(node.split("/")[-1] or node, pub_names, sub_names)

    console.print(tbl)
    console.print()

    if topics:
        console.print(f"  [{t['section_title']}]Topic Details:[/]\n")
        for topic, conns in list(topics.items())[:15]:
            pubs = conns.get("publishers", [])
            subs = conns.get("subscribers", [])
            pub_str = ", ".join(p.split("/")[-1] for p in pubs[:3]) or "?"
            sub_str = ", ".join(s.split("/")[-1] for s in subs[:3]) or "?"
            icon = "✓" if pubs and subs else "⚠"
            console.print(f"  {icon}  [{t['highlight']}]{topic}[/]")
            console.print(f"       Pub: [{t['ok_style']}]{pub_str}[/]")
            console.print(f"       Sub: [{t['logo_color2']}]{sub_str}[/]")

    console.print()


@main.command("benchmark")
@click.option("--duration", "-d", default=10, help="Duration in seconds")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--json", "out_json", is_flag=True, help="Output raw JSON")
def cmd_benchmark(duration, theme, out_json):
    """⚡ Benchmark ROS2 topic rates and system performance."""
    from fetch_info.collector import ros2
    from fetch_info.collector import system as sys_collector
    from rich.table import Table
    from rich.text import Text
    from rich import box
    import time as time_module

    console = Console()
    t = get_theme(theme)

    console.print()
    console.print(f"  [bold {t['logo_color1']}]Performance Benchmark[/]")
    console.print(f"  [{t['dim_style']}]Measuring for {duration}s...[/]\n")

    # Initial state
    sys_info = sys_collector.collect_all()
    topics = ros2.get_active_topics(timeout=3)
    nodes = ros2.get_active_nodes(timeout=3)

    # Measure topic rates
    topic_rates = {}
    with console.status("[cyan]Measuring topic rates...", spinner="dots"):
        for tp in topics[:10]:
            name = tp["name"]
            try:
                out = subprocess.run(
                    ["ros2", "topic", "hz", name, "--window", "10"],
                    capture_output=True, text=True,
                    timeout=duration + 2,
                    env={**os.environ}
                ).stdout.strip()
                topic_rates[name] = out.split("\n")[-1] if out else "N/A"
            except Exception:
                topic_rates[name] = "N/A"

    # Final system state
    sys_info_end = sys_collector.collect_all()
    sys_info_end_cpu = sys_info_end.get("cpu", {})
    sys_info_cpu = sys_info.get("cpu", {})

    if out_json:
        click.echo(json.dumps({
            "duration": duration,
            "topic_count": len(topics),
            "node_count": len(nodes),
            "topic_rates": topic_rates,
            "cpu_avg": sys_info_cpu,
            "memory": sys_info.get("memory"),
        }, indent=2))
        return

    # Results table
    if topic_rates:
        tbl = Table(title="Topic Rates", border_style=t["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
        tbl.add_column("Topic", style=f"bold {t['key_style']}")
        tbl.add_column("Rate", style=t["value_style"])
        for name, rate in topic_rates.items():
            tbl.add_row(name, rate)
        console.print(tbl)
        console.print()

    # System info
    summary = Table(border_style=t["panel_border"], box=None)
    summary.add_column("Metric", style=t["key_style"])
    summary.add_column("Value", style=t["value_style"])
    summary.add_row("Nodes", str(len(nodes)))
    summary.add_row("Topics", str(len(topics)))
    mem = sys_info.get("memory", {})
    summary.add_row("Memory", f"{mem.get('used_gb', '?')}/{mem.get('total_gb', '?')} GB")
    summary.add_row("CPU", f"{sys_info_cpu.get('model', '?')[:50]}")
    summary.add_row("Duration", f"{duration}s")
    console.print(summary)
    console.print()


@main.command("trend")
@click.option("--hours", "-h", default=24, help="Time window in hours")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--json", "out_json", is_flag=True, help="Output raw JSON")
@click.option("--record", is_flag=True, help="Record a snapshot now")
@click.option("--chart", "-c", is_flag=True, help="Show ASCII sparkline charts")
@click.option("--daemon", is_flag=True, help="Start background recording daemon")
@click.option("--interval", default=60, help="Recording interval in seconds (default: 60)")
def cmd_trend(hours, theme, out_json, record, chart, daemon, interval):
    """📈 Show historical system trends (SQLite-backed).

    Use --daemon to start background recording (runs until Ctrl+C).
    Use --record to capture a single snapshot.
    """
    from fetch_info.collector.trends import record_snapshot, get_trend, get_summary, get_chart_data, get_ascii_sparkline, prune_old_data
    from fetch_info.collector import system, ros2
    from rich.table import Table, box
    from rich.panel import Panel
    from rich.text import Text
    import signal
    import sys

    console = Console()
    t = get_theme(theme)

    # Daemon mode
    if daemon:
        console.print()
        console.print(f"  [bold {t['logo_color1']}]📈 Trend Recording Daemon[/]")
        console.print(f"  [{t['dim_style']}]Recording every {interval}s. Press Ctrl+C to stop.[/]\n")

        running = True
        def signal_handler(sig, frame):
            nonlocal running
            running = False
            console.print(f"\n  [{t['dim_style']}]Daemon stopped. Data saved to ~/.ros2_info/trends.db[/]")
            sys.exit(0)

        signal.signal(signal.SIGINT, signal_handler)

        count = 0
        while running:
            try:
                with console.status(f"[cyan]Recording snapshot {count+1}...", spinner="dots"):
                    sys_data = system.collect_all()
                    ros2_data = ros2.collect_all(check_live=True, live_timeout=3, check_updates=False)
                    mem = sys_data.get("memory", {})
                    bat = sys_data.get("battery", {})
                    record_snapshot(
                        cpu_percent=sys_data.get("cpu", {}).get("usage_percent", 0) or 0,
                        memory_percent=mem.get("percent", 0),
                        disk_percent=sys_data.get("disk", {}).get("percent", 0),
                        battery_percent=bat.get("percent") if bat.get("percent") is not None else None,
                        node_count=len(ros2_data.get("nodes", [])),
                        topic_count=len(ros2_data.get("topics", [])),
                        service_count=len(ros2_data.get("services", [])),
                    )
                count += 1
                console.print(f"  [{t['ok_style']}]✓ Snapshot #{count} recorded[/]")

                # Prune data older than 7 days to prevent unbounded growth
                if count % 10 == 0:
                    pruned = prune_old_data(days=7)
                    if pruned > 0:
                        console.print(f"  [{t['dim_style']}]Pruned {pruned} old records[/]")

            except Exception as e:
                console.print(f"  [{t['error_style']}]Error recording: {e}[/]")

            time.sleep(interval)
        return

    if record:
        with console.status("[cyan]Recording snapshot...", spinner="dots"):
            sys_data = system.collect_all()
            ros2_data = ros2.collect_all(check_live=True, live_timeout=3, check_updates=False)
            mem = sys_data.get("memory", {})
            bat = sys_data.get("battery", {})
            record_snapshot(
                cpu_percent=sys_data.get("cpu", {}).get("usage_percent", 0) or 0,
                memory_percent=mem.get("percent", 0),
                disk_percent=sys_data.get("disk", {}).get("percent", 0),
                battery_percent=bat.get("percent") if bat.get("percent") is not None else None,
                node_count=len(ros2_data.get("nodes", [])),
                topic_count=len(ros2_data.get("topics", [])),
                service_count=len(ros2_data.get("services", [])),
            )
            console.print(f"  [{t['ok_style']}]✓ Snapshot recorded[/]")
        return

    if out_json:
        result = get_summary()
        result["data_points"] = get_trend(duration_hours=hours)
        click.echo(json.dumps(result, indent=2, default=str))
        return

    summary = get_summary()
    console.print()
    console.print(f"  [bold {t['logo_color1']}]📈 System Trends (last {hours}h)[/]")
    console.print()

    if summary["total_snapshots"] == 0:
        console.print(f"  [{t['warn_style']}]No historical data yet. Run: ros2_info trend --record or ros2_info trend --daemon[/]")
        console.print()
        return

    # Summary table
    tbl = Table(border_style=t["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
    tbl.add_column("Metric", style=f"bold {t['key_style']}")
    tbl.add_column("Min", style=t["value_style"])
    tbl.add_column("Max", style=t["value_style"])
    tbl.add_column("Avg", style=t["value_style"])

    for label, key in [("CPU %", "cpu"), ("Memory %", "memory"), ("Disk %", "disk"),
                        ("Battery %", "battery"), ("Nodes", "nodes"), ("Topics", "topics")]:
        info = summary.get(key, {})
        tbl.add_row(label,
                    str(round(info.get("min", 0), 1) if info.get("min") else "—"),
                    str(round(info.get("max", 0), 1) if info.get("max") else "—"),
                    str(round(info.get("avg", 0), 1) if info.get("avg") else "—"))
    console.print(tbl)

    # ASCII sparkline charts
    if chart or summary["total_snapshots"] >= 3:
        console.print()
        timestamps, cpu_values, memory_values, node_counts = get_chart_data(duration_hours=hours, max_points=50)

        if cpu_values:
            # CPU Chart
            console.print(f"\n  [{t['section_title']}]CPU Usage Trend:[/]")
            cpu_chart = get_ascii_sparkline(cpu_values, height=5, width=50)
            for row in cpu_chart:
                console.print(f"  [{t['ok_style']}]{row}[/]")
            console.print(f"  [{' ' * 50}]", style=t['dim_style'])
            console.print(f"  [{'Now':<25}{'Past':>25}]", style=t['dim_style'])

            # Memory Chart
            console.print(f"\n  [{t['section_title']}]Memory Usage Trend:[/]")
            mem_chart = get_ascii_sparkline(memory_values, height=5, width=50)
            for row in mem_chart:
                console.print(f"  [{t['warn_style']}]{row}[/]")
            console.print(f"  [{' ' * 50}]", style=t['dim_style'])

            # Node Count Chart
            console.print(f"\n  [{t['section_title']}]ROS2 Node Count Trend:[/]")
            node_chart = get_ascii_sparkline([float(n) for n in node_counts], height=4, width=50)
            for row in node_chart:
                console.print(f"  [{t['highlight']}]{row}[/]")
            console.print(f"  [{' ' * 50}]", style=t['dim_style'])

    console.print(f"\n  [{t['dim_style']}]Based on {summary['total_snapshots']} snapshots. Start daemon: ros2_info trend --daemon[/]")
    console.print()


@main.command("launch-verify")
@click.argument("path", required=False, default="")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--json", "out_json", is_flag=True, help="Output raw JSON")
@click.option("--deps", is_flag=True, help="Check cross-referenced dependencies")
def cmd_launch_verify(path, theme, out_json, deps):
    """🚀 Verify ROS2 launch files for common issues.

    Analyze launch files for: missing files, syntax errors, missing attributes,
    port conflicts, resource constraints, and missing dependencies.
    """
    from fetch_info.collector.launch_verify import verify_launch_file, verify_workspace_launch_files, find_missing_dependencies
    from fetch_info.collector import workspace as ws_collector
    from rich.table import Table, box
    from rich.panel import Panel

    console = Console()
    t = get_theme(theme)

    if not path:
        workspaces = ws_collector.find_workspaces()
        if not workspaces:
            console.print(f"  [{t['error_style']}]No workspace found. Specify a path: ros2_info launch-verify <path>[/]")
            return
        path = workspaces[0]
        console.print(f"  [{t['dim_style']}]Using workspace: {path}[/]")

    if os.path.isdir(path):
        with console.status("[cyan]Verifying workspace launch files...", spinner="dots"):
            result = verify_workspace_launch_files(path)
    else:
        with console.status("[cyan]Verifying launch file...", spinner="dots"):
            result = verify_launch_file(path)

    if out_json:
        if deps and not os.path.isdir(path):
            dep_result = find_missing_dependencies(path, os.path.dirname(os.path.dirname(path)))
            result["dependencies"] = dep_result
        click.echo(json.dumps(result, indent=2, default=str))
        return

    console.print()
    if isinstance(result, dict) and "total_launch_files" in result:
        console.print(f"  [bold {t['logo_color1']}]Launch File Verification[/]")
        console.print(f"  [{t['dim_style']}]Files: {result['total_launch_files']} | "
                      f"Errors: {result['total_errors']} | Warnings: {result['total_warnings']}[/]\n")
        for r in result.get("results", []):
            if r.get("checks"):
                for c in r["checks"]:
                    icon = "❌" if c["severity"] == "error" else "⚠" if c["severity"] == "warning" else "ℹ"
                    style = t["error_style"] if c["severity"] == "error" else t["warn_style"] if c["severity"] == "warning" else t["dim_style"]
                    console.print(f"  {icon} [{style}]{c['message']}[/]")
                    if c.get("fix"):
                        console.print(f"     [{t['highlight']}]→ {c['fix']}[/]")
    else:
        file_info = result
        console.print(f"  [bold {t['logo_color1']}]Verification: {os.path.basename(file_info.get('file', path))}[/]\n")
        for c in file_info.get("checks", []):
            icon = "❌" if c["severity"] == "error" else "⚠" if c["severity"] == "warning" else "ℹ"
            style = t["error_style"] if c["severity"] == "error" else t["warn_style"] if c["severity"] == "warning" else t["dim_style"]
            console.print(f"  {icon} [{style}]{c['message']}[/]")
            if c.get("fix"):
                console.print(f"     [{t['highlight']}]→ {c['fix']}[/]")
        if not file_info.get("checks"):
            console.print(f"  [{t['ok_style']}]✓ No issues found[/]")

        if deps:
            console.print(f"\n  [{t['section_title']}]Dependency Check:[/]\n")
            ws_path = os.path.dirname(os.path.dirname(path)) if os.path.isfile(path) else path
            dep_issues = find_missing_dependencies(path, ws_path)
            for d in dep_issues:
                icon = "❌" if d["severity"] == "error" else "⚠" if d["severity"] == "warning" else "ℹ"
                style = t["error_style"] if d["severity"] == "error" else t["warn_style"] if d["severity"] == "warning" else t["dim_style"]
                console.print(f"  {icon} [{style}]{d['message']}[/]")

    console.print()


@main.command("bag-analyze")
@click.argument("bag_path")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--json", "out_json", is_flag=True, help="Output raw JSON")
@click.option("--compare", "-c", help="Compare with another bag")
def cmd_bag_analyze(bag_path, theme, out_json, compare):
    """📦 Analyze a ROS2 bag file for health and diagnostics.

    Shows: topic timeline, message counts, dropped messages, health issues.
    Use --compare to diff two bags.
    """
    from fetch_info.collector.bag_forensics import analyze_bag, check_bag_health, get_topic_timeline, compare_bags
    from rich.table import Table, box
    from rich.panel import Panel

    console = Console()
    t = get_theme(theme)

    if compare:
        with console.status("[cyan]Comparing bags...", spinner="dots"):
            result = compare_bags(bag_path, compare)
        if out_json:
            click.echo(json.dumps(result, indent=2, default=str))
            return
        console.print(f"\n  [bold {t['logo_color1']}]📊 Bag Comparison[/]\n")
        tbl = Table(border_style=t["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
        tbl.add_column("Metric", style=f"bold {t['key_style']}")
        tbl.add_column("Bag 1", style=t["value_style"])
        tbl.add_column("Bag 2", style=t["value_style"])
        v1_dur = f"{result.get('bag_1_duration', '?'):.1f}s" if result.get('bag_1_duration') else "?"
        v2_dur = f"{result.get('bag_2_duration', '?'):.1f}s" if result.get('bag_2_duration') else "?"
        tbl.add_row("Duration", v1_dur, v2_dur)
        tbl.add_row("Messages", str(result.get("bag_1_message_count", "?")), str(result.get("bag_2_message_count", "?")))
        tbl.add_row("Dropped %", str(result.get("bag_1_dropped_pct", "?")), str(result.get("bag_2_dropped_pct", "?")))
        tbl.add_row("Shared Topics", str(len(result.get("shared_topics", []))), "")
        console.print(tbl)
        return

    with console.status("[cyan]Analyzing bag...", spinner="dots"):
        info = analyze_bag(bag_path)
        health = check_bag_health(bag_path)
        timeline = get_topic_timeline(bag_path)

    if out_json:
        result = {"info": info, "health": health, "timeline": timeline}
        click.echo(json.dumps(result, indent=2, default=str))
        return

    if "error" in info:
        console.print(f"\n  [{t['error_style']}]Error: {info['error']}[/]\n")
        return

    console.print()
    console.print(f"  [bold {t['logo_color1']}]📦 Bag Analysis: {bag_path}[/]\n")

    info_tbl = Table(border_style=t["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
    info_tbl.add_column("Property", style=f"bold {t['key_style']}")
    info_tbl.add_column("Value", style=t["value_style"])
    info_tbl.add_row("Duration", f"{info.get('duration', '?'):.1f}s" if info.get('duration') else "?")
    info_tbl.add_row("Size", info.get("size", "?"))
    info_tbl.add_row("Messages", str(info.get("messages", "?")))
    info_tbl.add_row("Compression", info.get("compression", "none"))
    console.print(info_tbl)
    console.print()

    if isinstance(timeline, dict) and "error" not in timeline:
        console.print(f"  [{t['section_title']}]Topic Timeline:[/]\n")
        t_tbl = Table(border_style=t["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
        t_tbl.add_column("Topic", style=f"bold {t['key_style']}")
        t_tbl.add_column("Messages", style=t["value_style"])
        t_tbl.add_column("Rate (Hz)", style=t["value_style"])
        for topic_name, tdata in list(timeline.items())[:20]:
            t_tbl.add_row(topic_name, str(tdata.get("message_count", 0)), str(tdata.get("rate_hz", 0)))
        console.print(t_tbl)

    if not health.get("healthy", True):
        console.print(f"\n  [{t['warn_style']}]⚠ Health Issues:[/]")
        for issue in health.get("issues", []):
            console.print(f"    [{t['error_style']}]• {issue}[/]")
    else:
        console.print(f"\n  [{t['ok_style']}]✓ Bag is healthy[/]")
    console.print()


@main.command("fleet")
@click.argument("hosts", nargs=-1, required=False)
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
@click.option("--json", "out_json", is_flag=True, help="Output raw JSON")
@click.option("--subnet", default="192.168.1.", help="Subnet prefix for discovery")
@click.option("--discover", is_flag=True, help="Scan subnet for responsive hosts")
@click.option("--user", default="root", help="SSH username")
@click.option("--key", help="SSH key path")
@click.option("--port", default=22, help="SSH port")
def cmd_fleet(hosts, theme, out_json, subnet, discover, user, key, port):
    """🤖 Multi-robot fleet dashboard via SSH.

    Examples:
      ros2_info fleet robot1.local robot2.local
      ros2_info fleet --discover
      ros2_info fleet --subnet 10.0.0.
    """
    from fetch_info.collector.fleet import FleetHost, check_host, collect_remote_info, collect_fleet, discover_hosts
    from rich.table import Table, box

    console = Console()
    t = get_theme(theme)

    discovered_ips = []
    if discover:
        with console.status(f"[cyan]Scanning {subnet}0/24...", spinner="dots"):
            discovered_ips = discover_hosts(subnet_prefix=subnet)
        if not discovered_ips:
            console.print(f"\n  [{t['warn_style']}]No hosts found on {subnet}0/24[/]\n")
            return
        console.print(f"\n  [{t['ok_style']}]Found {len(discovered_ips)} hosts[/]\n")
        for ip in discovered_ips:
            console.print(f"  [{t['value_style']}]• {ip}[/]")
        console.print()
        return

    if not hosts:
        console.print(f"  [{t['error_style']}]Usage: ros2_info fleet <host1> [host2...] or ros2_info fleet --discover[/]")
        console.print(f"  [{t['dim_style']}]Examples:[/]")
        console.print(f"  [{t['dim_style']}]  ros2_info fleet robot1.local robot2.local[/]")
        console.print(f"  [{t['dim_style']}]  ros2_info fleet --discover[/]")
        console.print(f"  [{t['dim_style']}]  ros2_info fleet 192.168.1.100 192.168.1.101[/]")
        return

    fleet_hosts = [
        FleetHost(hostname=h, ip=h, username=user, port=port, key_path=key)
        for h in hosts
    ]

    with console.status(f"[cyan]Checking {len(fleet_hosts)} hosts...", spinner="dots"):
        results = collect_fleet(fleet_hosts)

    if out_json:
        click.echo(json.dumps(results, indent=2, default=str))
        return

    console.print(f"\n  [bold {t['logo_color1']}]🤖 Fleet Status ({len(results)} hosts)[/]\n")

    tbl = Table(border_style=t["panel_border"], box=box.MINIMAL_HEAVY_HEAD)
    tbl.add_column("Host", style=f"bold {t['key_style']}")
    tbl.add_column("Status", style=t["value_style"])
    tbl.add_column("ROS2", style=t["value_style"])
    tbl.add_column("Memory", style=t["value_style"])
    tbl.add_column("Disk", style=t["value_style"])

    for r in results:
        host_label = r.get("hostname", r.get("ip", "?"))
        if r.get("reachable"):
            status = f"[{t['ok_style']}]🟢 Online[/]"
            ros = r.get("ros_distro") or f"[{t['dim_style']}]—[/]"
            mem = r.get("memory") or f"[{t['dim_style']}]—[/]"
            disk = r.get("disk") or f"[{t['dim_style']}]—[/]"
        else:
            status = f"[{t['error_style']}]🔴 Offline[/]"
            ros = "—"
            mem = "—"
            disk = "—"
        tbl.add_row(host_label, status, ros, mem, disk)

    console.print(tbl)
    console.print()


@main.command("tui")
@click.option("--release", is_flag=True, default=True, help="Use release build")
def cmd_tui(release):
    """🖥 Launch the full-screen Rust TUI dashboard.

    Opens a real-time terminal UI with tabs for system, ROS2,
    workspace, diagnostics, trends, and fleet monitoring.
    Includes a VS Code-like file explorer, command bar, and
    sandbox/global mode toggle.
    """
    import subprocess
    import os

    base = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(__file__))))
    tui_dir = os.path.join(base, "src", "tui")
    if release:
        binary = os.path.join(tui_dir, "target", "release", "ros2-info-tui")
    else:
        binary = os.path.join(tui_dir, "target", "debug", "ros2-info-tui")

    if not os.path.exists(binary):
        console = Console()
        console.print(f"  [yellow]TUI binary not found at:[/] {binary}")
        console.print(f"  [yellow]Build it with:[/]")
        console.print(f"    cd {tui_dir} && cargo build --release")
        return

    os.execv(binary, [binary])


@main.command("sandbox")
@click.argument("action", type=click.Choice(["run", "launch", "export", "status"]))
@click.argument("args", nargs=-1)
@click.option("--namespace", "-n", default="/sandbox", help="Sandbox namespace")
@click.option("--domain", "-d", default="42", help="ROS_DOMAIN_ID for isolation")
@click.option("--theme", "-t", default="default", type=click.Choice(list_themes()))
def cmd_sandbox(action, args, namespace, domain, theme):
    """🧪 Run ROS 2 nodes in an isolated sandbox environment.

    Sandbox mode isolates nodes via custom ROS_NAMESPACE and an
    optional ROS_DOMAIN_ID so they don't interfere with production.

    Examples:\n
      ros2_info sandbox run demo_nodes_cpp talker\n
      ros2_info sandbox launch my_pkg my_file.launch.py\n
      ros2_info sandbox export ~/.ros2_info/global.json\n
      ros2_info sandbox status\n
    """
    from fetch_info.sandbox import create_sandbox, export_to_global, SandboxConfig
    console = Console()
    t = get_theme(theme)

    if action == "run":
        if len(args) < 2:
            console.print(f"  [{t['error_style']}]Usage: sandbox run <package> <executable> [args...][/]")
            return
        cfg = SandboxConfig(namespace=namespace, domain_id=domain)
        sx = create_sandbox(namespace=namespace)
        console.print(f"  [{t['ok_style']}]Running {args[0]}/{args[1]} in namespace '{namespace}' (domain {domain})[/]")
        proc = sx.run_node(args[0], args[1], list(args[2:]))
        console.print(f"  [{t['dim_style']}]PID {proc.pid} — Ctrl+C to stop[/]")
        try:
            for line in proc.stdout:
                console.print(f"  [{t['value_style']}]{line.rstrip()}[/]")
        except KeyboardInterrupt:
            sx.stop_all()
            console.print(f"\n  [{t['warn_style']}]Sandbox stopped.[/]")

    elif action == "launch":
        if len(args) < 2:
            console.print(f"  [{t['error_style']}]Usage: sandbox launch <package> <launch_file>[/]")
            return
        sx = create_sandbox(namespace=namespace)
        console.print(f"  [{t['ok_style']}]Launching {args[0]}/{args[1]} in sandbox[/]")
        proc = sx.run_launch(args[0], args[1])
        try:
            for line in proc.stdout:
                console.print(f"  [{t['value_style']}]{line.rstrip()}[/]")
        except KeyboardInterrupt:
            sx.stop_all()
            console.print(f"\n  [{t['warn_style']}]Sandbox stopped.[/]")

    elif action == "export":
        target = os.path.expanduser(args[0] if args else "~/.ros2_info/global_config.json")
        cfg = SandboxConfig(namespace=namespace, domain_id=domain)
        if export_to_global(cfg, target):
            console.print(f"  [{t['ok_style']}]Exported sandbox configuration → {target}[/]")
            console.print(f"  [{t['dim_style']}]Note: namespace stripped for global use[/]")
        else:
            console.print(f"  [{t['error_style']}]Export failed.[/]")

    elif action == "status":
        in_sb = os.environ.get("ROS_NAMESPACE", "")
        if in_sb:
            console.print(f"  [{t['warn_style']}]Sandbox active: namespace={in_sb}[/]")
        else:
            console.print(f"  [{t['dim_style']}]Not in sandbox mode (running global namespace)[/]")
            console.print(f"  [{t['dim_style']}]Use: ros2_info sandbox run ... to start a sandboxed node[/]")


CHECK_RESULTS = {
    "pass": "✅",
    "warn": "⚠️ ",
    "fail": "❌",
    "info": "ℹ️ ",
}

if __name__ == "__main__":
    main()
