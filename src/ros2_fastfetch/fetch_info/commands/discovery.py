"""Discovery commands - nodes, topics, services, actions, packages, env."""

import click
from rich.console import Console
from rich.table import Table
from rich.columns import Columns
from rich.text import Text

from fetch_info.collector import ros2, workspace
from fetch_info.display.themes import get_theme
from fetch_info.render import create_table, print_error, print_success


def collect(theme_name: str = "default"):
    """Collect basic ROS2 info."""
    console = Console()
    theme = get_theme(theme_name)

    with console.status("[cyan]Collecting ROS2 info...", spinner="dots"):
        data = {
            "ros2": ros2.collect_all(check_live=False, live_timeout=3, check_updates=False),
        }
    return data


@click.command("nodes")
@click.option("--theme", "-t", default="default", type=click.Choance(list(get_theme.__globals__["list_themes"]())))
@click.option("--timeout", default=5)
@click.option("--info", "-I", "show_info", is_flag=True, help="Show pub/sub info for each node")
def cmd_nodes(theme: str = "default", timeout: int = 5, show_info: bool = False):
    """List all active ROS2 nodes."""
    console = Console()
    t = get_theme(theme)

    with console.status("[cyan]Querying nodes...", spinner="dots"):
        nodes = ros2.get_active_nodes(timeout)

    if not nodes:
        console.print("[yellow]No active nodes found. Is ROS2 running?[/]")
        return

    table = create_table(title="Active ROS2 Nodes", border_style=t["panel_border"])
    table.add_column("Node", style=t["key_style"])
    table.add_column("Status", style=t["ok_style"])

    for n in nodes:
        table.add_row(n, "● Running")
    console.print(table)

    if show_info:
        from fetch_info.terminal import _run
        from rich.panel import Panel
        from rich.syntax import Syntax
        from rich import box

        for n in nodes:
            stdout, _, rc = _run(["ros2", "node", "info", n], timeout=timeout)
            if rc == 0:
                syntax = Syntax(stdout, "yaml", theme="monokai", background_color="default")
                console.print(Panel(syntax, title=f"[bold]{n}[/]", border_style=t["panel_border"], box=box.ROUNDED))


@click.command("topics")
@click.option("--theme", "-t", default="default")
@click.option("--verbose", "-v", is_flag=True)
@click.option("--timeout", default=5)
@click.option("--hz", is_flag=True, help="Show publish rate for each topic (slow)")
def cmd_topics(theme: str = "default", verbose: bool = False, timeout: int = 5, hz: bool = False):
    """List all active ROS2 topics."""
    console = Console()
    t = get_theme(theme)

    with console.status("[cyan]Querying topics...", spinner="dots"):
        topics = ros2.get_active_topics(timeout)

    if not topics:
        console.print("[yellow]No active topics found.[/]")
        return

    table = create_table(title=f"Active Topics ({len(topics)})", border_style=t["panel_border"])
    table.add_column("Topic", style=t["key_style"])
    if verbose:
        table.add_column("Type", style=t["value_style"])

    for tp in topics:
        if verbose:
            table.add_row(tp["name"], tp["type"])
        else:
            table.add_row(tp["name"])
    console.print(table)


@click.command("services")
@click.option("--theme", "-t", default="default")
@click.option("--timeout", default=5)
def cmd_services(theme: str = "default", timeout: int = 5):
    """List all active ROS2 services."""
    console = Console()
    t = get_theme(theme)

    with console.status("[cyan]Querying services...", spinner="dots"):
        services = ros2.get_active_services(timeout)

    if not services:
        console.print("[yellow]No active services found.[/]")
        return

    table = create_table(title=f"Active Services ({len(services)})", border_style=t["panel_border"])
    table.add_column("Service", style=t["key_style"])
    for s in services:
        table.add_row(s)
    console.print(table)


@click.command("actions")
@click.option("--theme", "-t", default="default")
@click.option("--timeout", default=5)
def cmd_actions(theme: str = "default", timeout: int = 5):
    """List all active ROS2 actions."""
    console = Console()
    t = get_theme(theme)

    with console.status("[cyan]Querying actions...", spinner="dots"):
        actions = ros2.get_active_actions(timeout)

    if not actions:
        console.print("[yellow]No active actions found.[/]")
        return

    table = create_table(title=f"Active Actions ({len(actions)})", border_style=t["panel_border"])
    table.add_column("Action", style=t["key_style"])
    for a in actions:
        table.add_row(a)
    console.print(table)


@click.command("packages")
@click.option("--theme", "-t", default="default")
@click.option("--filter", "-f", "pkg_filter", default="")
def cmd_packages(theme: str = "default", pkg_filter: str = ""):
    """List all installed ROS2 packages."""
    import os
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


@click.command("workspace")
@click.option("--theme", "-t", default="default")
def cmd_workspace(theme: str = "default"):
    """Scan for colcon workspaces."""
    from fetch_info.display.panels import render_workspace_panel
    console = Console()
    t = get_theme(theme)

    with console.status("[cyan]Scanning...", spinner="dots"):
        data = {"workspace": workspace.collect_all()}
    render_workspace_panel(console, data, t)


@click.command("env")
@click.option("--theme", "-t", default="default")
def cmd_env(theme: str = "default"):
    """Show all ROS2 environment variables."""
    from fetch_info.terminal import _run
    console = Console()
    t = get_theme(theme)

    env = ros2.get_ros2_environment()
    if not env:
        console.print("[yellow]No ROS2 env vars found. Source ROS2 first.[/]")
        return

    table = create_table(title="ROS2 Environment", border_style=t["panel_border"], show_lines=True)
    table.add_column("Variable", style=t["key_style"])
    table.add_column("Value", style=t["value_style"])

    for k, v in env.items():
        table.add_row(k, v)
    console.print(table)