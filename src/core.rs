use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub steps: i64,
    pub mtime: String,
    pub uris: Vec<String>,
    pub primary_workspace: String,
    pub status: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactInfo {
    pub name: String,
    pub size: u64,
    pub mtime: u64,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainStats {
    pub exists: bool,
    pub path: PathBuf,
    pub md_count: usize,
    pub png_count: usize,
    pub scratch_count: usize,
    pub total_files: usize,
    pub recent_artifacts: Vec<ArtifactInfo>,
}

pub fn get_default_app_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("AGY_APP_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return p;
        }
    }
    if let Ok(dir) = std::env::var("ANTIGRAVITY_APP_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return p;
        }
    }

    if let Some(home) = dirs::home_dir() {
        let gemini_path = home.join(".gemini").join("antigravity-cli");
        if gemini_path.is_dir() {
            return gemini_path;
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = dirs::data_local_dir() {
            let agy_path = local_app_data.join("agy");
            if agy_path.is_dir() {
                return agy_path;
            }
        }
        if let Some(config_dir) = dirs::config_dir() {
            let agy_roaming = config_dir.join("antigravity-cli");
            if agy_roaming.is_dir() {
                return agy_roaming;
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            let mac_path = home
                .join("Library")
                .join("Application Support")
                .join("antigravity-cli");
            if mac_path.is_dir() {
                return mac_path;
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(data_dir) = dirs::data_dir() {
            let linux_path = data_dir.join("antigravity-cli");
            if linux_path.is_dir() {
                return linux_path;
            }
        }
        if let Some(config_dir) = dirs::config_dir() {
            let linux_config = config_dir.join("antigravity-cli");
            if linux_config.is_dir() {
                return linux_config;
            }
        }
    }

    dirs::home_dir()
        .map(|h| h.join(".gemini").join("antigravity-cli"))
        .unwrap_or_else(|| PathBuf::from(".gemini/antigravity-cli"))
}

pub fn get_db_path(app_dir: Option<&Path>) -> PathBuf {
    match app_dir {
        Some(p) => p.join("conversation_summaries.db"),
        None => get_default_app_dir().join("conversation_summaries.db"),
    }
}

pub fn get_pins_file(app_dir: Option<&Path>) -> PathBuf {
    match app_dir {
        Some(p) => p.join("workspace_pins.json"),
        None => get_default_app_dir().join("workspace_pins.json"),
    }
}

pub fn get_brain_root(app_dir: Option<&Path>) -> PathBuf {
    match app_dir {
        Some(p) => p.join("brain"),
        None => get_default_app_dir().join("brain"),
    }
}

pub fn normalize_workspace_path(path_str: &str) -> String {
    if path_str.is_empty() {
        return String::new();
    }
    let mut clean = path_str.trim();
    if clean.starts_with("file:///") {
        clean = &clean[8..];
    } else if clean.starts_with("file://") {
        clean = &clean[7..];
    }
    let mut s = clean.replace('\\', "/");
    while s.ends_with('/') && s.len() > 1 {
        s.pop();
    }
    #[cfg(target_os = "windows")]
    {
        s = s.to_lowercase();
    }
    s
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

    // 1. Walk up directory tree looking for .brain
    let mut cur = ws.clone();
    loop {
        let candidate = cur.join(".brain");
        if candidate.is_file() {
            if let Ok(content) = fs::read_to_string(&candidate) {
                let trimmed = content.trim();
                if trimmed.len() >= 30 && trimmed.contains('-') {
                    return Some(trimmed.to_string());
                }
            }
        }
        match cur.parent() {
            Some(p) if p != cur => cur = p.to_path_buf(),
            _ => break,
        }
    }

    // 2. Global workspace_pins.json
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

    // Write global pins
    let pins_file = get_pins_file(app_dir);
    let mut pins = if pins_file.is_file() {
        let content = fs::read_to_string(&pins_file).unwrap_or_else(|_| "{}".to_string());
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
            .unwrap_or_default()
    } else {
        serde_json::Map::new()
    };

    pins.insert(norm_ws, serde_json::Value::String(brain_id.to_string()));

    if let Some(parent) = pins_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&pins_file, serde_json::to_string_pretty(&pins)?)?;

    // Also write local .brain
    let _ = fs::write(ws.join(".brain"), format!("{}\n", brain_id.trim()));

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
                if pins.remove(&norm_ws).is_some() {
                    let _ = fs::write(
                        &pins_file,
                        serde_json::to_string_pretty(&pins).unwrap_or_default(),
                    );
                    removed = true;
                }
            }
        }
    }

    let dot_brain = ws.join(".brain");
    if dot_brain.is_file() {
        let _ = fs::remove_file(dot_brain);
        removed = true;
    }

    removed
}

