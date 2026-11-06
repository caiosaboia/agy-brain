"""
Unit tests for agy_brain.cli.
"""

import sqlite3
import sys

import pytest

from agy_brain.cli import main


def test_cli_version(capsys):
    with pytest.raises(SystemExit) as exc:
        sys.argv = ["agy-brain", "--version"]
        main()
    assert exc.value.code == 0
    captured = capsys.readouterr()
    assert "0.1.0" in captured.out or "0.1.0" in captured.err


def test_cli_init_bash(capsys):
    sys.argv = ["agy-brain", "init", "bash"]
    main()
    captured = capsys.readouterr()
    assert "agy-brain() {" in captured.out


def test_cli_doctor(capsys, tmp_path):
    sys.argv = ["agy-brain", "--app-dir", str(tmp_path), "doctor"]
    main()
    captured = capsys.readouterr()
    assert "AGY-BRAIN DOCTOR" in captured.out


def test_cli_resolve_target(capsys, tmp_path):
    app_dir = tmp_path / "agy_home"
    app_dir.mkdir()
    db_file = app_dir / "conversation_summaries.db"

    conn = sqlite3.connect(str(db_file))
    conn.execute("""
        CREATE TABLE conversation_summaries (
            conversation_id text PRIMARY KEY,
            title text NOT NULL DEFAULT '',
            preview text NOT NULL DEFAULT '',
            step_count integer NOT NULL DEFAULT 0,
            last_modified_time datetime NOT NULL,
            workspace_uris text NOT NULL,
            status text NOT NULL DEFAULT ''
        )
    """)
    conn.execute("""
        INSERT INTO conversation_summaries VALUES
        ('uuid-target-1234-1234-123456789abc', 'Test', '', 100, '2026-11-05 12:00:00', '["file:///C:/test"]', 'IDLE')
    """)
    conn.commit()
    conn.close()

    sys.argv = [
        "agy-brain",
        "--app-dir", str(app_dir),
        "resolve", "uuid-target-1234-1234-123456789abc",
    ]
    with pytest.raises(SystemExit) as exc:
        main()
    assert exc.value.code == 0
    captured = capsys.readouterr()
    assert "uuid-target-1234-1234-123456789abc" in captured.out
