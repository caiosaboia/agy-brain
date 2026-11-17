#!/usr/bin/env python3
"""Deterministic teardown and audit module for Antigravity and Claude Code test sessions.

This module guarantees deterministic cleanup of test sessions, temporary plan files,
and sandbox directories, restoring session counts to the pre-test snapshot and auditing
that zero residues remain.
Adheres strictly to zero emojis across all code, comments, and outputs.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import sqlite3
import stat
import subprocess
import sys
from pathlib import Path
from typing import Any

ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")


def strip_ansi(text: str) -> str:
    """Strip ANSI escape sequences from terminal text."""
    return ANSI_RE.sub("", text)


def find_brain_executable() -> str:
    """Locate the compiled brain binary in target/release or system PATH."""
    repo_root = Path(__file__).resolve().parent.parent
    candidates = [
        repo_root / "target" / "release" / "brain.exe",
        repo_root / "target" / "release" / "brain",
        Path(shutil.which("brain") or ""),
    ]
    for cand in candidates:
        if cand and cand.is_file():
            return str(cand.resolve())
    raise FileNotFoundError("Could not locate brain executable in target/release or PATH.")


def run_cmd(
    args: list[str],
    cwd: Path | str | None = None,
    check: bool = False,
) -> tuple[int, str, str]:
    """Execute a command synchronously and return (returncode, stdout, stderr)."""
    proc = subprocess.run(
        args,
        cwd=str(cwd) if cwd else None,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if check and proc.returncode != 0:
        raise RuntimeError(
            f"Command failed with exit code {proc.returncode}: {' '.join(args)}\n"
            f"STDOUT:\n{proc.stdout}\nSTDERR:\n{proc.stderr}"
        )
    return proc.returncode, proc.stdout, proc.stderr


def parse_brain_list_sessions(raw_stdout: str) -> list[dict[str, Any]]:
    """Parse output of `brain list` table into a list of session records."""
    clean = strip_ansi(raw_stdout)
    sessions: list[dict[str, Any]] = []
    for line in clean.splitlines():
        line = line.strip()
        if not line.startswith("|") or not line.endswith("|"):
            continue
        parts = [p.strip() for p in line.split("|")]
        # Standard table row format: ['', idx, status, title, steps, date, id, '']
        if len(parts) >= 8 and parts[1].isdigit():
            idx = int(parts[1])
            status = parts[2]
            title = parts[3]
            steps = parts[4]
            date = parts[5]
            raw_id = parts[6]
            short_id = raw_id.rstrip(".")
            sessions.append({
                "idx": idx,
                "status": status,
                "title": title,
                "steps": steps,
                "date": date,
                "raw_id": raw_id,
                "short_id": short_id,
            })
    return sessions


def get_antigravity_db_path(app_dir: str | Path | None = None) -> Path | None:
    """Locate Antigravity SQLite database path."""
    if app_dir:
        cand = Path(app_dir) / "conversation_summaries.db"
        if cand.is_file():
            return cand
    env_dir = os.environ.get("AGY_APP_DIR") or os.environ.get("ANTIGRAVITY_APP_DIR")
    if env_dir:
        cand = Path(env_dir) / "conversation_summaries.db"
        if cand.is_file():
            return cand
    default_path = Path.home() / ".gemini" / "antigravity-cli" / "conversation_summaries.db"
    if default_path.is_file():
        return default_path
    return None


def get_claude_projects_root(app_dir: str | Path | None = None) -> Path | None:
    """Locate Claude Code projects directory root."""
    if app_dir:
        cand = Path(app_dir) / "projects"
        if cand.is_dir():
            return cand
    env_dir = os.environ.get("AGY_CLAUDE_APP_DIR") or os.environ.get("CLAUDE_CONFIG_DIR")
    if env_dir:
        cand = Path(env_dir) / "projects"
        if cand.is_dir():
            return cand
    default_path = Path.home() / ".claude" / "projects"
    if default_path.is_dir():
        return default_path
    return None


def load_antigravity_metadata(db_path: Path | None) -> dict[str, dict[str, Any]]:
    """Load conversation metadata from Antigravity SQLite database."""
    metadata_by_uuid: dict[str, dict[str, Any]] = {}
    if not db_path or not db_path.is_file():
        return metadata_by_uuid
    try:
        conn = sqlite3.connect(db_path)
        cur = conn.cursor()
        cur.execute(
            "SELECT conversation_id, title, workspace_uris, preview FROM conversation_summaries"
        )
        for cid, title, uris, preview in cur.fetchall():
            metadata_by_uuid[cid] = {
                "uuid": cid,
                "title": title or "",
                "preview": preview or "",
                "workspace": uris or "",
            }
    except Exception:
        pass
    return metadata_by_uuid


def load_claude_metadata(projects_root: Path | None) -> dict[str, dict[str, Any]]:
    """Load session metadata from Claude Code projects folder."""
    metadata_by_uuid: dict[str, dict[str, Any]] = {}
    if not projects_root or not projects_root.is_dir():
        return metadata_by_uuid
    try:
        for jsonl_file in projects_root.glob("*/*.jsonl"):
            cid = jsonl_file.stem
            encoded_ws = jsonl_file.parent.name
            cwd = encoded_ws
            title = ""
            try:
                with open(jsonl_file, "r", encoding="utf-8", errors="replace") as f:
                    for _ in range(15):
                        line = f.readline()
                        if not line:
                            break
                        if '"cwd"' in line and not cwd:
                            m = re.search(r'"cwd"\s*:\s*"([^"]+)"', line)
                            if m:
                                cwd = m.group(1)
                        if not title and ('"user"' in line or '"text"' in line):
                            m = re.search(r'"text"\s*:\s*"([^"]+)"', line)
                            if m:
                                title = m.group(1)
            except Exception:
                pass
            metadata_by_uuid[cid] = {
                "uuid": cid,
                "title": title,
                "workspace": cwd,
                "encoded_workspace": encoded_ws,
                "file_path": str(jsonl_file),
            }
    except Exception:
        pass
    return metadata_by_uuid


def resolve_session_uuid(
    provider: str,
    short_id: str,
    metadata_map: dict[str, dict[str, Any]] | None = None,
    app_dir: str | Path | None = None,
    brain_exe: str | Path | None = None,
) -> str:
    """Resolve a short session ID or table prefix into its canonical full UUID."""
    if len(short_id) >= 30 and "-" in short_id:
        return short_id

    short_lower = short_id.lower()
    if metadata_map:
        for full_uuid in metadata_map:
            if full_uuid.lower().startswith(short_lower):
                return full_uuid

    # Fallback to brain info command
    if brain_exe:
        cmd = [str(brain_exe), "--provider", provider]
        if app_dir:
            cmd.extend(["--app-dir", str(app_dir)])
        cmd.extend(["info", short_id])
        ret, stdout, _ = run_cmd(cmd)
        if ret == 0:
            m = re.search(r"ID:\s*([a-f0-9\-]+)", strip_ansi(stdout), re.IGNORECASE)
            if m:
                return m.group(1).strip()

    return short_id


def snapshot_sessions(
    brain_exe: str | Path | None = None,
    app_dir: str | Path | None = None,
) -> dict[str, set[str]]:
    """Capture current set of session UUIDs for antigravity and claude-code.

    Queries:
      - Antigravity via `brain --provider antigravity list -a -s`
      - Claude Code via `brain --provider claude-code list -a`
    Returns:
      { "antigravity": set(...), "claude-code": set(...) }
    """
    brain_bin = str(brain_exe) if brain_exe else find_brain_executable()
    snapshots: dict[str, set[str]] = {
        "antigravity": set(),
        "claude-code": set(),
    }

    # 1. Antigravity
    agy_cmd = [brain_bin, "--provider", "antigravity", "list", "-a", "-s"]
    if app_dir:
        agy_cmd.extend(["--app-dir", str(app_dir)])
    _, agy_out, _ = run_cmd(agy_cmd)
    agy_sessions = parse_brain_list_sessions(agy_out)

    agy_db = get_antigravity_db_path(app_dir)
    agy_meta = load_antigravity_metadata(agy_db)

    for s in agy_sessions:
        full_uuid = resolve_session_uuid(
            "antigravity", s["short_id"], agy_meta, app_dir, brain_bin
        )
        snapshots["antigravity"].add(full_uuid)

    # 2. Claude Code
    cc_cmd = [brain_bin, "--provider", "claude-code", "list", "-a"]
    if app_dir:
        cc_cmd.extend(["--app-dir", str(app_dir)])
    _, cc_out, _ = run_cmd(cc_cmd)
    cc_sessions = parse_brain_list_sessions(cc_out)

    cc_root = get_claude_projects_root(app_dir)
    cc_meta = load_claude_metadata(cc_root)

    for s in cc_sessions:
        full_uuid = resolve_session_uuid(
            "claude-code", s["short_id"], cc_meta, app_dir, brain_bin
        )
        snapshots["claude-code"].add(full_uuid)

    return snapshots


def clean_temporary_plans(app_dir: str | Path | None = None) -> list[Path]:
    """Clean any temporary markdown plan files matching ~/.claude/plans/teste-*.md."""
    deleted_plans: list[Path] = []
    candidate_dirs = [Path.home() / ".claude" / "plans"]
    if app_dir:
        candidate_dirs.append(Path(app_dir) / "plans")
        candidate_dirs.append(Path(app_dir) / ".claude" / "plans")

    for pdir in candidate_dirs:
        if pdir.is_dir():
            for pfile in pdir.glob("teste-*.md"):
                try:
                    pfile.unlink(missing_ok=True)
                    deleted_plans.append(pfile)
                except OSError:
                    pass
    return deleted_plans


def delete_sandbox_dirs(sandbox_dirs: list[str | Path] | None) -> list[Path]:
    """Recursively delete all sandbox directory paths."""
    deleted_dirs: list[Path] = []
    if not sandbox_dirs:
        return deleted_dirs

    def on_rm_error(func: Any, path: str, exc_info: Any) -> None:
        try:
            os.chmod(path, stat.S_IWRITE)
            func(path)
        except Exception:
            pass

    for sdir in sandbox_dirs:
        p = Path(sdir)
        if p.exists():
            if p.is_dir():
                shutil.rmtree(p, onerror=on_rm_error)
            else:
                try:
                    p.unlink(missing_ok=True)
                except OSError:
                    pass
            deleted_dirs.append(p)
    return deleted_dirs


def stop_omniroute_mgr(omniroute_mgr: Any) -> bool:
    """Stop OmniRoute manager if provided."""
    if omniroute_mgr is not None:
        if hasattr(omniroute_mgr, "stop") and callable(omniroute_mgr.stop):
            try:
                omniroute_mgr.stop()
                return True
            except Exception:
                pass
    return False


def cleanup_all_test_sessions(
    pre_snapshot: dict[str, set[str]] | None = None,
    brain_exe: str | Path | None = None,
    sandbox_dirs: list[str | Path] | None = None,
    omniroute_mgr: Any = None,
    app_dir: str | Path | None = None,
) -> dict[str, Any]:
    """Deterministically clean all test sessions across Antigravity and Claude Code.

    1. Identifies any session whose UUID is not in pre_snapshot OR whose title contains
       'teste-' OR whose workspace path contains 'teste-ws-'.
    2. Invokes `brain --provider <provider> delete <uuid> --force` for every identified session.
    3. Cleans any temporary markdown plan files matching ~/.claude/plans/teste-*.md.
    4. Recursively deletes all sandbox directory paths in sandbox_dirs.
    5. If omniroute_mgr is provided, calls its stop() method.
    6. Post-cleanup audit: queries `brain list -a -s` for both providers and verifies
       that 0 sessions containing 'teste-' remain and that session counts match pre_snapshot.
    """
    brain_bin = str(brain_exe) if brain_exe else find_brain_executable()

    deleted_sessions: dict[str, list[str]] = {
        "antigravity": [],
        "claude-code": [],
    }

    # Prepare metadata sources
    agy_db = get_antigravity_db_path(app_dir)
    agy_meta = load_antigravity_metadata(agy_db)

    cc_root = get_claude_projects_root(app_dir)
    cc_meta = load_claude_metadata(cc_root)

    # 1. Inspect Antigravity
    agy_cmd = [brain_bin, "--provider", "antigravity", "list", "-a", "-s"]
    if app_dir:
        agy_cmd.extend(["--app-dir", str(app_dir)])
    _, agy_out, _ = run_cmd(agy_cmd)
    agy_sessions = parse_brain_list_sessions(agy_out)

    seen_agy_uuids: set[str] = set()
    for s in agy_sessions:
        short_id = s["short_id"]
        full_uuid = resolve_session_uuid("antigravity", short_id, agy_meta, app_dir, brain_bin)
        seen_agy_uuids.add(full_uuid)
        meta = agy_meta.get(full_uuid, {})
        full_title = meta.get("title", "")
        preview = meta.get("preview", "")
        workspace = meta.get("workspace", "")
        list_title = s["title"]

        to_delete = False
        if pre_snapshot is not None and "antigravity" in pre_snapshot:
            known = pre_snapshot["antigravity"]
            if full_uuid not in known and short_id not in known:
                to_delete = True

        if "teste-" in full_title.lower() or "teste-" in list_title.lower() or "teste-" in preview.lower():
            to_delete = True

        if "teste-ws-" in workspace.lower():
            to_delete = True

        if "teste-" in full_uuid.lower():
            to_delete = True

        if to_delete:
            del_cmd = [brain_bin, "--provider", "antigravity", "delete", full_uuid, "--force"]
            if app_dir:
                del_cmd.extend(["--app-dir", str(app_dir)])
            ret, _, _ = run_cmd(del_cmd)
            if ret != 0:
                # Fallback to short_id
                del_cmd_short = [brain_bin, "--provider", "antigravity", "delete", short_id, "--force"]
                if app_dir:
                    del_cmd_short.extend(["--app-dir", str(app_dir)])
                run_cmd(del_cmd_short)
            deleted_sessions["antigravity"].append(full_uuid)

    # Also check unlisted sessions in DB (e.g. ghost test sessions not shown in list)
    for full_uuid, meta in agy_meta.items():
        if full_uuid in seen_agy_uuids or full_uuid in deleted_sessions["antigravity"]:
            continue
        full_title = meta.get("title", "")
        preview = meta.get("preview", "")
        workspace = meta.get("workspace", "")
        to_delete = False

        if pre_snapshot is not None and "antigravity" in pre_snapshot:
            known = pre_snapshot["antigravity"]
            if full_uuid not in known:
                to_delete = True

        if "teste-" in full_title.lower() or "teste-" in preview.lower():
            to_delete = True

        if "teste-ws-" in workspace.lower():
            to_delete = True

        if "teste-" in full_uuid.lower():
            to_delete = True

        if to_delete:
            del_cmd = [brain_bin, "--provider", "antigravity", "delete", full_uuid, "--force"]
            if app_dir:
                del_cmd.extend(["--app-dir", str(app_dir)])
            run_cmd(del_cmd)
            deleted_sessions["antigravity"].append(full_uuid)

    # 2. Inspect Claude Code
    cc_cmd = [brain_bin, "--provider", "claude-code", "list", "-a"]
    if app_dir:
        cc_cmd.extend(["--app-dir", str(app_dir)])
    _, cc_out, _ = run_cmd(cc_cmd)
    cc_sessions = parse_brain_list_sessions(cc_out)

    seen_cc_uuids: set[str] = set()
    for s in cc_sessions:
        short_id = s["short_id"]
        full_uuid = resolve_session_uuid("claude-code", short_id, cc_meta, app_dir, brain_bin)
        seen_cc_uuids.add(full_uuid)
        meta = cc_meta.get(full_uuid, {})
        full_title = meta.get("title", "")
        workspace = meta.get("workspace", "")
        list_title = s["title"]

        to_delete = False
        if pre_snapshot is not None and "claude-code" in pre_snapshot:
            known = pre_snapshot["claude-code"]
            if full_uuid not in known and short_id not in known:
                to_delete = True

        if "teste-" in full_title.lower() or "teste-" in list_title.lower():
            to_delete = True

        if "teste-ws-" in workspace.lower():
            to_delete = True

        if "teste-" in full_uuid.lower():
            to_delete = True

        if to_delete:
            del_cmd = [brain_bin, "--provider", "claude-code", "delete", full_uuid, "--force"]
            if app_dir:
                del_cmd.extend(["--app-dir", str(app_dir)])
            ret, _, _ = run_cmd(del_cmd)
            if ret != 0:
                del_cmd_short = [brain_bin, "--provider", "claude-code", "delete", short_id, "--force"]
                if app_dir:
                    del_cmd_short.extend(["--app-dir", str(app_dir)])
                run_cmd(del_cmd_short)
            deleted_sessions["claude-code"].append(full_uuid)

    # Also check unlisted files in projects root
    for full_uuid, meta in cc_meta.items():
        if full_uuid in seen_cc_uuids or full_uuid in deleted_sessions["claude-code"]:
            continue
        full_title = meta.get("title", "")
        workspace = meta.get("workspace", "")
        to_delete = False

        if pre_snapshot is not None and "claude-code" in pre_snapshot:
            known = pre_snapshot["claude-code"]
            if full_uuid not in known:
                to_delete = True

        if "teste-" in full_title.lower():
            to_delete = True

        if "teste-ws-" in workspace.lower():
            to_delete = True

        if "teste-" in full_uuid.lower():
            to_delete = True

        if to_delete:
            del_cmd = [brain_bin, "--provider", "claude-code", "delete", full_uuid, "--force"]
            if app_dir:
                del_cmd.extend(["--app-dir", str(app_dir)])
            run_cmd(del_cmd)
            deleted_sessions["claude-code"].append(full_uuid)

    # 3. Clean temporary markdown plans
    deleted_plans = clean_temporary_plans(app_dir)

    # 4. Recursively delete sandbox dirs
    deleted_sandboxes = delete_sandbox_dirs(sandbox_dirs)

    # 5. Stop OmniRoute manager if provided
    omniroute_stopped = stop_omniroute_mgr(omniroute_mgr)

    # 6. Post-cleanup audit
    audit_errors: list[str] = []

    # Audit Antigravity
    audit_agy_cmd = [brain_bin, "--provider", "antigravity", "list", "-a", "-s"]
    if app_dir:
        audit_agy_cmd.extend(["--app-dir", str(app_dir)])
    _, audit_agy_out, _ = run_cmd(audit_agy_cmd)
    post_agy_sessions = parse_brain_list_sessions(audit_agy_out)

    for s in post_agy_sessions:
        if "teste-" in s["title"].lower() or "teste-" in s["raw_id"].lower():
            audit_errors.append(
                f"Residual test session found in Antigravity: ID={s['raw_id']} Title='{s['title']}'"
            )

    # Audit Claude Code
    audit_cc_cmd = [brain_bin, "--provider", "claude-code", "list", "-a", "-s"]
    if app_dir:
        audit_cc_cmd.extend(["--app-dir", str(app_dir)])
    _, audit_cc_out, _ = run_cmd(audit_cc_cmd)
    post_cc_sessions = parse_brain_list_sessions(audit_cc_out)

    for s in post_cc_sessions:
        if "teste-" in s["title"].lower() or "teste-" in s["raw_id"].lower():
            audit_errors.append(
                f"Residual test session found in Claude Code: ID={s['raw_id']} Title='{s['title']}'"
            )

    # Audit session counts match pre_snapshot if provided
    if pre_snapshot is not None:
        expected_agy = len(pre_snapshot.get("antigravity", set()))
        actual_agy = len(post_agy_sessions)
        if actual_agy != expected_agy:
            audit_errors.append(
                f"Antigravity session count mismatch: expected {expected_agy}, got {actual_agy}"
            )

        expected_cc = len(pre_snapshot.get("claude-code", set()))
        actual_cc = len(post_cc_sessions)
        if actual_cc != expected_cc:
            audit_errors.append(
                f"Claude Code session count mismatch: expected {expected_cc}, got {actual_cc}"
            )

    # Audit temporary plan files
    candidate_dirs = [Path.home() / ".claude" / "plans"]
    if app_dir:
        candidate_dirs.append(Path(app_dir) / "plans")
        candidate_dirs.append(Path(app_dir) / ".claude" / "plans")
    for pdir in candidate_dirs:
        if pdir.is_dir():
            remaining_plans = list(pdir.glob("teste-*.md"))
            if remaining_plans:
                audit_errors.append(
                    f"Residual temporary plan files found in {pdir}: {[p.name for p in remaining_plans]}"
                )

    if audit_errors:
        error_msg = "Post-cleanup audit failure:\n" + "\n".join(f"  - {err}" for err in audit_errors)
        raise RuntimeError(error_msg)

    return {
        "deleted_sessions": deleted_sessions,
        "deleted_plans": [str(p) for p in deleted_plans],
        "deleted_sandboxes": [str(p) for p in deleted_sandboxes],
        "omniroute_stopped": omniroute_stopped,
        "audit_passed": True,
        "post_counts": {
            "antigravity": len(post_agy_sessions),
            "claude-code": len(post_cc_sessions),
        },
    }


def main() -> int:
    """CLI entrypoint for standalone cleanup execution."""
    parser = argparse.ArgumentParser(
        description="Deterministic teardown and audit utility for Antigravity and Claude Code test sessions."
    )
    parser.add_argument(
        "--cleanup-only",
        action="store_true",
        help="Perform deterministic cleanup of test sessions without a pre-snapshot.",
    )
    parser.add_argument(
        "--brain-exe",
        type=str,
        default=None,
        help="Path to brain executable binary.",
    )
    parser.add_argument(
        "--app-dir",
        type=str,
        default=None,
        help="Override Antigravity / Claude Code application directory.",
    )
    parser.add_argument(
        "--sandbox-dirs",
        nargs="*",
        default=[],
        help="Paths to sandbox directories to remove recursively.",
    )

    args = parser.parse_args()

    try:
        brain_exe = args.brain_exe or find_brain_executable()
        print(f"[INFO] Using brain executable: {brain_exe}")
        print("[INFO] Starting cleanup...")

        results = cleanup_all_test_sessions(
            pre_snapshot=None,
            brain_exe=brain_exe,
            sandbox_dirs=args.sandbox_dirs,
            app_dir=args.app_dir,
        )

        print("[OK] Teardown and audit completed successfully.")
        print(f"[OK] Deleted Antigravity sessions: {len(results['deleted_sessions']['antigravity'])}")
        print(f"[OK] Deleted Claude Code sessions: {len(results['deleted_sessions']['claude-code'])}")
        print(f"[OK] Deleted temporary plans: {len(results['deleted_plans'])}")
        print(f"[OK] Deleted sandbox directories: {len(results['deleted_sandboxes'])}")
        print(
            f"[OK] Post-cleanup session counts: Antigravity={results['post_counts']['antigravity']}, Claude Code={results['post_counts']['claude-code']}"
        )
        return 0
    except Exception as exc:
        print(f"[ERROR] Teardown failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
