"""
Shell integration generators and automatic installer for Bash, Zsh, Fish and PowerShell.
"""

from __future__ import annotations

import os
import platform
from pathlib import Path

BASH_INTEGRATION = """# agy-brain shell integration for Bash
agy-brain() {
  if [ "$1" = "go" ]; then
    shift
    local target
    target="$(command agy-brain resolve "$@")"
    if [ -n "$target" ] && [ -d "$target" ]; then
      cd "$target" || return
      return 0
    fi
  elif [ "$#" -eq 1 ] && [[ "$1" =~ ^[0-9]+$|^[a-f0-9-]{30,}$ ]]; then
    local target
    target="$(command agy-brain resolve "$1")"
    if [ -n "$target" ] && [ -d "$target" ]; then
      cd "$target" || return
      return 0
    fi
  elif [ "$#" -eq 0 ]; then
    if command -v fzf >/dev/null 2>&1; then
      local selected
      selected="$(command agy-brain fzf-lines | fzf \\
        --delimiter="\\t" \\
        --with-nth="2.." \\
        --preview="command agy-brain preview {1}" \\
        --preview-window="right,62%,border-left" \\
        --header="[Enter] Navegar | [Ctrl+O] Abrir | [Ctrl+P] Fixar | [Ctrl+A] Todos" \\
        --expect="ctrl-o,ctrl-p,enter" \\
        --bind="ctrl-a:reload(command agy-brain fzf-lines --all)")"

      [ -z "$selected" ] && return 0
      local key line id target
      key="$(echo "$selected" | head -n1)"
      line="$(echo "$selected" | tail -n+2)"
      id="$(echo "$line" | cut -f1)"

      if [ "$key" = "ctrl-o" ]; then
        command agy-brain open "$id"
      elif [ "$key" = "ctrl-p" ]; then
        command agy-brain pin "$id"
      else
        target="$(command agy-brain resolve "$id")"
        if [ -n "$target" ] && [ -d "$target" ]; then
          cd "$target" || return
        fi
      fi
      return 0
    fi
  fi
  command agy-brain "$@"
}
alias cd-brain=agy-brain
"""

ZSH_INTEGRATION = """# agy-brain shell integration for Zsh
agy-brain() {
  if [ "$1" = "go" ]; then
    shift
    local target
    target="$(command agy-brain resolve "$@")"
    if [ -n "$target" ] && [ -d "$target" ]; then
      cd "$target" || return
      return 0
    fi
  elif [ "$#" -eq 1 ] && [[ "$1" =~ ^[0-9]+$|^[a-f0-9-]{30,}$ ]]; then
    local target
    target="$(command agy-brain resolve "$1")"
    if [ -n "$target" ] && [ -d "$target" ]; then
      cd "$target" || return
      return 0
    fi
  elif [ "$#" -eq 0 ]; then
    if command -v fzf >/dev/null 2>&1; then
      local selected
      selected="$(command agy-brain fzf-lines | fzf \\
        --delimiter="\\t" \\
        --with-nth="2.." \\
        --preview="command agy-brain preview {1}" \\
        --preview-window="right,62%,border-left" \\
        --header="[Enter] Navegar | [Ctrl+O] Abrir | [Ctrl+P] Fixar | [Ctrl+A] Todos" \\
        --expect="ctrl-o,ctrl-p,enter" \\
        --bind="ctrl-a:reload(command agy-brain fzf-lines --all)")"

      [ -z "$selected" ] && return 0
      local key line id target
      key="$(echo "$selected" | head -n1)"
      line="$(echo "$selected" | tail -n+2)"
      id="$(echo "$line" | cut -f1)"

      if [ "$key" = "ctrl-o" ]; then
        command agy-brain open "$id"
      elif [ "$key" = "ctrl-p" ]; then
        command agy-brain pin "$id"
      else
        target="$(command agy-brain resolve "$id")"
        if [ -n "$target" ] && [ -d "$target" ]; then
          cd "$target" || return
        fi
      fi
      return 0
    fi
  fi
  command agy-brain "$@"
}
alias cd-brain=agy-brain
"""

