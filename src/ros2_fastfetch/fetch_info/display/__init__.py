# display sub-package
"""
Display module — rendering pipeline for ros2_info terminal output.

Public API
----------
render_logo(distro, theme, width_cols=None)
    Return a Rich-renderable for the distro logo (bitmap or ANSI).

detect_terminal()
    Return the terminal type: 'kitty', 'iterm', 'wezterm', 'vscode', or 'unknown'.
"""


def render_logo(distro: str, theme: dict, width_cols: int | None = None):
    """Return a Rich-compatible renderable for the given ROS2 distro logo.

    Delegates to :func:`fastfetch.get_fastfetch_art` which walks the
    full bitmap → chafa → ANSI fallback chain.
    """
    from fetch_info.display.fastfetch import get_fastfetch_art

    return get_fastfetch_art(distro, theme, width_cols=width_cols)


def detect_terminal() -> str:
    """Detect the current terminal emulator.

    Returns one of: ``'kitty'``, ``'iterm'``, ``'wezterm'``,
    ``'vscode'``, or ``'unknown'``.
    """
    from fetch_info.display.graphics import GraphicsEngine

    return GraphicsEngine._detect_terminal()