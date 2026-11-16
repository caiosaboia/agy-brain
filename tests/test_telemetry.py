#!/usr/bin/env python3
"""Comprehensive test suite for consumer discovery telemetry module.

Tests deterministic parsing, prompt construction, CLI argument verification,
metric computation, and mock execution without emojis.
"""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from benchmarks.telemetry import (
    DEFAULT_GROUND_TRUTH,
    ConsumerTelemetryResult,
    build_consumer_command,
    build_consumer_prompt,
    compute_discovery_metrics,
    extract_discovered_files,
    find_agy_executable,
    find_brain_executable,
    find_claude_executable,
    normalize_provider,
    parse_session_output,
    run_consumer_session,
)


class TestTelemetryPromptsAndCommands(unittest.TestCase):
    """Test prompt building and CLI command composition."""

    def test_normalize_provider(self):
        self.assertEqual(normalize_provider("antigravity"), "antigravity")
        self.assertEqual(normalize_provider("AGY"), "antigravity")
        self.assertEqual(normalize_provider("google"), "antigravity")
        self.assertEqual(normalize_provider("claude-code"), "claude-code")
        self.assertEqual(normalize_provider("claude"), "claude-code")
        self.assertEqual(normalize_provider("cc"), "claude-code")

    def test_build_consumer_prompt_with_brain(self):
        prompt = build_consumer_prompt("antigravity", use_brain=True, brain_exe="brain.exe")
        self.assertIn("brain.exe plan --all --paths", prompt)
        self.assertIn("teste-plan-*.md", prompt)

    def test_build_consumer_prompt_without_brain(self):
        prompt = build_consumer_prompt("claude-code", use_brain=False)
        self.assertIn("Do NOT use brain", prompt)
        self.assertIn("filesystem directly", prompt)
        self.assertIn("teste-plan-*.md", prompt)

    def test_build_consumer_command_antigravity(self):
        cmd = build_consumer_command(
            "antigravity",
            prompt="test prompt",
            workspace_dir="C:/test/ws",
            binary_path="agy.exe",
        )
        self.assertEqual(cmd[0], "agy.exe")
        self.assertIn("--model", cmd)
        self.assertIn("gemini-3.8-flash-low", cmd)
        self.assertIn("--effort", cmd)
        self.assertIn("low", cmd)
        self.assertIn("--dangerously-skip-permissions", cmd)
        self.assertIn("-p", cmd)
        self.assertIn("test prompt", cmd)
        self.assertIn("--output-format", cmd)
        self.assertIn("json", cmd)

    def test_build_consumer_command_claude_code(self):
        cmd = build_consumer_command(
            "claude-code",
            prompt="test prompt",
            workspace_dir="C:/test/ws",
            binary_path="claude.cmd",
        )
        self.assertEqual(cmd[0], "claude.cmd")
        self.assertIn("--model", cmd)
        self.assertIn("combo-1", cmd)
        self.assertIn("--effort", cmd)
        self.assertIn("low", cmd)
        self.assertIn("--dangerously-skip-permissions", cmd)
        self.assertIn("-n", cmd)
        self.assertIn("teste-consumer", cmd)
        self.assertIn("-p", cmd)
        self.assertIn("test prompt", cmd)
        self.assertIn("--output-format", cmd)
        self.assertIn("json", cmd)


class TestFileExtraction(unittest.TestCase):
    """Test extracting planning filenames from diverse text and output formats."""

    def test_extract_exact_matches(self):
        text = "Discovered: teste-plan-1.md and teste-plan-2.md."
        files = extract_discovered_files(text)
        self.assertEqual(files, {"teste-plan-1.md", "teste-plan-2.md"})

    def test_extract_noise_and_existing_plans(self):
        text = (
            "Found candidates:\n"
            "- C:\\Users\\user\\workspace\\teste-plan-1.md\n"
            "- C:\\Users\\user\\workspace\\teste-plan-2.md\n"
            "- C:\\Users\\user\\other\\teste-plan-ruido.md\n"
            "- C:\\Users\\user\\other\\existing_plan.md\n"
        )
        files = extract_discovered_files(text, include_all_plans=True)
        self.assertEqual(
            files,
            {"teste-plan-1.md", "teste-plan-2.md", "teste-plan-ruido.md", "existing_plan.md"},
        )

    def test_extract_ignores_wildcard_glob(self):
        text = "Searching pattern teste-plan-*.md returned teste-plan-1.md."
        files = extract_discovered_files(text)
        self.assertEqual(files, {"teste-plan-1.md"})
        self.assertNotIn("teste-plan-*.md", files)

    def test_extract_prompt_exclusion(self):
        prompt = "Locate all planning files matching teste-plan-*.md or old_plan.md."
        text = f"{prompt}\nFound files: teste-plan-1.md, teste-plan-2.md, old_plan.md."
        files = extract_discovered_files(text, prompt_to_exclude=prompt, include_all_plans=True)
        self.assertIn("teste-plan-1.md", files)
        self.assertIn("teste-plan-2.md", files)
        self.assertNotIn("old_plan.md", files)


