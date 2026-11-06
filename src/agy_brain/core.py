"""
Core engine for agy-brain: discovery, database queries, pin management and file scanning.
"""

from __future__ import annotations

import json
import os
import platform
import re
import sqlite3
from pathlib import Path
from typing import Any


def get_default_app_dir() -> Path:
    """Discovers the Antigravity CLI application directory across OSs."""
    # 1. Explicit environment variable override
    env_dir = os.environ.get("AGY_APP_DIR") or os.environ.get("ANTIGRAVITY_APP_DIR")
    if env_dir and os.path.isdir(env_dir):
        return Path(env_dir).expanduser().resolve()

    home = Path.home()

    # 2. Universal default location used by antigravity-cli
    default_gemini = home / ".gemini" / "antigravity-cli"
    if default_gemini.is_dir():
        return default_gemini.resolve()

    system = platform.system()

    # 3. OS-specific discovery
    if system == "Windows":
        local_app_data = os.environ.get("LOCALAPPDATA")
        if local_app_data:
            agy_local = Path(local_app_data) / "agy"
            if agy_local.is_dir():
                return agy_local.resolve()
        app_data = os.environ.get("APPDATA")
        if app_data:
            antigravity_roaming = Path(app_data) / "antigravity-cli"
            if antigravity_roaming.is_dir():
                return antigravity_roaming.resolve()
    elif system == "Darwin":
        mac_app_support = home / "Library" / "Application Support" / "antigravity-cli"
        if mac_app_support.is_dir():
            return mac_app_support.resolve()
    else:
        # Linux / BSD
        xdg_data_home = os.environ.get("XDG_DATA_HOME")
        if xdg_data_home:
            xdg_path = Path(xdg_data_home) / "antigravity-cli"
            if xdg_path.is_dir():
                return xdg_path.resolve()
        linux_config = home / ".config" / "antigravity-cli"
        if linux_config.is_dir():
            return linux_config.resolve()

    # Fallback default
    return default_gemini.resolve()


def get_db_path(app_dir: Path | None = None) -> Path:
    """Returns path to conversation_summaries.db."""
    base = app_dir or get_default_app_dir()
    return base / "conversation_summaries.db"


def get_pins_file(app_dir: Path | None = None) -> Path:
    """Returns path to workspace_pins.json."""
    base = app_dir or get_default_app_dir()
    return base / "workspace_pins.json"


def get_brain_root(app_dir: Path | None = None) -> Path:
    """Returns path to brain directory."""
    base = app_dir or get_default_app_dir()
    return base / "brain"


def normalize_workspace_path(path_str: str) -> str:
    """Normalizes a workspace URI or file path for consistent matching across OSs."""
    if not path_str:
        return ""
    # Strip file:/// or file://
    clean = re.sub(r"^file:///?", "", path_str.strip())
    # Standardize forward slashes and strip trailing slash
    clean = clean.replace("\\", "/").rstrip("/")
    if platform.system() == "Windows":
        clean = clean.lower()
    return clean


def get_pinned_id_for_workspace(
    workspace_path: str | None = None,
    app_dir: Path | None = None
) -> str | None:
    """Finds pinned brain ID for a workspace via local .brain file or workspace_pins.json."""
    if not workspace_path:
        workspace_path = os.getcwd()

    norm_ws = normalize_workspace_path(workspace_path)

    # 1. Check local .brain file by walking up the directory tree
    try:
        cur = Path(workspace_path).resolve()
        while True:
            candidate = cur / ".brain"
            if candidate.is_file():
                try:
                    content = candidate.read_text(encoding="utf-8").strip()
                    if len(content) >= 30 and "-" in content:
                        return content
                except Exception:
                    pass
            parent = cur.parent
            if parent == cur:
                break
            cur = parent
    except Exception:
        pass

    # 2. Check global workspace_pins.json
    pins_file = get_pins_file(app_dir)
    if pins_file.is_file():
        try:
            with open(pins_file, "r", encoding="utf-8") as f:
                pins = json.load(f)
                if isinstance(pins, dict):
                    # Check direct or prefix match
                    for pin_path, brain_id in pins.items():
                        if normalize_workspace_path(pin_path) == norm_ws:
                            return brain_id
        except Exception:
            pass

    return None


def set_pin(
    brain_id: str,
    workspace_path: str | None = None,
    app_dir: Path | None = None
) -> None:
    """Pins a brain ID to a workspace locally (.brain) and in workspace_pins.json."""
    ws = Path(workspace_path or os.getcwd()).resolve()
    norm_ws = normalize_workspace_path(str(ws))

    pins_file = get_pins_file(app_dir)
    pins = {}
    if pins_file.is_file():
        try:
            with open(pins_file, "r", encoding="utf-8") as f:
                pins = json.load(f)
        except Exception:
            pins = {}

    pins[norm_ws] = brain_id
    try:
        pins_file.parent.mkdir(parents=True, exist_ok=True)
        with open(pins_file, "w", encoding="utf-8") as f:
            json.dump(pins, f, indent=2)
    except Exception:
        pass

    # Also save local .brain file in workspace root
    try:
        dot_brain = ws / ".brain"
        dot_brain.write_text(brain_id.strip() + "\n", encoding="utf-8")
    except Exception:
        pass


