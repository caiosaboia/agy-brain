use std::fs;
use std::path::PathBuf;

pub const BASH_INTEGRATION: &str = r#"# agy-brain shell integration for Bash
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
    local target
    target="$(command agy-brain)"
    if [ -n "$target" ] && [ -d "$target" ]; then
      cd "$target" || return
      return 0
    fi
    return 0
  fi
  command agy-brain "$@"
}
alias cd-brain=agy-brain
alias brain=agy-brain
"#;

pub const ZSH_INTEGRATION: &str = r#"# agy-brain shell integration for Zsh
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
    local target
    target="$(command agy-brain)"
    if [ -n "$target" ] && [ -d "$target" ]; then
      cd "$target" || return
      return 0
    fi
    return 0
  fi
  command agy-brain "$@"
}
alias cd-brain=agy-brain
alias brain=agy-brain
"#;

pub const FISH_INTEGRATION: &str = r#"# agy-brain shell integration for Fish
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
        set -l target (command agy-brain)
        if test -n "$target" -a -d "$target"
            cd "$target"
            return 0
        end
        return 0
    end
    command agy-brain $argv
end
alias cd-brain=agy-brain
alias brain=agy-brain
"#;

pub const POWERSHELL_INTEGRATION: &str = r#"# agy-brain shell integration for PowerShell
function agy-brain {
    [CmdletBinding()]
    param(
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$RemainingArgs
    )

    $exe = (Get-Command agy-brain.exe, agy-brain -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source)
    if (-not $exe) {
        $exe = "agy-brain.exe"
    }

    $sub = if ($RemainingArgs -and $RemainingArgs.Count -ge 1) { $RemainingArgs[0] } else { $null }
    $arg1 = if ($RemainingArgs -and $RemainingArgs.Count -ge 2) { $RemainingArgs[1] } else { $null }

    if ($sub -eq "go") {
        $target = (& $exe resolve $arg1)
        if ($target -and (Test-Path $target)) {
            Set-Location $target
            Write-Host "[OK] Cerebro Antigravity ($target)" -ForegroundColor Green
            return
        }
    }

    if ($sub -and ($sub -match '^[0-9]+$' -or ($sub.Length -ge 30 -and $sub -match '-'))) {
        $target = (& $exe resolve $sub)
        if ($target -and (Test-Path $target)) {
            Set-Location $target
            Write-Host "[OK] Cerebro Antigravity ($target)" -ForegroundColor Green
            return
        }
    }

    if (-not $sub) {
        $target = (& $exe)
        if ($target -and (Test-Path $target)) {
            Set-Location $target
            Write-Host "[OK] Cerebro Antigravity ($target)" -ForegroundColor Green
            return
        }
        return
    }

    & $exe @RemainingArgs
}
Set-Alias -Name cd-brain -Value agy-brain -ErrorAction SilentlyContinue
Set-Alias -Name brain -Value agy-brain -ErrorAction SilentlyContinue
"#;

pub fn get_shell_integration(shell_name: &str) -> Option<&'static str> {
    match shell_name.to_lowercase().trim() {
        "bash" => Some(BASH_INTEGRATION),
        "zsh" => Some(ZSH_INTEGRATION),
        "fish" => Some(FISH_INTEGRATION),
        "powershell" | "pwsh" | "posh" => Some(POWERSHELL_INTEGRATION),
        _ => None,
    }
}

pub fn detect_active_shell_and_rc() -> (&'static str, PathBuf) {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    #[cfg(target_os = "windows")]
    {
        let ps_profile = home
            .join("Documents")
            .join("PowerShell")
            .join("Microsoft.PowerShell_profile.ps1");
        ("powershell", ps_profile)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let shell_env = std::env::var("SHELL").unwrap_or_default();
        if shell_env.contains("zsh") {
            ("zsh", home.join(".zshrc"))
        } else if shell_env.contains("fish") {
            (
                "fish",
                home.join(".config").join("fish").join("config.fish"),
            )
        } else {
            ("bash", home.join(".bashrc"))
        }
    }
}

pub fn install_shell_integration(target_shell: Option<&str>) -> (bool, String) {
    let (shell_name, rc_file) = if let Some(s) = target_shell {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let norm = s.to_lowercase();
        let path = if norm.contains("power") || norm == "pwsh" {
            home.join("Documents")
                .join("PowerShell")
                .join("Microsoft.PowerShell_profile.ps1")
        } else if norm == "zsh" {
            home.join(".zshrc")
        } else if norm == "fish" {
            home.join(".config").join("fish").join("config.fish")
        } else {
            home.join(".bashrc")
        };
        (s.to_string(), path)
    } else {
        let (s, p) = detect_active_shell_and_rc();
        (s.to_string(), p)
    };

    let hook_line = match shell_name.as_str() {
        "powershell" | "pwsh" => "Invoke-Expression (& agy-brain init powershell | Out-String)",
        "fish" => "agy-brain init fish | source",
        _ => "eval \"$(agy-brain init bash)\"",
    };

    if let Some(parent) = rc_file.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let content = fs::read_to_string(&rc_file).unwrap_or_default();
    if content.contains("agy-brain init") || content.contains(hook_line) {
        return (
            true,
            format!(
                "Integracao agy-brain ja esta instalada em: {}",
                rc_file.display()
            ),
        );
    }

    let new_content = format!(
        "{}\n\n# agy-brain shell hook\n{}\n",
        content.trim_end(),
        hook_line
    );
    match fs::write(&rc_file, new_content) {
        Ok(_) => (
            true,
            format!(
                "Integracao instalada com sucesso em: {}\nRecarregue sua shell para ativar.",
                rc_file.display()
            ),
        ),
        Err(e) => (
            false,
            format!("Falha ao escrever em {}: {}", rc_file.display(), e),
        ),
    }
}