class TestDiscoveryMetrics(unittest.TestCase):
    """Test precision, recall, and F1 calculation against ground truth."""

    def test_perfect_match(self):
        pred = {"teste-plan-1.md", "teste-plan-2.md"}
        gt = {"teste-plan-1.md", "teste-plan-2.md"}
        m = compute_discovery_metrics(pred, gt)
        self.assertEqual(m["precision"], 1.0)
        self.assertEqual(m["recall"], 1.0)
        self.assertEqual(m["f1"], 1.0)
        self.assertEqual(m["true_positives"], ["teste-plan-1.md", "teste-plan-2.md"])
        self.assertEqual(m["false_positives"], [])
        self.assertEqual(m["false_negatives"], [])

    def test_noise_file_false_positive(self):
        pred = {"teste-plan-1.md", "teste-plan-2.md", "teste-plan-ruido.md"}
        gt = {"teste-plan-1.md", "teste-plan-2.md"}
        m = compute_discovery_metrics(pred, gt)
        self.assertAlmostEqual(m["precision"], 2.0 / 3.0, places=4)
        self.assertEqual(m["recall"], 1.0)
        self.assertAlmostEqual(m["f1"], 0.8, places=4)
        self.assertEqual(m["false_positives"], ["teste-plan-ruido.md"])

    def test_missed_file_false_negative(self):
        pred = {"teste-plan-1.md"}
        gt = {"teste-plan-1.md", "teste-plan-2.md"}
        m = compute_discovery_metrics(pred, gt)
        self.assertEqual(m["precision"], 1.0)
        self.assertEqual(m["recall"], 0.5)
        self.assertAlmostEqual(m["f1"], 2.0 * 0.5 / 1.5, places=4)
        self.assertEqual(m["false_negatives"], ["teste-plan-2.md"])

    def test_empty_predictions(self):
        pred = set()
        gt = {"teste-plan-1.md", "teste-plan-2.md"}
        m = compute_discovery_metrics(pred, gt)
        self.assertEqual(m["precision"], 0.0)
        self.assertEqual(m["recall"], 0.0)
        self.assertEqual(m["f1"], 0.0)
        self.assertEqual(m["false_negatives"], ["teste-plan-1.md", "teste-plan-2.md"])

    def test_both_empty(self):
        m = compute_discovery_metrics(set(), set())
        self.assertEqual(m["precision"], 1.0)
        self.assertEqual(m["recall"], 1.0)
        self.assertEqual(m["f1"], 1.0)


