#!/usr/bin/env python3
"""Unit and integration tests for omniroute_manager and seeding modules.

Strict adherence to zero emojis and strict error handling.
"""

import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

# Add workspace and benchmarks directories to path
repo_root = pathlib.Path(__file__).resolve().parent.parent
benchmarks_dir = repo_root / "benchmarks"
if str(benchmarks_dir) not in sys.path:
    sys.path.insert(0, str(benchmarks_dir))

from omniroute_manager import OmniRouteManager
from seeding import (
    create_sandbox_dirs,
    find_brain_executable,
    validate_zero_byte_files,
)


class TestOmniRouteManager(unittest.TestCase):
    """Test lifecycle management of OmniRoute server."""

    def test_lifecycle_and_started_by_us(self):
        manager = OmniRouteManager()

        # Ensure server is stopped initially
        if manager.is_listening():
            manager.stop(force=True)
        self.assertFalse(manager.is_listening())

        # Test start
        self.assertTrue(manager.start(timeout=15.0))
        self.assertTrue(manager.is_listening())
        self.assertTrue(manager.started_by_us)

        # Test second manager does not claim ownership
        second_manager = OmniRouteManager()
        self.assertTrue(second_manager.start())
        self.assertFalse(second_manager.started_by_us)

        # Second manager stop should not kill server
        stopped = second_manager.stop()
        self.assertFalse(stopped)
        self.assertTrue(manager.is_listening())

        # First manager stop must terminate server
        stopped = manager.stop()
        self.assertTrue(stopped)
        self.assertFalse(manager.is_listening())

    def test_cli_check_entrypoint(self):
        mgr = OmniRouteManager()
        if mgr.is_listening():
            mgr.stop(force=True)

        # When stopped: exit code 1
        res_stopped = subprocess.run(
            [sys.executable, str(benchmarks_dir / "omniroute_manager.py"), "--check"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(res_stopped.returncode, 1)
        self.assertIn("[FAIL] OmniRoute is not listening", res_stopped.stdout)

        # When running: exit code 0
        mgr.start()
        try:
            res_running = subprocess.run(
                [sys.executable, str(benchmarks_dir / "omniroute_manager.py"), "--check"],
                capture_output=True,
                text=True,
            )
            self.assertEqual(res_running.returncode, 0)
            self.assertIn("[OK] OmniRoute is listening", res_running.stdout)
        finally:
            mgr.stop()


class TestSeedingModule(unittest.TestCase):
    """Test sandbox creation and file validation functions."""

    def test_create_sandbox_dirs(self):
        with tempfile.TemporaryDirectory() as tmp:
            alvo, ruido = create_sandbox_dirs(tmp)
            self.assertTrue(alvo.is_dir())
            self.assertTrue(ruido.is_dir())
            self.assertEqual(alvo.name, "teste-ws-alvo")
            self.assertEqual(ruido.name, "teste-ws-ruido")

    def test_find_brain_executable(self):
        exe = find_brain_executable()
        self.assertTrue(pathlib.Path(exe).is_file())

    def test_validate_zero_byte_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            f1 = tmp_path / "teste-plan-1.md"
            f2 = tmp_path / "teste-plan-2.md"

            f1.touch()
            f2.write_bytes(b"non-empty dummy data")

            # With enforce_zero_byte=False, non-empty raises ValueError
            with self.assertRaises(ValueError):
                validate_zero_byte_files([f1, f2], enforce_zero_byte=False)

            # With enforce_zero_byte=True, non-empty is truncated to 0
            results = validate_zero_byte_files([f1, f2], enforce_zero_byte=True)
            self.assertEqual(results[str(f1.resolve())], 0)
            self.assertEqual(results[str(f2.resolve())], 0)
            self.assertEqual(f2.stat().st_size, 0)

            # Missing file raises FileNotFoundError
            missing = tmp_path / "does_not_exist.md"
            with self.assertRaises(FileNotFoundError):
                validate_zero_byte_files([missing])


if __name__ == "__main__":
    unittest.main()