def remove_pin(
    workspace_path: str | None = None,
    app_dir: Path | None = None
) -> bool:
    """Removes pin from workspace locally and globally."""
    ws = Path(workspace_path or os.getcwd()).resolve()
    norm_ws = normalize_workspace_path(str(ws))
    removed = False

    pins_file = get_pins_file(app_dir)
    if pins_file.is_file():
        try:
            with open(pins_file, "r", encoding="utf-8") as f:
                pins = json.load(f)
            if norm_ws in pins:
                del pins[norm_ws]
                with open(pins_file, "w", encoding="utf-8") as f:
                    json.dump(pins, f, indent=2)
                removed = True
        except Exception:
            pass

    try:
        dot_brain = ws / ".brain"
        if dot_brain.is_file():
            dot_brain.unlink()
            removed = True
    except Exception:
        pass

    return removed


def fetch_sessions(
    workspace_path: str | None = None,
    all_workspaces: bool = False,
    app_dir: Path | None = None,
    db_path: Path | None = None,
) -> list[dict[str, Any]]:
    """Queries conversation_summaries.db and returns structured session records."""
    target_db = db_path or get_db_path(app_dir)
    if not target_db.is_file():
        return []

    norm_ws = normalize_workspace_path(workspace_path) if workspace_path else ""
    pinned_id = get_pinned_id_for_workspace(workspace_path, app_dir) if workspace_path else None

    # Connect in read-only URI mode when possible
    uri = f"file:{target_db.as_posix()}?mode=ro"
    try:
        conn = sqlite3.connect(uri, uri=True, timeout=5.0)
    except Exception:
        conn = sqlite3.connect(str(target_db), timeout=5.0)

    try:
        cursor = conn.cursor()
        cursor.execute("""
            SELECT conversation_id, title, step_count, last_modified_time, workspace_uris, status
            FROM conversation_summaries
            ORDER BY step_count DESC, last_modified_time DESC
        """)
        rows = cursor.fetchall()
    except Exception:
        return []
    finally:
        conn.close()

    results: list[dict[str, Any]] = []
    for cid, title, steps, mtime, uris_raw, status in rows:
        try:
            uris = json.loads(uris_raw) if uris_raw else []
        except Exception:
            uris = [uris_raw] if uris_raw else []

        clean_uris = [normalize_workspace_path(u) for u in uris]
        primary_uri = clean_uris[0] if clean_uris else ""

        is_match = False
        if all_workspaces or not norm_ws:
            is_match = True
        else:
            for u in clean_uris:
                if norm_ws == u or norm_ws.startswith(u + "/") or u.startswith(norm_ws + "/"):
                    is_match = True
                    break

        if is_match:
            is_pinned = (cid == pinned_id)
            results.append({
                "id": cid,
                "title": (title or "(sem titulo)").strip(),
                "steps": steps or 0,
                "mtime": (mtime or "")[:19].replace("T", " "),
                "uris": clean_uris,
                "primary_workspace": primary_uri,
                "status": status or "",
                "pinned": is_pinned,
            })

    results.sort(key=lambda s: (not s["pinned"], -s["steps"]))
    return results


def get_brain_stats(
    brain_id: str,
    app_dir: Path | None = None
) -> dict[str, Any] | None:
    """Calculates file inventory and stats for a given brain directory."""
    brain_dir = get_brain_root(app_dir) / brain_id
    if not brain_dir.is_dir():
        return None

    stats: dict[str, Any] = {
        "exists": True,
        "path": str(brain_dir),
        "md_count": 0,
        "png_count": 0,
        "scratch_count": 0,
        "total_files": 0,
        "recent_artifacts": [],
    }

    try:
        entries = list(brain_dir.iterdir())
        stats["total_files"] = len(entries)
        md_files = []
        for entry in entries:
            if entry.is_file():
                name = entry.name
                if name.endswith(".md") and not name.endswith(".metadata.json"):
                    stats["md_count"] += 1
                    try:
                        sz = entry.stat().st_size
                        mt = entry.stat().st_mtime
                        md_files.append((name, sz, mt))
                    except Exception:
                        pass
                elif name.endswith(".png"):
                    stats["png_count"] += 1

        scratch_dir = brain_dir / "scratch"
        if scratch_dir.is_dir():
            stats["scratch_count"] = len(list(scratch_dir.iterdir()))

        md_files.sort(key=lambda x: -x[2])
        stats["recent_artifacts"] = md_files[:10]
    except Exception:
        pass

    return stats


def resolve_brain_target(
    target: str | None = None,
    workspace_path: str | None = None,
    app_dir: Path | None = None,
) -> str | None:
    """Resolves an index ('0', '1'), UUID, or default to the matching conversation ID."""
    t = (target or "default").strip()

    # Direct UUID
    if len(t) >= 30 and "-" in t:
        return t

    sessions = fetch_sessions(workspace_path, all_workspaces=False, app_dir=app_dir)
    if not sessions:
        sessions = fetch_sessions(workspace_path, all_workspaces=True, app_dir=app_dir)

    # Numeric index
    if t.isdigit():
        idx = int(t)
        if 0 <= idx < len(sessions):
            return sessions[idx]["id"]
        return None

    if t == "default":
        # Check pinned first
        pinned = get_pinned_id_for_workspace(workspace_path, app_dir)
        if pinned:
            return pinned
        if sessions:
            return sessions[0]["id"]

    return None
