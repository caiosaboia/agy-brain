#!/usr/bin/env python3
"""Consumer session execution and telemetry parsing module.

Compares Antigravity (agy.exe) and Claude Code CLI sessions with brain
versus without brain when discovering planning Markdown artifacts.
Parses JSON and text output streams to extract:
- Wall-clock latency (ms)
- Turns and tool call counts
- Input, output, and total token usage (or deterministic estimation)
- Discovered matching files
- Precision, recall, and F1 metrics against ground truth.

Adheres strictly to zero emojis.
"""

from __future__ import annotations

import argparse
import dataclasses
from dataclasses import dataclass, field
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
from typing import Any

DEFAULT_GROUND_TRUTH: frozenset[str] = frozenset({"teste-plan-1.md", "teste-plan-2.md"})
DEFAULT_AGY_MODEL: str = "gemini-3.8-flash-low"
DEFAULT_AGY_EFFORT: str = "low"
DEFAULT_CLAUDE_MODEL: str = "combo-1"
DEFAULT_CLAUDE_EFFORT: str = "low"
DEFAULT_CLAUDE_SESSION_NAME: str = "teste-consumer"

ANSI_ESCAPE_RE = re.compile(r"\x1b\[[0-9;]*[a-zA-Z]")
PLAN_FILE_RE = re.compile(r"\b([a-zA-Z0-9_\-]*plan[a-zA-Z0-9_\-]*\.md)\b", re.IGNORECASE)
TESTE_PLAN_EXACT_RE = re.compile(r"\b(teste-plan-[a-zA-Z0-9_\-]+\.md)\b", re.IGNORECASE)


def strip_ansi(text: str) -> str:
    """Strip ANSI escape sequences from text for deterministic parsing."""
    return ANSI_ESCAPE_RE.sub("", text)


def find_brain_executable(custom_path: str | Path | None = None) -> str:
    """Locate the brain executable or return fallback command."""
    if custom_path:
        p = Path(custom_path).resolve()
        if p.is_file():
            return str(p)

    repo_root = Path(__file__).resolve().parent.parent
    candidates = [
        repo_root / "target" / "release" / "brain.exe",
        repo_root / "target" / "release" / "brain",
        repo_root / "target" / "debug" / "brain.exe",
        repo_root / "target" / "debug" / "brain",
        Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")) / "bin" / "brain.exe",
        Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")) / "bin" / "brain",
    ]
    for cand in candidates:
        if cand.is_file():
            return str(cand)

    which_brain = shutil.which("brain") or shutil.which("brain.exe")
    if which_brain:
        return which_brain

    return "brain"


def find_agy_executable(custom_path: str | Path | None = None) -> str:
    """Locate the Antigravity (agy.exe) executable or return fallback command."""
    if custom_path:
        p = Path(custom_path).resolve()
        if p.is_file():
            return str(p)

    which_agy = shutil.which("agy.exe") or shutil.which("agy")
    if which_agy:
        return which_agy

    local_app_data = os.environ.get("LOCALAPPDATA")
    if local_app_data:
        cand = Path(local_app_data) / "agy" / "bin" / "agy.exe"
        if cand.is_file():
            return str(cand)

    cand_home = Path.home() / ".local" / "bin" / "agy"
    if cand_home.is_file():
        return str(cand_home)

    return "agy.exe"


def find_claude_executable(custom_path: str | Path | None = None) -> str:
    """Locate the Claude Code executable or return fallback command."""
    if custom_path:
        p = Path(custom_path).resolve()
        if p.is_file():
            return str(p)

    which_claude = shutil.which("claude.cmd") or shutil.which("claude") or shutil.which("claude.exe")
    if which_claude:
        return which_claude

    app_data = os.environ.get("APPDATA")
    if app_data:
        cand = Path(app_data) / "npm" / "claude.cmd"
        if cand.is_file():
            return str(cand)

    cand_home = Path.home() / ".local" / "bin" / "claude"
    if cand_home.is_file():
        return str(cand_home)

    return "claude"


def normalize_provider(provider: str) -> str:
    """Normalize provider name into antigravity or claude-code."""
    lowered = provider.strip().lower()
    if lowered in {"antigravity", "agy", "google", "gemini"}:
        return "antigravity"
    if lowered in {"claude-code", "claude", "cc", "anthropic"}:
        return "claude-code"
    return lowered


