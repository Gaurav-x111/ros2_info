"""
ROS2 Info — Display Panels (v2)
Responsive two-column layout: distro pixel art on left, info on right.
Panels auto-adapt to terminal width.
"""

from rich.console import Console
from rich.panel import Panel
from rich.table import Table
from rich.text import Text
from rich.rule import Rule
from rich.columns import Columns
from rich import box


def _bar(pct: float, width: int = 20) -> Text:
    """Create a progress bar showing percentage with colors.
    
    Args:
        pct: Percentage (0-100)
        width: Width of the bar in characters
        
    Returns:
        Rich Text object with colored bar visualization
    """
    fill = int(width * pct / 100)
    color = "red" if pct >= 90 else "yellow" if pct >= 70 else "cyan"
    t = Text()
    t.append("█" * fill, style=color)
    t.append("░" * (width - fill), style="grey30")
    t.append(f" {pct:.1f}%", style="white")
    return t


def _term_width(console: Console) -> int:
    """Current terminal width."""
    return console.width or 120


def render_header(console, theme, hostname, distro):
    from fetch_info.display.logo import get_distro_art, get_main_banner
    w = _term_width(console)

    # On narrow terminals just show compact header
    if w < 100:
        console.print()
        art = get_distro_art(distro, theme)
        console.print(art)
        row = Text()
        row.append(f"  {hostname}", style=f"bold {theme['logo_color1']}")
        row.append("  ●  ", style=theme["dim_style"])
        row.append(f"ROS2 {distro.capitalize()}" if distro else "ROS2 not sourced",
                   style=f"bold {theme['highlight']}" if distro else theme["error_style"])
        console.print(row)
    else:
        console.print()
        console.print(get_main_banner(theme))
    console.print(Rule(style=theme["panel_border"]))


def render_ros2_panel(console, data, theme):
    ros = data.get("ros2", {})
    tbl = Table.grid(padding=(0, 2))
    tbl.add_column(style=theme["key_style"], no_wrap=True, min_width=22)
    tbl.add_column(style=theme["value_style"])

    distro = ros.get("distro")
    di = ros.get("distro_info", {})
    if distro:
        dt = Text()
        dt.append(di.get("full", distro.capitalize()), style=f"bold {theme['highlight']}")
        if di.get("lts"):
            dt.append("  LTS", style=theme["ok_style"])
        tbl.add_row("  ROS2 Distro", dt)

        eol = di.get("eol", "Unknown")
        from datetime import datetime
        et = Text()
        try:
            if eol != "N/A":
                d = datetime.strptime(eol, "%Y-%m")
                if d < datetime.now():
                    et.append(f"EOL: {eol}  ⚠ EXPIRED", style=theme["error_style"])
                elif (d - datetime.now()).days < 180:
                    et.append(f"EOL: {eol}  Expiring Soon", style=theme["warn_style"])
                else:
                    et.append(f"Supported until {eol}", style=theme["ok_style"])
            else:
                et.append("Rolling — No EOL", style=theme["warn_style"])
        except Exception:
            et.append(eol)
        tbl.add_row("  Support Status", et)
    else:
        tbl.add_row("  ROS2 Distro", Text("Not sourced — run: source /opt/ros/<distro>/setup.bash",
                                           style=theme["error_style"]))

    tbl.add_row("  ROS2 CLI", Text("Available ✓" if ros.get("available") else "Not found",
                                    style=theme["ok_style"] if ros.get("available") else theme["error_style"]))
    tbl.add_row("  DDS",        ros.get("dds", "Unknown"))
    tbl.add_row("  Domain ID",  str(ros.get("domain_id", "0")))

    ws = ros.get("workspace_source")
    tbl.add_row("  Workspace",  ws if ws else Text("None sourced", style=theme["dim_style"]))

    pkg = ros.get("packages", {})
    if pkg.get("total", 0):
        tbl.add_row("  Pkgs Installed", str(pkg["total"]))
        cats = pkg.get("categories", {})
        if cats:
            ct = Text()
            for i, c in enumerate(list(cats.keys())[:6]):
                if i: ct.append("  •  ", style=theme["dim_style"])
                ct.append(c, style=theme["highlight"])
            tbl.add_row("  Categories", ct)

    last_upd = ros.get("last_updated")
    if last_upd:
        tbl.add_row("  Last Updated", Text(last_upd, style=theme["value_style"]))

    upd = ros.get("updates")
    if upd:
        ut = Text()
        if "up to date" in upd.lower():
            ut.append("✓ " + upd, style=theme["ok_style"])
        else:
            ut.append("⚠ " + upd, style=theme["warn_style"])
        tbl.add_row("  Updates", ut)

    console.print(Panel(tbl, title=f"[{theme['section_title']}]  ROS2 Environment[/]",
                        border_style=theme["panel_border"], padding=(0,1), box=box.ROUNDED))


