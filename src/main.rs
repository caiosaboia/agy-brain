mod cli;
mod core;
mod shells;
mod tui;
mod ui;

use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cli::{Cli, Commands};
use core::{
    delete_session, fetch_sessions_ext, get_brain_root, get_brain_stats, get_db_path,
    get_default_app_dir, get_pinned_id_for_workspace, prune_sessions, remove_pin, rename_session,
    resolve_brain_target, set_pin,
};
use shells::{get_shell_integration, install_shell_integration};
use tui::{run_interactive_tui, TuiAction};
use ui::{
    list_artifacts, open_in_editor, open_in_file_manager, open_in_neovim, render_preview,
    render_table, CYAN, GREEN, RED, RESET, WHITE, YELLOW,
};

fn run_doctor(app_dir: Option<&Path>) {
    let target_app = app_dir
        .map(PathBuf::from)
        .unwrap_or_else(get_default_app_dir);
    let db_file = get_db_path(Some(&target_app));
    let brain_dir = get_brain_root(Some(&target_app));

    println!(
        "\n{CYAN}+------------------------------------------------------------------------+{RESET}"
    );
    println!(
        "{CYAN}| AGY-BRAIN DOCTOR - DIAGNOSTICO DO SISTEMA (RUST STANDALONE)           |{RESET}"
    );
    println!(
        "{CYAN}+------------------------------------------------------------------------+{RESET}\n"
    );

    if target_app.is_dir() {
        println!(
            "  {GREEN}[OK]{RESET} Diretorio Antigravity: {}",
            target_app.display()
        );
    } else {
        println!(
            "  {RED}[X]{RESET}  Diretorio Antigravity nao encontrado: {}",
            target_app.display()
        );
    }

    if db_file.is_file() {
        println!(
            "  {GREEN}[OK]{RESET} Banco de dados SQLite: {}",
            db_file.display()
        );
        let root_sessions = fetch_sessions_ext(None, true, false, Some(&target_app), None);
        let all_sessions = fetch_sessions_ext(None, true, true, Some(&target_app), None);
        let sub_count = all_sessions.len().saturating_sub(root_sessions.len());
        println!(
            "  {GREEN}[OK]{RESET} Sessoes principais: {} (Subagentes: {})",
            root_sessions.len(),
            sub_count
        );
    } else {
        println!(
            "  {RED}[X]{RESET}  Banco SQLite nao encontrado em: {}",
            db_file.display()
        );
    }

    if brain_dir.is_dir() {
        println!(
            "  {GREEN}[OK]{RESET} Pasta de cerebros: {}",
            brain_dir.display()
        );
    } else {
        println!(
            "  {YELLOW}[!]{RESET} Pasta de cerebros ainda nao existe: {}",
            brain_dir.display()
        );
    }

    let cur_ws = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Some(pin) = get_pinned_id_for_workspace(Some(&cur_ws), Some(&target_app)) {
        println!("  {GREEN}[OK]{RESET} Workspace atual fixado no cerebro: {pin}");
    } else {
        println!(
            "  {WHITE}[i]{RESET} Workspace atual sem fixacao manual (usando resolucao dinamica)"
        );
    }

    println!("\n{CYAN}+------------------------------------------------------------------------+{RESET}\n");
}