pub fn fetch_sessions(
    workspace_path: Option<&Path>,
    all_workspaces: bool,
    app_dir: Option<&Path>,
    db_path: Option<&Path>,
) -> Vec<SessionSummary> {
    let target_db = match db_path {
        Some(p) => p.to_path_buf(),
        None => get_db_path(app_dir),
    };

    if !target_db.is_file() {
        return Vec::new();
    }

    let conn = match Connection::open_with_flags(&target_db, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(_) => match Connection::open(&target_db) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        },
    };

    let norm_ws = workspace_path
        .map(|p| normalize_workspace_path(&p.to_string_lossy()))
        .unwrap_or_default();
    let pinned_id = get_pinned_id_for_workspace(workspace_path, app_dir);

    let query = "
        SELECT conversation_id, title, step_count, last_modified_time, workspace_uris, status
        FROM conversation_summaries
        ORDER BY step_count DESC, last_modified_time DESC
    ";

    let mut stmt = match conn.prepare(query) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let rows = stmt.query_map([], |row| {
        let cid: String = row.get(0)?;
        let title: String = row.get(1).unwrap_or_default();
        let steps: i64 = row.get(2).unwrap_or(0);
        let mtime: String = row.get(3).unwrap_or_default();
        let uris_raw: String = row.get(4).unwrap_or_default();
        let status: String = row.get(5).unwrap_or_default();
        Ok((cid, title, steps, mtime, uris_raw, status))
    });

    let mut results = Vec::new();
    if let Ok(mapped) = rows {
        for item in mapped.flatten() {
            let (cid, title, steps, mtime, uris_raw, status) = item;

            let uris: Vec<String> =
                if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&uris_raw) {
                    parsed
                        .into_iter()
                        .map(|u| normalize_workspace_path(&u))
                        .collect()
                } else if !uris_raw.is_empty() {
                    vec![normalize_workspace_path(&uris_raw)]
                } else {
                    Vec::new()
                };

            let primary_ws = uris.first().cloned().unwrap_or_default();

            let is_match = if all_workspaces || norm_ws.is_empty() {
                true
            } else {
                uris.iter().any(|u| {
                    norm_ws == *u
                        || norm_ws.starts_with(&format!("{}/", u))
                        || u.starts_with(&format!("{}/", norm_ws))
                })
            };

            if is_match {
                let is_pinned = pinned_id.as_deref() == Some(&cid);
                let clean_mtime = if mtime.len() >= 19 {
                    mtime[..19].replace('T', " ")
                } else {
                    mtime
                };
                results.push(SessionSummary {
                    id: cid,
                    title: if title.trim().is_empty() {
                        "(sem titulo)".to_string()
                    } else {
                        title.trim().to_string()
                    },
                    steps,
                    mtime: clean_mtime,
                    uris,
                    primary_workspace: primary_ws,
                    status,
                    pinned: is_pinned,
                });
            }
        }
    }

    results.sort_by(|a, b| b.pinned.cmp(&a.pinned).then_with(|| b.steps.cmp(&a.steps)));

    results
}

