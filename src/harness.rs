use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::backend::{self, ProviderKind};
use crate::claude_code;
use crate::core::{self, BrainStats, SessionSummary};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanItem {
    pub path: PathBuf,
    pub filename: String,
    pub session_id: String,
    pub provider: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfoJson<'a> {
    pub id: &'a str,
    pub session: Option<&'a SessionSummary>,
    pub stats: Option<&'a BrainStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactItemJson {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub mtime: u64,
    pub category: String,
}

fn is_plan_filename(name: &str) -> bool {
    let lower = name.to_lowercase();
    if lower.ends_with(".metadata.json") {
        return false;
    }
    lower.contains("plan") && (lower.ends_with(".md") || lower.ends_with(".json"))
}

fn canonical_or_normalized(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn scan_dir_for_plans(
    dir: &Path,
    session_id: &str,
    provider: &str,
    out: &mut Vec<PlanItem>,
    seen: &mut HashSet<PathBuf>,
) {
    if !dir.is_dir() {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let name = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if is_plan_filename(&name) {
                    let canon = canonical_or_normalized(&p);
                    if seen.insert(canon) {
                        let size = p.metadata().map(|m| m.len()).unwrap_or(0);
                        out.push(PlanItem {
                            path: p,
                            filename: name,
                            session_id: session_id.to_string(),
                            provider: provider.to_string(),
                            size_bytes: size,
                        });
                    }
                }
            }
        }
    }
}

fn parse_file_uri(uri: &str) -> Option<PathBuf> {
    if let Some(stripped) = uri.strip_prefix("file:///") {
        #[cfg(windows)]
        {
            Some(PathBuf::from(stripped.replace('/', "\\")))
        }
        #[cfg(not(windows))]
        {
            Some(PathBuf::from(format!("/{}", stripped)))
        }
    } else if let Some(stripped) = uri.strip_prefix("file://") {
        Some(PathBuf::from(stripped))
    } else {
        None
    }
}

pub fn find_plans_for_antigravity_session(
    session_id: &str,
    app_dir: Option<&Path>,
    workspace: Option<&Path>,
) -> Vec<PlanItem> {
    let mut plans = Vec::new();
    let mut seen = HashSet::new();

    // 1. Brain folders (standard + cross-platform IDE/CLI candidate locations)
    let mut candidate_brain_dirs = vec![core::get_brain_root(app_dir).join(session_id)];
    if app_dir.is_none() {
        if let Some(home) = dirs::home_dir() {
            let alt_ide = home.join(".gemini").join("antigravity").join("brain").join(session_id);
            if !candidate_brain_dirs.contains(&alt_ide) {
                candidate_brain_dirs.push(alt_ide);
            }
            let alt_cli = home.join(".gemini").join("antigravity-cli").join("brain").join(session_id);
            if !candidate_brain_dirs.contains(&alt_cli) {
                candidate_brain_dirs.push(alt_cli);
            }
        }
    }

    for b_dir in candidate_brain_dirs {
        if b_dir.is_dir() {
            scan_dir_for_plans(&b_dir, session_id, "antigravity", &mut plans, &mut seen);

            let scratch_dir = b_dir.join("scratch");
            if scratch_dir.is_dir() {
                scan_dir_for_plans(&scratch_dir, session_id, "antigravity", &mut plans, &mut seen);
            }
        }
    }

    // 2. Session URIs and workspace
    let all_sessions = core::fetch_sessions_ext(workspace, true, true, app_dir, None);
    if let Some(session) = all_sessions.iter().find(|s| s.id == session_id) {
        if !session.primary_workspace.is_empty() {
            let ws_path = PathBuf::from(&session.primary_workspace);
            scan_dir_for_plans(&ws_path, session_id, "antigravity", &mut plans, &mut seen);
        }

        for uri in &session.uris {
            if let Some(p) = parse_file_uri(uri) {
                if p.is_file() {
                    let name = p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if is_plan_filename(&name) {
                        let canon = canonical_or_normalized(&p);
                        if seen.insert(canon) {
                            let size = p.metadata().map(|m| m.len()).unwrap_or(0);
                            plans.push(PlanItem {
                                path: p,
                                filename: name,
                                session_id: session_id.to_string(),
                                provider: "antigravity".to_string(),
                                size_bytes: size,
                            });
                        }
                    }
                } else if p.is_dir() {
                    scan_dir_for_plans(&p, session_id, "antigravity", &mut plans, &mut seen);
                }
            }
        }
    }

    plans
}

