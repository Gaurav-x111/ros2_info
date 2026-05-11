import os
import shutil
from rich.console import Console
from rich.text import Text

def get_clean_os(sys_data):
    osi = sys_data.get("os", {})
    os_name = osi.get("name", "Unknown OS")
    os_ver = osi.get("version", "")
    os_cod = osi.get("codename", "")
    full = f"{os_name} {os_ver}".strip()
    if os_cod:
        full += f" ({os_cod})"
    return full

class TerminalNativeImage:
    def __init__(self, distro, fallback_ansi, width_cols=None):
        self.distro = distro
        self.fallback = fallback_ansi
        self.width_cols = width_cols
    
    def __rich_console__(self, console, options):
        # Hybrid Render (Option 2 Bitmap with Option 3 Unicode Fallback built-in)
        from fetch_info.display.graphics import GraphicsEngine
        width_cols = self.width_cols or resolve_logo_width_cols(console)

        # Rich expects __rich_console__ to yield renderables, not return one directly.
        yield GraphicsEngine.render(
            resolve_logo_image_path(self.distro),
            self.fallback,
            width_cols=width_cols,
        )

    def __rich_measure__(self, console, options):
        from rich.measure import Measurement
        width_cols = self.width_cols or resolve_logo_width_cols(console)
        return Measurement(width_cols, width_cols)

def get_fastfetch_art(distro, theme, width_cols=None):
    distro_key = (distro or "unknown").lower()
    try:
        # High-quality HTML-converted ANSI logos are width-aware. They make the
        # terminal fallback responsive while bitmap-capable terminals use PNGs.
        ansi_str = ""
        try:
            from fetch_info.display.ascii_logos import get_logo as get_ascii_logo
            ansi_str = get_ascii_logo(distro_key, width_cols or 80)
        except Exception:
            pass
        if not ansi_str:
            from fetch_info.display.distro_art import DISTRO_ART
            ansi_str = DISTRO_ART.get(distro_key, DISTRO_ART.get("generic", ""))
        if not ansi_str:
            # Fall back to handcrafted ANSI logos when no unicode block art exists
            from fetch_info.display.logos import get_logo
            ansi_str = get_logo(distro_key)
        return TerminalNativeImage(distro_key, ansi_str, width_cols=width_cols)
    except Exception:
        # Try ascii_logos directly
        try:
            from fetch_info.display.ascii_logos import get_logo as get_ascii_logo
            from rich.text import Text
            return Text.from_ansi(get_ascii_logo(distro_key, width_cols or 80))
        except Exception:
            pass
        # Ultimate fallback: try logos.py directly
        try:
            from fetch_info.display.logos import get_logo
            from rich.text import Text
            return Text.from_ansi(get_logo(distro_key))
        except Exception:
            from rich.text import Text
            t = Text()
            t.append("ROS2 Generic Logo Fallback\n", style="bold white")
            return t


def resolve_logo_image_path(distro: str) -> str:
    """Resolve the best bitmap asset for a ROS2 distro logo."""
    base_dir = os.path.dirname(__file__)
    distro_key = (distro or "").lower()
    candidates = (
        f"{distro_key}_cropped.png",
        f"{distro_key}.png",
    )
    for candidate in candidates:
        img_path = os.path.join(base_dir, "assets", candidate)
        if os.path.exists(img_path):
            return img_path
    return ""


