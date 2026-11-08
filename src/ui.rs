use crate::core::{truncate_str, ArtifactInfo, BrainStats, SessionSummary};
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const CYAN: &str = "\x1b[96m";
pub const GREEN: &str = "\x1b[92m";
pub const YELLOW: &str = "\x1b[93m";
pub const MAGENTA: &str = "\x1b[95m";
pub const WHITE: &str = "\x1b[97m";
pub const RED: &str = "\x1b[91m";

pub fn render_preview(
    session: Option<&SessionSummary>,
    stats: Option<&BrainStats>,
    brain_id: &str,
) {
    println!("{CYAN}{BOLD}+------------------------------------------------------------------------+{RESET}");
    println!("{CYAN}{BOLD}| ANTIGRAVITY BRAIN INSPECTOR                                            |{RESET}");
    println!("{CYAN}{BOLD}+------------------------------------------------------------------------+{RESET}\n");

    if let Some(s) = session {
        let mut tags = String::new();
        if s.pinned {
            tags.push_str(&format!("{GREEN}{BOLD}[FIXADO]{RESET} "));
        }
        if s.is_subagent {
            tags.push_str(&format!("{MAGENTA}{BOLD}[SUB]{RESET} "));
        }
        println!("{WHITE}{BOLD}SESSAO{RESET}");
        println!("  {DIM}Titulo:{RESET}     {BOLD}{}{RESET} {tags}", s.title);
        println!("  {DIM}ID:{RESET}         {CYAN}{}{RESET}", s.id);
        println!(
            "  {DIM}Passos:{RESET}     {YELLOW}{}{RESET} interacoes",
            s.steps
        );
        if s.is_subagent {
            if !s.agent_name.is_empty() {
                println!("  {DIM}Agente:{RESET}     {MAGENTA}{}{RESET}", s.agent_name);
            }
            if !s.parent_id.is_empty() {
                let p_title = s.parent_title.as_deref().unwrap_or("(sessao pai)");
                println!(
                    "  {DIM}Sessao Pai:{RESET} {CYAN}{}{RESET} ({})",
                    truncate_str(&s.parent_id, 11),
                    p_title
                );
            }
        } else if s.subagent_count > 0 {
            println!(
                "  {DIM}Subagentes:{RESET} {CYAN}{} sessoes filhas{RESET}",
                s.subagent_count
            );
        }
        println!("  {DIM}Status:{RESET}     {}", s.status);
        println!("  {DIM}Modificado:{RESET} {}", s.mtime);
        if !s.primary_workspace.is_empty() {
            println!("  {DIM}Workspace:{RESET}  {}", s.primary_workspace);
        }
    } else {
        println!("{WHITE}{BOLD}SESSAO{RESET}");
        println!("  {DIM}ID:{RESET}         {CYAN}{}{RESET}", brain_id);
    }

    println!();
    if let Some(st) = stats {
        if st.exists {
            println!("{WHITE}{BOLD}ESTATISTICAS DO CEREBRO{RESET}");
            println!(
                "  {DIM}Artefatos MD:{RESET}   {GREEN}{}{RESET} planos/relatorios",
                st.md_count
            );
            println!(
                "  {DIM}Imagens PNG:{RESET}    {YELLOW}{}{RESET} capturas",
                st.png_count
            );
            println!("  {DIM}Scripts Scratch:{RESET}{}", st.scratch_count);
            println!("  {DIM}Caminho Local:{RESET}  {}", st.path.display());
            println!();

            if !st.recent_artifacts.is_empty() {
                println!("{WHITE}{BOLD}ULTIMOS ARTEFATOS GERADOS{RESET}");
                for art in &st.recent_artifacts {
                    let kb = format!("{:.1} KB", art.size as f64 / 1024.0);
                    let dt = if let Some(utc) = DateTime::<Utc>::from_timestamp(art.mtime as i64, 0)
                    {
                        utc.format("%d/%m %H:%M").to_string()
                    } else {
                        "-".to_string()
                    };
                    println!("  * {BOLD}{:<40}{RESET} {DIM}({kb}, {dt}){RESET}", art.name);
                }
            } else {
                println!("{DIM}  Nenhum artefato markdown gerado nesta sessao.{RESET}");
            }
        } else {
            println!("{RED}[!] Pasta do cerebro ainda nao foi criada ou nao contem arquivos no disco.{RESET}");
        }
    } else {
        println!("{RED}[!] Pasta do cerebro nao encontrada no disco.{RESET}");
    }

    println!(
        "\n{CYAN}--------------------------------------------------------------------------{RESET}"
    );
    println!("{DIM}Atalhos: {WHITE}[Enter]{DIM} Navegar | {WHITE}[Ctrl+O]{DIM} Explorer | {WHITE}[Ctrl+P]{DIM} Fixar | {WHITE}[Ctrl+A]{DIM} Todos | {WHITE}[Ctrl+E]{DIM} Editar{RESET}");
}

