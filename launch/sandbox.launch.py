"""
ROS2 launch file for sandbox execution.

This launch file runs nodes in an isolated namespace to prevent
interference with production robots on the global ROS 2 network.
"""

from launch import LaunchDescription
from launch.actions import DeclareLaunchArgument, GroupAction
from launch.substitutions import LaunchConfiguration
from launch_ros.actions import Node


def generate_launch_description():
    """Generate the launch description for sandbox execution."""

    namespace_arg = DeclareLaunchArgument(
        'namespace',
        default_value='/sandbox',
        description='Namespace for sandboxed nodes'
    )

    domain_id_arg = DeclareLaunchArgument(
        'domain_id',
        default_value='42',
        description='ROS_DOMAIN_ID for sandbox isolation'
    )

    # Group all sandboxed nodes under the namespace
    # ponytail: single GroupAction, no factory needed for this demo
    sandbox_group = GroupAction(
        namespace=LaunchConfiguration('namespace'),
        actions=[
            # Add your sandbox nodes here
            # Node(
            #     package='demo_nodes_cpp',
            #     executable='talker',
            #     name='sandbox_talker',
            # ),
        ]
    )

    return LaunchDescription([
        namespace_arg,
        domain_id_arg,
        sandbox_group,
    ])