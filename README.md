# ROS2 Info ⊙
        ╔══════════════════════════════════════════════════════════════════════===════╗
        ║                                                                             ║
        ║    ██████╗  ██████╗ ███████╗    ██████╗     ██╗   ███╗   ██╗███████╗ ██████╗║
        ║    ██╔══██╗██╔═══██╗██╔════╝    ╚════██╗    ██╔╝  ████╗  ██║██╔════╝██╔═══██╗
        ║    ██████╔╝██║   ██║███████╗     █████╔╝    ██╔╝  ██╔██╗ ██║█████╗  ██║   ██║
        ║    ██╔══██╗██║   ██║╚════██║    ██╔═══╝     ██╔╝  ██║╚██╗██║██╔══╝  ██║   ██║
        ║    ██║  ██║╚██████╔╝███████║    ███████╗    ██╔╝  ██║ ╚████║██║     ╚██████╔╝
        ║    ╚═╝  ╚═╝ ╚═════╝ ╚══════╝    ╚══════╝    ╚═╝    ╚═╝  ╚═══╝╚═╝      ╚═════╝ 
        ║            ~"Created by roboticists, for roboticists   ║                    ║
        ║           ⬡  The fastfetch you always wanted — for ROS2  ⬡                 ║
        ╚═════════════════════════════════════════════════════════════════════===═════╝

        
                            A beautiful, interactive Fastfetch-like system information tool and web dashboard built specifically for **ROS2 developers**
A fastfetch-style ROS2 workstation lens: **know what, where, and which is working**.


![ROS2 Info demo](asset/howitwork.gif)

<div align="center">

[![ROS2](https://img.shields.io/badge/ROS2-Jazzy%20%7C%20Humble%20%7C%20Iron%20%7C%20Rolling-22D3EE?style=for-the-badge&logo=ros&logoColor=white)](https://docs.ros.org)
[![Python](https://img.shields.io/badge/Python-3.10%2B-3B82F6?style=for-the-badge&logo=python&logoColor=white)](https://python.org)
[![License](https://img.shields.io/badge/License-MIT-10B981?style=for-the-badge)](LICENSE)
[![colcon](https://img.shields.io/badge/Built%20with-colcon-F97316?style=for-the-badge)](https://colcon.readthedocs.io)

*Born from curiosity. Built for roboticists.*

</div>

## Why this was made
**Short answer:** this was made to know **what**, **where**, and **which** is working.

**Fun answer:** I wanted to know how a fastfetch for ROS2 would look, kept working on it, and turned it into a true ROS2 workspace tool.<!--  -->

**Joke:** If `ros2 node list` is empty, did the robots take a coffee break or did we forget to source again? ☕🤖

## Features
```

╔═══════════════════════════════════════════════════════════════════════════╗
║               ROS2 Info — Interactive Terminal  v2.0                      ║
╚═══════════════════════════════════════════════════════════════════════════╝
        DISCOVERY
                info                System and ROS2 fastfetch overview
                nodes               List active nodes
                topics              List active topics
                services            List active services
                actions             List active actions
                env                 Show ROS2 env variables
                node info <name>    Show pub/sub/service info
                interface show <t>  Show message/srv definition

        MONITORING
                echo <topic> [--once]   Stream topic messages
                hz <topic>              Publish rate
                bw <topic>              Bandwidth
                watch [interval]        Live node refresh (2s)
                ping <node>             Liveness check
                graph [timeout]         ASCII pub→sub graph
                rqt                     Launch rqt_graph GUI (bg)

        ACTIONS
                pub <topic> <type> <yaml> [--once]
                service call <srv> <type> [<yaml>]
                param list [/node]
                param get <node> <param>
                param set <node> <param> <value>
                bag record [-a | topics...]
                bag play <file>
                bag info <file>
                launch <pkg> <file> [key:=val ...]
                run <pkg> <exe> [args...]
                colcon build [--packages-select pkg]
                colcon test

        TMUX
                tmux new [name]     New session
                tmux list           List sessions
                tmux attach [name]  Attach session
                tmux kill <name>    Kill session
                tmux split          Horizontal split
                tmux vsplit         Vertical split
                web [port]          Launch web dashboard (bg)

        SYSTEM
                cd <path>           Change directory
                ls [path]           List files
                pwd                 Print working directory
                shell <cmd>         Run any shell command
                source <file>       Source bash file → env
                history             Show command history
                help                Show this help
                clear               Clear screen
                quit / exit / q     Exit terminal
```

## Importance
- Stops the “where am I sourced?” guessing game.
- Shows live ROS2 health (nodes/topics/services/actions) in one view.
- Gives a fast, clean picture of workspace + system state during bring-up.

## How it works (visuals from assets)
All images and videos live in the asset folder and are used directly in this README.

![Startup](asset/startup.png)

![Info view](asset/info.png)

<<<<<<< HEAD
Interactive TUI: A built-in terminal user interface powered by click and rich to selectively view different system panels.

🌐 Web Dashboard (Inspired by ros.org)
Elegant, Light-Themed Web UI: A modern dashboard that live-updates your system and ROS2 stats.

Community Integration: Seamlessly embeds the official ROS2 community blog for news and updates.
## ⬡ Screenshots

<div align="center">

<!-- Screenshot 1 — main dashboard -->
<img src="https://github.com/zang7777/ros2_info/blob/main/Screenshot%20from%202026-03-11%2011-41-01.png" width="800"/>

<br/>

<!-- Screenshot 2 — live nodes/topics -->
<img src="https://github.com/zang7777/ros2_info/blob/main/Screenshot%20from%202026-03-11%2011-41-29.png" width="800"/>

<br/>

<!-- Screenshot 3 — theme or web dashboard -->
<img src="https://github.com/zang7777/ros2_info/blob/main/Screenshot%20from%202026-03-11%2011-41-57.png" width="800"/>

</div>



### 🚀 How to Use
=======
## Install (all packages)
```bash
# Create and activate a venv (optional but recommended)
python -m venv .venv
source .venv/bin/activate
source /opt/ros/jazzy/setup.zsh     
# Install the project
pip install -e src/ros2_fastfetch
```
>>>>>>> d272d78 (v2.0.0)

## Run
*(Ensure you have sourced your ROS2 environment first: `source /opt/ros/jazzy/setup.bash`)*

```bash
ros2_info --help
ros2_info
ros2_info terminal
ros2_info web
```

<<<<<<< HEAD
### 2. Live Watch Mode
Keep the terminal open and automatically refresh the runtime stats (nodes/topics) every 2 seconds:
```bash
ros2 run ros2_info ros2_info --watch 2
```

### 3. Interactive Menu(BEST RECOMMENDED)
Don't want to type flags? Open the interactive terminal UI to browse your system:
```bash
ros2 run ros2_info ros2_info --interactive
```

### 4. 🌐 Web Dashboard
Launch a rich, interactive web UI that you can view in your browser (defaults to `http://localhost:8099`):
```bash
ros2 run ros2_info ros2_info web
```

### 5. Other Commands
```bash
ros2 run ros2_info ros2_info nodes     # List all active nodes
ros2 run ros2_info ros2_info topics    # List all active topics
ros2 run ros2_info ros2_info packages  # List installed ROS2 packages
ros2 run ros2_info ros2_info env       # Show ROS2 environment variables
```

*Built with Python, Click, Rich, and Flask. Designed for roboticists.*

---
# ros2_info
=======
## Future updates (coming soon)
- More themes and layout presets
- Expanded web dashboard telemetry
- Additional workspace and build insights
>>>>>>> d272d78 (v2.0.0)