pub fn find_plans_for_claude_session(
    session_id: &str,
    app_dir: Option<&Path>,
) -> Vec<PlanItem> {
    let mut plans = Vec::new();
    let mut seen = HashSet::new();

    let session_file = claude_code::find_session_file(session_id, app_dir);
    let plans_root = claude_code::get_plans_root(app_dir);

    if let Some(file_path) = session_file {
        let parsed = claude_code::parse_session_file(&file_path);

        // 1. Session cwd
        if let Some(cwd_str) = &parsed.cwd {
            let cwd = PathBuf::from(cwd_str);
            scan_dir_for_plans(&cwd, session_id, "claude-code", &mut plans, &mut seen);

            let dot_claude_plans = cwd.join(".claude").join("plans");
            if dot_claude_plans.is_dir() {
                scan_dir_for_plans(&dot_claude_plans, session_id, "claude-code", &mut plans, &mut seen);
            }
        }

        // 2. Specific slugs referenced in transcript
        for slug in &parsed.plan_slugs {
            let candidates = [
                plans_root.join(slug),
                plans_root.join(format!("{slug}.md")),
            ];
            for cand in candidates {
                if cand.is_file() {
                    let canon = canonical_or_normalized(&cand);
                    if seen.insert(canon) {
                        let size = cand.metadata().map(|m| m.len()).unwrap_or(0);
                        plans.push(PlanItem {
                            path: cand.clone(),
                            filename: cand
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                            session_id: session_id.to_string(),
                            provider: "claude-code".to_string(),
                            size_bytes: size,
                        });
                    }
                }
            }
        }
    }

    plans
}

pub fn find_plans_for_session(
    provider: ProviderKind,
    session_id: &str,
    app_dir: Option<&Path>,
    workspace: Option<&Path>,
) -> Vec<PlanItem> {
    match provider {
        ProviderKind::Antigravity => {
            find_plans_for_antigravity_session(session_id, app_dir, workspace)
        }
        ProviderKind::ClaudeCode => find_plans_for_claude_session(session_id, app_dir),
    }
}