def resolve_logo_width_cols(console: Console, stacked: bool = False, terminal_width: int | None = None) -> int:
    """Choose the logo width from the current terminal layout."""
    override = os.environ.get("ROS2_INFO_IMAGE_COLS", "").strip()
    if override.isdigit():
        return max(1, int(override))

    if terminal_width is None:
        terminal_width = shutil.get_terminal_size((80, 24)).columns

    if stacked:
        return max(1, terminal_width)
    return max(1, terminal_width // 3)


def prefers_stacked_logo_layout(console: Console, terminal_width: int | None = None) -> bool:
    """Use stacked layout on terminals narrower than 100 columns."""
    override = os.environ.get("ROS2_INFO_STACK_LOGO", "").strip().lower()
    if override in {"1", "true", "yes", "on"}:
        return True
    if override in {"0", "false", "no", "off"}:
        return False

    if terminal_width is None:
        terminal_width = shutil.get_terminal_size((80, 24)).columns
    return terminal_width < 100

def render_fastfetch(console: Console, data: dict, theme: dict, show_logo: bool = True, 
                     force_ascii: bool = False, force_image: bool = False, verbose: bool = False) -> None:
    """Render system info in fastfetch style.
    
    Args:
        console: Rich console for output
        data: Collected system/ROS2 data dictionary
        theme: Theme dictionary for colors and styles
        show_logo: Whether to display distro logo
        force_ascii: Force ASCII logo rendering
        force_image: Force image rendering
        verbose: Show expanded information
    """
    terminal_size = shutil.get_terminal_size((80, 24))
    terminal_width = terminal_size.columns
    terminal_height = terminal_size.lines

    sys_data = data.get("system", {})
    ros = data.get("ros2", {})

    distro = ros.get("distro", "none")
    distro_full = ros.get("distro_info", {}).get("full", distro.capitalize() if distro != "none" else "Not Sourced")
    domain_id = ros.get("domain_id", "0")
    rmw = ros.get("dds", "rmw_fastrtps_cpp")
    if rmw == "Unknown" and os.environ.get("RMW_IMPLEMENTATION"):
        rmw = os.environ.get("RMW_IMPLEMENTATION")
    
    nodes_count = len(ros.get("nodes", []))
    topics_count = len(ros.get("topics", []))
    services_count = len(ros.get("services", []))
    actions_count = len(ros.get("actions", []))
    workspace_path = ros.get("workspace_source", "")

    # CPU usage (live %)
    cpu_pct = ""
    try:
        import psutil
        cpu_pct = f"{psutil.cpu_percent(interval=0.1):.0f}%"
    except Exception:
        pass

    # Calculate padding for keys
    info_lines = [
        ("OS", get_clean_os(sys_data)),
        ("Kernel", sys_data.get("os", {}).get("kernel", "Unknown")),
        ("Hardware", sys_data.get("os", {}).get("arch", "Unknown")),
        ("Uptime", sys_data.get("uptime", "Unknown")),
        ("Shell", sys_data.get("shell", "Unknown")),
        ("Terminal", sys_data.get("terminal", "Unknown")),
    ]

    # CPU info
    cpu = sys_data.get("cpu", {})
    if cpu and cpu.get("model", "Unknown") != "Unknown":
        model = cpu["model"]
        if len(model) > 40:
            model = model[:37] + "..."
        freq = f" @ {cpu['freq_mhz']}MHz" if cpu.get('freq_mhz') else ""
        info_lines.append(("CPU", f"{model} ({cpu.get('cores','?')}C/{cpu.get('threads','?')}T{freq})"))

    # GPU info
    gpu = sys_data.get("gpu")
    if gpu:
        info_lines.append(("GPU", gpu if len(gpu) <= 50 else gpu[:47] + "..."))

    # Memory
    mem = sys_data.get("memory", {})
    if mem and mem.get("total_gb", 0):
        pct = mem["percent"]
        bar_w = 15
        fill = int(bar_w * pct / 100)
        bar_color = "red" if pct >= 90 else "yellow" if pct >= 70 else "green"
        info_lines.append(("Memory", f"{mem['used_gb']}/{mem['total_gb']} GB ({pct}%)"))

    # Disk
    disk = sys_data.get("disk", {})
    if disk and disk.get("total_gb", 0):
        info_lines.append(("Disk /", f"{disk['used_gb']}/{disk['total_gb']} GB ({disk['percent']}%)"))

    info_lines.append(("", ""))  # separator

    info_lines += [
        ("ROS2 Distro", distro_full),
        ("ROS_DOMAIN_ID", str(domain_id)),
        ("RMW Impl", rmw),
        ("Active Nodes", str(nodes_count)),
        ("Active Topics", str(topics_count)),
        ("Active Services", str(services_count)),
    ]

    # Conditionally add actions count (only if > 0)
    if actions_count > 0:
        info_lines.append(("Active Actions", str(actions_count)))

    # Conditionally add workspace path
    if workspace_path:
        info_lines.append(("Workspace", workspace_path))

    # CPU usage if available
    if cpu_pct:
        info_lines.append(("CPU Usage", cpu_pct))

    bat = sys_data.get("battery", {})
    if bat and bat.get("percent") is not None:
        b_icon = "🔌" if bat.get("plugged") else "🔋"
        info_lines.append(("Battery", f"{bat['percent']}% {b_icon}"))

    net = sys_data.get("network", {})
    if net and net.get("sent_mb") is not None:
        info_lines.append(("Network", f"↓ {net['recv_mb']}MB ↑ {net['sent_mb']}MB"))

    # Python version
    info_lines.append(("Python", sys_data.get("python", "Unknown")))

    # CPU temperature if available
    temps = sys_data.get("temperatures", {})
    if temps:
        t_parts = []
        for k, v in list(temps.items())[:2]:
            name = k.split()[0].replace("coretemp", "CPU").replace("acpitz", "ACPI")
            t_parts.append(f"{name}: {v}°C")
        if t_parts:
            info_lines.append(("Sensors", "  ".join(t_parts)))

    # Pre-measure the maximum key length for perfect alignment
    max_k_len = max(len(k) for k, _ in info_lines) + 1  # +1 for the colon

    right_col = Text()
    # Adding a header similar to neofetch username@hostname
    user = os.environ.get("USER", "user")
    host = sys_data.get("hostname", "hostname")
    right_col.append(f"{user}", style=f"bold {theme.get('logo_color1', '#22D3EE')}")
    right_col.append("@", style="default")
    right_col.append(f"{host}\n", style=f"bold {theme.get('logo_color2', '#A3E635')}")
    right_col.append("-" * (len(user) + len(host) + 1) + "\n", style=theme.get("dim_style", "dim"))

    for k, v in info_lines:
        if not k and not v:
            # Render a dim separator line between sections
            right_col.append("─" * (max_k_len + 2) + "\n", style=theme.get("dim_style", "dim"))
            continue
        key_str = f"{k}:"
        # Pad string to align everything
        padded_key = key_str.ljust(max_k_len + 1)
        right_col.append(padded_key, style=f"bold {theme.get('logo_color1', '#22D3EE')}")
        right_col.append(f"{v}\n", style=theme.get("value_style", "white"))
    
    # Custom color bar row at bottom of the right column
    colors = ["#EF4444", "#F59E0B", "#10B981", "#3B82F6", "#8B5CF6", "#EC4899", "#E5E7EB", "#374151"]
    right_col.append("\n  ")
    for c in colors[:4]:
        right_col.append("███", style=c)
    right_col.append("\n  ")
    for c in colors[4:]:
        right_col.append("███", style=c)

    stacked_logo = prefers_stacked_logo_layout(console, terminal_width=terminal_width)
    logo_width = resolve_logo_width_cols(
        console,
        stacked=stacked_logo,
        terminal_width=terminal_width,
    )
    left_art = get_fastfetch_art(
        distro,
        theme,
        width_cols=logo_width,
    ) if show_logo else None
    
    from rich.table import Table
    console.print()
    if not show_logo:
        # No logo - just show info on left
        console.print(right_col)
    elif stacked_logo:
        console.print(left_art)
        console.print()
        console.print(right_col)
    else:
        gap = 4
        info_width = max(1, terminal_width - logo_width - gap)
        grid = Table.grid(padding=(0, 0))
        grid.add_column(width=logo_width, no_wrap=True)
        grid.add_column(width=gap)
        grid.add_column(width=info_width)
        grid.add_row(left_art, " " * gap, right_col)
        console.print(grid)
    console.print()