def build_consumer_prompt(
    provider: str,
    use_brain: bool,
    brain_exe: str | Path | None = None,
) -> str:
    """Build the exact benchmark prompt for consumer discovery."""
    brain_bin = str(brain_exe) if brain_exe else "brain"
    # Format binary reference cleanly without escape bugs
    if " " in brain_bin:
        brain_ref = f'"{brain_bin}"'
    else:
        brain_ref = brain_bin

    if use_brain:
        return (
            f"Use the brain CLI ({brain_ref} plan --all --paths) to locate all "
            "planning files matching 'teste-plan-*.md' for the current workspace. "
            "List each discovered filename."
        )
    else:
        return (
            "Do NOT use brain or any brain CLI tool. Find all planning files matching "
            "'teste-plan-*.md' for the current workspace by searching the filesystem directly. "
            "List each discovered filename."
        )


def build_consumer_command(
    provider: str,
    prompt: str,
    workspace_dir: str | Path,
    binary_path: str | Path | None = None,
) -> list[str]:
    """Construct the command line argument list for the provider."""
    norm_provider = normalize_provider(provider)

    if norm_provider == "antigravity":
        bin_cmd = str(binary_path) if binary_path else find_agy_executable()
        return [
            bin_cmd,
            "--model", DEFAULT_AGY_MODEL,
            "--effort", DEFAULT_AGY_EFFORT,
            "--dangerously-skip-permissions",
            "-p", prompt,
            "--output-format", "json",
        ]
    elif norm_provider == "claude-code":
        bin_cmd = str(binary_path) if binary_path else find_claude_executable()
        return [
            bin_cmd,
            "--model", DEFAULT_CLAUDE_MODEL,
            "--effort", DEFAULT_CLAUDE_EFFORT,
            "--dangerously-skip-permissions",
            "-n", DEFAULT_CLAUDE_SESSION_NAME,
            "-p", prompt,
            "--output-format", "json",
        ]
    else:
        raise ValueError(f"Unsupported provider: {provider}")


def extract_discovered_files(
    text: str,
    prompt_to_exclude: str | None = None,
    include_all_plans: bool = True,
) -> set[str]:
    """Extract planning filenames from CLI output text or logs.

    Filters out the literal glob string 'teste-plan-*.md' and any exact prompt echo.
    Matches concrete filenames like 'teste-plan-1.md', 'teste-plan-2.md',
    and noise/false positive candidates like 'teste-plan-ruido.md' or 'existing_plan.md'.
    """
    clean_text = strip_ansi(text)
    discovered: set[str] = set()

    # Determine exclude set from prompt if present
    excluded_names: set[str] = set()
    if prompt_to_exclude:
        for m in PLAN_FILE_RE.finditer(prompt_to_exclude):
            cand = m.group(1).lower().strip()
            if "*" not in cand:
                excluded_names.add(cand)

    pattern = PLAN_FILE_RE if include_all_plans else TESTE_PLAN_EXACT_RE

    for match in pattern.finditer(clean_text):
        fname = match.group(1).lower().strip()
        # Disregard wildcards or invalid tokens
        if "*" in fname or fname.startswith("*"):
            continue
        if fname in excluded_names:
            continue
        discovered.add(fname)

    return discovered


def compute_discovery_metrics(
    predicted_files: set[str] | list[str],
    ground_truth: set[str] | list[str] | None = None,
) -> dict[str, Any]:
    """Compute precision, recall, F1, and confusion sets."""
    if ground_truth is None:
        gt_set = set(DEFAULT_GROUND_TRUTH)
    else:
        gt_set = {f.lower().strip() for f in ground_truth if f and f.strip()}

    pred_set = {f.lower().strip() for f in predicted_files if f and f.strip()}

    tp_set = pred_set & gt_set
    fp_set = pred_set - gt_set
    fn_set = gt_set - pred_set

    if not pred_set and not gt_set:
        precision = 1.0
        recall = 1.0
        f1 = 1.0
    else:
        precision = len(tp_set) / len(pred_set) if pred_set else 0.0
        recall = len(tp_set) / len(gt_set) if gt_set else 0.0
        f1 = (2.0 * precision * recall / (precision + recall)) if (precision + recall) > 0.0 else 0.0

    return {
        "precision": precision,
        "recall": recall,
        "f1": f1,
        "true_positives": sorted(list(tp_set)),
        "false_positives": sorted(list(fp_set)),
        "false_negatives": sorted(list(fn_set)),
        "ground_truth": sorted(list(gt_set)),
        "predicted": sorted(list(pred_set)),
    }