fn main() -> ExitCode {
    let args = Cli::parse();
    let app_dir = args.app_dir.as_deref();
    let cur_ws = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    match args.command {
        None | Some(Commands::Tui) => {
            let mut sessions =
                fetch_sessions_ext(Some(&cur_ws), false, args.subagents, app_dir, None);
            if sessions.is_empty() {
                sessions = fetch_sessions_ext(Some(&cur_ws), true, args.subagents, app_dir, None);
            }

            match run_interactive_tui(sessions, &cur_ws, app_dir, args.subagents) {
                Ok(TuiAction::Navigate(path)) => {
                    if let Some(tf) = &args.target_file {
                        let _ = std::fs::write(tf, path.to_string_lossy().as_bytes());
                    }
                    println!("{}", path.display());
                    ExitCode::SUCCESS
                }
                Ok(TuiAction::Exit) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{RED}[!] Erro ao executar TUI: {err}{RESET}");
                    ExitCode::FAILURE
                }
            }
        }

        Some(Commands::Resolve { target }) => {
            if let Some(bid) = resolve_brain_target(target.as_deref(), Some(&cur_ws), app_dir) {
                let p = get_brain_root(app_dir).join(bid);
                println!("{}", p.display());
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }

        Some(Commands::Go { target }) => {
            if let Some(bid) = resolve_brain_target(target.as_deref(), Some(&cur_ws), app_dir) {
                let p = get_brain_root(app_dir).join(bid);
                println!("{}", p.display());
                ExitCode::SUCCESS
            } else {
                eprintln!("{RED}[!] Nenhum cerebro encontrado para resolver.{RESET}");
                ExitCode::FAILURE
            }
        }

        Some(Commands::List { all }) => {
            let sessions = fetch_sessions_ext(Some(&cur_ws), all, args.subagents, app_dir, None);
            render_table(&sessions);
            ExitCode::SUCCESS
        }

        Some(Commands::Info { target }) => {
            let bid = match resolve_brain_target(target.as_deref(), Some(&cur_ws), app_dir) {
                Some(b) => b,
                None => {
                    eprintln!("{RED}[!] Cerebro nao encontrado.{RESET}");
                    return ExitCode::FAILURE;
                }
            };
            let sessions = fetch_sessions_ext(Some(&cur_ws), true, true, app_dir, None);
            let session = sessions.iter().find(|s| s.id == bid);
            let stats = get_brain_stats(&bid, app_dir);
            render_preview(session, stats.as_ref(), &bid);
            ExitCode::SUCCESS
        }

        Some(Commands::Rename { target, title }) => {
            match rename_session(&target, &title, Some(&cur_ws), app_dir, None) {
                Ok((resolved_id, saved_title)) => {
                    println!(
                        "{GREEN}[OK] Sessao {resolved_id} renomeada para: \"{saved_title}\"{RESET}"
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("{RED}[!] Erro ao renomear sessao: {err}{RESET}");
                    ExitCode::FAILURE
                }
            }
        }

        Some(Commands::Prune { dry_run }) => match prune_sessions(dry_run, app_dir, None) {
            Ok(pruned) => {
                if pruned.is_empty() {
                    println!("{GREEN}[OK] Nenhuma sessao fantasma encontrada.{RESET}");
                } else if dry_run {
                    println!(
                        "{YELLOW}[DRY-RUN] {} sessoes fantasmas seriam removidas:{RESET}",
                        pruned.len()
                    );
                    for p in &pruned {
                        println!("  - {} ({} passos, {})", p.id, p.steps, p.mtime);
                    }
                } else {
                    println!(
                        "{GREEN}[OK] {} sessoes fantasmas removidas com sucesso:{RESET}",
                        pruned.len()
                    );
                    for p in &pruned {
                        println!("  - {}", p.id);
                    }
                }
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("{RED}[!] Erro ao limpar sessoes fantasmas: {err}{RESET}");
                ExitCode::FAILURE
            }
        },

        Some(Commands::Artifacts { target }) => {
            let bid = match resolve_brain_target(target.as_deref(), Some(&cur_ws), app_dir) {
                Some(b) => b,
                None => {
                    eprintln!("{RED}[!] Cerebro nao encontrado.{RESET}");
                    return ExitCode::FAILURE;
                }
            };
            let stats = get_brain_stats(&bid, app_dir);
            list_artifacts(&bid, stats.as_ref());
            ExitCode::SUCCESS
        }

        Some(Commands::Open { target }) => {
            let bid = match resolve_brain_target(target.as_deref(), Some(&cur_ws), app_dir) {
                Some(b) => b,
                None => {
                    eprintln!("{RED}[!] Cerebro nao encontrado.{RESET}");
                    return ExitCode::FAILURE;
                }
            };
            let brain_path = get_brain_root(app_dir).join(&bid);
            if open_in_file_manager(&brain_path) {
                println!("{GREEN}[OK] Abrindo cerebro no gerenciador de arquivos: {bid}{RESET}");
                ExitCode::SUCCESS
            } else {
                eprintln!(
                    "{RED}[!] Falha ao abrir pasta do cerebro: {}{RESET}",
                    brain_path.display()
                );
                ExitCode::FAILURE
            }
        }

        Some(Commands::Edit { target }) => {
            let bid = match resolve_brain_target(target.as_deref(), Some(&cur_ws), app_dir) {
                Some(b) => b,
                None => {
                    eprintln!("{RED}[!] Cerebro nao encontrado.{RESET}");
                    return ExitCode::FAILURE;
                }
            };
            let brain_path = get_brain_root(app_dir).join(&bid);
            if open_in_editor(&brain_path) {
                ExitCode::SUCCESS
            } else {
                eprintln!(
                    "{RED}[!] Falha ao abrir pasta no editor: {}{RESET}",
                    brain_path.display()
                );
                ExitCode::FAILURE
            }
        }

        Some(Commands::Nvim { target }) => {
            let bid = match resolve_brain_target(target.as_deref(), Some(&cur_ws), app_dir) {
                Some(b) => b,
                None => {
                    eprintln!("{RED}[!] Nenhum cerebro encontrado para abrir com Neovim.{RESET}");
                    return ExitCode::FAILURE;
                }
            };
            let brain_path = get_brain_root(app_dir).join(&bid);
            if open_in_neovim(&brain_path) {
                ExitCode::SUCCESS
            } else {
                eprintln!(
                    "{RED}[!] Falha ao abrir pasta no Neovim: {}{RESET}",
                    brain_path.display()
                );
                ExitCode::FAILURE
            }
        }

        Some(Commands::Pin { target }) => {
            let bid = match resolve_brain_target(target.as_deref(), Some(&cur_ws), app_dir) {
                Some(b) => b,
                None => {
                    eprintln!("{RED}[!] Informe um ID de sessao valido para fixar.{RESET}");
                    return ExitCode::FAILURE;
                }
            };
            if let Err(e) = set_pin(&bid, Some(&cur_ws), app_dir) {
                eprintln!("{RED}[!] Erro ao salvar fixacao: {e}{RESET}");
                ExitCode::FAILURE
            } else {
                println!(
                    "{GREEN}[OK] Cerebro {bid} fixado com sucesso para este workspace!{RESET}"
                );
                ExitCode::SUCCESS
            }
        }

        Some(Commands::Unpin) => {
            if remove_pin(Some(&cur_ws), app_dir) {
                println!("{YELLOW}[OK] Fixacao removida do workspace atual.{RESET}");
            } else {
                println!("{WHITE}[i] Nenhuma fixacao encontrada para este workspace.{RESET}");
            }
            ExitCode::SUCCESS
        }

        Some(Commands::Delete { target, force }) => {
            let target_str = target.as_deref().unwrap_or("default");
            let resolved_id = match resolve_brain_target(Some(target_str), Some(&cur_ws), app_dir) {
                Some(id) => id,
                None => {
                    eprintln!(
                        "{RED}[!] Sessao '{target_str}' nao encontrada para exclusao.{RESET}"
                    );
                    return ExitCode::FAILURE;
                }
            };

            let brain_path = get_brain_root(app_dir).join(&resolved_id);

            if !force {
                println!(
                    "\n{YELLOW}+------------------------------------------------------------------------+{RESET}"
                );
                println!(
                    "{YELLOW}| CONFIRMACAO DE EXCLUSAO DE SESSAO                                      |{RESET}"
                );
                println!(
                    "{YELLOW}+------------------------------------------------------------------------+{RESET}"
                );
                println!("  ID:        {WHITE}{resolved_id}{RESET}");
                println!("  Diretorio: {WHITE}{}{RESET}", brain_path.display());
                println!(
                    "{YELLOW}+------------------------------------------------------------------------+{RESET}"
                );
                println!("\n  Opcoes disponiveis:");
                println!("    {GREEN}[1] Cancelar e manter sessao (Padrao / Enter){RESET}");
                println!("    {RED}[2] Confirmar exclusao definitiva dos arquivos{RESET}");
                print!("\n  Digite sua escolha [1 para cancelar, 2 para apagar]: ");
                use std::io::Write;
                let _ = std::io::stdout().flush();

                let mut input = String::new();
                if std::io::stdin().read_line(&mut input).is_err() {
                    println!("\n{YELLOW}[i] Operacao cancelada.{RESET}");
                    return ExitCode::SUCCESS;
                }

                let answer = input.trim().to_lowercase();
                if answer != "2"
                    && answer != "s"
                    && answer != "sim"
                    && answer != "y"
                    && answer != "yes"
                    && answer != "apagar"
                {
                    println!(
                        "{YELLOW}[i] Exclusao cancelada pelo usuario (sessao mantida).{RESET}"
                    );
                    return ExitCode::SUCCESS;
                }
            }

            match delete_session(&resolved_id, Some(&cur_ws), app_dir, None) {
                Ok(res) => {
                    println!(
                        "{GREEN}[OK] Sessao {} apagada com sucesso!{RESET}",
                        res.conversation_id
                    );
                    if res.db_deleted {
                        println!("  [OK] Registro removido do SQLite.");
                    }
                    if res.folder_deleted {
                        println!("  [OK] Pasta do cerebro removida do disco.");
                    }
                    if res.pins_removed {
                        println!("  [OK] Fixacoes desvinculadas.");
                    }
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("{RED}[!] Erro ao apagar sessao: {err}{RESET}");
                    ExitCode::FAILURE
                }
            }
        }

        Some(Commands::Init { shell }) => {
            if let Some(code) = get_shell_integration(&shell) {
                print!("{code}");
                ExitCode::SUCCESS
            } else {
                eprintln!("{RED}[!] Shell desconhecida: {shell}{RESET}");
                ExitCode::FAILURE
            }
        }

        Some(Commands::Install { shell }) => {
            let (ok, msg) = install_shell_integration(shell.as_deref());
            if ok {
                println!("{GREEN}[OK] {msg}{RESET}");
                ExitCode::SUCCESS
            } else {
                eprintln!("{RED}[X] {msg}{RESET}");
                ExitCode::FAILURE
            }
        }

        Some(Commands::Doctor) => {
            run_doctor(app_dir);
            ExitCode::SUCCESS
        }
    }
}
