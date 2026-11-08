use crate::core::{
    clean_single_line, normalize_workspace_path, resolve_target_generic, ArtifactInfo, BrainStats,
    DeleteResult, PrunedSession, SessionSummary,
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub fn get_default_app_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("AGY_CLAUDE_APP_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return p;
        }
    }
    if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return p;
        }
    }
    dirs::home_dir()
        .map(|h| h.join(".claude"))
        .unwrap_or_else(|| PathBuf::from(".claude"))
}

pub fn get_projects_root(app_dir: Option<&Path>) -> PathBuf {
    match app_dir {
        Some(p) => p.join("projects"),
        None => get_default_app_dir().join("projects"),
    }
}

pub fn get_todos_root(app_dir: Option<&Path>) -> PathBuf {
    match app_dir {
        Some(p) => p.join("todos"),
        None => get_default_app_dir().join("todos"),
    }
}

pub fn get_pins_file(app_dir: Option<&Path>) -> PathBuf {
    match app_dir {
        Some(p) => p.join("claude_workspace_pins.json"),
        None => get_default_app_dir().join("claude_workspace_pins.json"),
    }
}

pub fn get_titles_file(app_dir: Option<&Path>) -> PathBuf {
    match app_dir {
        Some(p) => p.join("agy_titles.json"),
        None => get_default_app_dir().join("agy_titles.json"),
    }
}

/// Encodes a project working directory the same way Claude Code does when it
/// names the folder holding that project's session transcripts: path
/// separators and `:` are each replaced with a single `-`. This is a
/// lossy/ambiguous encoding, so it is only ever used as a fast lookup path;
/// the authoritative workspace association is always the `cwd` recorded
/// inside the transcript entries themselves.
fn encode_project_dir(path: &Path) -> String {
    path.to_string_lossy().replace(['\\', '/', ':'], "-")
}

fn extract_text_content(message: &serde_json::Value) -> Option<String> {
    let content = message.get("content")?;
    if let Some(s) = content.as_str() {
        if !s.trim().is_empty() {
            return Some(s.to_string());
        }
        return None;
    }
    if let Some(arr) = content.as_array() {
        for block in arr {
            if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    if !t.trim().is_empty() {
                        return Some(t.to_string());
                    }
                }
            }
        }
    }
    None
}

struct ParsedFile {
    summary_title: Option<String>,
    first_user_text: Option<String>,
    steps: i64,
    last_root_timestamp: Option<String>,
    last_root_type: Option<String>,
    cwd: Option<String>,
    subagent_count: usize,
}

