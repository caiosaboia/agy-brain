# agy-brain

[![CI](https://github.com/caiosaboia/agy-brain/actions/workflows/ci.yml/badge.svg)](https://github.com/caiosaboia/agy-brain/actions/workflows/ci.yml)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Pure Rust](https://img.shields.io/badge/Pure%20Rust-100%25-blue.svg)](https://github.com/caiosaboia/agy-brain)
[![Zero External Dependencies](https://img.shields.io/badge/dependencies-0%20runtime-brightgreen.svg)](https://github.com/caiosaboia/agy-brain)

**agy-brain** is a blazing-fast, standalone CLI navigator and session inspector for **Antigravity CLI**, written 100% in Pure Rust. It allows developers to instantly jump to workspace brain directories, inspect generated artifacts and plan files, explore sessions via an embedded terminal user interface (TUI), and manage session pins across Windows, macOS, and Linux.

---

## Highlights

* **100% Pure Rust & Single Binary:** Zero external runtime dependencies. No Python, Node, or system SQLite library needed (bundled C SQLite included).
* **Embedded Interactive TUI:** Built-in dashboard powered by `ratatui` and `crossterm` with instant fuzzy search, side-by-side session inspection, live metrics, and keyboard shortcuts.
* **Smart Auto-Discovery:** Seamlessly discovers Antigravity database files and brain directories across platform-specific locations or custom `$AGY_APP_DIR` paths.
* **Workspace Pinning:** Pin primary sessions per workspace using local `.brain` files or the global pin registry (`workspace_pins.json`).
* **Universal Shell Integration:** Native `cd` navigation support across **PowerShell**, **Bash**, **Zsh**, and **Fish**.
* **Native Desktop Integration:** Open sessions in your platform's default file manager (`explorer.exe`, `open`, `xdg-open`) or preferred editor (`$EDITOR`, `code`, `nvim`, `nano`).

---

## Quick Installation

### Windows (PowerShell)
Run in PowerShell:
```powershell
irm https://raw.githubusercontent.com/caiosaboia/agy-brain/main/install.ps1 | iex
```

### Linux & macOS (Bash / Zsh)
Run in your terminal:
```bash
curl -fsSL https://raw.githubusercontent.com/caiosaboia/agy-brain/main/install.sh | bash
```

### Via Cargo
If you have the Rust toolchain installed:
```bash
cargo install --git https://github.com/caiosaboia/agy-brain.git
```

### Manual Download
Download the pre-compiled binary for your architecture from [GitHub Releases](https://github.com/caiosaboia/agy-brain/releases):
* **Windows (x86_64):** `agy-brain-x86_64-pc-windows-msvc.zip`
* **Linux (x86_64):** `agy-brain-x86_64-unknown-linux-gnu.tar.gz`
* **macOS (Apple Silicon / aarch64):** `agy-brain-aarch64-apple-darwin.tar.gz`
* **macOS (Intel / x86_64):** `agy-brain-x86_64-apple-darwin.tar.gz`

Extract the binary and place it in your `$PATH`.

---

## Shell Setup (Directory Navigation)

To enable automatic directory jumping (`cd`) directly to session brain folders, configure the shell integration hook.

### Automatic Configuration
```bash
agy-brain install
```
*Detects your active shell (PowerShell, Bash, Zsh, or Fish) and appends the hook idempotently to your profile.*

### Manual Configuration

* **PowerShell (`$PROFILE`):**
  ```powershell
  Invoke-Expression (& agy-brain init powershell | Out-String)
  ```

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

After reloading your shell, you can use either `brain` or `cd-brain`.

---

## Usage

### 1. Interactive TUI Mode
Run without arguments to launch the embedded TUI dashboard:
```bash
brain
# or
agy-brain tui
```

**Keyboard Shortcuts in TUI:**
* `Up` / `Down` or `j` / `k`: Navigate session list.
* `Type any query`: Instant fuzzy filter across UUID, title, status, and path.
* `Enter`: Jump (`cd`) into the selected brain directory.
* `Ctrl + O`: Open the brain directory in OS file manager.
* `Ctrl + P`: Pin or unpin the selected session for the current workspace.
* `Ctrl + E`: Open the brain folder in your default code editor.
* `Ctrl + A`: Toggle between sessions of the current workspace and all global sessions.
* `Esc`: Exit dashboard.

---

### 2. Direct Subcommands

* **Navigate directly (`cd`):**
  ```bash
  # Jump to primary/pinned session for the current workspace:
  brain go

  # Jump to session by index (from 'list') or UUID:
  brain go 0
  brain go f6e483f5-5c92-49a2-a3cb-7597a280aa13
  ```

* **List sessions:**
  ```bash
  # List sessions belonging to current project:
  agy-brain list

  # List all sessions across the machine:
  agy-brain list --all
  ```

* **Inspect session details:**
  ```bash
  agy-brain info
  agy-brain info 0
  agy-brain info <UUID>
  ```

* **Inspect generated artifacts:**
  ```bash
  # View plans, markdown files, logs, and scratch scripts:
  agy-brain artifacts
  ```

* **Pin / Unpin workspace session:**
  ```bash
  # Pin current or specified session to the workspace:
  agy-brain pin
  agy-brain pin f6e483f5-5c92-49a2-a3cb-7597a280aa13

  # Remove workspace pin:
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

* **Run Health Diagnostics:**
  ```bash
  agy-brain doctor
  ```

---

## Command Reference

| Command | Alias | Description |
|---|---|---|
| `brain` / `cd-brain` | | Interactive Ratatui TUI dashboard with live preview |
| `agy-brain go [target]` | | Navigate directly into the resolved brain directory |
| `agy-brain list [-a]` | `ls` | Display sessions table with status, steps, and dates |
| `agy-brain info [target]` | `show` | Render session card with detailed statistics |
| `agy-brain artifacts [target]` | `art` | List `.md` plans, screenshots, and scratch files |
| `agy-brain open [target]` | | Open session directory in Explorer / Finder / xdg-open |
| `agy-brain edit [target]` | | Open session directory in code editor |
| `agy-brain pin [id]` | | Pin session as primary for the current workspace |
| `agy-brain unpin` | | Remove pin from current workspace |
| `agy-brain doctor` | | Validate Antigravity DB paths and environment health |
| `agy-brain init <shell>` | | Output shell integration function script |
| `agy-brain install [shell]`| | Automatically append hook to user's shell profile |

---

## Building from Source

```bash
# Clone the repository
git clone https://github.com/caiosaboia/agy-brain.git
cd agy-brain

# Run tests
cargo test

# Run linter
cargo clippy --all-targets -- -D warnings

# Build optimized release binary
cargo build --release
```

The compiled binary will be located at `target/release/agy-brain` (or `agy-brain.exe` on Windows).

---

## License

MIT License &copy; 2026 Caio Saboia.
