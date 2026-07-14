"""Shared rendering utilities for CLI and terminal output."""

from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from rich.text import Text
from rich import box


def create_table(title: str = None, border_style: str = "cyan", show_lines: bool = False, **kwargs):
    """Create a styled table with consistent defaults."""
    return Table(
        title=title,
        border_style=border_style,
        show_lines=show_lines,
        box=box.MINIMAL_HEAVY_HEAD if not kwargs.get("box") else kwargs.pop("box"),
        **kwargs
    )


def create_panel(content, title: str = None, border_style: str = "cyan", **kwargs):
    """Create a styled panel with consistent defaults."""
    try:
        return Panel(
            content,
            title=title,
            border_style=border_style,
            box=box.ROUNDED,
            **kwargs
        )
    except Exception:
        # Fallback for terminals that don't support certain box styles
        return Panel(content, title=title, border_style=border_style, box=box.SQUARE, **kwargs)


def render_section_title(console: Console, text: str, style: str = "section_title"):
    """Render a section title with consistent styling."""
    theme_key = style if style else "section_title"
    console.print(f"  [{theme_key}]{text}[/]")


def print_status(console: Console, message: str, style: str = "dim_style"):
    """Print a status message with consistent styling."""
    console.print(f"  [{style}]{message}[/]")


def print_error(console: Console, message: str, style: str = "error_style"):
    """Print an error message with consistent styling."""
    console.print(f"  [{style}]{message}[/]")


def print_success(console: Console, message: str, style: str = "ok_style"):
    """Print a success message with consistent styling."""
    console.print(f"  [{style}]{message}[/]")