FISH_INTEGRATION = """# agy-brain shell integration for Fish
function agy-brain --description "Antigravity Brain CLI Navigator"
    if test (count $argv) -eq 1 -a "$argv[1]" = "go"
        set -l target (command agy-brain resolve)
        if test -n "$target" -a -d "$target"
            cd "$target"
            return 0
        end
    else if test (count $argv) -eq 2 -a "$argv[1]" = "go"
        set -l target (command agy-brain resolve "$argv[2]")
        if test -n "$target" -a -d "$target"
            cd "$target"
            return 0
        end
    else if test (count $argv) -eq 0
        if type -q fzf
            set -l selected (command agy-brain fzf-lines | fzf \\
                --delimiter="\\t" \\
                --with-nth="2.." \\
                --preview="command agy-brain preview {1}" \\
                --preview-window="right,62%,border-left" \\
                --header="[Enter] Navegar | [Ctrl+O] Abrir | [Ctrl+P] Fixar | [Ctrl+A] Todos" \\
                --expect="ctrl-o,ctrl-p,enter" \\
                --bind="ctrl-a:reload(command agy-brain fzf-lines --all)")

            if test -n "$selected"
                set -l key (echo "$selected" | head -n1)
                set -l line (echo "$selected" | tail -n+2)
                set -l id (echo "$line" | cut -f1)
                if test "$key" = "ctrl-o"
                    command agy-brain open "$id"
                else if test "$key" = "ctrl-p"
                    command agy-brain pin "$id"
                else
                    set -l target (command agy-brain resolve "$id")
                    if test -n "$target" -a -d "$target"
                        cd "$target"
                    end
                end
                return 0
            end
        end
    end
    command agy-brain $argv
end
alias cd-brain=agy-brain
"""

POWERSHELL_INTEGRATION = """# agy-brain shell integration for PowerShell
function agy-brain {
    [CmdletBinding()]
    param(
        [Parameter(Position = 0)]
        [string]$Subcommand,
        [Parameter(Position = 1)]
        [string]$Arg1,
        [Alias("a")]
        [switch]$All,
        [Alias("o")]
        [switch]$Open,
        [Alias("l")]
        [switch]$List,
        [Alias("p")]
        [string]$Pin
    )

    if ($Open) {
        command agy-brain open $Subcommand
        return
    }
    if ($List) {
        command agy-brain list $(if ($All) { "--all" })
        return
    }
    if ($Pin) {
        command agy-brain pin $Pin
        return
    }

    if ($Subcommand -eq "go") {
        $target = (command agy-brain resolve $Arg1)
        if ($target -and (Test-Path $target)) {
            Set-Location $target
            Write-Host "[OK] Cerebro Antigravity ($target)" -ForegroundColor Green
        }
        return
    }

    if ($Subcommand -and ($Subcommand -match "^[0-9]+$" -or ($Subcommand.Length -ge 30 -and $Subcommand -match "-"))) {
        $target = (command agy-brain resolve $Subcommand)
        if ($target -and (Test-Path $target)) {
            Set-Location $target
            Write-Host "[OK] Cerebro Antigravity ($target)" -ForegroundColor Green
        }
        return
    }

    if (-not $Subcommand) {
        if (Get-Command fzf -ErrorAction SilentlyContinue) {
            $fzfOut = command agy-brain fzf-lines $(if ($All) { "--all" }) | fzf `
                --delimiter="`t" `
                --with-nth="2.." `
                --preview="command agy-brain preview {1}" `
                --preview-window="right,62%,border-left" `
                --header="[Enter] Navegar | [Ctrl+O] Abrir | [Ctrl+P] Fixar | [Ctrl+E] Editar | [Ctrl+A] Todos" `
                --expect="ctrl-o,ctrl-p,ctrl-e,enter" `
                --bind="ctrl-a:reload(command agy-brain fzf-lines --all)" `
                --color="header:cyan,info:yellow,pointer:green,marker:green"

            if (-not $fzfOut -or $fzfOut.Count -eq 0) {
                return
            }

            $key = ""
            $selectedLine = ""
            if ($fzfOut.Count -ge 2) {
                $key = $fzfOut[0]
                $selectedLine = $fzfOut[1]
            } else {
                $selectedLine = $fzfOut[0]
            }

            if ([string]::IsNullOrWhiteSpace($selectedLine)) {
                return
            }

            $selectedId = ($selectedLine -split "`t")[0].Trim()

            switch ($key) {
                "ctrl-o" {
                    command agy-brain open $selectedId
                }
                "ctrl-p" {
                    command agy-brain pin $selectedId
                }
                "ctrl-e" {
                    command agy-brain edit $selectedId
                }
                default {
                    $target = (command agy-brain resolve $selectedId)
                    if ($target -and (Test-Path $target)) {
                        Set-Location $target
                        Write-Host "[OK] Cerebro Antigravity ($target)" -ForegroundColor Green
                    }
                }
            }
            return
        }
    }

    command agy-brain @args
}
Set-Alias -Name cd-brain -Value agy-brain -ErrorAction SilentlyContinue
"""


