# agy-brain

[![CI](https://github.com/caiosaboia/agy-brain/actions/workflows/ci.yml/badge.svg)](https://github.com/caiosaboia/agy-brain/actions/workflows/ci.yml)
[![Python 3.10+](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Zero Dependencies](https://img.shields.io/badge/dependencies-0%20external-brightgreen.svg)](https://github.com/caiosaboia/agy-brain)

**agy-brain** is an ultra-fast, cross-platform CLI navigator and session inspector for **Antigravity CLI**. It allows you to jump directly to any workspace brain, inspect generated artifacts, preview session statistics in a rich terminal UI, and manage pins across Windows, macOS, and Linux.

---

## Highlights

* **Zero External Dependencies:** Built 100% on Python standard library with native ANSI color styling. Instant installation and sub-millisecond execution.
* **Interactive TUI Dashboard:** Seamless integration with `fzf` featuring a live 62% side preview panel showing session metrics, step counts, and generated artifacts.
* **Smart Auto-Discovery:** Discovers the Antigravity database and brain directories automatically across OS-specific paths, environment variables (`$AGY_APP_DIR`), or custom locations.
* **Pinning System:** Pin primary sessions per workspace using local `.brain` files (tracked or untracked) or the global pin registry.
* **Universal Shell Integration:** Native `cd` support across **Bash**, **Zsh**, **Fish**, and **PowerShell**.
* **Cross-Platform Launchers:** Open brains in your operating system's native file manager (`explorer.exe`, `open`, `xdg-open`) or preferred editor (`$EDITOR`, `nvim`, `code`, `nano`).

---

## Installation

### Via `uv` (Recommended)
```bash
uv tool install agy-brain
```

### Via `pipx`
```bash
pipx install agy-brain
```

### Via `pip`
```bash
pip install agy-brain
```

### From Source
```bash
git clone https://github.com/caiosaboia/agy-brain.git
cd agy-brain
pip install -e .
```

---

## Shell Setup (Automatic Navigation)

To enable directory jumping (`cd`) directly to brain folders, initialize `agy-brain` in your shell.

### Automatic One-Line Setup
```bash
agy-brain install
```
*Detects your active shell (Bash, Zsh, Fish, or PowerShell) and adds the initialization hook to your profile.*

### Manual Setup

* **Bash (`~/.bashrc`):**
  ```bash
  eval "$(agy-brain init bash)"
  ```

* **Zsh (`~/.zshrc`):**
  ```bash
  eval "$(agy-brain init zsh)"
  ```

* **Fish (`~/.config/fish/config.fish`):**
  ```fish
  agy-brain init fish | source
  ```

* **PowerShell (`$PROFILE`):**
  ```powershell
  Invoke-Expression (& agy-brain init powershell | Out-String)
  ```

---

## Usage

### 1. Interactive TUI Mode
Run without arguments to launch the fuzzy finder with live preview:
```bash
agy-brain
# or alias:
cd-brain
```

**Keyboard Shortcuts in `fzf`:**
* `Enter`: Jump (`cd`) into the selected brain directory.
* `Ctrl + O`: Open the brain directory in your OS file manager.
* `Ctrl + P`: Pin the selected session to the current workspace.
* `Ctrl + E`: Open the brain folder in your default code editor (`nvim`, `code`, etc.).
* `Ctrl + A`: Toggle between sessions of the current workspace and all global sessions.

---

### 2. Direct Subcommands

* **Navigate directly (`cd`):**
  ```bash
  # Go to primary/pinned brain of current workspace:
  agy-brain go

  # Go to specific index or UUID:
  agy-brain go 0
  agy-brain go f6e483f5-5c92-49a2-a3cb-7597a280aa13
  ```

* **List sessions:**
  ```bash
  # List sessions for current project:
  agy-brain list

  # List all sessions across the whole machine:
  agy-brain list --all
  ```

* **Inspect session info & metrics:**
  ```bash
  agy-brain info
  agy-brain info 1
  ```

* **List generated artifacts:**
  ```bash
  agy-brain artifacts
  ```

* **Pin / Unpin workspace session:**
  ```bash
  # Pin current or specific brain:
  agy-brain pin
  agy-brain pin f6e483f5-5c92-49a2-a3cb-7597a280aa13

  # Remove pin:
  agy-brain unpin
  ```

* **Open in File Manager:**
  ```bash
  agy-brain open
  ```

* **Open in Code Editor:**
  ```bash
  agy-brain edit
  ```

* **Run Diagnostics:**
  ```bash
  agy-brain doctor
  ```

---

## Command Reference

| Command | Alias | Description |
|---|---|---|
| `agy-brain` | `cd-brain` | Interactive `fzf` dashboard with live preview |
| `agy-brain go [target]` | | Navigate directly to the resolved brain folder |
| `agy-brain list [-a]` | `ls` | Display sessions table with status, steps, and dates |
| `agy-brain info [target]` | `show` | Render session card with detailed stats |
| `agy-brain artifacts [target]` | `art` | List `.md` plans, `.png` screenshots, and scratch scripts |
| `agy-brain open [target]` | | Open brain folder in Explorer / Finder / xdg-open |
| `agy-brain edit [target]` | | Open brain folder in code editor (`nvim`, `code`, etc.) |
| `agy-brain pin [id]` | | Pin session as primary for current workspace |
| `agy-brain unpin` | | Remove pin from current workspace |
| `agy-brain doctor` | | Check Antigravity paths, SQLite DB health, and tools |
| `agy-brain init <shell>` | | Generate shell integration hook (`bash`, `zsh`, `fish`, `pwsh`) |
| `agy-brain install` | | Automatically configure active shell profile |

---

## Development & Testing

```bash
# Install development dependencies
uv pip install -e ".[dev]"

# Run tests
pytest -v

# Run linter
ruff check .
```

---

## License

MIT License &copy; 2026 Caio Saboia.