pub fn render_table(sessions: &[SessionSummary]) {
    if sessions.is_empty() {
        println!("{YELLOW}[*] Nenhuma sessao de cerebro encontrada.{RESET}");
        return;
    }

    println!("\n{CYAN}{BOLD}+----------------------------------------------------------------------------------------------------+{RESET}");
    println!("{CYAN}{BOLD}| SESSOES DO ANTIGRAVITY BRAIN                                                                       |{RESET}");
    println!("{CYAN}{BOLD}+-----+----------+------------------------------------------+--------------+------------+------------+{RESET}");
    println!("{CYAN}{BOLD}| IDX | STATUS   | TITULO DA SESSAO                         | PASSOS       | DATA       | ID         |{RESET}");
    println!("{CYAN}{BOLD}+-----+----------+------------------------------------------+--------------+------------+------------+{RESET}");

    for (i, s) in sessions.iter().enumerate() {
        let tag = if s.pinned {
            format!("{GREEN}[FIXADO]{RESET}")
        } else if s.is_subagent {
            format!("{MAGENTA}[SUB]   {RESET}")
        } else if s.subagent_count > 0 {
            let badge = format!("[+{}sub]", s.subagent_count);
            format!("{CYAN}{badge:<8}{RESET}")
        } else {
            "        ".to_string()
        };
        let title = truncate_str(&s.title, 40);
        let short_id = truncate_str(&s.id, 11);
        let steps_str = format!("{} p.", s.steps);
        let mtime_str = if s.mtime.len() >= 10 {
            &s.mtime[..10]
        } else {
            &s.mtime
        };
        println!(
            "| {i:<3} | {tag} | {title:<40} | {steps_str:<12} | {mtime_str:<10} | {short_id:<10} |"
        );
    }

    println!("{CYAN}{BOLD}+-----+----------+------------------------------------------+--------------+------------+------------+{RESET}\n");
}

pub fn list_artifacts(brain_id: &str, stats: Option<&BrainStats>) {
    let st = match stats {
        Some(s) if s.exists => s,
        _ => {
            println!("{RED}[!] Cerebro {brain_id} nao encontrado no disco.{RESET}");
            return;
        }
    };

    println!("\n{CYAN}{BOLD}+------------------------------------------------------------------------+{RESET}");
    println!(
        "{CYAN}{BOLD}| ARTEFATOS DO CEREBRO: {:<48} |{RESET}",
        if brain_id.len() > 16 {
            format!("{}...", &brain_id[..16])
        } else {
            brain_id.to_string()
        }
    );
    println!("{CYAN}{BOLD}+------------------------------------------------------------------------+{RESET}");
    println!("{DIM}Caminho: {}{RESET}\n", st.path.display());

    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&st.path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let name = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if name.ends_with(".metadata.json") {
                    continue;
                }
                if let Ok(meta) = p.metadata() {
                    let mt = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    files.push(ArtifactInfo {
                        name,
                        size: meta.len(),
                        mtime: mt,
                        category: "root".to_string(),
                    });
                }
            }
        }
    }

    let scratch_dir = st.path.join("scratch");
    if scratch_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&scratch_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let name = format!(
                        "scratch/{}",
                        p.file_name().unwrap_or_default().to_string_lossy()
                    );
                    if let Ok(meta) = p.metadata() {
                        let mt = meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        files.push(ArtifactInfo {
                            name,
                            size: meta.len(),
                            mtime: mt,
                            category: "scratch".to_string(),
                        });
                    }
                }
            }
        }
    }

    files.sort_by_key(|a| std::cmp::Reverse(a.mtime));

    if files.is_empty() {
        println!("{YELLOW}[*] Nenhum artefato de usuario (.md, .png, scratch) gerado nesta sessao.{RESET}\n");
        return;
    }

    println!(
        "{WHITE}{BOLD}{:<48} {:>10}  MODIFICADO{RESET}",
        "ARQUIVO", "TAMANHO"
    );
    println!("{DIM}{:-<48} {:-<10}  {:-<16}{RESET}", "", "", "");
    for art in &files {
        let kb = format!("{:.1} KB", art.size as f64 / 1024.0);
        let dt = if let Some(utc) = DateTime::<Utc>::from_timestamp(art.mtime as i64, 0) {
            utc.format("%Y-%m-%d %H:%M").to_string()
        } else {
            "-".to_string()
        };
        let color = if art.name.ends_with(".md") {
            GREEN
        } else if art.name.ends_with(".png") {
            YELLOW
        } else if art.category == "scratch" {
            MAGENTA
        } else {
            WHITE
        };
        println!("{color}{:<48}{RESET} {DIM}{kb:>10}  {dt}{RESET}", art.name);
    }

    println!(
        "\n{DIM}Total de artefatos visiveis: {}{RESET}\n",
        files.len()
    );
}