def render_system_panel(console, data, theme):
    """Responsive two-column: pixel art LEFT, system info RIGHT."""
    from fetch_info.display.logo import get_distro_art
    sys_data = data.get("system", {})
    if not sys_data:
        return

    distro = data.get("ros2", {}).get("distro")
    w = _term_width(console)

    # Build info table
    info_tbl = Table.grid(padding=(0, 2))
    info_tbl.add_column(style=theme["key_style"], no_wrap=True, min_width=18)
    info_tbl.add_column(style=theme["value_style"])

    osi = sys_data.get("os", {})
    os_str = f"{osi.get('name', '')} {osi.get('version', '')}".strip()
    if osi.get("codename"):
        os_str += f" ({osi['codename']})"

    info_tbl.add_row("  OS",        os_str)
    info_tbl.add_row("  Kernel",    osi.get("kernel", "Unknown"))
    info_tbl.add_row("  Arch",      osi.get("arch", "Unknown"))
    info_tbl.add_row("  Host",      sys_data.get("hostname", "unknown"))
    info_tbl.add_row("  Uptime",    sys_data.get("uptime", "Unknown"))
    info_tbl.add_row("  Shell",     sys_data.get("shell", "Unknown"))
    info_tbl.add_row("  Terminal",  sys_data.get("terminal", "Unknown"))
    info_tbl.add_row("  Python",    sys_data.get("python", "Unknown"))

    cpu = sys_data.get("cpu", {})
    if cpu:
        model = cpu.get("model", "Unknown")
        if len(model) > 42:
            model = model[:39] + "..."
        freq_str = f" @ {cpu['freq_mhz']} MHz" if cpu.get("freq_mhz") else ""
        info_tbl.add_row("  CPU", f"{model} ({cpu.get('cores','?')}C/{cpu.get('threads','?')}T{freq_str})")

    gpu = sys_data.get("gpu")
    if gpu:
        info_tbl.add_row("  GPU", gpu)

    mem = sys_data.get("memory", {})
    if mem and mem.get("total_gb", 0):
        mt = Text()
        mt.append(f"{mem['used_gb']} / {mem['total_gb']} GB  ")
        mt.append_text(_bar(mem["percent"], width=15))
        info_tbl.add_row("  RAM", mt)

    disk = sys_data.get("disk", {})
    if disk and disk.get("total_gb", 0):
        dt = Text()
        dt.append(f"{disk['used_gb']} / {disk['total_gb']} GB  ")
        dt.append_text(_bar(disk["percent"], width=15))
        info_tbl.add_row("  Disk /", dt)

    bat = sys_data.get("battery", {})
    if bat:
        bt = Text()
        bt.append(f"{bat['percent']}% ")
        bt.append("🔌" if bat["plugged"] else "🔋", style=theme["ok_style"] if bat["plugged"] else theme["warn_style"])
        if not bat["plugged"] and bat["time_left"] > 0:
            hrs = bat["time_left"] // 3600
            mins = (bat["time_left"] % 3600) // 60
            bt.append(f" ({hrs}h {mins}m)", style=theme["dim_style"])
        info_tbl.add_row("  Battery", bt)

    net = sys_data.get("network", {})
    if net:
        nt = Text(f"↓ {net['recv_mb']} MB  ↑ {net['sent_mb']} MB", style=theme["value_style"])
        info_tbl.add_row("  Network", nt)

    temps = sys_data.get("temperatures", {})
    if temps:
        # Just show the first 2-3 most relevant temperatures to avoid clutter
        t_list = []
        for k, v in list(temps.items())[:3]:
            # Simplify name
            name = k.split()[0].replace("coretemp", "CPU").replace("acpitz", "ACPI").replace("iwlwifi", "WiFi")
            color = theme["error_style"] if v > 80 else theme["warn_style"] if v > 65 else theme["ok_style"]
            t_list.append(f"[{theme['dim_style']}]{name}[/] [{color}]{v}°C[/]")
        if t_list:
            info_tbl.add_row("  Sensors", "  ".join(t_list))

    if w >= 110:
        # Wide terminal: art on left, info on right inside a two-column panel
        art = get_distro_art(distro, theme)
        art_panel = Panel(
            art,
            border_style=theme["logo_color2"],
            box=box.ROUNDED,
            padding=(0, 1),
            width=24,
        )
        info_panel = Panel(
            info_tbl,
            title=f"[{theme['section_title']}]  System Info[/]",
            border_style=theme["panel_border"],
            padding=(0, 1),
            box=box.ROUNDED,
        )
        console.print(Columns([art_panel, info_panel], expand=True))
    else:
        # Narrow: art on top, info below
        art = get_distro_art(distro, theme)
        console.print(art)
        console.print(Panel(info_tbl, title=f"[{theme['section_title']}]  System Info[/]",
                            border_style=theme["panel_border"], padding=(0,1), box=box.ROUNDED))


