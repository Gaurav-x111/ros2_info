# Scripts & Utilities

Development and utility scripts for the ros2_info project.

## generate_climage_assets.py

Regenerates the distro poster artwork for terminal display.

### When to use
- When distro posters change on Wikipedia
- When updating the color palette or resolution of terminal art
- One-time setup only (not needed for normal operation)

### How to run

```bash
# From project root:
python scripts/generate_climage_assets.py

# Dependencies:
# - climage: pip install climage
# - PIL/Pillow: pip install Pillow
```

### What it does
1. Downloads ROS2 distro posters from Wikimedia Commons
2. Converts images to ANSI Unicode art (40 chars wide)
3. Generates `src/ros2_fastfetch/fetch_info/display/distro_art.py`
4. Output file contains colored Unicode block characters for terminal rendering

**Note:** The output file is pre-generated; this script only needs to run when updating assets.

---

## Main Program: ros2_info

### Quick Start

```bash
# From project root:
python -m src.ros2_fastfetch.fetch_info.cli --help
```

### Common Commands

```bash
# Display system info with ROS2 details
ros2-info

# Show without logo
ros2-info --no-logo

# Force ASCII rendering (no colors)
ros2-info --ascii

# Use specific theme
ros2-info --theme matrix

# JSON output
ros2-info --info --json

# Interactive dashboard
ros2-info --shell

# Web dashboard
ros2-info --web
```

### Installation

```bash
# Install in development mode
pip install -e .

# Or directly
pip install .
```

### Python Usage

```python
from src.ros2_fastfetch.fetch_info.cli import collect
from src.ros2_fastfetch.fetch_info.display.fastfetch import render_fastfetch
from src.ros2_fastfetch.fetch_info.display.themes import get_theme
from rich.console import Console

console = Console()
data = collect()
theme = get_theme("default")
render_fastfetch(console, data, theme)
```
