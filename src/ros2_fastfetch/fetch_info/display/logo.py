"""
ROS2 Info — Distro Pixel Art & Banners
Colored block-pixel art for each ROS2 distro (renders as side image in terminal).
"""

from rich.text import Text


# ── Pixel art using colored Unicode block characters ─────────────────────────
# Each entry: list of (char, color_hex) per cell, one list per row

def _px(color: str) -> str:
    """Return a colored full-block using Rich markup."""
    return f"[{color}]██[/]"


# ── Jazzy Jalisco — musical purple/gold ──────────────────────────────────────
def art_jazzy() -> list[str]:
    p = "#A855F7"; g = "#EAB308"; w = "#F5F5F5"
    rows = [
        f"  {_px(p)}{_px(p)}{_px(g)}{_px(g)}{_px(p)}{_px(p)}  ",
        f"{_px(p)}{_px(g)}{_px(w)}{_px(w)}{_px(g)}{_px(p)}{_px(g)}{_px(p)}",
        f"{_px(p)}{_px(w)}{_px(g)}{_px(p)}{_px(p)}{_px(g)}{_px(w)}{_px(p)}",
        f"{_px(g)}{_px(w)}{_px(p)}{_px(g)}{_px(g)}{_px(p)}{_px(w)}{_px(g)}",
        f"{_px(g)}{_px(w)}{_px(g)}{_px(p)}{_px(p)}{_px(g)}{_px(w)}{_px(g)}",
        f"{_px(p)}{_px(g)}{_px(w)}{_px(w)}{_px(w)}{_px(w)}{_px(g)}{_px(p)}",
        f"  {_px(p)}{_px(g)}{_px(g)}{_px(p)}{_px(g)}{_px(p)}  ",
        f"    {_px(g)}{_px(p)}{_px(p)}{_px(g)}    ",
        f"  ♪ [bold #EAB308]Jazzy Jalisco[/] ♫  ",
    ]
    return rows


# ── Humble Hawksbill — turtle green/teal ──────────────────────────────────────
def art_humble() -> list[str]:
    g = "#16A34A"; t = "#0D9488"; s = "#86EFAC"
    rows = [
        f"    {_px(g)}{_px(g)}{_px(g)}{_px(g)}    ",
        f"  {_px(g)}{_px(t)}{_px(s)}{_px(s)}{_px(t)}{_px(g)}  ",
        f"{_px(g)}{_px(t)}{_px(s)}{_px(g)}{_px(g)}{_px(s)}{_px(t)}{_px(g)}",
        f"{_px(t)}{_px(s)}{_px(g)}{_px(s)}{_px(s)}{_px(g)}{_px(s)}{_px(t)}",
        f"{_px(t)}{_px(s)}{_px(g)}{_px(s)}{_px(s)}{_px(g)}{_px(s)}{_px(t)}",
        f"{_px(g)}{_px(t)}{_px(s)}{_px(g)}{_px(g)}{_px(s)}{_px(t)}{_px(g)}",
        f"  {_px(g)}{_px(t)}{_px(t)}{_px(t)}{_px(t)}{_px(g)}  ",
        f"    {_px(g)}{_px(g)}{_px(g)}{_px(g)}    ",
        f" 🐢 [bold #16A34A]Humble Hawksbill[/] 🐢 ",
    ]
    return rows


# ── Jazzy Jalisco was above; Iron Irwini — steel blue/gray ───────────────────
def art_iron() -> list[str]:
    s = "#64748B"; l = "#CBD5E1"; w = "#E2E8F0"
    rows = [
        f"  {_px(s)}{_px(s)}{_px(s)}{_px(s)}{_px(s)}{_px(s)}  ",
        f"  {_px(l)}{_px(w)}{_px(w)}{_px(w)}{_px(w)}{_px(l)}  ",
        f"{_px(s)}{_px(l)}{_px(w)}{_px(s)}{_px(s)}{_px(w)}{_px(l)}{_px(s)}",
        f"{_px(s)}{_px(w)}{_px(s)}{_px(w)}{_px(w)}{_px(s)}{_px(w)}{_px(s)}",
        f"{_px(s)}{_px(w)}{_px(s)}{_px(w)}{_px(w)}{_px(s)}{_px(w)}{_px(s)}",
        f"{_px(s)}{_px(l)}{_px(w)}{_px(s)}{_px(s)}{_px(w)}{_px(l)}{_px(s)}",
        f"  {_px(l)}{_px(w)}{_px(w)}{_px(w)}{_px(w)}{_px(l)}  ",
        f"  {_px(s)}{_px(s)}{_px(s)}{_px(s)}{_px(s)}{_px(s)}  ",
        f"  ⚙  [bold #94A3B8]Iron  Irwini[/]  ⚙  ",
    ]
    return rows


