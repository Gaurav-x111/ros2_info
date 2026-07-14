"""
ROS2 launch file for the demo nodes.

Used for testing the sandbox execution and ROS2 telemetry features.
"""

from launch import LaunchDescription
from launch_ros.actions import Node


def generate_launch_description():
    """Generate the launch description for demo nodes."""
    return LaunchDescription([
        Node(
            package='demo_nodes_cpp',
            executable='talker',
            name='demo_talker',
            output='screen',
        ),
        Node(
            package='demo_nodes_cpp',
            executable='listener',
            name='demo_listener',
            output='screen',
        ),
    ])