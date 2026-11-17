#!/usr/bin/env python3
"""Deterministic verification test suite for benchmarks/teardown.py.

Tests snapshotting, deletion of test sessions via brain delete, temporary plan cleanup,
sandbox directory removal, OmniRoute manager stoppage, count parity audit,
and CLI invocation.
Adheres strictly to zero emojis.
"""

from __future__ import annotations

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import unittest
import uuid
from pathlib import Path

from benchmarks.teardown import (
    clean_temporary_plans,
    cleanup_all_test_sessions,
    delete_sandbox_dirs,
    find_brain_executable,
    get_antigravity_db_path,
    parse_brain_list_sessions,
    snapshot_sessions,
    stop_omniroute_mgr,
    strip_ansi,
)


class MockOmniRouteManager:
    """Mock manager verifying stop() invocation."""

    def __init__(self) -> None:
        self.stopped = False

    def stop(self) -> None:
        self.stopped = True


class TestTeardownModule(unittest.TestCase):
    """Unit and integration test cases for teardown module."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.brain_exe = find_brain_executable()

    def test_01_find_brain_executable(self) -> None:
        exe = find_brain_executable()
        self.assertTrue(Path(exe).is_file())
        self.assertTrue("brain" in Path(exe).name.lower())

    def test_02_strip_ansi(self) -> None:
        raw = "\x1b[96m\x1b[1m| 0 | [FIXADO] | Title |\x1b[0m"
        clean = strip_ansi(raw)
        self.assertEqual(clean, "| 0 | [FIXADO] | Title |")

    def test_03_parse_brain_list_sessions(self) -> None:
        sample_output = """