class TestParseSessionOutput(unittest.TestCase):
    """Test parsing of JSON, streaming NDJSON, and raw text outputs."""

    def test_parse_antigravity_json(self):
        raw_json = json.dumps({
            "response": "Found files:\n- teste-plan-1.md\n- teste-plan-2.md\n",
            "usageMetadata": {
                "promptTokenCount": 750,
                "candidatesTokenCount": 35,
                "totalTokenCount": 785,
            },
            "turn_count": 2,
            "tool_calls": [
                {"name": "run_command", "args": {"cmd": "brain list"}},
                {"name": "run_command", "args": {"cmd": "brain artifacts 0"}},
            ],
        })

        res = parse_session_output(
            raw_stdout=raw_json,
            wall_clock_latency_ms=125.5,
            provider="antigravity",
            workspace_dir="C:/proj/test",
            use_brain=True,
        )

        self.assertEqual(res.provider, "antigravity")
        self.assertTrue(res.use_brain)
        self.assertEqual(res.turns, 2)
        self.assertEqual(res.tool_calls, 2)
        self.assertEqual(res.input_tokens, 750)
        self.assertEqual(res.output_tokens, 35)
        self.assertEqual(res.total_tokens, 785)
        self.assertFalse(res.tokens_estimated)
        self.assertEqual(res.files_found, ["teste-plan-1.md", "teste-plan-2.md"])
        self.assertEqual(res.precision, 1.0)
        self.assertEqual(res.recall, 1.0)
        self.assertEqual(res.f1, 1.0)

    def test_parse_claude_code_json(self):
        raw_json = json.dumps({
            "type": "result",
            "subtype": "success",
            "result": "Discovered files: teste-plan-1.md, teste-plan-2.md, teste-plan-ruido.md",
            "num_turns": 3,
            "usage": {
                "input_tokens": 1500,
                "output_tokens": 65,
                "cache_creation_input_tokens": 100,
                "cache_read_input_tokens": 50,
            },
            "tool_calls": [
                {"name": "Bash", "input": {"command": "find ."}},
            ],
        })

        res = parse_session_output(
            raw_stdout=raw_json,
            wall_clock_latency_ms=210.0,
            provider="claude-code",
            workspace_dir="C:/proj/test",
            use_brain=False,
        )

        self.assertEqual(res.provider, "claude-code")
        self.assertFalse(res.use_brain)
        self.assertEqual(res.turns, 3)
        self.assertEqual(res.tool_calls, 1)
        self.assertEqual(res.input_tokens, 1650)
        self.assertEqual(res.output_tokens, 65)
        self.assertEqual(res.total_tokens, 1715)
        self.assertFalse(res.tokens_estimated)
        self.assertIn("teste-plan-ruido.md", res.files_found)
        self.assertEqual(res.false_positives, ["teste-plan-ruido.md"])
        self.assertAlmostEqual(res.precision, 2.0 / 3.0, places=4)
        self.assertEqual(res.recall, 1.0)
        self.assertAlmostEqual(res.f1, 0.8, places=4)

    def test_parse_ndjson_streaming(self):
        ndjson_lines = "\n".join([
            json.dumps({"type": "turn", "step": 1}),
            json.dumps({"type": "tool_use", "name": "Bash", "input": {"command": "brain list"}}),
            json.dumps({"type": "tool_use", "name": "Bash", "input": {"command": "brain artifacts 0"}}),
            json.dumps({
                "type": "result",
                "result": "Discovered teste-plan-1.md and teste-plan-2.md",
                "usage": {"input_tokens": 800, "output_tokens": 40},
            }),
        ])

        res = parse_session_output(
            raw_stdout=ndjson_lines,
            wall_clock_latency_ms=95.0,
            provider="claude-code",
        )

        self.assertEqual(res.tool_calls, 2)
        self.assertGreaterEqual(res.turns, 1)
        self.assertEqual(res.input_tokens, 800)
        self.assertEqual(res.output_tokens, 40)
        self.assertEqual(res.total_tokens, 840)
        self.assertFalse(res.tokens_estimated)
        self.assertEqual(res.files_found, ["teste-plan-1.md", "teste-plan-2.md"])

    def test_parse_text_fallback_with_token_estimation(self):
        text_out = "Found files: teste-plan-1.md and teste-plan-2.md directly on disk."
        prompt = "Locate all planning files matching teste-plan-*.md"

        res = parse_session_output(
            raw_stdout=text_out,
            wall_clock_latency_ms=50.0,
            prompt=prompt,
            provider="antigravity",
        )

        self.assertTrue(res.tokens_estimated)
        self.assertGreater(res.input_tokens, 0)
        self.assertGreater(res.output_tokens, 0)
        self.assertEqual(res.total_tokens, res.input_tokens + res.output_tokens)
        self.assertEqual(res.files_found, ["teste-plan-1.md", "teste-plan-2.md"])
        self.assertEqual(res.precision, 1.0)
        self.assertEqual(res.recall, 1.0)


