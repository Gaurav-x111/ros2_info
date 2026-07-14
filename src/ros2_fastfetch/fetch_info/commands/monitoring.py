"""Monitoring commands - graph, matrix, benchmark, trend, watch."""

import click
import time
import subprocess
import os
from rich.console import Console
from rich.table import Table
from rich import box

from fetch_info.display.themes import get_theme
from fetch_info.collector import ros2, system
from fetch_info.render import create_table, print_error, print_success, print_status


def build_topic_graph(timeout: int = 5) -> dict:
    """Build a node→topic→node connection map."""
    graph = {"nodes": {}, "topics": {}}

    if not __import__("shutil").which("ros2"):
        return graph

    from fetch_info.terminal import _run

    stdout, _, rc = _run(["ros2", "node", "list"], timeout=timeout)
    if rc != 0 or not stdout:
        return graph

    nodes = [n.strip() for n in stdout.split("\n") if n.strip()]

    for node in nodes:
        graph["nodes"][node] = {"pubs": [], "subs": []}
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
                if topic not in graph["topics"]:
                    graph["topics"][topic] = {"publishers": [], "subscribers": []}
                if section == "pubs" and node not in graph["topics"][topic]["publishers"]:
                    graph["topics"][topic]["publishers"].append(node)
                if section == "subs" and node not in graph["topics"][topic]["subscribers"]:
                    graph["topics"][topic]["subscribers"].append(node)

    return graph


def render_ascii_graph(console: Console, theme: dict, timeout: int = 5):
    """Render an ASCII rqt-graph style visualization."""
    from rich.panel import Panel
    from rich.rule import Rule
    from rich.text import Text

    console.print(f"\n  [{theme['highlight']}]Building RQT-style graph...[/]")
    graph = build_topic_graph(timeout)

    if not graph["nodes"]:
        console.print(f"  [{theme['warn_style']}]No nodes found. Is ROS2 running?[/]")
        return

    nodes = list(graph["nodes"].keys())
    topics = graph["topics"]
    connected = {t: v for t, v in topics.items() if v["publishers"] and v["subscribers"]}

    console.print()

    title = Text()
    title.append("  ⬡ RQT GRAPH", style=f"bold {theme['logo_color1']}")
    title.append(f"  {len(nodes)} nodes", style=theme['dim_style'])
    title.append(f"  •  {len(topics)} topics", style=theme['dim_style'])
    title.append(f"  •  {len(connected)} connections", style=theme['dim_style'])
    console.print(title)
    console.print(Rule(style=theme["panel_border"]))
    console.print()

    if connected:
        print_status(console, "Topic Connections:", theme.get("section_title", "highlight"))
        for topic, conns in list(connected.items())[:20]:
            pubs = conns["publishers"]
            subs = conns["subscribers"]

            t = Text()
            for i, pub in enumerate(pubs[:3]):
                short = pub.split("/")[-1]
                if i > 0:
                    t.append("\n" + " " * 6)
                t.append(f"[{short}]", style=f"bold {theme['ok_style']}")

            t.append(" ──► ", style=theme["dim_style"])
            t.append(topic, style=f"bold {theme['highlight']}")
            t.append(" ──► ", style=theme["dim_style"])

            for i, sub in enumerate(subs[:3]):
                short = sub.split("/")[-1]
                if i > 0:
                    t.append(", ")
                t.append(f"[{short}]", style=f"bold {theme['logo_color2']}")

            console.print("  ", t)

        if len(connected) > 20:
            console.print(f"\n  [{theme['dim_style']}]... and {len(connected)-20} more connections[/]")
    else:
        console.print(f"  [{theme['dim_style']}]No pub→sub connections found.[/]")

    console.print()
    print_status(console, "Node Overview:", theme.get("section_title", "highlight"))

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


@click.command("graph")
@click.option("--theme", "-t", default="default")
@click.option("--timeout", default=5, help="Discovery timeout (seconds)")
def cmd_graph(theme: str = "default", timeout: int = 5):
    """Show ASCII RQT-like node/topic graph."""
    console = Console()
    t = get_theme(theme)
    render_ascii_graph(console, t, timeout=timeout)


