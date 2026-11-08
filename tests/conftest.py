import os
import sys

# Lets `python -m pytest tests/` find fetch_info without `pip install -e`.
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src", "ros2_fastfetch"))
