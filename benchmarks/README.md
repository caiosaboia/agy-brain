# Harness Planning Artifact Benchmark

Deterministic simulation and real End-to-End (E2E) sandbox benchmark measuring how `brain` (`agy-brain`) impacts **Google Antigravity CLI** (`agy.exe`) and **Claude Code CLI** (`claude` via OmniRoute) agent harnesses when discovering planning Markdown artifacts (`*_plan.md`, `teste-plan-*.md`).

Strict adherence to zero emojis across all code, tests, and documentation.

---

## Architecture & Modular Components

The benchmark is decomposed into modular, isolated components under `benchmarks/`:

1. `omniroute_manager.py`:
   - Lifecycle manager for OmniRoute gateway on `127.0.0.1:20128`.
   - Starts `omniroute serve --no-open --no-tray` in background if not already listening.
   - Restores initial daemon state upon teardown.
   - CLI check: `python benchmarks/omniroute_manager.py --check`

2. `seeding.py`:
   - Provisions isolated ephemeral sandbox directories (`teste-ws-alvo` and `teste-ws-ruido`).
   - Seeds 3 sessions per provider (`agy.exe` and `claude`) with strictly 0-byte planning markdowns (`teste-plan-1.md`, `teste-plan-2.md`, `teste-plan-ruido.md`) for minimal token expenditure.
   - CLI test: `python benchmarks/seeding.py --provider antigravity` or `--provider claude-code`

3. `telemetry.py`:
   - Executes real consumer sessions comparing discovery **WITH brain** vs **WITHOUT brain**.
   - Captures wall-clock latency (ms), turns, tool calls, token usage, and evaluates Ground Truth `{teste-plan-1.md, teste-plan-2.md}`.
   - CLI test: `python benchmarks/telemetry.py --help`

4. `teardown.py`:
   - Captures initial session snapshots via `brain list -a -s`.
   - Enforces fail-safe deterministic cleanup in `try...finally`:
     - Invokes `brain --provider <p> delete <uuid> --force` for every created test session.
     - Removes sandbox folders and temporary plans.
     - Stops OmniRoute if started by the benchmark.
     - Audits that 0 `teste-*` sessions remain and session counts match pre-snapshot.
   - CLI cleanup: `python benchmarks/teardown.py --cleanup-only`

5. `harness_planning_benchmark.py`:
   - Unified master runner tying together seeding, telemetry, teardown, and reporting.

---

## Execution Modes

### 1. Real E2E Sandbox Benchmark (Default)
Runs real sessions using `gemini-3.8-flash-low` (`--effort low`) on `agy.exe` and `combo-1` (`--effort low`) on `claude`:
```powershell
python benchmarks/harness_planning_benchmark.py --json-out benchmark_results.json
```

### 2. Standalone Teardown & Audit
Removes any residual test sessions or sandboxes without running benchmarks:
```powershell
python benchmarks/harness_planning_benchmark.py --cleanup-only
```

### 3. Environment & Readiness Pre-Flight
Verifies presence of `brain.exe`, OmniRoute port `20128`, and session snapshot counts:
```powershell
python benchmarks/harness_planning_benchmark.py --check-env
```

---

## Automated Test Suites

```powershell
# Run benchmark unit and integration tests:
python -m unittest discover -s tests -p "test_*.py" -v
python -m unittest discover -s benchmarks -p "test_*.py" -v

# Run Rust crate tests (48 tests in 7 suites):
cargo test --quiet
```