/// Parses a single Claude Code session transcript (JSONL). Malformed lines
/// are skipped individually; they never abort parsing of the rest of the
/// file, since the format can drift across Claude Code versions.
fn parse_session_file(path: &Path) -> ParsedFile {
    let mut out = ParsedFile {
        summary_title: None,
        first_user_text: None,
        steps: 0,
        last_root_timestamp: None,
        last_root_type: None,
        cwd: None,
        subagent_count: 0,
    };

    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return out,
    };
    let reader = std::io::BufReader::new(file);

    let mut prev_is_sidechain = false;

    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if out.cwd.is_none() {
            if let Some(c) = value.get("cwd").and_then(|v| v.as_str()) {
                if !c.trim().is_empty() {
                    out.cwd = Some(c.to_string());
                }
            }
        }

        let entry_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match entry_type {
            "summary" => {
                if out.summary_title.is_none() {
                    if let Some(s) = value.get("summary").and_then(|v| v.as_str()) {
                        if !s.trim().is_empty() {
                            out.summary_title = Some(s.to_string());
                        }
                    }
                }
            }
            "user" | "assistant" => {
                let is_sidechain = value
                    .get("isSidechain")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if is_sidechain {
                    if !prev_is_sidechain {
                        out.subagent_count += 1;
                    }
                    prev_is_sidechain = true;
                    continue;
                }
                prev_is_sidechain = false;

                out.steps += 1;
                if let Some(ts) = value.get("timestamp").and_then(|v| v.as_str()) {
                    out.last_root_timestamp = Some(ts.to_string());
                }
                out.last_root_type = Some(entry_type.to_string());

                if entry_type == "user" && out.first_user_text.is_none() {
                    if let Some(msg) = value.get("message") {
                        if let Some(text) = extract_text_content(msg) {
                            out.first_user_text = Some(text);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    out
}

fn format_mtime(timestamp: Option<&str>, file_path: &Path) -> String {
    if let Some(ts) = timestamp {
        if ts.len() >= 19 {
            return ts[..19].replace('T', " ");
        }
    }
    if let Ok(meta) = file_path.metadata() {
        if let Ok(modified) = meta.modified() {
            if let Ok(dur) = modified.duration_since(UNIX_EPOCH) {
                if let Some(dt) = DateTime::<Utc>::from_timestamp(dur.as_secs() as i64, 0) {
                    return dt.format("%Y-%m-%d %H:%M:%S").to_string();
                }
            }
        }
    }
    String::new()
}

fn load_title_overrides(app_dir: Option<&Path>) -> HashMap<String, String> {
    let path = get_titles_file(app_dir);
    if !path.is_file() {
        return HashMap::new();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

fn save_title_overrides(
    app_dir: Option<&Path>,
    titles: &HashMap<String, String>,
) -> std::io::Result<()> {
    let path = get_titles_file(app_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(titles).unwrap_or_default(),
    )
}

fn resolve_claude_title(
    id: &str,
    parsed: &ParsedFile,
    overrides: &HashMap<String, String>,
) -> (String, String) {
    if let Some(custom) = overrides.get(id) {
        if !custom.trim().is_empty() {
            return (clean_single_line(custom, 100), "title".to_string());
        }
    }
    if let Some(summary) = &parsed.summary_title {
        if !summary.trim().is_empty() {
            return (clean_single_line(summary, 100), "summary".to_string());
        }
    }
    if let Some(preview) = &parsed.first_user_text {
        if !preview.trim().is_empty() {
            return (clean_single_line(preview, 100), "preview".to_string());
        }
    }
    ("(sem titulo)".to_string(), "empty".to_string())
}

fn find_session_file(id: &str, app_dir: Option<&Path>) -> Option<PathBuf> {
    let root = get_projects_root(app_dir);
    let entries = fs::read_dir(&root).ok()?;
    for e in entries.flatten() {
        let candidate = e.path().join(format!("{id}.jsonl"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_todo_files(id: &str, app_dir: Option<&Path>) -> Vec<PathBuf> {
    let root = get_todos_root(app_dir);
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with(id) && name.ends_with(".json") {
                        out.push(p);
                    }
                }
            }
        }
    }
    out
}

fn has_todo_files(id: &str, app_dir: Option<&Path>) -> bool {
    !find_todo_files(id, app_dir).is_empty()
}

pub fn get_pinned_id_for_workspace(
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Option<String> {
    let ws = match workspace_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    let norm_ws = normalize_workspace_path(&ws.to_string_lossy());

    let candidate = ws.join(".claude-brain");
    if candidate.is_file() {
        if let Ok(content) = fs::read_to_string(&candidate) {
            let trimmed = content.trim();
            if trimmed.len() >= 30 && trimmed.contains('-') {
                return Some(trimmed.to_string());
            }
        }
    }

    let pins_file = get_pins_file(app_dir);
    if pins_file.is_file() {
        if let Ok(content) = fs::read_to_string(&pins_file) {
            if let Ok(pins) =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
            {
                for (pin_ws, val) in pins {
                    if normalize_workspace_path(&pin_ws) == norm_ws {
                        if let Some(s) = val.as_str() {
                            return Some(s.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn set_pin(
    brain_id: &str,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws = match workspace_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let norm_ws = normalize_workspace_path(&ws.to_string_lossy());

    let pins_file = get_pins_file(app_dir);
    let mut pins = if pins_file.is_file() {
        let content = fs::read_to_string(&pins_file).unwrap_or_else(|_| "{}".to_string());
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
            .unwrap_or_default()
    } else {
        serde_json::Map::new()
    };

    pins.insert(
        norm_ws.clone(),
        serde_json::Value::String(brain_id.to_string()),
    );

    if let Some(parent) = pins_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&pins_file, serde_json::to_string_pretty(&pins)?)?;

    let _ = fs::write(ws.join(".claude-brain"), format!("{}\n", brain_id.trim()));

    Ok(())
}

pub fn remove_pin(workspace_path: Option<&Path>, app_dir: Option<&Path>) -> bool {
    let ws = match workspace_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    let norm_ws = normalize_workspace_path(&ws.to_string_lossy());
    let mut removed = false;

    let pins_file = get_pins_file(app_dir);
    if pins_file.is_file() {
        if let Ok(content) = fs::read_to_string(&pins_file) {
            if let Ok(mut pins) =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
            {
                let before_len = pins.len();
                pins.retain(|k, _| normalize_workspace_path(k) != norm_ws);
                if pins.len() < before_len {
                    let _ = fs::write(
                        &pins_file,
                        serde_json::to_string_pretty(&pins).unwrap_or_default(),
                    );
                    removed = true;
                }
            }
        }
    }

    let marker = ws.join(".claude-brain");
    if marker.is_file() {
        let _ = fs::remove_file(marker);
        removed = true;
    }

    removed
}

pub fn fetch_sessions(
    workspace_path: Option<&Path>,
    all_workspaces: bool,
    app_dir: Option<&Path>,
) -> Vec<SessionSummary> {
    fetch_sessions_ext(workspace_path, all_workspaces, false, app_dir)
}

pub fn fetch_sessions_ext(
    workspace_path: Option<&Path>,
    all_workspaces: bool,
    _include_subagents: bool,
    app_dir: Option<&Path>,
) -> Vec<SessionSummary> {
    let projects_root = get_projects_root(app_dir);
    if !projects_root.is_dir() {
        return Vec::new();
    }

    let norm_ws = workspace_path
        .map(|p| normalize_workspace_path(&p.to_string_lossy()))
        .unwrap_or_default();
    let pinned_id = get_pinned_id_for_workspace(workspace_path, app_dir);
    let titles = load_title_overrides(app_dir);

    let mut dirs_to_scan: Vec<PathBuf> = Vec::new();
    if all_workspaces || workspace_path.is_none() {
        if let Ok(entries) = fs::read_dir(&projects_root) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    dirs_to_scan.push(e.path());
                }
            }
        }
    } else if let Some(ws) = workspace_path {
        let encoded = projects_root.join(encode_project_dir(ws));
        if encoded.is_dir() {
            dirs_to_scan.push(encoded);
        }
    }

    // A pinned session may live outside the cwd-matched project folder;
    // make sure its transcript is still reachable so it can surface.
    if let Some(pid) = &pinned_id {
        let already_covered = dirs_to_scan
            .iter()
            .any(|d| d.join(format!("{pid}.jsonl")).is_file());
        if !already_covered {
            if let Ok(entries) = fs::read_dir(&projects_root) {
                for e in entries.flatten() {
                    let dir = e.path();
                    if dir.join(format!("{pid}.jsonl")).is_file() {
                        dirs_to_scan.push(dir);
                        break;
                    }
                }
            }
        }
    }

    let mut jsonl_files: Vec<PathBuf> = Vec::new();
    let mut seen_dirs = std::collections::HashSet::new();
    for dir in &dirs_to_scan {
        if !seen_dirs.insert(dir.clone()) {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("jsonl") {
                    jsonl_files.push(p);
                }
            }
        }
    }

    let mut results = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for file_path in jsonl_files {
        let id = match file_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        if !seen_ids.insert(id.clone()) {
            continue;
        }

        let parsed = parse_session_file(&file_path);
        let is_pinned = pinned_id.as_deref() == Some(id.as_str());
        let (resolved_title, title_source) = resolve_claude_title(&id, &parsed, &titles);

        if !is_pinned
            && parsed.steps == 0
            && title_source == "empty"
            && !has_todo_files(&id, app_dir)
        {
            continue;
        }

        let norm_cwd = parsed
            .cwd
            .as_deref()
            .map(normalize_workspace_path)
            .unwrap_or_default();
        let uris = if norm_cwd.is_empty() {
            Vec::new()
        } else {
            vec![norm_cwd.clone()]
        };

        let is_match = if all_workspaces || norm_ws.is_empty() {
            true
        } else {
            is_pinned || norm_cwd == norm_ws
        };

        if !is_match {
            continue;
        }

        let mtime = format_mtime(parsed.last_root_timestamp.as_deref(), &file_path);
        let status = match parsed.last_root_type.as_deref() {
            Some("user") => "WAITING",
            _ => "IDLE",
        }
        .to_string();

        results.push(SessionSummary {
            id,
            title: resolved_title,
            steps: parsed.steps,
            mtime,
            uris,
            primary_workspace: norm_cwd,
            status,
            pinned: is_pinned,
            is_subagent: false,
            agent_name: String::new(),
            parent_id: String::new(),
            parent_title: None,
            nesting_depth: 0,
            subagent_count: parsed.subagent_count,
            title_source,
        });
    }

    results.sort_by(|a, b| b.pinned.cmp(&a.pinned).then_with(|| b.steps.cmp(&a.steps)));
    results
}

pub fn get_brain_stats(id: &str, app_dir: Option<&Path>) -> Option<BrainStats> {
    let file_path = find_session_file(id, app_dir)?;
    let todo_files = find_todo_files(id, app_dir);

    let mut recent_artifacts = Vec::new();
    if let Ok(meta) = file_path.metadata() {
        recent_artifacts.push(ArtifactInfo {
            name: file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            size: meta.len(),
            mtime: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
            category: "transcript".to_string(),
        });
    }
    for todo in &todo_files {
        if let Ok(meta) = todo.metadata() {
            recent_artifacts.push(ArtifactInfo {
                name: todo
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                size: meta.len(),
                mtime: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                category: "todo".to_string(),
            });
        }
    }
    recent_artifacts.sort_by_key(|a| std::cmp::Reverse(a.mtime));

    Some(BrainStats {
        exists: true,
        path: file_path,
        md_count: 0,
        png_count: 0,
        scratch_count: 0,
        total_files: 1 + todo_files.len(),
        recent_artifacts,
    })
}

/// Resolves the directory `go`/`open`/`edit`/`nvim` should jump to for a
/// Claude Code session: the project's real working directory as recorded in
/// the transcript, falling back to Claude Code's internal bookkeeping
/// folder (the one holding the transcript) if that directory no longer
/// exists on disk.
pub fn resolve_session_root(
    id: &str,
    app_dir: Option<&Path>,
    _workspace_path: Option<&Path>,
) -> Option<PathBuf> {
    let file_path = find_session_file(id, app_dir)?;
    let parsed = parse_session_file(&file_path);
    if let Some(cwd) = &parsed.cwd {
        let p = PathBuf::from(cwd);
        if p.is_dir() {
            return Some(p);
        }
    }
    file_path.parent().map(|p| p.to_path_buf())
}

pub fn resolve_brain_target(
    target: Option<&str>,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Option<String> {
    let sessions = fetch_sessions(workspace_path, false, app_dir);
    let pinned_id = get_pinned_id_for_workspace(workspace_path, app_dir);

    resolve_target_generic(target, &sessions, pinned_id.as_deref(), |t| {
        let all = fetch_sessions_ext(workspace_path, true, true, app_dir);
        all.into_iter().find(|s| s.id.starts_with(t)).map(|s| s.id)
    })
}

pub fn rename_session(
    target: &str,
    new_title: &str,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let clean_title = clean_single_line(new_title, 120);
    if clean_title.is_empty() {
        return Err("O novo titulo nao pode ser vazio.".into());
    }

    let resolved_id = resolve_brain_target(Some(target), workspace_path, app_dir)
        .ok_or_else(|| format!("Sessao '{}' nao encontrada.", target))?;

    if find_session_file(&resolved_id, app_dir).is_none() {
        return Err(format!("Sessao '{}' nao encontrada no disco.", resolved_id).into());
    }

    let mut titles = load_title_overrides(app_dir);
    titles.insert(resolved_id.clone(), clean_title.clone());
    save_title_overrides(app_dir, &titles)?;

    Ok((resolved_id, clean_title))
}

pub fn prune_sessions(
    dry_run: bool,
    app_dir: Option<&Path>,
) -> Result<Vec<PrunedSession>, Box<dyn std::error::Error>> {
    let projects_root = get_projects_root(app_dir);
    if !projects_root.is_dir() {
        return Ok(Vec::new());
    }

    let titles = load_title_overrides(app_dir);
    let mut pruned = Vec::new();

    let mut jsonl_files: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&projects_root) {
        for e in entries.flatten() {
            let dir = e.path();
            if !dir.is_dir() {
                continue;
            }
            if let Ok(inner) = fs::read_dir(&dir) {
                for f in inner.flatten() {
                    let p = f.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("jsonl") {
                        jsonl_files.push(p);
                    }
                }
            }
        }
    }

    for file_path in jsonl_files {
        let id = match file_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let parsed = parse_session_file(&file_path);
        let (_, title_source) = resolve_claude_title(&id, &parsed, &titles);

        if !has_todo_files(&id, app_dir) && (parsed.steps == 0 || title_source == "empty") {
            let mtime = format_mtime(parsed.last_root_timestamp.as_deref(), &file_path);
            if !dry_run {
                let _ = fs::remove_file(&file_path);
                let mut titles = load_title_overrides(app_dir);
                if titles.remove(&id).is_some() {
                    let _ = save_title_overrides(app_dir, &titles);
                }
            }
            pruned.push(PrunedSession {
                id,
                steps: parsed.steps,
                mtime,
            });
        }
    }

    Ok(pruned)
}

pub fn delete_session(
    target: &str,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Result<DeleteResult, Box<dyn std::error::Error>> {
    let resolved_id = resolve_brain_target(Some(target), workspace_path, app_dir)
        .ok_or_else(|| format!("Sessao '{}' nao encontrada.", target))?;

    let file_path = find_session_file(&resolved_id, app_dir);

    let titles = load_title_overrides(app_dir);
    let mut title = titles.get(&resolved_id).cloned().unwrap_or_default();

    let mut db_deleted = false;
    if let Some(fp) = &file_path {
        if title.is_empty() {
            let parsed = parse_session_file(fp);
            title = parsed
                .summary_title
                .or(parsed.first_user_text)
                .unwrap_or_default();
        }
        db_deleted = fs::remove_file(fp).is_ok();
    }

    let todo_files = find_todo_files(&resolved_id, app_dir);
    let mut folder_deleted = false;
    for t in &todo_files {
        if fs::remove_file(t).is_ok() {
            folder_deleted = true;
        }
    }

    let mut pins_removed = false;

    let ws = match workspace_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    let marker = ws.join(".claude-brain");
    if marker.is_file() {
        if let Ok(content) = fs::read_to_string(&marker) {
            if content.trim() == resolved_id {
                let _ = fs::remove_file(&marker);
                pins_removed = true;
            }
        }
    }

    let pins_file = get_pins_file(app_dir);
    if pins_file.is_file() {
        if let Ok(content) = fs::read_to_string(&pins_file) {
            if let Ok(mut pins) =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
            {
                let mut changed = false;
                pins.retain(|_, v| {
                    if let Some(s) = v.as_str() {
                        if s == resolved_id {
                            changed = true;
                            return false;
                        }
                    }
                    true
                });
                if changed {
                    let _ = fs::write(
                        &pins_file,
                        serde_json::to_string_pretty(&pins).unwrap_or_default(),
                    );
                    pins_removed = true;
                }
            }
        }
    }

    if !titles.is_empty() {
        let mut titles = titles;
        if titles.remove(&resolved_id).is_some() {
            let _ = save_title_overrides(app_dir, &titles);
            pins_removed = true;
        }
    }

    Ok(DeleteResult {
        conversation_id: resolved_id,
        title,
        db_deleted,
        folder_deleted,
        pins_removed,
    })
}