def render_live_panel(console, data, theme):
    ros = data.get("ros2", {})
    nodes    = ros.get("nodes", [])
    topics   = ros.get("topics", [])
    services = ros.get("services", [])
    actions  = ros.get("actions", [])
    w = _term_width(console)
    limit = 8 if w >= 120 else 5

    tbl = Table.grid(padding=(0, 2))
    tbl.add_column(style=theme["key_style"], no_wrap=True, min_width=22)
    tbl.add_column(style=theme["value_style"])

    def fmt_list(items, lim=limit, bullet="●", style=None):
        t = Text()
        style = style or theme["value_style"]
        if not items:
            return Text("None", style=theme["dim_style"])
        for i, item in enumerate(items[:lim]):
            if i: t.append("\n" + " " * 24)
            t.append(f"{bullet} ", style=theme["ok_style"])
            t.append(item, style=style)
        if len(items) > lim:
            t.append(f"\n{' '*24}... and {len(items)-lim} more", style=theme["dim_style"])
        return t

    tbl.add_row(f"  Nodes ({len(nodes)})",    fmt_list(nodes))
    tbl.add_row(f"  Topics ({len(topics)})",   fmt_list([t['name'] for t in topics]))
    tbl.add_row(f"  Services ({len(services)})", fmt_list([s for s in services if "parameter" not in s][:8]))
    if actions:
        tbl.add_row(f"  Actions ({len(actions)})", fmt_list(actions, bullet="▶"))

    console.print(Panel(tbl, title=f"[{theme['section_title']}]  Live Runtime[/]",
                        border_style=theme["panel_border"], padding=(0,1), box=box.ROUNDED))


def render_workspace_panel(console, data, theme):
    workspaces = data.get("workspace", {}).get("workspaces", [])
    if not workspaces:
        console.print(Panel(
            Text("  No colcon workspaces found.\n  mkdir -p ~/ros2_ws/src && cd ~/ros2_ws && colcon build",
                 style=theme["dim_style"]),
            title=f"[{theme['section_title']}]  Workspaces[/]",
            border_style=theme["panel_border"], box=box.ROUNDED))
        return

    tbl = Table.grid(padding=(0, 2))
    tbl.add_column(style=theme["key_style"], no_wrap=True, min_width=22)
    tbl.add_column(style=theme["value_style"])

    for ws in workspaces:
        wt = Text(ws["path"], style=f"bold {theme['highlight']}")
        tbl.add_row("  Workspace", wt)
        st = Text()
        st.append(f"{ws['package_count']} packages", style=theme["ok_style"])
        st.append("  |  ", style=theme["dim_style"])
        st.append("Built ✓" if ws.get("has_install") else "Not built",
                  style=theme["ok_style"] if ws.get("has_install") else theme["warn_style"])
        if ws.get("launches"):
            st.append(f"  |  {ws['launches']} launch files", style=theme["dim_style"])
        tbl.add_row("    Stats", st)
        pkgs = [p["name"] for p in ws.get("packages", [])[:6]]
        if pkgs: tbl.add_row("    Packages", "  ".join(pkgs))
        tbl.add_row("", "")

    console.print(Panel(tbl, title=f"[{theme['section_title']}]  Workspaces[/]",
                        border_style=theme["panel_border"], padding=(0,1), box=box.ROUNDED))


def render_env_panel(console, data, theme):
    env = data.get("ros2", {}).get("environment", {})
    if not env: return
    tbl = Table.grid(padding=(0, 2))
    tbl.add_column(style=theme["key_style"], no_wrap=True, min_width=30)
    tbl.add_column(style=theme["value_style"])
    for k, v in env.items():
        tbl.add_row(f"  {k}", v)
    console.print(Panel(tbl, title=f"[{theme['section_title']}]  Environment Vars[/]",
                        border_style=theme["panel_border"], padding=(0,1), box=box.ROUNDED))


def render_footer(console, theme):
    console.print(Rule(style=theme["panel_border"]))
    t = Text()
    t.append("  Tips: ", style=f"bold {theme['logo_color1']}")
    for tip in ["terminal", "--live", "--watch 2", "--theme neon", "--json", "graph", "bag", "param"]:
        t.append(f"ros2_info {tip}  ", style=theme["dim_style"])
    console.print(t)
    console.print()


def render_all(console, data, theme_name, show_live=False, show_env=False):
    from fetch_info.display.themes import get_theme
    theme = get_theme(theme_name)
    sys_data = data.get("system", {})
    hostname = sys_data.get("hostname", "unknown")
    distro   = data.get("ros2", {}).get("distro")

    render_header(console, theme, hostname, distro)
    render_ros2_panel(console, data, theme)
    if show_live or data.get("ros2", {}).get("nodes"):
        render_live_panel(console, data, theme)
    render_system_panel(console, data, theme)
    render_workspace_panel(console, data, theme)
    if show_env:
        render_env_panel(console, data, theme)
    render_footer(console, theme)
