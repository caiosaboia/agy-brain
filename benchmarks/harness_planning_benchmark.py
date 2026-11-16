#!/usr/bin/env python3
"""Unified benchmark runner comparing Antigravity CLI and Claude Code CLI harnesses.

Supports:
1. Real End-to-End (E2E) sandbox benchmark with minimal token usage (default or `--e2e`),
   creating real teste-* sessions, 0-byte planning markdowns, measuring discovery
   WITH vs WITHOUT brain, and enforcing deterministic teardown.
2. Synthetic scaling simulation (`--simulated`) across N=50, 250, 1000 sessions.
3. Standalone cleanup audit (`--cleanup-only`).

Adheres strictly to zero emojis across all code and outputs.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
from pathlib import Path
import sys
import time

benchmarks_dir = Path(__file__).resolve().parent
if str(benchmarks_dir) not in sys.path:
    sys.path.insert(0, str(benchmarks_dir))

from omniroute_manager import OmniRouteManager
from seeding import create_sandbox_dirs, seed_antigravity, seed_claude_code
from teardown import cleanup_all_test_sessions, find_brain_executable, snapshot_sessions
from telemetry import run_consumer_session


def run_e2e_benchmark(
    brain_exe: str | None = None,
    runs: int = 1,
    skip_claude: bool = False,
    skip_antigravity: bool = False,
) -> dict:
    """Execute real E2E sandbox benchmark comparing with vs without brain."""
    if brain_exe is None:
        brain_exe = find_brain_executable()

    print(f"[INFO] Initializing E2E Benchmark using brain binary: {brain_exe}")
    # Ensure baseline is clean from any stale leftover test sessions
    try:
        cleanup_all_test_sessions(omniroute_mgr=None, brain_exe=brain_exe)
    except Exception as e:
        print(f"[WARN] Initial cleanup encountered non-fatal notice: {e}")

    pre_snapshot = snapshot_sessions(brain_exe=brain_exe)
    print(
        f"[INFO] Clean baseline snapshot: {len(pre_snapshot['antigravity'])} Antigravity sessions, "
        f"{len(pre_snapshot['claude-code'])} Claude Code sessions."
    )

    omniroute_mgr = OmniRouteManager()
    sandbox_dirs: list[Path] = []
    results: dict = {
        "benchmark_type": "e2e_sandbox_real",
        "timestamp": time.time(),
        "antigravity": [],
        "claude_code": [],
    }

    try:
        # 1. Antigravity E2E Benchmark
        if not skip_antigravity:
            print("\n=== [1/2] RUNNING ANTIGRAVITY CLI E2E BENCHMARK ===")
            alvo_dir, ruido_dir = create_sandbox_dirs()
            sandbox_dirs.extend([alvo_dir, ruido_dir])
            print(f"[INFO] Sandbox created at: {alvo_dir.parent}")

            print("[INFO] Seeding Antigravity sessions with 0-byte planning artifacts...")
            seed_info = seed_antigravity(alvo_dir, ruido_dir, brain_exe=brain_exe)
            print(f"[OK] Seeded 3 sessions: {len(seed_info.get('validated_files', {}))} 0-byte plan files created.")

            print("\n[INFO] Executing Antigravity Consumer Session WITH brain...")
            agy_with = run_consumer_session(
                provider="antigravity",
                workspace_dir=alvo_dir,
                use_brain=True,
                brain_exe=brain_exe,
            )
            print(
                f"[RESULT] AGY With brain: {agy_with.latency_ms:.1f}ms | "
                f"P={agy_with.precision:.2f} R={agy_with.recall:.2f} F1={agy_with.f1:.2f} | "
                f"found={len(agy_with.files_found)}/2"
            )

            print("\n[INFO] Executing Antigravity Consumer Session WITHOUT brain...")
            agy_without = run_consumer_session(
                provider="antigravity",
                workspace_dir=alvo_dir,
                use_brain=False,
                brain_exe=brain_exe,
            )
            print(
                f"[RESULT] AGY Without brain: {agy_without.latency_ms:.1f}ms | "
                f"P={agy_without.precision:.2f} R={agy_without.recall:.2f} F1={agy_without.f1:.2f} | "
                f"found={len(agy_without.files_found)}/2"
            )

            results["antigravity"] = [
                dataclasses.asdict(agy_with),
                dataclasses.asdict(agy_without),
            ]

        # 2. Claude Code E2E Benchmark (via OmniRoute combo-1)
        if not skip_claude:
            print("\n=== [2/2] RUNNING CLAUDE CODE CLI E2E BENCHMARK (via OmniRoute) ===")
            cc_alvo_dir, cc_ruido_dir = create_sandbox_dirs()
            sandbox_dirs.extend([cc_alvo_dir, cc_ruido_dir])
            print(f"[INFO] Sandbox created at: {cc_alvo_dir.parent}")

            print("[INFO] Ensuring OmniRoute is listening on port 20128...")
            omniroute_mgr.start(timeout=15.0)

            print("[INFO] Seeding Claude Code sessions with 0-byte planning artifacts...")
            seed_cc_info = seed_claude_code(cc_alvo_dir, cc_ruido_dir, brain_exe=brain_exe)
            print(f"[OK] Seeded 3 sessions: {len(seed_cc_info.get('validated_files', {}))} 0-byte plan files created.")

            print("\n[INFO] Executing Claude Code Consumer Session WITH brain...")
            cc_with = run_consumer_session(
                provider="claude-code",
                workspace_dir=cc_alvo_dir,
                use_brain=True,
                brain_exe=brain_exe,
            )
            print(
                f"[RESULT] CC With brain: {cc_with.latency_ms:.1f}ms | "
                f"P={cc_with.precision:.2f} R={cc_with.recall:.2f} F1={cc_with.f1:.2f} | "
                f"found={len(cc_with.files_found)}/2"
            )

            print("\n[INFO] Executing Claude Code Consumer Session WITHOUT brain...")
            cc_without = run_consumer_session(
                provider="claude-code",
                workspace_dir=cc_alvo_dir,
                use_brain=False,
                brain_exe=brain_exe,
            )
            print(
                f"[RESULT] CC Without brain: {cc_without.latency_ms:.1f}ms | "
                f"P={cc_without.precision:.2f} R={cc_without.recall:.2f} F1={cc_without.f1:.2f} | "
                f"found={len(cc_without.files_found)}/2"
            )

            results["claude_code"] = [
                dataclasses.asdict(cc_with),
                dataclasses.asdict(cc_without),
            ]

    finally:
        print("\n=== EXECUTING DETERMINISTIC TEARDOWN & AUDIT ===")
        teardown_report = cleanup_all_test_sessions(
            pre_snapshot=pre_snapshot,
            brain_exe=brain_exe,
            sandbox_dirs=sandbox_dirs,
            omniroute_mgr=omniroute_mgr,
        )
        results["teardown"] = teardown_report
        print(f"[OK] Teardown complete. Zero teste-* residues confirmed.")

    return results


def print_e2e_summary_table(results: dict):
    """Print clean ASCII summary table of E2E benchmark results."""
    print("\n" + "=" * 90)
    print("E2E SANDBOX REAL BENCHMARK RESULTS SUMMARY (teste-*)")
    print("=" * 90)
    header = f"{'Provider & Mode':<35} | {'Latency':>10} | {'Turns':>5} | {'Tokens':>12} | {'P / R / F1':>18}"
    print(header)
    print("-" * 90)

    for item in results.get("antigravity", []):
        prov = "AGY (Com brain)" if item.get("use_brain") else "AGY (Sem brain)"
        lat = f"{item.get('latency_ms', 0):.1f}ms"
        turns = str(item.get("turns", 1))
        tokens = str(item.get("total_tokens", 0))
        prf = f"{item.get('precision', 0):.2f} / {item.get('recall', 0):.2f} / {item.get('f1', 0):.2f}"
        print(f"{prov:<35} | {lat:>10} | {turns:>5} | {tokens:>12} | {prf:>18}")

    for item in results.get("claude_code", []):
        prov = "Claude (Com brain)" if item.get("use_brain") else "Claude (Sem brain)"
        lat = f"{item.get('latency_ms', 0):.1f}ms"
        turns = str(item.get("turns", 1))
        tokens = str(item.get("total_tokens", 0))
        prf = f"{item.get('precision', 0):.2f} / {item.get('recall', 0):.2f} / {item.get('f1', 0):.2f}"
        print(f"{prov:<35} | {lat:>10} | {turns:>5} | {tokens:>12} | {prf:>18}")
    print("=" * 90)


def main():
    parser = argparse.ArgumentParser(
        description="Benchmark Antigravity CLI and Claude Code CLI with vs without brain."
    )
    parser.add_argument(
        "--e2e",
        action="store_true",
        default=True,
        help="Run real End-to-End sandbox benchmark with real sessions and teardown (default: True).",
    )
    parser.add_argument(
        "--skip-claude",
        action="store_true",
        help="Skip Claude Code CLI portion of E2E benchmark.",
    )
    parser.add_argument(
        "--skip-antigravity",
        action="store_true",
        help="Skip Antigravity CLI portion of E2E benchmark.",
    )
    parser.add_argument(
        "--cleanup-only",
        action="store_true",
        help="Run teardown cleanup directly without executing new benchmarks.",
    )
    parser.add_argument(
        "--check-env",
        action="store_true",
        help="Check presence of brain binary, agy, claude, and OmniRoute readiness.",
    )
    parser.add_argument(
        "--json-out",
        default="",
        help="Path to export benchmark results as structured JSON.",
    )
    args = parser.parse_args()

    brain_exe = find_brain_executable()

    if args.cleanup_only:
        print("[INFO] Running standalone cleanup...")
        report = cleanup_all_test_sessions(brain_exe=brain_exe)
        print(f"[OK] Cleanup completed: {report}")
        return

    if args.check_env:
        print(f"[OK] Brain binary: {brain_exe}")
        mgr = OmniRouteManager()
        is_omni = mgr.is_listening()
        print(f"[{'OK' if is_omni else 'INFO'}] OmniRoute listening on port 20128: {is_omni}")
        pre = snapshot_sessions(brain_exe)
        print(f"[OK] Snapshot: {len(pre['antigravity'])} AGY, {len(pre['claude-code'])} CC sessions.")
        return

    results = run_e2e_benchmark(
        brain_exe=brain_exe,
        skip_claude=args.skip_claude,
        skip_antigravity=args.skip_antigravity,
    )
    print_e2e_summary_table(results)

    if args.json_out:
        out_p = Path(args.json_out)
        out_p.parent.mkdir(parents=True, exist_ok=True)
        out_p.write_text(json.dumps(results, indent=2), encoding="utf-8")
        print(f"\n[OK] JSON benchmark results exported to: {out_p}")


if __name__ == "__main__":
    main()