pub fn find_all_plans(
    provider: ProviderKind,
    workspace: Option<&Path>,
    app_dir: Option<&Path>,
    all_workspaces: bool,
    include_subagents: bool,
) -> Vec<PlanItem> {
    let mut all_plans = Vec::new();
    let mut seen = HashSet::new();

    let sessions = backend::fetch_sessions_ext(
        provider,
        workspace,
        all_workspaces,
        include_subagents,
        app_dir,
    );

    for s in &sessions {
        let plans = find_plans_for_session(provider, &s.id, app_dir, workspace);
        for item in plans {
            let canon = canonical_or_normalized(&item.path);
            if seen.insert(canon) {
                all_plans.push(item);
            }
        }
    }

    // Direct scan of current workspace
    if let Some(ws) = workspace {
        scan_dir_for_plans(ws, "", provider.label(), &mut all_plans, &mut seen);
        let dot_plans = ws.join(".claude").join("plans");
        if dot_plans.is_dir() {
            scan_dir_for_plans(&dot_plans, "", provider.label(), &mut all_plans, &mut seen);
        }
    }

    // Direct scan of ~/.claude/plans if claude-code provider is active
    if provider == ProviderKind::ClaudeCode {
        let plans_root = claude_code::get_plans_root(app_dir);
        if plans_root.is_dir() {
            if let Ok(entries) = fs::read_dir(&plans_root) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        let name = p
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        if name.ends_with(".md") {
                            let canon = canonical_or_normalized(&p);
                            if seen.insert(canon) {
                                let size = p.metadata().map(|m| m.len()).unwrap_or(0);
                                all_plans.push(PlanItem {
                                    path: p,
                                    filename: name,
                                    session_id: String::new(),
                                    provider: "claude-code".to_string(),
                                    size_bytes: size,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    all_plans.sort_by(|a, b| a.filename.cmp(&b.filename));
    all_plans
}

pub fn handle_plan_command(
    provider: ProviderKind,
    target: Option<&str>,
    all: bool,
    cat: bool,
    paths_only: bool,
    json_output: bool,
    workspace: Option<&Path>,
    app_dir: Option<&Path>,
    include_subagents: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let plans = if all {
        find_all_plans(provider, workspace, app_dir, true, include_subagents)
    } else if let Some(t) = target {
        let resolved = backend::resolve_brain_target(provider, Some(t), workspace, app_dir);
        if let Some(bid) = resolved {
            find_plans_for_session(provider, &bid, app_dir, workspace)
        } else {
            // Target might be a direct workspace directory or direct path
            let mut direct = Vec::new();
            let mut seen = HashSet::new();
            let path_cand = PathBuf::from(t);
            if path_cand.is_dir() {
                scan_dir_for_plans(&path_cand, "", provider.label(), &mut direct, &mut seen);
            } else if path_cand.is_file() && is_plan_filename(&t) {
                let size = path_cand.metadata().map(|m| m.len()).unwrap_or(0);
                direct.push(PlanItem {
                    path: path_cand.clone(),
                    filename: path_cand
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    session_id: String::new(),
                    provider: provider.label().to_string(),
                    size_bytes: size,
                });
            }
            direct
        }
    } else {
        // Default: try active session for current workspace first
        let resolved = backend::resolve_brain_target(provider, None, workspace, app_dir);
        let mut session_plans = if let Some(bid) = resolved {
            find_plans_for_session(provider, &bid, app_dir, workspace)
        } else {
            Vec::new()
        };

        if session_plans.is_empty() {
            // Fallback: check workspace and ~/.claude/plans
            session_plans = find_all_plans(provider, workspace, app_dir, false, include_subagents);
        }
        session_plans
    };

    if cat {
        if plans.is_empty() {
            eprintln!("[!] Nenhum arquivo de plano encontrado para exibir.");
            return Err("Plano nao encontrado".into());
        }
        for item in &plans {
            let content = fs::read_to_string(&item.path)?;
            print!("{content}");
        }
        return Ok(());
    }

    if paths_only {
        for item in &plans {
            println!("{}", item.path.display());
        }
        return Ok(());
    }

    if json_output {
        let serialized = serde_json::to_string(&plans)?;
        println!("{serialized}");
        return Ok(());
    }

    // Default human-readable format
    if plans.is_empty() {
        println!("[*] Nenhum plano de execucao encontrado.");
    } else {
        println!("\n+------------------------------------------------------------------------+");
        println!("| PLANOS DE EXECUCAO E HARNESS                                           |");
        println!("+------------------------------------------------------------------------+");
        for item in &plans {
            let kb = format!("{:.1} KB", item.size_bytes as f64 / 1024.0);
            let s_tag = if !item.session_id.is_empty() {
                format!(" [{}]", &item.session_id[..item.session_id.len().min(8)])
            } else {
                String::new()
            };
            println!("  * {:<36} ({kb}) [{}]{s_tag}", item.filename, item.provider);
            println!("    Caminho: {}", item.path.display());
        }
        println!("+------------------------------------------------------------------------+\n");
    }

    Ok(())
}

pub fn render_artifacts_custom(
    brain_id: &str,
    brain_path: &Path,
    stats: Option<&BrainStats>,
    paths_only: bool,
    json_output: bool,
    plan_filter: bool,
) {
    let mut files = Vec::new();

    if let Some(st) = stats {
        for art in &st.recent_artifacts {
            if plan_filter && !is_plan_filename(&art.name) {
                continue;
            }
            let full_path = if art.category == "transcript" {
                st.path.clone()
            } else if art.name.starts_with("scratch/") {
                brain_path.join(&art.name)
            } else {
                brain_path.join(&art.name)
            };
            files.push(ArtifactItemJson {
                name: art.name.clone(),
                path: full_path,
                size_bytes: art.size,
                mtime: art.mtime,
                category: art.category.clone(),
            });
        }
    }

    // If stats had no direct artifacts or files in brain_path, scan brain_path and candidate paths
    if files.is_empty() {
        let mut candidate_paths = vec![brain_path.to_path_buf()];
        if let Some(home) = dirs::home_dir() {
            candidate_paths.push(home.join(".gemini").join("antigravity").join("brain").join(brain_id));
            candidate_paths.push(home.join(".gemini").join("antigravity-cli").join("brain").join(brain_id));
        }
        for cand in candidate_paths {
            if cand.is_dir() {
                if let Ok(entries) = fs::read_dir(&cand) {
                    for e in entries.flatten() {
                        let p = e.path();
                        if p.is_file() {
                            let name = p
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            if name.ends_with(".metadata.json") {
                                continue;
                            }
                            if plan_filter && !is_plan_filename(&name) {
                                continue;
                            }
                            let meta = p.metadata().ok();
                            let sz = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                            let mt = meta
                                .and_then(|m| m.modified().ok())
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            files.push(ArtifactItemJson {
                                name,
                                path: p,
                                size_bytes: sz,
                                mtime: mt,
                                category: "root".to_string(),
                            });
                        }
                    }
                }
                if !files.is_empty() {
                    break;
                }
            }
        }
    }

    if paths_only {
        for f in &files {
            println!("{}", f.path.display());
        }
        return;
    }

    if json_output {
        let serialized = serde_json::to_string(&files).unwrap_or_else(|_| "[]".to_string());
        println!("{serialized}");
        return;
    }

    // Filtered human output if plan_filter is active without --paths/--json
    if files.is_empty() {
        println!("[*] Nenhum artefato de planejamento encontrado para a sessao {brain_id}.");
    } else {
        println!("\n+------------------------------------------------------------------------+");
        println!("| ARTEFATOS DE PLANEJAMENTO: {:<43} |", brain_id);
        println!("+------------------------------------------------------------------------+");
        for f in &files {
            let kb = format!("{:.1} KB", f.size_bytes as f64 / 1024.0);
            println!("  * {:<36} ({kb})", f.name);
            println!("    Caminho: {}", f.path.display());
        }
        println!("+------------------------------------------------------------------------+\n");
    }
}

pub fn render_sessions_custom(
    sessions: &[SessionSummary],
    paths_only: bool,
    json_output: bool,
    provider: ProviderKind,
    cur_ws: Option<&Path>,
    app_dir: Option<&Path>,
) {
    if paths_only {
        for s in sessions {
            if let Some(p) = backend::session_root_dir(provider, &s.id, cur_ws, app_dir) {
                println!("{}", p.display());
            }
        }
    } else if json_output {
        let json_str = serde_json::to_string(sessions).unwrap_or_else(|_| "[]".to_string());
        println!("{json_str}");
    }
}

pub fn render_info_custom(
    session: Option<&SessionSummary>,
    stats: Option<&BrainStats>,
    id: &str,
    paths_only: bool,
    json_output: bool,
) {
    if paths_only {
        if let Some(st) = stats {
            println!("{}", st.path.display());
        } else {
            println!("{id}");
        }
    } else if json_output {
        let obj = SessionInfoJson { id, session, stats };
        let json_str = serde_json::to_string(&obj).unwrap_or_else(|_| "{}".to_string());
        println!("{json_str}");
    }
}