@click.command("matrix")
@click.option("--theme", "-t", default="default")
@click.option("--timeout", default=5)
@click.option("--json", "out_json", is_flag=True, help="Output raw JSON")
def cmd_matrix(theme: str = "default", timeout: int = 5, out_json: bool = False):
    """Show topic communication matrix (who talks to whom)."""
    import json
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
        print_status(console, "Topic Details:", t.get("section_title", "highlight"))
        for topic, conns in list(topics.items())[:15]:
            pubs = conns.get("publishers", [])
            subs = conns.get("subscribers", [])
            icon = "✓" if pubs and subs else "⚠"
            pub_str = ", ".join(p.split("/")[-1] for p in pubs[:3]) or "?"
            sub_str = ", ".join(s.split("/")[-1] for s in subs[:3]) or "?"
            console.print(f"  {icon}  [{t['highlight']}]{topic}[/]")
            console.print(f"       Pub: [{t['ok_style']}]{pub_str}[/]")
            console.print(f"       Sub: [{t['logo_color2']}]{sub_str}[/]")

    console.print()


@click.command("benchmark")
@click.option("--duration", "-d", default=10, help="Duration in seconds")
@click.option("--theme", "-t", default="default")
@click.option("--json", "out_json", is_flag=True, help="Output raw JSON")
def cmd_benchmark(duration: int = 10, theme: str = "default", out_json: bool = False):
    """Benchmark ROS2 topic rates and system performance."""
    import json as json_module
    console = Console()
    t = get_theme(theme)

    console.print()
    console.print(f"  [bold {t['logo_color1']}]Performance Benchmark[/]")
    console.print(f"  [{t['dim_style']}]Measuring for {duration}s...[/]\n")

    sys_info = system.collect_all()
    topics = ros2.get_active_topics(timeout=3)
    nodes = ros2.get_active_nodes(timeout=3)

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

    if out_json:
        click.echo(json_module.dumps({
            "duration": duration,
            "topic_count": len(topics),
            "node_count": len(nodes),
            "topic_rates": topic_rates,
            "cpu_avg": sys_info.get("cpu", {}),
            "memory": sys_info.get("memory"),
        }, indent=2, default=str))
        return

    if topic_rates:
        tbl = create_table(title="Topic Rates", border_style=t["panel_border"])
        tbl.add_column("Topic", style=f"bold {t['key_style']}")
        tbl.add_column("Rate", style=t["value_style"])
        for name, rate in topic_rates.items():
            tbl.add_row(name, rate)
        console.print(tbl)
        console.print()

    mem = sys_info.get("memory", {})
    summary = Table(border_style=t["panel_border"], box=None)
    summary.add_column("Metric", style=t["key_style"])
    summary.add_column("Value", style=t["value_style"])
    summary.add_row("Nodes", str(len(nodes)))
    summary.add_row("Topics", str(len(topics)))
    summary.add_row("Memory", f"{mem.get('used_gb', '?')}/{mem.get('total_gb', '?')} GB")
    console.print(summary)
    console.print()


@click.command("trend")
@click.option("--hours", "-h", default=24, help="Time window in hours")
@click.option("--theme", "-t", default="default")
@click.option("--json", "out_json", is_flag=True, help="Output raw JSON")
@click.option("--record", is_flag=True, help="Record a snapshot now")
def cmd_trend(hours: int = 24, theme: str = "default", out_json: bool = False, record: bool = False):
    """Show historical system trends (SQLite-backed)."""
    import json as json_module
    from fetch_info.collector.trends import record_snapshot, get_trend, get_summary, get_chart_data, get_ascii_sparkline
    from fetch_info.collector import ros2 as ros2_col

    console = Console()
    t = get_theme(theme)

    if record:
        with console.status("[cyan]Recording snapshot...", spinner="dots"):
            sys_data = system.collect_all()
            ros2_data = ros2_col.collect_all(check_live=True, live_timeout=3, check_updates=False)
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
        click.echo(json_module.dumps(result, indent=2, default=str))
        return

    summary = get_summary()
    console.print()
    console.print(f"  [bold {t['logo_color1']}]📈 System Trends (last {hours}h)[/]")
    console.print()

    if summary["total_snapshots"] == 0:
        console.print(f"  [{t['warn_style']}]No historical data yet. Run: ros2_info trend --record[/]")
        console.print()
        return

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

    timestamps, cpu_values, memory_values, node_counts = get_chart_data(duration_hours=hours, max_points=50)

    if cpu_values:
        print_status(console, "CPU Usage Trend:", t.get("section_title", "highlight"))
        cpu_chart = get_ascii_sparkline(cpu_values, height=5, width=50)
        for row in cpu_chart:
            console.print(f"  [{t['ok_style']}]{row}[/]")

    if memory_values:
        print_status(console, "Memory Usage Trend:", t.get("section_title", "highlight"))
        mem_chart = get_ascii_sparkline(memory_values, height=5, width=50)
        for row in mem_chart:
            console.print(f"  [{t['warn_style']}]{row}[/]")

    console.print(f"\n  [{t['dim_style']}]Based on {summary['total_snapshots']} snapshots.[/]")
    console.print()