pub fn get_brain_stats(brain_id: &str, app_dir: Option<&Path>) -> Option<BrainStats> {
    let brain_dir = get_brain_root(app_dir).join(brain_id);
    if !brain_dir.is_dir() {
        return None;
    }

    let mut stats = BrainStats {
        exists: true,
        path: brain_dir.clone(),
        md_count: 0,
        png_count: 0,
        scratch_count: 0,
        total_files: 0,
        recent_artifacts: Vec::new(),
    };

    if let Ok(entries) = fs::read_dir(&brain_dir) {
        let mut md_artifacts = Vec::new();
        for entry in entries.flatten() {
            stats.total_files += 1;
            let path = entry.path();
            if path.is_file() {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if name.ends_with(".md") && !name.ends_with(".metadata.json") {
                    stats.md_count += 1;
                    if let Ok(meta) = path.metadata() {
                        let sz = meta.len();
                        let mt = meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        md_artifacts.push(ArtifactInfo {
                            name,
                            size: sz,
                            mtime: mt,
                            category: "root".to_string(),
                        });
                    }
                } else if name.ends_with(".png") {
                    stats.png_count += 1;
                }
            }
        }
        md_artifacts.sort_by_key(|a| std::cmp::Reverse(a.mtime));
        md_artifacts.truncate(10);
        stats.recent_artifacts = md_artifacts;
    }

    let scratch_dir = brain_dir.join("scratch");
    if scratch_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(scratch_dir) {
            stats.scratch_count = entries.flatten().count();
        }
    }

    Some(stats)
}

pub fn resolve_brain_target(
    target: Option<&str>,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
) -> Option<String> {
    let t = target.unwrap_or("default").trim();

    if t.len() >= 30 && t.contains('-') {
        return Some(t.to_string());
    }

    let mut sessions = fetch_sessions(workspace_path, false, app_dir, None);
    if sessions.is_empty() {
        sessions = fetch_sessions(workspace_path, true, app_dir, None);
    }

    if let Ok(idx) = t.parse::<usize>() {
        if idx < sessions.len() {
            return Some(sessions[idx].id.clone());
        }
        return None;
    }

    // Try prefix match on session ids
    for s in &sessions {
        if s.id.starts_with(t) {
            return Some(s.id.clone());
        }
    }

    if t == "default" {
        if let Some(pinned) = get_pinned_id_for_workspace(workspace_path, app_dir) {
            return Some(pinned);
        }
        if let Some(first) = sessions.first() {
            return Some(first.id.clone());
        }
    }

    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteResult {
    pub conversation_id: String,
    pub title: String,
    pub db_deleted: bool,
    pub folder_deleted: bool,
    pub pins_removed: bool,
}

pub fn delete_session(
    target: &str,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
    db_path: Option<&Path>,
) -> Result<DeleteResult, Box<dyn std::error::Error>> {
    let target_db = match db_path {
        Some(p) => p.to_path_buf(),
        None => get_db_path(app_dir),
    };

    let resolved_id = resolve_brain_target(Some(target), workspace_path, app_dir)
        .ok_or_else(|| format!("Sessao '{}' nao encontrada.", target))?;

    let mut title = String::new();
    let mut db_deleted = false;

    if target_db.is_file() {
        if let Ok(conn) = Connection::open(&target_db) {
            if let Ok(t) = conn.query_row(
                "SELECT title FROM conversation_summaries WHERE conversation_id = ?1",
                rusqlite::params![&resolved_id],
                |row| row.get::<_, String>(0),
            ) {
                title = t;
            }

            if let Ok(rows) = conn.execute(
                "DELETE FROM conversation_summaries WHERE conversation_id = ?1",
                rusqlite::params![&resolved_id],
            ) {
                db_deleted = rows > 0;
            }

            // Clean up full text search index if present
            let _ = conn.execute(
                "DELETE FROM conversations_fts WHERE conversation_id = ?1",
                rusqlite::params![&resolved_id],
            );
        }
    }

    // Remove folder on disk
    let brain_dir = get_brain_root(app_dir).join(&resolved_id);
    let mut folder_deleted = false;
    if brain_dir.is_dir() && fs::remove_dir_all(&brain_dir).is_ok() {
        folder_deleted = true;
    }

    // Clean up pins
    let mut pins_removed = false;
    let ws = match workspace_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    let dot_brain = ws.join(".brain");
    if dot_brain.is_file() {
        if let Ok(content) = fs::read_to_string(&dot_brain) {
            if content.trim() == resolved_id {
                let _ = fs::remove_file(dot_brain);
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

    Ok(DeleteResult {
        conversation_id: resolved_id,
        title,
        db_deleted,
        folder_deleted,
        pins_removed,
    })
}
