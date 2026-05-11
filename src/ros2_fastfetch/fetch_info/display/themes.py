THEMES = {
    "default": {
        "logo_color1": "#22D3EE", "logo_color2": "#06B6D4",
        "logo_color3": "#0891B2", "logo_color4": "#0E7490",
        "logo_color5": "#155E75", "logo_color6": "#164E63",
        "subtitle_color": "#94A3B8", "panel_border": "#0E7490",
        "section_title": "bold cyan", "key_style": "bold #22D3EE",
        "value_style": "#E2E8F0", "ok_style": "bold green",
        "warn_style": "bold yellow", "error_style": "bold red",
        "dim_style": "#64748B", "highlight": "#F97316",
    },
    "matrix": {
        "logo_color1": "#22C55E", "logo_color2": "#16A34A",
        "logo_color3": "#15803D", "logo_color4": "#166534",
        "logo_color5": "#14532D", "logo_color6": "#052E16",
        "subtitle_color": "#4ADE80", "panel_border": "#16A34A",
        "section_title": "bold green", "key_style": "bold #22C55E",
        "value_style": "#BBF7D0", "ok_style": "bold bright_green",
        "warn_style": "bold yellow", "error_style": "bold red",
        "dim_style": "#166534", "highlight": "#86EFAC",
    },
    "ros": {
        "logo_color1": "#EF4444", "logo_color2": "#DC2626",
        "logo_color3": "#B91C1C", "logo_color4": "#991B1B",
        "logo_color5": "#7F1D1D", "logo_color6": "#6B0808",
        "subtitle_color": "#94A3B8", "panel_border": "#DC2626",
        "section_title": "bold red", "key_style": "bold #EF4444",
        "value_style": "#E2E8F0", "ok_style": "bold green",
        "warn_style": "bold yellow", "error_style": "bold red",
        "dim_style": "#64748B", "highlight": "#F97316",
    },
    "ocean": {
        "logo_color1": "#38BDF8", "logo_color2": "#0EA5E9",
        "logo_color3": "#0284C7", "logo_color4": "#0369A1",
        "logo_color5": "#075985", "logo_color6": "#0C4A6E",
        "subtitle_color": "#7DD3FC", "panel_border": "#0284C7",
        "section_title": "bold blue", "key_style": "bold #38BDF8",
        "value_style": "#E0F2FE", "ok_style": "bold green",
        "warn_style": "bold yellow", "error_style": "bold red",
        "dim_style": "#0369A1", "highlight": "#FDE047",
    },
    "dark": {
        "logo_color1": "#A78BFA", "logo_color2": "#8B5CF6",
        "logo_color3": "#7C3AED", "logo_color4": "#6D28D9",
        "logo_color5": "#5B21B6", "logo_color6": "#4C1D95",
        "subtitle_color": "#94A3B8", "panel_border": "#6D28D9",
        "section_title": "bold magenta", "key_style": "bold #A78BFA",
        "value_style": "#E2E8F0", "ok_style": "bold green",
        "warn_style": "bold yellow", "error_style": "bold red",
        "dim_style": "#64748B", "highlight": "#F59E0B",
    },
    "neon": {
        # Hot pink / electric lime neon-cyberpunk palette
        "logo_color1": "#F0ABFC", "logo_color2": "#E879F9",
        "logo_color3": "#D946EF", "logo_color4": "#A21CAF",
        "logo_color5": "#86198F", "logo_color6": "#4A044E",
        "subtitle_color": "#C026D3", "panel_border": "#E879F9",
        "section_title": "bold magenta", "key_style": "bold #F0ABFC",
        "value_style": "#FAE8FF", "ok_style": "bold #A3E635",
        "warn_style": "bold #FDE047", "error_style": "bold #F87171",
        "dim_style": "#9D4EDD", "highlight": "#A3E635",
    },
    "solar": {
        # Warm amber/orange/gold solar flare palette
        "logo_color1": "#FCD34D", "logo_color2": "#F59E0B",
        "logo_color3": "#D97706", "logo_color4": "#B45309",
        "logo_color5": "#92400E", "logo_color6": "#78350F",
        "subtitle_color": "#FDE68A", "panel_border": "#D97706",
        "section_title": "bold yellow", "key_style": "bold #FCD34D",
        "value_style": "#FEF3C7", "ok_style": "bold #34D399",
        "warn_style": "bold #FB923C", "error_style": "bold #F87171",
        "dim_style": "#92400E", "highlight": "#FB923C",
    },
}

def get_theme(name: str) -> dict:
    """Get a theme dictionary by name.
    
    Args:
        name: Theme name (default, matrix, ros, ocean, dark, neon, solar)
        
    Returns:
        Dictionary with theme color and style definitions
    """
    return THEMES.get(name, THEMES["default"])


def list_themes() -> list:
    """List all available theme names.
    
    Returns:
        List of theme name strings
    """
    return list(THEMES.keys())
