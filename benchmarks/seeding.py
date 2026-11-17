#!/usr/bin/env python3
"""Sandbox creation and seeding module for Antigravity and Claude Code.

Creates teste-ws-alvo and teste-ws-ruido sandbox workspaces and runs
minimal-token seeding prompts creating 0-byte teste-plan-*.md files.
Strict adherence to zero emojis.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

# Add benchmarks directory to sys.path to allow sibling imports
benchmarks_dir = Path(__file__).resolve().parent
if str(benchmarks_dir) not in sys.path:
    sys.path.insert(0, str(benchmarks_dir))

from omniroute_manager import OmniRouteManager


def find_brain_executable() -> str:
    """Locate the compiled brain binary in target/release, ~/.cargo/bin, or PATH."""
    repo_root = Path(__file__).resolve().parent.parent
    candidates = [
        repo_root / "target" / "release" / "brain.exe",
        repo_root / "target" / "release" / "brain",
        Path.home() / ".cargo" / "bin" / "brain.exe",
        Path.home() / ".cargo" / "bin" / "brain",
        Path(shutil.which("brain") or ""),
    ]
    for cand in candidates:
        if cand and Path(cand).is_file():
            return str(cand)
    raise FileNotFoundError(
        "Could not locate brain executable in target/release, ~/.cargo/bin, or PATH."
    )


def create_sandbox_dirs(base_path: str | Path | None = None) -> tuple[Path, Path]:
    """Create sandbox folders teste-ws-alvo and teste-ws-ruido.

    If base_path is None, a new temporary directory is created.
    Returns (alvo_dir, ruido_dir).
    """
    if base_path is None:
        base_dir = Path(tempfile.mkdtemp(prefix="benchmark_sandbox_"))
    else:
        base_dir = Path(base_path)
        base_dir.mkdir(parents=True, exist_ok=True)

    alvo_dir = (base_dir / "teste-ws-alvo").resolve()
    ruido_dir = (base_dir / "teste-ws-ruido").resolve()

    alvo_dir.mkdir(parents=True, exist_ok=True)
    ruido_dir.mkdir(parents=True, exist_ok=True)

    return alvo_dir, ruido_dir


def validate_zero_byte_files(
    expected_files: list[Path | str],
    enforce_zero_byte: bool = True,
) -> dict[str, int]:
    """Validate that expected files exist and are exactly 0 bytes on disk.

    If enforce_zero_byte is True, any file with non-zero size is truncated
    to 0 bytes to guarantee minimal token consumption in subsequent benchmark runs.
    """
    results: dict[str, int] = {}
    for item in expected_files:
        path = Path(item).resolve()
        if not path.is_file():
            raise FileNotFoundError(f"Expected seeded file does not exist: {path}")

        size = path.stat().st_size
        if size != 0 and enforce_zero_byte:
            path.write_bytes(b"")
            size = path.stat().st_size

        if size != 0:
            raise ValueError(
                f"Validation failed: file {path} has size {size} bytes, expected 0 bytes."
            )

        results[str(path)] = size

    return results


def _run_subprocess_cmd(
    args: list[str],
    cwd: Path | str,
    timeout: float = 60.0,
) -> tuple[int, str, str, float]:
    """Execute a subprocess command with UTF-8/latin-1 decoding resilience."""
    t0 = time.perf_counter()
    use_shell = sys.platform == "win32"
    proc = subprocess.run(
        args,
        cwd=str(cwd),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        shell=use_shell,
        timeout=timeout,
    )
    dt_ms = (time.perf_counter() - t0) * 1000.0

    stdout_str = proc.stdout.decode("utf-8", errors="replace")
    stderr_str = proc.stderr.decode("utf-8", errors="replace")
    return proc.returncode, stdout_str, stderr_str, dt_ms


def seed_antigravity(
    alvo_dir: str | Path,
    ruido_dir: str | Path,
    brain_exe: str | Path | None = None,
) -> dict:
    """Run Antigravity seeding prompts and pin primary session in alvo_dir.

    1. Runs agy.exe creating teste-plan-1.md in alvo_dir.
    2. Runs agy.exe creating teste-plan-2.md in alvo_dir.
    3. Runs agy.exe creating teste-plan-ruido.md in ruido_dir.
    4. Pins the primary session in alvo_dir via brain pin.
    5. Validates that all created teste-plan-*.md files are 0 bytes on disk.
    """
    alvo_path = Path(alvo_dir).resolve()
    ruido_path = Path(ruido_dir).resolve()
    brain_bin = str(brain_exe) if brain_exe else find_brain_executable()

    agy_cmd = shutil.which("agy.exe") or shutil.which("agy") or "agy.exe"

    runs = []

    # Run 1: teste-plan-1.md in alvo_dir
    cmd1 = [
        agy_cmd,
        "--model", "gemini-3.8-flash-low",
        "--effort", "low",
        "--dangerously-skip-permissions",
        "-p", "Crie um arquivo teste-plan-1.md totalmente vazio neste diretorio e termine imediatamente.",
        "--output-format", "json",
    ]
    rc1, out1, err1, dt1 = _run_subprocess_cmd(cmd1, cwd=alvo_path, timeout=120.0)
    runs.append({"cmd": "teste-plan-1.md", "rc": rc1, "dt_ms": dt1, "stdout": out1, "stderr": err1})
    if rc1 != 0:
        raise RuntimeError(f"Antigravity run 1 failed (rc={rc1}): {err1 or out1}")

    # Extract conversation_id from run 1 if available
    conv1_id = None
    try:
        data1 = json.loads(out1)
        conv1_id = data1.get("conversation_id")
    except Exception:
        pass

    # Run 2: teste-plan-2.md in alvo_dir
    cmd2 = [
        agy_cmd,
        "--model", "gemini-3.8-flash-low",
        "--effort", "low",
        "--dangerously-skip-permissions",
        "-p", "Crie um arquivo teste-plan-2.md totalmente vazio neste diretorio e termine imediatamente.",
        "--output-format", "json",
    ]
    rc2, out2, err2, dt2 = _run_subprocess_cmd(cmd2, cwd=alvo_path, timeout=120.0)
    runs.append({"cmd": "teste-plan-2.md", "rc": rc2, "dt_ms": dt2, "stdout": out2, "stderr": err2})
    if rc2 != 0:
        raise RuntimeError(f"Antigravity run 2 failed (rc={rc2}): {err2 or out2}")

    # Run 3: teste-plan-ruido.md in ruido_dir
    cmd3 = [
        agy_cmd,
        "--model", "gemini-3.8-flash-low",
        "--effort", "low",
        "--dangerously-skip-permissions",
        "-p", "Crie um arquivo teste-plan-ruido.md totalmente vazio neste diretorio e termine imediatamente.",
        "--output-format", "json",
    ]
    rc3, out3, err3, dt3 = _run_subprocess_cmd(cmd3, cwd=ruido_path, timeout=120.0)
    runs.append({"cmd": "teste-plan-ruido.md", "rc": rc3, "dt_ms": dt3, "stdout": out3, "stderr": err3})
    if rc3 != 0:
        raise RuntimeError(f"Antigravity run 3 failed (rc={rc3}): {err3 or out3}")

    # Pin primary session in alvo_dir via brain pin
    pin_cmd = [brain_bin, "pin"]
    if conv1_id:
        pin_cmd.append(conv1_id)
    rc_pin, out_pin, err_pin, dt_pin = _run_subprocess_cmd(pin_cmd, cwd=alvo_path, timeout=30.0)
    if rc_pin != 0 and conv1_id:
        # Fallback to brain pin without explicit id
        pin_cmd_fallback = [brain_bin, "pin"]
        rc_pin, out_pin, err_pin, dt_pin = _run_subprocess_cmd(pin_cmd_fallback, cwd=alvo_path, timeout=30.0)

    if rc_pin != 0:
        raise RuntimeError(f"Failed to pin brain session in alvo_dir (rc={rc_pin}): {err_pin or out_pin}")

    # Validate 0-byte files on disk
    expected_files = [
        alvo_path / "teste-plan-1.md",
        alvo_path / "teste-plan-2.md",
        ruido_path / "teste-plan-ruido.md",
    ]
    validation = validate_zero_byte_files(expected_files, enforce_zero_byte=True)

    return {
        "provider": "antigravity",
        "alvo_dir": str(alvo_path),
        "ruido_dir": str(ruido_path),
        "runs": runs,
        "pinned_output": out_pin.strip(),
        "validated_files": validation,
        "status": "SUCCESS",
    }


def seed_claude_code(
    alvo_dir: str | Path,
    ruido_dir: str | Path,
    brain_exe: str | Path | None = None,
) -> dict:
    """Run Claude Code seeding prompts and validate created 0-byte plan files.

    1. Ensures OmniRoute is running on port 20128 for model combo-1.
    2. Runs claude creating teste-plan-1.md in alvo_dir.
    3. Runs claude creating teste-plan-2.md in alvo_dir.
    4. Runs claude creating teste-plan-ruido.md in ruido_dir.
    5. Validates that all created teste-plan-*.md files are 0 bytes on disk.
    """
    alvo_path = Path(alvo_dir).resolve()
    ruido_path = Path(ruido_dir).resolve()
    brain_bin = str(brain_exe) if brain_exe else find_brain_executable()

    claude_cmd = shutil.which("claude.cmd") or shutil.which("claude") or "claude"

    # Ensure OmniRoute is active
    omni = OmniRouteManager()
    if not omni.is_listening():
        omni.start()

    runs = []

    # Run 1: teste-plan-1.md in alvo_dir
    cmd1 = [
        claude_cmd,
        "--model", "combo-1",
        "--effort", "low",
        "--dangerously-skip-permissions",
        "-n", "teste-cc-1",
        "-p", "Crie um arquivo teste-plan-1.md totalmente vazio neste diretorio e termine imediatamente.",
        "--output-format", "json",
    ]
    rc1, out1, err1, dt1 = _run_subprocess_cmd(cmd1, cwd=alvo_path, timeout=120.0)
    runs.append({"cmd": "teste-plan-1.md", "rc": rc1, "dt_ms": dt1, "stdout": out1, "stderr": err1})
    if rc1 != 0:
        raise RuntimeError(f"Claude Code run 1 failed (rc={rc1}): {err1 or out1}")

    # Run 2: teste-plan-2.md in alvo_dir
    cmd2 = [
        claude_cmd,
        "--model", "combo-1",
        "--effort", "low",
        "--dangerously-skip-permissions",
        "-n", "teste-cc-2",
        "-p", "Crie um arquivo teste-plan-2.md totalmente vazio neste diretorio e termine imediatamente.",
        "--output-format", "json",
    ]
    rc2, out2, err2, dt2 = _run_subprocess_cmd(cmd2, cwd=alvo_path, timeout=120.0)
    runs.append({"cmd": "teste-plan-2.md", "rc": rc2, "dt_ms": dt2, "stdout": out2, "stderr": err2})
    if rc2 != 0:
        raise RuntimeError(f"Claude Code run 2 failed (rc={rc2}): {err2 or out2}")

    # Run 3: teste-plan-ruido.md in ruido_dir
    cmd3 = [
        claude_cmd,
        "--model", "combo-1",
        "--effort", "low",
        "--dangerously-skip-permissions",
        "-n", "teste-cc-ruido",
        "-p", "Crie um arquivo teste-plan-ruido.md totalmente vazio neste diretorio e termine imediatamente.",
        "--output-format", "json",
    ]
    rc3, out3, err3, dt3 = _run_subprocess_cmd(cmd3, cwd=ruido_path, timeout=120.0)
    runs.append({"cmd": "teste-plan-ruido.md", "rc": rc3, "dt_ms": dt3, "stdout": out3, "stderr": err3})
    if rc3 != 0:
        raise RuntimeError(f"Claude Code run 3 failed (rc={rc3}): {err3 or out3}")

    # Validate 0-byte files on disk
    expected_files = [
        alvo_path / "teste-plan-1.md",
        alvo_path / "teste-plan-2.md",
        ruido_path / "teste-plan-ruido.md",
    ]
    validation = validate_zero_byte_files(expected_files, enforce_zero_byte=True)

    return {
        "provider": "claude-code",
        "alvo_dir": str(alvo_path),
        "ruido_dir": str(ruido_path),
        "runs": runs,
        "validated_files": validation,
        "status": "SUCCESS",
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Sandbox seeding CLI for Antigravity and Claude Code"
    )
    parser.add_argument(
        "--provider",
        choices=["antigravity", "claude-code", "both"],
        default="antigravity",
        help="Target provider to seed (default: antigravity)",
    )
    parser.add_argument(
        "--base-dir",
        type=str,
        default=None,
        help="Base directory for sandbox folders (default: auto tempdir)",
    )
    args = parser.parse_args()

    alvo_dir, ruido_dir = create_sandbox_dirs(args.base_dir)
    print(f"[INFO] Sandbox directories created:")
    print(f"  Target: {alvo_dir}")
    print(f"  Noise:  {ruido_dir}")

    brain_bin = find_brain_executable()
    print(f"[INFO] Brain executable resolved: {brain_bin}")

    if args.provider in ("antigravity", "both"):
        print("[INFO] Seeding Antigravity sandbox...")
        try:
            res_agy = seed_antigravity(alvo_dir, ruido_dir, brain_bin)
            print("[OK] Antigravity seeded successfully:")
            for path, size in res_agy["validated_files"].items():
                print(f"  File: {path} ({size} bytes)")
            print(f"  Pin output: {res_agy['pinned_output']}")
        except Exception as exc:
            print(f"[FAIL] Antigravity seeding failed: {exc}")
            return 1

    if args.provider in ("claude-code", "both"):
        print("[INFO] Seeding Claude Code sandbox...")
        try:
            res_cc = seed_claude_code(alvo_dir, ruido_dir, brain_bin)
            print("[OK] Claude Code seeded successfully:")
            for path, size in res_cc["validated_files"].items():
                print(f"  File: {path} ({size} bytes)")
        except Exception as exc:
            print(f"[FAIL] Claude Code seeding failed: {exc}")
            return 1

    print("[OK] Seeding process completed successfully.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