# ── Kilted Kaiju — Scottish tartan plaid (red/green/navy) ────────────────────
def art_kilted() -> list[str]:
    r = "#DC2626"; gr = "#15803D"; n = "#1E3A5F"; w = "#F9FAFB"
    rows = [
        f"{_px(r)}{_px(n)}{_px(r)}{_px(gr)}{_px(r)}{_px(n)}{_px(r)}{_px(gr)}",
        f"{_px(n)}{_px(w)}{_px(n)}{_px(w)}{_px(n)}{_px(w)}{_px(n)}{_px(w)}",
        f"{_px(r)}{_px(n)}{_px(r)}{_px(gr)}{_px(r)}{_px(n)}{_px(r)}{_px(gr)}",
        f"{_px(gr)}{_px(w)}{_px(gr)}{_px(w)}{_px(gr)}{_px(w)}{_px(gr)}{_px(w)}",
        f"{_px(r)}{_px(n)}{_px(r)}{_px(gr)}{_px(r)}{_px(n)}{_px(r)}{_px(gr)}",
        f"{_px(n)}{_px(w)}{_px(n)}{_px(w)}{_px(n)}{_px(w)}{_px(n)}{_px(w)}",
        f"{_px(r)}{_px(gr)}{_px(r)}{_px(n)}{_px(r)}{_px(gr)}{_px(r)}{_px(n)}",
        f"{_px(gr)}{_px(r)}{_px(n)}{_px(gr)}{_px(n)}{_px(r)}{_px(gr)}{_px(r)}",
        f" 🏴 [bold #DC2626]Kilted  Kaiju[/] 🏴 ",
    ]
    return rows


# ── Rolling Ridley — infinity blue gradient ────────────────────────────────
def art_rolling() -> list[str]:
    b1="#1D4ED8"; b2="#3B82F6"; b3="#93C5FD"; b4="#DBEAFE"
    rows = [
        f"  {_px(b1)}{_px(b2)}{_px(b3)}{_px(b3)}{_px(b2)}{_px(b1)}  ",
        f"{_px(b1)}{_px(b2)}{_px(b3)}{_px(b4)}{_px(b4)}{_px(b3)}{_px(b2)}{_px(b1)}",
        f"{_px(b2)}{_px(b3)}{_px(b4)}{_px(b3)}{_px(b3)}{_px(b4)}{_px(b3)}{_px(b2)}",
        f"{_px(b2)}{_px(b4)}{_px(b3)}{_px(b2)}{_px(b2)}{_px(b3)}{_px(b4)}{_px(b2)}",
        f"{_px(b2)}{_px(b3)}{_px(b4)}{_px(b3)}{_px(b3)}{_px(b4)}{_px(b3)}{_px(b2)}",
        f"{_px(b1)}{_px(b2)}{_px(b3)}{_px(b4)}{_px(b4)}{_px(b3)}{_px(b2)}{_px(b1)}",
        f"  {_px(b1)}{_px(b2)}{_px(b3)}{_px(b3)}{_px(b2)}{_px(b1)}  ",
        f"    {_px(b2)}{_px(b3)}{_px(b3)}{_px(b2)}    ",
        f" ◎▶ [bold #3B82F6]Rolling Ridley[/] ◎▶ ",
    ]
    return rows