class TestMockSessionExecution(unittest.TestCase):
    """Test full session execution pipeline with mock responses."""

    def test_run_consumer_session_mock_antigravity_with_brain(self):
        res = run_consumer_session(
            provider="antigravity",
            workspace_dir="C:/test/workspace",
            use_brain=True,
            mock=True,
        )

        self.assertEqual(res.provider, "antigravity")
        self.assertTrue(res.use_brain)
        self.assertEqual(res.precision, 1.0)
        self.assertEqual(res.recall, 1.0)
        self.assertEqual(res.f1, 1.0)
        self.assertEqual(res.false_positives, [])
        self.assertEqual(res.tool_calls, 2)
        self.assertFalse(res.tokens_estimated)

    def test_run_consumer_session_mock_antigravity_without_brain(self):
        res = run_consumer_session(
            provider="antigravity",
            workspace_dir="C:/test/workspace",
            use_brain=False,
            mock=True,
        )

        self.assertEqual(res.provider, "antigravity")
        self.assertFalse(res.use_brain)
        self.assertIn("teste-plan-ruido.md", res.false_positives)
        self.assertIn("existing_plan.md", res.false_positives)
        self.assertLess(res.precision, 1.0)
        self.assertLess(res.f1, 1.0)
        self.assertGreaterEqual(res.tool_calls, 4)

    def test_run_consumer_session_mock_claude_with_brain(self):
        res = run_consumer_session(
            provider="claude-code",
            workspace_dir="C:/test/workspace",
            use_brain=True,
            mock=True,
        )

        self.assertEqual(res.provider, "claude-code")
        self.assertTrue(res.use_brain)
        self.assertEqual(res.precision, 1.0)
        self.assertEqual(res.recall, 1.0)
        self.assertEqual(res.f1, 1.0)

    def test_run_consumer_session_mock_claude_without_brain(self):
        res = run_consumer_session(
            provider="claude-code",
            workspace_dir="C:/test/workspace",
            use_brain=False,
            mock=True,
        )

        self.assertEqual(res.provider, "claude-code")
        self.assertFalse(res.use_brain)
        self.assertIn("teste-plan-ruido.md", res.false_positives)
        self.assertAlmostEqual(res.precision, 2.0 / 3.0, places=4)
        self.assertEqual(res.recall, 1.0)
        self.assertAlmostEqual(res.f1, 0.8, places=4)

    def test_serialization(self):
        res = run_consumer_session(
            provider="antigravity",
            workspace_dir="C:/test/workspace",
            use_brain=True,
            mock=True,
        )
        d = res.to_dict()
        self.assertIsInstance(d, dict)
        self.assertEqual(d["provider"], "antigravity")
        self.assertEqual(d["f1"], 1.0)

        j = res.to_json()
        loaded = json.loads(j)
        self.assertEqual(loaded["provider"], "antigravity")
        self.assertEqual(loaded["f1"], 1.0)


class TestCliStandaloneExecution(unittest.TestCase):
    """Test running telemetry.py via subprocess CLI."""

    def test_cli_help(self):
        proc = subprocess.run(
            [sys.executable, "benchmarks/telemetry.py", "--help"],
            capture_output=True,
            text=True,
            check=True,
        )
        self.assertIn("Consumer session execution and telemetry parsing module", proc.stdout)
        self.assertIn("--provider", proc.stdout)
        self.assertIn("--use-brain", proc.stdout)
        self.assertIn("--no-brain", proc.stdout)

    def test_cli_mock_json_out(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            out_json = Path(tmpdir) / "output.json"
            proc = subprocess.run(
                [
                    sys.executable,
                    "benchmarks/telemetry.py",
                    "--mock",
                    "--provider", "antigravity",
                    "--use-brain",
                    "--format", "json",
                    "--json-out", str(out_json),
                ],
                capture_output=True,
                text=True,
                check=True,
            )
            self.assertTrue(out_json.is_file())
            data = json.loads(out_json.read_text(encoding="utf-8"))
            self.assertEqual(data["provider"], "antigravity")
            self.assertTrue(data["use_brain"])
            self.assertEqual(data["f1"], 1.0)

    def test_cli_parse_file(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            sample_file = Path(tmpdir) / "sample_session.json"
            sample_file.write_text(
                json.dumps({
                    "result": "Discovered files: teste-plan-1.md, teste-plan-2.md",
                    "num_turns": 1,
                    "usage": {"input_tokens": 500, "output_tokens": 30},
                }),
                encoding="utf-8",
            )

            proc = subprocess.run(
                [
                    sys.executable,
                    "benchmarks/telemetry.py",
                    "--parse-file", str(sample_file),
                    "--provider", "claude-code",
                    "--format", "json",
                ],
                capture_output=True,
                text=True,
                check=True,
            )
            data = json.loads(proc.stdout)
            self.assertEqual(data["files_found"], ["teste-plan-1.md", "teste-plan-2.md"])
            self.assertEqual(data["f1"], 1.0)


if __name__ == "__main__":
    unittest.main()
