use crate::claude_code;
use crate::cli::ProviderArg;
use crate::core;
use crate::core::{BrainStats, DeleteResult, PrunedSession, SessionSummary};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Antigravity,
    ClaudeCode,
}

impl ProviderKind {
    pub fn label(&self) -> &'static str {
        match self {
            ProviderKind::Antigravity => "antigravity",
            ProviderKind::ClaudeCode => "claude-code",
        }
    }
}

pub fn explicit_from_arg(arg: Option<ProviderArg>) -> Option<ProviderKind> {
    match arg {
        Some(ProviderArg::Antigravity) => Some(ProviderKind::Antigravity),
        Some(ProviderArg::ClaudeCode) => Some(ProviderKind::ClaudeCode),
        Some(ProviderArg::Auto) | None => None,
    }
}

pub fn explicit_from_env() -> Option<ProviderKind> {
    match std::env::var("AGY_PROVIDER").ok()?.to_lowercase().as_str() {
        "antigravity" => Some(ProviderKind::Antigravity),
        "claude-code" | "claudecode" | "claude_code" => Some(ProviderKind::ClaudeCode),
        _ => None,
    }
}

/// Resolves which provider to use, in priority order: explicit `--provider`
/// flag, then `AGY_PROVIDER` env var, then auto-detection based on
/// `--app-dir` (if given) or the default locations of each provider.
pub fn resolve_provider(
    flag: Option<ProviderArg>,
    app_dir_override: Option<&Path>,
) -> ProviderKind {
    let explicit = explicit_from_arg(flag).or_else(explicit_from_env);
    detect_provider(explicit, app_dir_override)
}

pub fn detect_provider(
    explicit: Option<ProviderKind>,
    app_dir_override: Option<&Path>,
) -> ProviderKind {
    if let Some(p) = explicit {
        return p;
    }

    if let Some(dir) = app_dir_override {
        if dir.join("conversation_summaries.db").is_file() {
            return ProviderKind::Antigravity;
        }
        if dir.join("projects").is_dir() {
            return ProviderKind::ClaudeCode;
        }
        return ProviderKind::Antigravity;
    }

    if core::get_db_path(None).is_file() {
        return ProviderKind::Antigravity;
    }
    if claude_code::get_projects_root(None).is_dir() {
        return ProviderKind::ClaudeCode;
    }
    ProviderKind::Antigravity
}

pub fn fetch_sessions_ext(
    provider: ProviderKind,
    workspace_path: Option<&Path>,
    all_workspaces: bool,
    include_subagents: bool,
    app_dir: Option<&Path>,
) -> Vec<SessionSummary> {
    match provider {
        ProviderKind::Antigravity => core::fetch_sessions_ext(
            workspace_path,
            all_workspaces,
            include_subagents,
            app_dir,
            None,
        ),
        ProviderKind::ClaudeCode => claude_code::fetch_sessions_ext(
            workspace_path,
            all_workspaces,
            include_subagents,
            app_dir,
        ),
    }
}

pub fn get_brain_stats(
    provider: ProviderKind,
    id: &str,
    app_dir: Option<&Path>,
) -> Option<BrainStats> {
    match provider {
        ProviderKind::Antigravity => core::get_brain_stats(id, app_dir),
        ProviderKind::ClaudeCode => claude_code::get_brain_stats(id, app_dir),
    }
}

pub fn resolve_brain_target(
    provider: ProviderKind,
    target: Option<&str>,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Option<String> {
    match provider {
        ProviderKind::Antigravity => core::resolve_brain_target(target, workspace_path, app_dir),
        ProviderKind::ClaudeCode => {
            claude_code::resolve_brain_target(target, workspace_path, app_dir)
        }
    }
}

/// Directory `go`/`open`/`edit`/`nvim` should navigate to for a resolved
/// session id. Always `Some` for Antigravity (the brain folder path is
/// deterministic even if it doesn't exist yet); may be `None` for Claude
/// Code if the session's transcript can no longer be found.
pub fn session_root_dir(
    provider: ProviderKind,
    id: &str,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Option<PathBuf> {
    match provider {
        ProviderKind::Antigravity => Some(core::get_brain_root(app_dir).join(id)),
        ProviderKind::ClaudeCode => claude_code::resolve_session_root(id, app_dir, workspace_path),
    }
}

pub fn set_pin(
    provider: ProviderKind,
    id: &str,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    match provider {
        ProviderKind::Antigravity => core::set_pin(id, workspace_path, app_dir),
        ProviderKind::ClaudeCode => claude_code::set_pin(id, workspace_path, app_dir),
    }
}

pub fn remove_pin(
    provider: ProviderKind,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> bool {
    match provider {
        ProviderKind::Antigravity => core::remove_pin(workspace_path, app_dir),
        ProviderKind::ClaudeCode => claude_code::remove_pin(workspace_path, app_dir),
    }
}

pub fn get_pinned_id_for_workspace(
    provider: ProviderKind,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Option<String> {
    match provider {
        ProviderKind::Antigravity => core::get_pinned_id_for_workspace(workspace_path, app_dir),
        ProviderKind::ClaudeCode => {
            claude_code::get_pinned_id_for_workspace(workspace_path, app_dir)
        }
    }
}

pub fn rename_session(
    provider: ProviderKind,
    target: &str,
    new_title: &str,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    match provider {
        ProviderKind::Antigravity => {
            core::rename_session(target, new_title, workspace_path, app_dir, None)
        }
        ProviderKind::ClaudeCode => {
            claude_code::rename_session(target, new_title, workspace_path, app_dir)
        }
    }
}

pub fn delete_session(
    provider: ProviderKind,
    target: &str,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Result<DeleteResult, Box<dyn std::error::Error>> {
    match provider {
        ProviderKind::Antigravity => core::delete_session(target, workspace_path, app_dir, None),
        ProviderKind::ClaudeCode => claude_code::delete_session(target, workspace_path, app_dir),
    }
}

pub fn prune_sessions(
    provider: ProviderKind,
    dry_run: bool,
    app_dir: Option<&Path>,
) -> Result<Vec<PrunedSession>, Box<dyn std::error::Error>> {
    match provider {
        ProviderKind::Antigravity => core::prune_sessions(dry_run, app_dir, None),
        ProviderKind::ClaudeCode => claude_code::prune_sessions(dry_run, app_dir),
    }
}