+-----+----------+------------------------------------------+--------------+------------+------------+
| IDX | STATUS   | TITULO DA SESSAO                         | PASSOS       | DATA       | ID         |
+-----+----------+------------------------------------------+--------------+------------+------------+
| 0   | [FIXADO] | Main Feature Implementation              | 100 p.       | 2026-11-16 | 12345678...|
| 1   | [SUB]    | Subagent Worker Session                  | 25 p.        | 2026-11-16 | abcdef01...|
+-----+----------+------------------------------------------+--------------+------------+------------+
"""
        sessions = parse_brain_list_sessions(sample_output)
        self.assertEqual(len(sessions), 2)
        self.assertEqual(sessions[0]["idx"], 0)
        self.assertEqual(sessions[0]["title"], "Main Feature Implementation")
        self.assertEqual(sessions[0]["short_id"], "12345678")
        self.assertEqual(sessions[1]["idx"], 1)
        self.assertEqual(sessions[1]["title"], "Subagent Worker Session")
        self.assertEqual(sessions[1]["short_id"], "abcdef01")

    def test_04_snapshot_sessions_structure(self) -> None:
        snapshot = snapshot_sessions(self.brain_exe)
        self.assertIn("antigravity", snapshot)
        self.assertIn("claude-code", snapshot)
        self.assertIsInstance(snapshot["antigravity"], set)
        self.assertIsInstance(snapshot["claude-code"], set)
        self.assertGreater(len(snapshot["antigravity"]), 0)
        self.assertGreater(len(snapshot["claude-code"]), 0)
        # All members must be strings
        for sid in snapshot["antigravity"]:
            self.assertIsInstance(sid, str)
        for sid in snapshot["claude-code"]:
            self.assertIsInstance(sid, str)

    def test_05_clean_temporary_plans(self) -> None:
        plans_dir = Path.home() / ".claude" / "plans"
        plans_dir.mkdir(parents=True, exist_ok=True)
        test_plan = plans_dir / "teste-benchmark-plan-mock.md"
        keep_plan = plans_dir / "keep-this-plan-intact.md"
        test_plan.write_text("# Test Plan", encoding="utf-8")
        keep_plan.write_text("# Keep Plan", encoding="utf-8")

        try:
            deleted = clean_temporary_plans()
            self.assertTrue(any(p.name == "teste-benchmark-plan-mock.md" for p in deleted))
            self.assertFalse(test_plan.exists())
            self.assertTrue(keep_plan.exists())
        finally:
            if test_plan.exists():
                test_plan.unlink(missing_ok=True)
            if keep_plan.exists():
                keep_plan.unlink(missing_ok=True)

    def test_06_delete_sandbox_dirs(self) -> None:
        temp_root = Path(tempfile.mkdtemp(prefix="benchmark_sandbox_test_"))
        sub_dir = temp_root / "teste-ws-sub"
        sub_dir.mkdir(parents=True)
        file_path = sub_dir / "dummy.txt"
        file_path.write_text("sandbox file", encoding="utf-8")

        deleted = delete_sandbox_dirs([temp_root])
        self.assertEqual(len(deleted), 1)
        self.assertFalse(temp_root.exists())

    def test_07_stop_omniroute_mgr(self) -> None:
        mgr = MockOmniRouteManager()
        self.assertFalse(mgr.stopped)
        result = stop_omniroute_mgr(mgr)
        self.assertTrue(result)
        self.assertTrue(mgr.stopped)

        # None should be handled gracefully
        self.assertFalse(stop_omniroute_mgr(None))

    def test_08_cleanup_all_test_sessions_full_lifecycle(self) -> None:
        # 1. Capture snapshot before creating test fixture
        pre_snap = snapshot_sessions(self.brain_exe)
        initial_agy_count = len(pre_snap["antigravity"])

        # 2. Inject a synthetic test session into Antigravity
        db_path = get_antigravity_db_path()
        self.assertIsNotNone(db_path)
        test_uuid = str(uuid.uuid4())
        test_title = "teste-synthetic-benchmark-session"
        test_ws = "file:///c:/users/cocad/appdata/local/temp/teste-ws-fixture"

        # Create dummy brain directory for this session so brain list does not treat it as ghost
        brain_root = db_path.parent / "brain"
        test_brain_dir = brain_root / test_uuid
        test_brain_dir.mkdir(parents=True, exist_ok=True)
        (test_brain_dir / "implementation_plan.md").write_text("# Plan", encoding="utf-8")

        conn = sqlite3.connect(db_path)
        conn.execute(
            """INSERT INTO conversation_summaries
               (conversation_id, title, preview, step_count, last_modified_time, workspace_uris,
                status, source, project_id, agent_name, parent_conversation_id, nesting_depth,
                battle_id, winning_conversation_id, not_fully_idle, killed, last_user_input_time,
                last_user_input_step_index, app_data_dir, raw_summary, group_id)
               VALUES (?, ?, ?, 5, '2026-11-16 12:00:00', ?,
                       'IDLE', '', '', '', '', 0,
                       '', '', 0, 0, '2026-11-16 12:00:00',
                       0, '', NULL, '')""",
            (test_uuid, test_title, f"Preview for {test_title}", f'["{test_ws}"]'),
        )
        conn.commit()
        conn.close()

        # Create dummy plan and sandbox dir
        plans_dir = Path.home() / ".claude" / "plans"
        plans_dir.mkdir(parents=True, exist_ok=True)
        dummy_plan = plans_dir / "teste-lifecycle-plan.md"
        dummy_plan.write_text("# Temp Plan", encoding="utf-8")

        dummy_sandbox = Path(tempfile.mkdtemp(prefix="teste_sandbox_dir_"))

        mgr = MockOmniRouteManager()

        # 3. Execute cleanup with pre_snapshot
        results = cleanup_all_test_sessions(
            pre_snapshot=pre_snap,
            brain_exe=self.brain_exe,
            sandbox_dirs=[dummy_sandbox],
            omniroute_mgr=mgr,
        )

        # 4. Verify results
        self.assertTrue(results["audit_passed"])
        self.assertIn(test_uuid, results["deleted_sessions"]["antigravity"])
        self.assertTrue(any(dummy_plan.name in p for p in results["deleted_plans"]))
        self.assertTrue(any(dummy_sandbox.name in s for s in results["deleted_sandboxes"]))
        self.assertTrue(mgr.stopped)
        self.assertFalse(test_brain_dir.exists())
        self.assertFalse(dummy_plan.exists())
        self.assertFalse(dummy_sandbox.exists())

        # Verify post counts match pre snapshot exactly
        self.assertEqual(results["post_counts"]["antigravity"], initial_agy_count)

    def test_11_cleanup_claude_code_lifecycle(self) -> None:
        pre_snap = snapshot_sessions(self.brain_exe)
        initial_cc_count = len(pre_snap["claude-code"])

        # Inject synthetic Claude Code test session
        projects_root = Path.home() / ".claude" / "projects"
        test_proj_dir = projects_root / "C--Users-cocad-AppData-Local-Temp-teste-ws-cc"
        test_proj_dir.mkdir(parents=True, exist_ok=True)
        test_uuid = str(uuid.uuid4())
        test_file = test_proj_dir / f"{test_uuid}.jsonl"
        test_file.write_text(
            '{"cwd": "C:\\\\Users\\\\cocad\\\\AppData\\\\Local\\\\Temp\\\\teste-ws-cc", "user": "teste-prompt"}\n',
            encoding="utf-8",
        )

        results = cleanup_all_test_sessions(
            pre_snapshot=pre_snap,
            brain_exe=self.brain_exe,
        )

        self.assertTrue(results["audit_passed"])
        self.assertIn(test_uuid, results["deleted_sessions"]["claude-code"])
        self.assertFalse(test_file.exists())
        self.assertEqual(results["post_counts"]["claude-code"], initial_cc_count)
        if test_proj_dir.exists() and not any(test_proj_dir.iterdir()):
            test_proj_dir.rmdir()

    def test_09_standalone_cli(self) -> None:
        cmd = [sys.executable, "benchmarks/teardown.py", "--cleanup-only"]
        proc = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8")
        self.assertEqual(proc.returncode, 0, f"CLI failed: {proc.stderr}")
        self.assertIn("[OK] Teardown and audit completed successfully.", proc.stdout)

    def test_10_zero_emojis(self) -> None:
        for pyfile in [
            Path(__file__).resolve().parent / "teardown.py",
            Path(__file__).resolve(),
        ]:
            text = pyfile.read_text(encoding="utf-8")
            for line_no, line in enumerate(text.splitlines(), start=1):
                for char in line:
                    code = ord(char)
                    is_emoji = (
                        0x1F600 <= code <= 0x1F64F
                        or 0x1F300 <= code <= 0x1F5FF
                        or 0x1F680 <= code <= 0x1F6FF
                        or 0x1F700 <= code <= 0x1F77F
                        or 0x1F780 <= code <= 0x1F7FF
                        or 0x1F800 <= code <= 0x1F8FF
                        or 0x1F900 <= code <= 0x1FAFF
                        or 0x2600 <= code <= 0x26FF
                        or 0x2700 <= code <= 0x27BF
                    )
                    self.assertFalse(
                        is_emoji,
                        f"Emoji '{char}' (U+{code:04X}) found in {pyfile.name}:{line_no}",
                    )


if __name__ == "__main__":
    unittest.main()