pub fn list_artifacts_from_stats(brain_id: &str, stats: Option<&BrainStats>) {
    let st = match stats {
        Some(s) if s.exists => s,
        _ => {
            println!("{RED}[!] Sessao {brain_id} nao encontrada no disco.{RESET}");
            return;
        }
    };

    println!("\n{CYAN}{BOLD}+------------------------------------------------------------------------+{RESET}");
    println!(
        "{CYAN}{BOLD}| ARTEFATOS DA SESSAO: {:<49} |{RESET}",
        if brain_id.len() > 16 {
            format!("{}...", &brain_id[..16])
        } else {
            brain_id.to_string()
        }
    );
    println!("{CYAN}{BOLD}+------------------------------------------------------------------------+{RESET}");
    println!("{DIM}Caminho: {}{RESET}\n", st.path.display());

    if st.recent_artifacts.is_empty() {
        println!("{YELLOW}[*] Nenhum artefato encontrado para esta sessao.{RESET}\n");
        return;
    }

    println!(
        "{WHITE}{BOLD}{:<48} {:>10}  MODIFICADO{RESET}",
        "ARQUIVO", "TAMANHO"
    );
    println!("{DIM}{:-<48} {:-<10}  {:-<16}{RESET}", "", "", "");
    for art in &st.recent_artifacts {
        let kb = format!("{:.1} KB", art.size as f64 / 1024.0);
        let dt = if let Some(utc) = DateTime::<Utc>::from_timestamp(art.mtime as i64, 0) {
            utc.format("%Y-%m-%d %H:%M").to_string()
        } else {
            "-".to_string()
        };
        let color = if art.category == "transcript" {
            GREEN
        } else {
            WHITE
        };
        println!("{color}{:<48}{RESET} {DIM}{kb:>10}  {dt}{RESET}", art.name);
    }

    println!(
        "\n{DIM}Total de artefatos visiveis: {}{RESET}\n",
        st.recent_artifacts.len()
    );
}

pub fn open_in_file_manager(target_path: &Path) -> bool {
    if !target_path.exists() {
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(target_path).spawn().is_ok()
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(target_path).spawn().is_ok()
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(target_path).spawn().is_ok()
    }
}

pub fn open_in_editor(target_path: &Path) -> bool {
    if !target_path.exists() {
        return false;
    }

    let mut editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .ok();

    if editor.is_none() {
        for candidate in ["nvim", "code", "nano", "vim", "notepad"] {
            #[cfg(target_os = "windows")]
            let cmd_check = Command::new("where").arg(candidate).output();
            #[cfg(not(target_os = "windows"))]
            let cmd_check = Command::new("which").arg(candidate).output();

            if let Ok(out) = cmd_check {
                if out.status.success() {
                    editor = Some(candidate.to_string());
                    break;
                }
            }
        }
    }

    match editor {
        Some(ed) => Command::new(ed).arg(target_path).status().is_ok(),
        None => open_in_file_manager(target_path),
    }
}

pub fn resolve_neovim_binary() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("NVIM_PATH") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }

    #[cfg(target_os = "windows")]
    {
        for candidate in ["nvim.exe", "nvim", "vim.exe", "vim"] {
            if let Ok(out) = Command::new("where").arg(candidate).output() {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout);
                    if let Some(first_line) = text.lines().next() {
                        let path = PathBuf::from(first_line.trim());
                        if path.is_file() {
                            return Some(path);
                        }
                    }
                }
            }
        }
        for loc in [
            r"C:\Program Files\Neovim\bin\nvim.exe",
            r"C:\Program Files (x86)\Neovim\bin\nvim.exe",
            r"C:\tools\neovim\bin\nvim.exe",
        ] {
            let pb = PathBuf::from(loc);
            if pb.is_file() {
                return Some(pb);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        for candidate in ["nvim", "vim"] {
            if let Ok(out) = Command::new("which").arg(candidate).output() {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout);
                    if let Some(first_line) = text.lines().next() {
                        let path = PathBuf::from(first_line.trim());
                        if path.is_file() {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn open_in_neovim(target_path: &Path) -> bool {
    if !target_path.exists() {
        return false;
    }

    let nvim_bin = match resolve_neovim_binary() {
        Some(b) => b,
        None => {
            eprintln!("[!] Neovim not found. Falling back to default editor.");
            return open_in_editor(target_path);
        }
    };

    let status = Command::new(&nvim_bin)
        .arg(".")
        .current_dir(target_path)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) => s.success(),
        Err(e) => {
            eprintln!(
                "[!] Failed to execute Neovim ({}): {}",
                nvim_bin.display(),
                e
            );
            false
        }
    }
}