def get_shell_integration(shell_name: str) -> str | None:
    """Returns shell initialization code for the specified shell."""
    s = shell_name.lower().strip()
    if s in ["bash"]:
        return BASH_INTEGRATION
    elif s in ["zsh"]:
        return ZSH_INTEGRATION
    elif s in ["fish"]:
        return FISH_INTEGRATION
    elif s in ["powershell", "pwsh", "posh"]:
        return POWERSHELL_INTEGRATION
    return None


def detect_active_shell_and_rc() -> tuple[str, Path]:
    """Detects active shell and candidate RC file."""
    home = Path.home()
    system = platform.system()

    if system == "Windows":
        # Check PowerShell profile location
        docs = home / "Documents" / "PowerShell"
        ps_profile = docs / "Microsoft.PowerShell_profile.ps1"
        return "powershell", ps_profile

    # Unix-like (macOS / Linux)
    shell_env = os.environ.get("SHELL", "")
    if "zsh" in shell_env:
        return "zsh", home / ".zshrc"
    elif "fish" in shell_env:
        return "fish", home / ".config" / "fish" / "config.fish"
    else:
        return "bash", home / ".bashrc"


def install_shell_integration(target_shell: str | None = None) -> tuple[bool, str]:
    """Automatically installs the init hook into the appropriate shell profile."""
    if target_shell:
        shell_name = target_shell.lower().strip()
        home = Path.home()
        if shell_name in ["powershell", "pwsh"]:
            rc_file = home / "Documents" / "PowerShell" / "Microsoft.PowerShell_profile.ps1"
        elif shell_name == "zsh":
            rc_file = home / ".zshrc"
        elif shell_name == "fish":
            rc_file = home / ".config" / "fish" / "config.fish"
        else:
            rc_file = home / ".bashrc"
    else:
        shell_name, rc_file = detect_active_shell_and_rc()

    if shell_name in ["powershell", "pwsh"]:
        hook_line = 'Invoke-Expression (& agy-brain init powershell | Out-String)'
    elif shell_name == "fish":
        hook_line = 'agy-brain init fish | source'
    else:
        hook_line = f'eval "$(agy-brain init {shell_name})"'

    rc_file.parent.mkdir(parents=True, exist_ok=True)
    content = ""
    if rc_file.is_file():
        try:
            content = rc_file.read_text(encoding="utf-8")
        except Exception:
            pass

    if hook_line in content or "agy-brain init" in content:
        return True, f"Integracao agy-brain ja esta instalada em: {rc_file}"

    new_content = content.rstrip() + f"\n\n# agy-brain shell hook\n{hook_line}\n"
    try:
        rc_file.write_text(new_content, encoding="utf-8")
        return True, f"Integracao instalada com sucesso em: {rc_file}\nRecarregue sua shell para ativar."
    except Exception as e:
        return False, f"Falha ao escrever em {rc_file}: {e}"