# ── Foxy Fitzroy — orange fox ─────────────────────────────────────────────────
def art_foxy() -> list[str]:
    o="#F97316"; r="#DC2626"; w="#FFF7ED"
    rows = [
        f"{_px(o)}{_px(o)}{_px(r)}{_px(o)}{_px(o)}{_px(r)}{_px(o)}{_px(o)}",
        f"{_px(o)}{_px(r)}{_px(w)}{_px(r)}{_px(r)}{_px(w)}{_px(r)}{_px(o)}",
        f"  {_px(o)}{_px(w)}{_px(o)}{_px(o)}{_px(w)}{_px(o)}  ",
        f"  {_px(r)}{_px(o)}{_px(w)}{_px(w)}{_px(o)}{_px(r)}  ",
        f"    {_px(o)}{_px(w)}{_px(w)}{_px(o)}    ",
        f"  {_px(o)}{_px(r)}{_px(o)}{_px(o)}{_px(r)}{_px(o)}  ",
        f"{_px(o)}{_px(r)}{_px(o)}{_px(r)}{_px(r)}{_px(o)}{_px(r)}{_px(o)}",
        f"{_px(r)}{_px(o)}{_px(r)}{_px(o)}{_px(o)}{_px(r)}{_px(o)}{_px(r)}",
        f" 🦊 [bold #F97316]Foxy  Fitzroy[/] 🦊 ",
    ]
    return rows


# ── Galactic Geochelone — cosmic purple stars ─────────────────────────────────
def art_galactic() -> list[str]:
    p="#7C3AED"; s="#A78BFA"; w="#DDD6FE"; k="#1E1B4B"
    rows = [
        f"{_px(k)}{_px(p)}{_px(k)}{_px(s)}{_px(k)}{_px(w)}{_px(k)}{_px(p)}",
        f"{_px(p)}{_px(s)}{_px(w)}{_px(p)}{_px(s)}{_px(p)}{_px(w)}{_px(s)}",
        f"{_px(k)}{_px(w)}{_px(p)}{_px(s)}{_px(w)}{_px(s)}{_px(p)}{_px(k)}",
        f"{_px(s)}{_px(p)}{_px(s)}{_px(w)}{_px(p)}{_px(w)}{_px(s)}{_px(p)}",
        f"{_px(k)}{_px(w)}{_px(p)}{_px(s)}{_px(w)}{_px(s)}{_px(p)}{_px(k)}",
        f"{_px(p)}{_px(s)}{_px(w)}{_px(p)}{_px(s)}{_px(p)}{_px(w)}{_px(s)}",
        f"{_px(k)}{_px(p)}{_px(k)}{_px(s)}{_px(k)}{_px(w)}{_px(k)}{_px(p)}",
        f"{_px(p)}{_px(k)}{_px(s)}{_px(k)}{_px(p)}{_px(k)}{_px(s)}{_px(k)}",
        f" ✦ [bold #A78BFA]Galactic Geochelone[/] ✦",
    ]
    return rows


# ── Generic ROS2 logo ──────────────────────────────────────────────────────────
def art_generic() -> list[str]:
    c1="#22D3EE"; c2="#0891B2"; c3="#0E7490"
    rows = [
        f"  {_px(c1)}{_px(c1)}{_px(c1)}{_px(c1)}{_px(c1)}{_px(c1)}  ",
        f"{_px(c1)}{_px(c2)}{_px(c1)}{_px(c1)}{_px(c1)}{_px(c1)}{_px(c2)}{_px(c1)}",
        f"{_px(c2)}{_px(c1)}{_px(c3)}{_px(c2)}{_px(c2)}{_px(c3)}{_px(c1)}{_px(c2)}",
        f"{_px(c1)}{_px(c3)}{_px(c2)}{_px(c1)}{_px(c1)}{_px(c2)}{_px(c3)}{_px(c1)}",
        f"{_px(c2)}{_px(c1)}{_px(c3)}{_px(c2)}{_px(c2)}{_px(c3)}{_px(c1)}{_px(c2)}",
        f"{_px(c1)}{_px(c2)}{_px(c1)}{_px(c1)}{_px(c1)}{_px(c1)}{_px(c2)}{_px(c1)}",
        f"  {_px(c3)}{_px(c2)}{_px(c1)}{_px(c1)}{_px(c2)}{_px(c3)}  ",
        f"    {_px(c2)}{_px(c1)}{_px(c1)}{_px(c2)}    ",
        f"  ⊙ [bold #22D3EE]ROS2  System[/] ⊙  ",
    ]
    return rows


DISTRO_ART = {
    "jazzy":    art_jazzy,
    "humble":   art_humble,
    "iron":     art_iron,
    "kilted":   art_kilted,
    "rolling":  art_rolling,
    "foxy":     art_foxy,
    "galactic": art_galactic,
}


