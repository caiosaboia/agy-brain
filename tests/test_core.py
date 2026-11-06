"""
Unit tests for agy_brain.core.
"""

import sqlite3

from agy_brain.core import (
    fetch_sessions,
    get_brain_stats,
    get_default_app_dir,
    get_pinned_id_for_workspace,
    normalize_workspace_path,
    remove_pin,
    resolve_brain_target,
    set_pin,
)


def test_normalize_workspace_path():
    assert normalize_workspace_path("file:///C:/Users/test/project/") == "c:/users/test/project" or normalize_workspace_path("file:///C:/Users/test/project/") == "C:/Users/test/project".lower()
    assert normalize_workspace_path("C:\\Users\\test\\project") in ["c:/users/test/project", "C:/Users/test/project".lower()]
    assert normalize_workspace_path("file:///home/user/code/") in ["home/user/code", "/home/user/code", "home/user/code".lower()]
    assert normalize_workspace_path("") == ""


def test_app_dir_env_override(tmp_path, monkeypatch):
    custom_dir = tmp_path / "custom_agy"
    custom_dir.mkdir()
    monkeypatch.setenv("AGY_APP_DIR", str(custom_dir))

    discovered = get_default_app_dir()
    assert discovered == custom_dir.resolve()


def test_pin_management(tmp_path):
    app_dir = tmp_path / "agy_home"
    app_dir.mkdir()
    ws_dir = tmp_path / "my_project"
    ws_dir.mkdir()

    brain_uuid = "12345678-1234-1234-1234-123456789abc"

    # Initially no pin
    assert get_pinned_id_for_workspace(str(ws_dir), app_dir=app_dir) is None

    # Set pin
    set_pin(brain_uuid, str(ws_dir), app_dir=app_dir)
    assert get_pinned_id_for_workspace(str(ws_dir), app_dir=app_dir) == brain_uuid

    # Check local .brain file
    dot_brain = ws_dir / ".brain"
    assert dot_brain.is_file()
    assert dot_brain.read_text().strip() == brain_uuid

    # Remove pin
    assert remove_pin(str(ws_dir), app_dir=app_dir) is True
    assert get_pinned_id_for_workspace(str(ws_dir), app_dir=app_dir) is None
    assert not dot_brain.is_file()


def test_fetch_sessions_with_mock_db(tmp_path):
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
        ('uuid-1111-1111-1111-111111111111', 'Main Session', 'Preview', 500, '2026-11-05 12:00:00', '["file:///C:/Users/dev/repo"]', 'IDLE'),
        ('uuid-2222-2222-2222-222222222222', 'Small Session', 'Preview', 10, '2026-11-05 13:00:00', '["file:///C:/Users/dev/repo"]', 'IDLE'),
        ('uuid-3333-3333-3333-333333333333', 'Other Project', 'Preview', 200, '2026-11-05 14:00:00', '["file:///C:/Users/dev/other"]', 'IDLE')
    """)
    conn.commit()
    conn.close()

    # Query for repo workspace
    sessions = fetch_sessions("C:/Users/dev/repo", app_dir=app_dir)
    assert len(sessions) == 2
    # Ordered by step_count DESC
    assert sessions[0]["id"] == "uuid-1111-1111-1111-111111111111"
    assert sessions[0]["steps"] == 500
    assert sessions[1]["id"] == "uuid-2222-2222-2222-222222222222"

    # Query all
    all_sessions = fetch_sessions(all_workspaces=True, app_dir=app_dir)
    assert len(all_sessions) == 3


def test_get_brain_stats(tmp_path):
    app_dir = tmp_path / "agy_home"
    brain_root = app_dir / "brain"
    target_brain = brain_root / "test-brain-uuid"
    target_brain.mkdir(parents=True)

    # Add artifacts
    (target_brain / "plan.md").write_text("# Plan")
    (target_brain / "plan.md.metadata.json").write_text("{}")  # should be excluded
    (target_brain / "screen.png").write_bytes(b"\x89PNG")

    scratch_dir = target_brain / "scratch"
    scratch_dir.mkdir()
    (scratch_dir / "script.py").write_text("print(1)")

    stats = get_brain_stats("test-brain-uuid", app_dir=app_dir)
    assert stats is not None
    assert stats["exists"] is True
    assert stats["md_count"] == 1
    assert stats["png_count"] == 1
    assert stats["scratch_count"] == 1
    assert len(stats["recent_artifacts"]) == 1
    assert stats["recent_artifacts"][0][0] == "plan.md"


def test_resolve_brain_target(tmp_path):
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
        ('uuid-1111-1111-1111-111111111111', 'First', '', 100, '2026-11-05 12:00:00', '["file:///C:/Users/dev/repo"]', 'IDLE'),
        ('uuid-2222-2222-2222-222222222222', 'Second', '', 50, '2026-11-05 13:00:00', '["file:///C:/Users/dev/repo"]', 'IDLE')
    """)
    conn.commit()
    conn.close()

    ws = "C:/Users/dev/repo"

    # Default resolves to highest step_count
    assert resolve_brain_target("default", ws, app_dir=app_dir) == "uuid-1111-1111-1111-111111111111"

    # Numeric index
    assert resolve_brain_target("1", ws, app_dir=app_dir) == "uuid-2222-2222-2222-222222222222"

    # Direct UUID
    direct_uuid = "99999999-9999-9999-9999-999999999999"
    assert resolve_brain_target(direct_uuid, ws, app_dir=app_dir) == direct_uuid