@dataclass
class ConsumerTelemetryResult:
    """Structured telemetry metrics resulting from a consumer discovery session."""

    provider: str
    workspace_dir: str
    use_brain: bool
    latency_ms: float
    turns: int
    tool_calls: int
    input_tokens: int
    output_tokens: int
    total_tokens: int
    tokens_estimated: bool
    files_found: list[str] = field(default_factory=list)
    ground_truth: list[str] = field(default_factory=list)
    precision: float = 0.0
    recall: float = 0.0
    f1: float = 0.0
    true_positives: list[str] = field(default_factory=list)
    false_positives: list[str] = field(default_factory=list)
    false_negatives: list[str] = field(default_factory=list)
    exit_code: int = 0
    timed_out: bool = False
    raw_output: str = ""
    error_message: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert dataclass to standard dictionary."""
        return dataclasses.asdict(self)

    def to_json(self, indent: int = 2) -> str:
        """Serialize metrics to JSON string."""
        return json.dumps(self.to_dict(), indent=indent)

    def format_summary(self) -> str:
        """Format a human-readable telemetry summary table without emojis."""
        lines = [
            "=" * 78,
            "CONSUMER DISCOVERY TELEMETRY RESULT",
            "=" * 78,
            f"Provider:         {self.provider}",
            f"Workspace:        {self.workspace_dir}",
            f"Use Brain:        {self.use_brain}",
            f"Latency:          {self.latency_ms:.2f} ms",
            f"Turns:            {self.turns}",
            f"Tool Calls:       {self.tool_calls}",
            f"Input Tokens:     {self.input_tokens}",
            f"Output Tokens:    {self.output_tokens}",
            f"Total Tokens:     {self.total_tokens} (estimated: {self.tokens_estimated})",
            f"Files Found:      {', '.join(self.files_found) if self.files_found else 'none'}",
            f"Ground Truth:     {', '.join(self.ground_truth)}",
            f"True Positives:   {', '.join(self.true_positives) if self.true_positives else 'none'}",
            f"False Positives:  {', '.join(self.false_positives) if self.false_positives else 'none'}",
            f"False Negatives:  {', '.join(self.false_negatives) if self.false_negatives else 'none'}",
            f"Precision:        {self.precision:.4f}",
            f"Recall:           {self.recall:.4f}",
            f"F1 Score:         {self.f1:.4f}",
            f"Exit Code:        {self.exit_code}",
            f"Timed Out:        {self.timed_out}",
        ]
        if self.error_message:
            lines.append(f"Error Message:    {self.error_message}")
        lines.append("=" * 78)
        return "\n".join(lines)


def _extract_json_payload(raw_text: str) -> tuple[dict[str, Any] | list[dict[str, Any]] | None, str]:
    """Attempt parsing JSON dictionary, list of NDJSON objects, or markdown code block."""
    stripped = raw_text.strip()
    if not stripped:
        return None, ""

    # 1. Direct JSON parse
    try:
        val = json.loads(stripped)
        if isinstance(val, (dict, list)):
            return val, stripped
    except json.JSONDecodeError:
        pass

    # 2. Markdown fenced code block
    m = re.search(r"```(?:json)?\s*([\{\[].*?[\}\]])\s*```", stripped, re.DOTALL)
    if m:
        try:
            val = json.loads(m.group(1))
            if isinstance(val, (dict, list)):
                return val, m.group(1)
        except json.JSONDecodeError:
            pass

    # 3. First brace to last brace substring
    first_brace = stripped.find("{")
    last_brace = stripped.rfind("}")
    if first_brace != -1 and last_brace > first_brace:
        sub = stripped[first_brace : last_brace + 1]
        try:
            val = json.loads(sub)
            if isinstance(val, dict):
                return val, sub
        except json.JSONDecodeError:
            pass

    # 4. NDJSON / streaming JSON lines
    ndjson_items: list[dict[str, Any]] = []
    for line in stripped.splitlines():
        line_s = line.strip()
        if line_s.startswith("{") and line_s.endswith("}"):
            try:
                item = json.loads(line_s)
                if isinstance(item, dict):
                    ndjson_items.append(item)
            except json.JSONDecodeError:
                continue

    if ndjson_items:
        return ndjson_items, stripped

    return None, stripped


def parse_session_output(
    raw_stdout: str,
    raw_stderr: str = "",
    wall_clock_latency_ms: float = 0.0,
    ground_truth: set[str] | list[str] | None = None,
    prompt: str | None = None,
    exit_code: int = 0,
    timed_out: bool = False,
    provider: str = "antigravity",
    workspace_dir: str = "",
    use_brain: bool = True,
    error_message: str | None = None,
) -> ConsumerTelemetryResult:
    """Parse JSON or text outputs from Antigravity or Claude Code sessions."""
    combined_raw = f"{raw_stdout}\n{raw_stderr}".strip()
    json_data, extracted_text = _extract_json_payload(raw_stdout)

    turns = 0
    tool_calls = 0
    input_tokens = 0
    output_tokens = 0
    total_tokens = 0
    tokens_estimated = False
    content_text_blocks: list[str] = [raw_stdout, raw_stderr]

    if isinstance(json_data, dict):
        # Result text
        for key in ("result", "response", "content", "text"):
            if key in json_data and isinstance(json_data[key], str):
                content_text_blocks.append(json_data[key])

        # Candidates structure (Antigravity)
        if "candidates" in json_data and isinstance(json_data["candidates"], list):
            for cand in json_data["candidates"]:
                if isinstance(cand, dict):
                    parts = cand.get("content", {}).get("parts", [])
                    for part in parts:
                        if isinstance(part, dict) and "text" in part:
                            content_text_blocks.append(part["text"])

        # Turns
        for tkey in ("num_turns", "turns", "turn_count", "turnCount"):
            if tkey in json_data:
                try:
                    turns = int(json_data[tkey])
                    break
                except (ValueError, TypeError):
                    pass

        # Tool calls
        if "tool_calls" in json_data:
            tc = json_data["tool_calls"]
            if isinstance(tc, list):
                tool_calls = len(tc)
            elif isinstance(tc, (int, float)):
                tool_calls = int(tc)
        elif "num_tool_calls" in json_data:
            try:
                tool_calls = int(json_data["num_tool_calls"])
            except (ValueError, TypeError):
                pass
        elif "tool_call_count" in json_data:
            try:
                tool_calls = int(json_data["tool_call_count"])
            except (ValueError, TypeError):
                pass

        # Token usage
        if "usage" in json_data and isinstance(json_data["usage"], dict):
            u = json_data["usage"]
            in_tok = int(u.get("input_tokens") or u.get("prompt_tokens") or 0)
            in_tok += int(u.get("cache_creation_input_tokens") or 0)
            in_tok += int(u.get("cache_read_input_tokens") or 0)
            out_tok = int(u.get("output_tokens") or u.get("completion_tokens") or 0)
            tot_tok = int(u.get("total_tokens") or (in_tok + out_tok))
            input_tokens, output_tokens, total_tokens = in_tok, out_tok, tot_tok
        elif "usageMetadata" in json_data and isinstance(json_data["usageMetadata"], dict):
            um = json_data["usageMetadata"]
            in_tok = int(um.get("promptTokenCount") or 0)
            out_tok = int(um.get("candidatesTokenCount") or 0)
            tot_tok = int(um.get("totalTokenCount") or (in_tok + out_tok))
            input_tokens, output_tokens, total_tokens = in_tok, out_tok, tot_tok
        elif "stats" in json_data and isinstance(json_data["stats"], dict) and "tokens" in json_data["stats"]:
            st = json_data["stats"]["tokens"]
            in_tok = int(st.get("input") or st.get("prompt") or 0)
            out_tok = int(st.get("output") or st.get("completion") or 0)
            tot_tok = int(st.get("total") or (in_tok + out_tok))
            input_tokens, output_tokens, total_tokens = in_tok, out_tok, tot_tok

    elif isinstance(json_data, list):
        # NDJSON streaming lines
        for item in json_data:
            itype = item.get("type", "")
            ievent = item.get("event", "")

            if itype in {"tool_use", "tool_call"} or ievent == "tool_call" or "tool_name" in item:
                tool_calls += 1
            if itype in {"turn", "assistant"} or ievent == "turn":
                turns += 1

            for key in ("result", "response", "content", "text"):
                if key in item and isinstance(item[key], str):
                    content_text_blocks.append(item[key])

            if "usage" in item and isinstance(item["usage"], dict):
                u = item["usage"]
                in_tok = int(u.get("input_tokens") or u.get("prompt_tokens") or 0)
                in_tok += int(u.get("cache_creation_input_tokens") or 0)
                in_tok += int(u.get("cache_read_input_tokens") or 0)
                out_tok = int(u.get("output_tokens") or u.get("completion_tokens") or 0)
                tot_tok = int(u.get("total_tokens") or (in_tok + out_tok))
                input_tokens, output_tokens, total_tokens = in_tok, out_tok, tot_tok

    # Fallback turn count
    if turns == 0 and (raw_stdout.strip() or tool_calls > 0):
        turns = max(1, tool_calls)

    # Estimate token usage if CLI did not report tokens
    if total_tokens == 0 and input_tokens == 0 and output_tokens == 0:
        tokens_estimated = True
        prompt_len = len(prompt or "")
        response_len = len("\n".join(content_text_blocks))
        input_tokens = max(1, prompt_len // 4)
        output_tokens = max(1, response_len // 4)
        total_tokens = input_tokens + output_tokens

    # Aggregate all discovered planning files
    combined_search_corpus = "\n".join(content_text_blocks)
    discovered_files = extract_discovered_files(
        combined_search_corpus,
        prompt_to_exclude=prompt,
        include_all_plans=True,
    )

    metrics = compute_discovery_metrics(discovered_files, ground_truth=ground_truth)

    return ConsumerTelemetryResult(
        provider=normalize_provider(provider),
        workspace_dir=str(workspace_dir),
        use_brain=use_brain,
        latency_ms=wall_clock_latency_ms,
        turns=turns,
        tool_calls=tool_calls,
        input_tokens=input_tokens,
        output_tokens=output_tokens,
        total_tokens=total_tokens,
        tokens_estimated=tokens_estimated,
        files_found=metrics["predicted"],
        ground_truth=metrics["ground_truth"],
        precision=metrics["precision"],
        recall=metrics["recall"],
        f1=metrics["f1"],
        true_positives=metrics["true_positives"],
        false_positives=metrics["false_positives"],
        false_negatives=metrics["false_negatives"],
        exit_code=exit_code,
        timed_out=timed_out,
        raw_output=combined_raw,
        error_message=error_message,
    )


def _generate_mock_output(provider: str, use_brain: bool) -> str:
    """Generate deterministic mock CLI JSON outputs for testing."""
    norm = normalize_provider(provider)
    if norm == "antigravity":
        if use_brain:
            payload = {
                "response": (
                    "Discovered planning files via brain artifacts for workspace:\n"
                    "- teste-plan-1.md\n"
                    "- teste-plan-2.md\n"
                ),
                "usageMetadata": {
                    "promptTokenCount": 850,
                    "candidatesTokenCount": 45,
                    "totalTokenCount": 895,
                },
                "turn_count": 2,
                "tool_calls": [
                    {"name": "run_command", "args": {"command": "brain list"}},
                    {"name": "run_command", "args": {"command": "brain artifacts 0"}},
                ],
            }
        else:
            payload = {
                "response": (
                    "Discovered planning files via recursive filesystem search:\n"
                    "- teste-plan-1.md\n"
                    "- teste-plan-2.md\n"
                    "- teste-plan-ruido.md\n"
                    "- existing_plan.md\n"
                ),
                "usageMetadata": {
                    "promptTokenCount": 2400,
                    "candidatesTokenCount": 110,
                    "totalTokenCount": 2510,
                },
                "turn_count": 5,
                "tool_calls": [
                    {"name": "run_command", "args": {"command": "dir /s teste-plan-*.md"}},
                    {"name": "run_command", "args": {"command": "Get-ChildItem -Recurse"}},
                    {"name": "run_command", "args": {"command": "Get-Content teste-plan-1.md"}},
                    {"name": "run_command", "args": {"command": "Get-Content teste-plan-2.md"}},
                    {"name": "run_command", "args": {"command": "Get-Content teste-plan-ruido.md"}},
                ],
            }
        return json.dumps(payload, indent=2)

    elif norm == "claude-code":
        if use_brain:
            payload = {
                "type": "result",
                "subtype": "success",
                "result": (
                    "Discovered planning files via brain:\n"
                    "- teste-plan-1.md\n"
                    "- teste-plan-2.md\n"
                ),
                "num_turns": 2,
                "usage": {
                    "input_tokens": 920,
                    "output_tokens": 52,
                },
                "tool_calls": [
                    {"name": "Bash", "input": {"command": "brain list"}},
                    {"name": "Bash", "input": {"command": "brain artifacts 0"}},
                ],
            }
        else:
            payload = {
                "type": "result",
                "subtype": "success",
                "result": (
                    "Discovered planning files via filesystem scan:\n"
                    "- teste-plan-1.md\n"
                    "- teste-plan-2.md\n"
                    "- teste-plan-ruido.md\n"
                ),
                "num_turns": 4,
                "usage": {
                    "input_tokens": 3100,
                    "output_tokens": 140,
                },
                "tool_calls": [
                    {"name": "Bash", "input": {"command": "find . -name '*plan*.md'"}},
                    {"name": "Bash", "input": {"command": "grep -rn 'teste-plan'"}},
                    {"name": "Bash", "input": {"command": "cat teste-plan-1.md"}},
                    {"name": "Bash", "input": {"command": "cat teste-plan-ruido.md"}},
                ],
            }
        return json.dumps(payload, indent=2)

    raise ValueError(f"Unknown mock provider: {provider}")


def run_consumer_session(
    provider: str,
    workspace_dir: str | Path,
    use_brain: bool = True,
    brain_exe: str | Path | None = None,
    timeout_s: float = 60.0,
    *,
    mock: bool = False,
    mock_output: str | None = None,
    extra_args: list[str] | None = None,
    ground_truth: set[str] | list[str] | None = None,
) -> ConsumerTelemetryResult:
    """Run a consumer discovery session and return parsed telemetry metrics.

    Args:
        provider: 'antigravity' or 'claude-code'.
        workspace_dir: Directory where the session executes.
        use_brain: If True, instructs the agent to use brain CLI; if False,
                   instructs to search filesystem without brain.
        brain_exe: Path or binary name of brain. If None, auto-detected.
        timeout_s: Command timeout limit in seconds.
        mock: If True, simulates deterministic mock execution without invoking CLI.
        mock_output: Custom mock output string for deterministic test assertions.
        extra_args: Additional command line flags passed to the provider binary.
        ground_truth: Expected plan files. Defaults to {'teste-plan-1.md', 'teste-plan-2.md'}.

    Returns:
        ConsumerTelemetryResult containing parsed telemetry and precision/recall/F1.
    """
    ws_path = Path(workspace_dir).resolve()
    if not mock and not ws_path.exists():
        ws_path.mkdir(parents=True, exist_ok=True)

    norm_provider = normalize_provider(provider)
    resolved_brain = find_brain_executable(brain_exe)
    prompt = build_consumer_prompt(norm_provider, use_brain=use_brain, brain_exe=resolved_brain)

    if mock:
        t0 = time.perf_counter()
        # Simulate slight deterministic execution latency
        time.sleep(0.015)
        latency_ms = (time.perf_counter() - t0) * 1000.0
        out_text = mock_output if mock_output is not None else _generate_mock_output(norm_provider, use_brain)
        return parse_session_output(
            raw_stdout=out_text,
            raw_stderr="",
            wall_clock_latency_ms=latency_ms,
            ground_truth=ground_truth,
            prompt=prompt,
            exit_code=0,
            timed_out=False,
            provider=norm_provider,
            workspace_dir=str(ws_path),
            use_brain=use_brain,
        )

    cmd = build_consumer_command(norm_provider, prompt, ws_path)
    if extra_args:
        cmd.extend(extra_args)

    t0 = time.perf_counter()
    exit_code = 0
    timed_out = False
    stdout = ""
    stderr = ""
    error_message: str | None = None

    try:
        proc = subprocess.run(
            cmd,
            cwd=str(ws_path),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout_s,
            shell=(sys.platform == "win32"),
        )
        latency_ms = (time.perf_counter() - t0) * 1000.0
        exit_code = proc.returncode
        stdout = proc.stdout
        stderr = proc.stderr
        if exit_code != 0:
            error_message = f"Process exited with non-zero code {exit_code}"
    except subprocess.TimeoutExpired as exc:
        latency_ms = (time.perf_counter() - t0) * 1000.0
        exit_code = -1
        timed_out = True
        error_message = f"Execution timed out after {timeout_s} seconds"
        stdout = exc.stdout or "" if isinstance(exc.stdout, str) else ""
        stderr = exc.stderr or "" if isinstance(exc.stderr, str) else ""
    except Exception as exc:
        latency_ms = (time.perf_counter() - t0) * 1000.0
        exit_code = 1
        error_message = f"Execution failed: {exc}"

    return parse_session_output(
        raw_stdout=stdout,
        raw_stderr=stderr,
        wall_clock_latency_ms=latency_ms,
        ground_truth=ground_truth,
        prompt=prompt,
        exit_code=exit_code,
        timed_out=timed_out,
        provider=norm_provider,
        workspace_dir=str(ws_path),
        use_brain=use_brain,
        error_message=error_message,
    )


def cli_main() -> None:
    """Standalone CLI entry point for testing and verification."""
    parser = argparse.ArgumentParser(
        description=(
            "Consumer session execution and telemetry parsing module. "
            "Benchmarks Antigravity and Claude Code with brain vs without brain."
        )
    )
    parser.add_argument(
        "--provider",
        choices=["antigravity", "claude-code"],
        default="antigravity",
        help="Provider harness to benchmark (default: antigravity).",
    )
    parser.add_argument(
        "--workspace-dir",
        default=".",
        help="Workspace directory for discovery (default: current directory).",
    )
    brain_group = parser.add_mutually_exclusive_group()
    brain_group.add_argument(
        "--use-brain",
        dest="use_brain",
        action="store_true",
        default=True,
        help="Instruct consumer session to use brain CLI (default).",
    )
    brain_group.add_argument(
        "--no-brain",
        dest="use_brain",
        action="store_false",
        help="Instruct consumer session to search filesystem without brain.",
    )
    parser.add_argument(
        "--brain-exe",
        default=None,
        help="Path to custom brain executable.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=60.0,
        help="Timeout in seconds for session execution (default: 60.0).",
    )
    parser.add_argument(
        "--ground-truth",
        default="teste-plan-1.md,teste-plan-2.md",
        help="Comma-separated expected plan filenames (default: teste-plan-1.md,teste-plan-2.md).",
    )
    parser.add_argument(
        "--mock",
        action="store_true",
        help="Run mock simulation without invoking live CLI.",
    )
    parser.add_argument(
        "--json-out",
        default=None,
        help="Optional file path to export result JSON.",
    )
    parser.add_argument(
        "--parse-file",
        default=None,
        help="Parse an existing output file directly instead of running a session.",
    )
    parser.add_argument(
        "--format",
        choices=["table", "json"],
        default="table",
        help="Output format on stdout (default: table).",
    )

    args = parser.parse_args()

    gt_files = [f.strip() for f in args.ground_truth.split(",") if f.strip()]

    if args.parse_file:
        file_path = Path(args.parse_file)
        if not file_path.is_file():
            sys.stderr.write(f"Error: File not found: {args.parse_file}\n")
            sys.exit(1)
        raw_content = file_path.read_text(encoding="utf-8", errors="replace")
        result = parse_session_output(
            raw_stdout=raw_content,
            ground_truth=gt_files,
            provider=args.provider,
            workspace_dir=args.workspace_dir,
            use_brain=args.use_brain,
        )
    else:
        result = run_consumer_session(
            provider=args.provider,
            workspace_dir=args.workspace_dir,
            use_brain=args.use_brain,
            brain_exe=args.brain_exe,
            timeout_s=args.timeout,
            mock=args.mock,
            ground_truth=gt_files,
        )

    if args.json_out:
        out_path = Path(args.json_out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(result.to_json(indent=2), encoding="utf-8")

    if args.format == "json":
        print(result.to_json(indent=2))
    else:
        print(result.format_summary())


if __name__ == "__main__":
    cli_main()