def get_distro_art(distro: str | None, theme: dict) -> Text:
    """Return a Rich Text colored pixel-art block for a given distro."""
    fn = DISTRO_ART.get((distro or "").lower(), art_generic)
    rows = fn()
    text = Text()
    for row in rows:
        text.append_text(Text.from_markup("  " + row + "\n"))
    return text


# ── Old ASCII logos (kept for compatibility) ──────────────────────────────────

LOGO_GENERIC = r"""
 ██████╗  ██████╗ ███████╗██████╗
 ██╔══██╗██╔═══██╗██╔════╝╚════██╗
 ██████╔╝██║   ██║███████╗   ██╔╝
 ██╔══██╗██║   ██║╚════██║╚██╗
 ██║  ██║╚██████╔╝███████║███████╗
 ╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚══════╝
          Robot Operating System 2
"""

def get_colored_logo(theme, distro=None):
    """Return a Rich Text logo (legacy compat)."""
    return get_distro_art(distro, theme)


MAIN_BANNER = r"""
            ╔══════════════════════════════════════════════════════════════════════════=═══=╗
            ║                                                                               ║
            ║    ██████╗  ██████╗ ███████╗    ██████╗     ██╗   ███╗   ██╗███████╗ ██████╗  ║
            ║    ██╔══██╗██╔═══██╗██╔════╝    ╚════██╗    ██╔╝  ████╗  ██║██╔════╝██╔═══██╗ ║
            ║    ██████╔╝██║   ██║███████╗     █████╔╝    ██╔╝  ██╔██╗ ██║█████╗  ██║   ██║ ║
            ║    ██╔══██╗██║   ██║╚════██║    ██╔═══╝     ██╔╝  ██║╚██╗██║██╔══╝  ██║   ██║ ║
            ║    ██║  ██║╚██████╔╝███████║    ███████╗    ██╔╝  ██║ ╚████║██║     ╚██████╔╝ ║
            ║    ╚═╝  ╚═╝ ╚═════╝ ╚══════╝    ╚══════╝    ╚═╝    ╚═╝  ╚═══╝╚═╝      ╚═════╝ ║
            ║        ~"Created by roboticists, for roboticists."                            ║
            ║           ⬡  The fastfetch you always wanted — for ROS2  ⬡                   ║
            ╚═══════════════════════════════════════════════════════════════════════════=══=╝
"""

# Medium banner — fits ~46+ column terminals
MEDIUM_BANNER = r"""
  ╔════════════════════════════════════════════╗
  ║  ██████╗ ███████╗███████╗ ██████╗██╗  ██╗  ║
  ║  ██╔══██╗██╔════╝██╔════╝██╔════╝██║  ██║  ║
  ║  ██████╔╝█████╗  ███████╗██║     ███████║  ║
  ║  ██╔═══╝ ██╔══╝  ╚════██║██║     ██╔══██║  ║
  ║  ██║     ███████╗███████║╚██████╗██║  ██║  ║
  ║  ╚═╝     ╚══════╝╚══════╝ ╚═════╝╚═╝  ╚═╝  ║
  ║   ⬡ fastfetch for ROS2 — by roboticists    ║
  ╚════════════════════════════════════════════╝
"""


def _colorize_banner(raw: str, theme: dict) -> Text:
    """Apply theme color to a raw multi-line banner string."""
    text = Text()
    color = theme.get("logo_color1", "#22D3EE")
    for line in raw.strip("\n").split("\n"):
        text.append(line.rstrip() + "\n", style=f"bold {color}")
    return text


def get_main_banner(theme, width: int | None = None):
    """Return a banner whose size adapts to the terminal width.

    Tiers:
      * width >= 90  -> full ASCII art banner
      * width >= 50  -> medium compact banner
      * width <  50  -> single-line compact title
    """
    import shutil as _sh
    if width is None:
        width = _sh.get_terminal_size((80, 24)).columns

    if width >= 90:
        return _colorize_banner(MAIN_BANNER, theme)
    if width >= 50:
        return _colorize_banner(MEDIUM_BANNER, theme)
    # Compact one-liner for very narrow terminals
    color = theme.get("logo_color1", "#22D3EE")
    return Text("  ≈ ROS2 Info — fastfetch for ROS2 ≈\n", style=f"bold {color}")
