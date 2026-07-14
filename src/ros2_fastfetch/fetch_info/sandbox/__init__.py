"""
Sandbox Execution Module
Provides isolated ROS 2 node execution with namespace isolation and export pipeline.
"""

import os
import shutil
import subprocess
import tempfile
from typing import Optional, List, Dict
from dataclasses import dataclass


@dataclass
class SandboxConfig:
    """Configuration for a sandboxed ROS 2 environment."""
    namespace: str = "/sandbox"
    isolated_tmp: bool = True
    domain_id: Optional[str] = None
    env_overrides: Optional[Dict[str, str]] = None
    # ponytail: simple fields, no factory needed yet


class SandboxExecutor:
    """
    Execute ROS 2 nodes in an isolated environment.

    Isolation features:
    - Custom ROS_NAMESPACE to prevent topic collisions
    - Optional isolated /tmp directory
    - Optional custom ROS_DOMAIN_ID
    - Environment variable sandboxing
    """

    def __init__(self, config: Optional[SandboxConfig] = None):
        self.config = config or SandboxConfig()
        self._tmp_dir: Optional[str] = None
        self._processes: List[subprocess.Popen] = []

        if self.config.isolated_tmp:
            self._tmp_dir = tempfile.mkdtemp(prefix="ros2_sandbox_")

    def __del__(self):
        """Cleanup on destruction."""
        self.cleanup()

    def _build_env(self) -> Dict[str, str]:
        """Build isolated environment variables."""
        env = dict(os.environ)  # ponytail: copy, not reference

        # Apply sandbox isolation
        env['ROS_NAMESPACE'] = self.config.namespace

        if self.config.domain_id:
            env['ROS_DOMAIN_ID'] = self.config.domain_id

        if self._tmp_dir:
            env['TMPDIR'] = self._tmp_dir
            env['TEMP'] = self._tmp_dir
            env['TMP'] = self._tmp_dir

        # Apply custom overrides
        if self.config.env_overrides:
            env.update(self.config.env_overrides)

        return env

    def run_node(self, package: str, executable: str,
                 args: Optional[List[str]] = None,
                 background: bool = True) -> subprocess.Popen:
        """
        Run a ROS 2 node in the sandbox.

        Args:
            package: ROS 2 package name
            executable: Node executable name
            args: Additional command-line arguments
            background: If True, run in background (non-blocking)

        Returns:
            Popen handle to the node process
        """
        cmd = ["ros2", "run", package, executable] + (args or [])
        env = self._build_env()

        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            env=env,
        )

        if not background:
            # Wait for completion
            proc.wait()
        else:
            self._processes.append(proc)

        return proc

    def run_launch(self, package: str, launch_file: str,
                   launch_args: Optional[Dict[str, str]] = None,
                   background: bool = True) -> subprocess.Popen:
        """
        Run a ROS 2 launch file in the sandbox.

        Args:
            package: ROS 2 package name
            launch_file: Launch file name (e.g., "demo.launch.py")
            launch_args: Key-value arguments for the launch file
            background: If True, run in background

        Returns:
            Popen handle to the launch process
        """
        cmd = ["ros2", "launch", package, launch_file]
        if launch_args:
            for key, val in launch_args.items():
                cmd.append(f"{key}:={val}")

        env = self._build_env()
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            env=env,
        )

        if not background:
            proc.wait()
        else:
            self._processes.append(proc)

        return proc

    def stop_all(self):
        """Stop all running sandbox processes."""
        for proc in self._processes:
            try:
                proc.terminate()
                proc.wait(timeout=5)
            except Exception:
                proc.kill()
        self._processes.clear()

    def cleanup(self):
        """Stop processes and cleanup temporary directories."""
        self.stop_all()

        if self._tmp_dir and os.path.exists(self._tmp_dir):
            try:
                shutil.rmtree(self._tmp_dir)
            except Exception:
                pass
        self._tmp_dir = None

    def get_output(self, proc: subprocess.Popen, timeout: float = 1.0) -> str:
        """Get output from a running process (non-blocking)."""
        try:
            return proc.stdout.readline() if proc.stdout else ""
        except Exception:
            return ""


def export_to_global(sandbox_config: SandboxConfig,
                     global_config_path: str) -> bool:
    """
    Export a sandbox configuration to global environment.

    This is the "promote to production" step - takes a tested
    sandbox configuration and applies it to the global ROS 2 network.

    Args:
        sandbox_config: The sandbox configuration to export
        global_config_path: Path to write the global config

    Returns:
        True if export succeeded
    """
    import json

    # Strip sandbox-specific settings
    global_config = {
        'domain_id': sandbox_config.domain_id,
        'env_overrides': sandbox_config.env_overrides,
        # Note: namespace is NOT exported - global runs on root namespace
    }

    try:
        with open(global_config_path, 'w') as f:
            json.dump(global_config, f, indent=2)
        return True
    except Exception:
        return False


def create_sandbox(namespace: str = "/sandbox",
                   isolated: bool = True) -> SandboxExecutor:
    """
    Factory function to create a sandbox executor.

    Args:
        namespace: ROS namespace for isolation
        isolated: Whether to use isolated /tmp

    Returns:
        Configured SandboxExecutor
    """
    return SandboxExecutor(SandboxConfig(
        namespace=namespace,
        isolated_tmp=isolated,
    ))