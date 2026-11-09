use std::fs;
use std::path::{Path, PathBuf};

pub const BASH_INTEGRATION: &str = r#"# brain shell integration for Bash
brain() {
  local exe="brain"
  if ! command -v brain >/dev/null 2>&1 && command -v agy-brain >/dev/null 2>&1; then
    exe="agy-brain"
  fi
  if [ "$1" = "go" ]; then
    shift
    local target
    target="$(command "$exe" resolve "$@")"
    if [ -n "$target" ] && [ -d "$target" ]; then
      cd "$target" || return
      return 0
    fi
  elif [ "$#" -eq 1 ] && [[ "$1" =~ ^[0-9]+$|^[a-f0-9-]{30,}$ ]]; then
    local target
    target="$(command "$exe" resolve "$1")"
    if [ -n "$target" ] && [ -d "$target" ]; then
      cd "$target" || return
      return 0
    fi
  elif [ "$#" -eq 0 ]; then
    local tmp
    tmp="$(mktemp 2>/dev/null || mktemp -t brain)"
    command "$exe" --target-file "$tmp"
    if [ -s "$tmp" ]; then
      local target
      target="$(cat "$tmp")"
      rm -f "$tmp"
      if [ -n "$target" ] && [ -d "$target" ]; then
        cd "$target" || return
        return 0
      fi
    fi
    rm -f "$tmp"
    return 0
  fi
  command "$exe" "$@"
}
alias agy-brain=brain
alias cd-brain=brain
"#;

pub const ZSH_INTEGRATION: &str = r#"# brain shell integration for Zsh
brain() {
  local exe="brain"
  if ! command -v brain >/dev/null 2>&1 && command -v agy-brain >/dev/null 2>&1; then
    exe="agy-brain"
  fi
  if [ "$1" = "go" ]; then
    shift
    local target
    target="$(command "$exe" resolve "$@")"
    if [ -n "$target" ] && [ -d "$target" ]; then
      cd "$target" || return
      return 0
    fi
  elif [ "$#" -eq 1 ] && [[ "$1" =~ ^[0-9]+$|^[a-f0-9-]{30,}$ ]]; then
    local target
    target="$(command "$exe" resolve "$1")"
    if [ -n "$target" ] && [ -d "$target" ]; then
      cd "$target" || return
      return 0
    fi
  elif [ "$#" -eq 0 ]; then
    local tmp
    tmp="$(mktemp 2>/dev/null || mktemp -t brain)"
    command "$exe" --target-file "$tmp"
    if [ -s "$tmp" ]; then
      local target
      target="$(cat "$tmp")"
      rm -f "$tmp"
      if [ -n "$target" ] && [ -d "$target" ]; then
        cd "$target" || return
        return 0
      fi
    fi
    rm -f "$tmp"
    return 0
  fi
  command "$exe" "$@"
}
alias agy-brain=brain
alias cd-brain=brain
"#;

pub const FISH_INTEGRATION: &str = r#"# brain shell integration for Fish
function brain --description "Antigravity Brain CLI Navigator"
    set -l exe "brain"
    if not command -v brain >/dev/null 2>&1; and command -v agy-brain >/dev/null 2>&1
        set exe "agy-brain"
    end
    if test (count $argv) -eq 1 -a "$argv[1]" = "go"
        set -l target (command $exe resolve)
        if test -n "$target" -a -d "$target"
            cd "$target"
            return 0
        end
    else if test (count $argv) -eq 2 -a "$argv[1]" = "go"
        set -l target (command $exe resolve "$argv[2]")
        if test -n "$target" -a -d "$target"
            cd "$target"
            return 0
        end
    else if test (count $argv) -eq 0
        set -l tmp (mktemp 2>/dev/null; or mktemp -t brain)
        command $exe --target-file "$tmp"
        if test -s "$tmp"
            set -l target (cat "$tmp")
            rm -f "$tmp"
            if test -n "$target" -a -d "$target"
                cd "$target"
                return 0
            end
        end
        rm -f "$tmp"
        return 0
    end
    command $exe $argv
end
alias agy-brain=brain
alias cd-brain=brain
"#;

pub const POWERSHELL_INTEGRATION: &str = r#"# brain shell integration for PowerShell
function brain {
    [CmdletBinding()]
    param(
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$RemainingArgs
    )

    $exe = (Get-Command brain.exe, brain, agy-brain.exe, agy-brain -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source)
    if (-not $exe) {
        $exe = "brain.exe"
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
        $tmp = [System.IO.Path]::GetTempFileName()
        try {
            & $exe --target-file $tmp
            if (Test-Path $tmp) {
                $target = (Get-Content -Path $tmp -Raw -ErrorAction SilentlyContinue)
                if ($target) { $target = $target.Trim() }
                if ($target -and (Test-Path $target)) {
                    Set-Location $target
                    Write-Host "[OK] Cerebro Antigravity ($target)" -ForegroundColor Green
                    return
                }
            }
        } finally {
            Remove-Item -Path $tmp -Force -ErrorAction SilentlyContinue
        }
        return
    }

    & $exe @RemainingArgs
}
Set-Alias -Name agy-brain -Value brain -ErrorAction SilentlyContinue
Set-Alias -Name cd-brain -Value brain -ErrorAction SilentlyContinue
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

pub fn detect_active_shell_and_rc_in_home(custom_home: Option<&Path>) -> (&'static str, PathBuf) {
    let home = custom_home
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));

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
    install_shell_integration_in_home(target_shell, None)
}

pub fn install_shell_integration_in_home(
    target_shell: Option<&str>,
    custom_home: Option<&Path>,
) -> (bool, String) {
    let (shell_name, rc_file) = if let Some(s) = target_shell {
        let home = custom_home
            .map(PathBuf::from)
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
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
        let (s, p) = detect_active_shell_and_rc_in_home(custom_home);
        (s.to_string(), p)
    };

    let hook_line = match shell_name.as_str() {
        "powershell" | "pwsh" => "Invoke-Expression (& brain init powershell | Out-String)",
        "fish" => "brain init fish | source",
        _ => "eval \"$(brain init bash)\"",
    };

    if let Some(parent) = rc_file.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let content = fs::read_to_string(&rc_file).unwrap_or_default();
    if content.contains("brain init")
        || content.contains("agy-brain init")
        || content.contains(hook_line)
    {
        return (
            true,
            format!(
                "Integracao brain ja esta instalada em: {}",
                rc_file.display()
            ),
        );
    }

    let new_content = format!(
        "{}\n\n# brain shell hook\n{}\n",
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
