import click
import shutil
from rich.console import Console
from rich.table import Table
from rich.text import Text
from fetch_info.display.panels import render_all
from fetch_info.display.themes import get_theme, list_themes
from fetch_info.collector import system, ros2, workspace
import json, time


def collect(live: bool = False, skip_ws: bool = False, skip_sys: bool = False, timeout: int = 3, updates: bool = True) -> dict:
    """Collect system, ROS2, and workspace information.
    
    Args:
        live: Whether to collect live ROS2 runtime data (nodes/topics/services)
        skip_ws: Skip workspace collection
        skip_sys: Skip system information collection
        timeout: Timeout in seconds for ROS2 graph discovery
        updates: Whether to check for ROS2 package updates
        
    Returns:
        Dictionary with keys: 'system', 'ros2', 'workspace' (conditionally included)
    """
    data: dict = {}
    if not skip_sys:
        data["system"] = system.collect_all()
    data["ros2"] = ros2.collect_all(check_live=live, live_timeout=timeout, check_updates=updates)
    if not skip_ws:
        data["workspace"] = workspace.collect_all()
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
        boot = bootstrap(console=console)

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
def cmd_web(port, host):
    """🌐 Launch the ROS2 Info web dashboard."""
    console = Console()
    console.print(f"\n  [bold cyan]🌐 ROS2 Info Web UI[/]")
    console.print(f"  [dim]Starting on http://localhost:{port}[/]\n")
    from fetch_info.web import run_web
    run_web(host=host, port=port)


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


if __name__ == "__main__":
    main()
