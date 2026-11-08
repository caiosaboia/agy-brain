use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
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
    pub is_subagent: bool,
    pub agent_name: String,
    pub parent_id: String,
    pub parent_title: Option<String>,
    pub nesting_depth: i64,
    pub subagent_count: usize,
    pub title_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrunedSession {
    pub id: String,
    pub steps: i64,
    pub mtime: String,
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

fn decode_percent_encoding(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn normalize_workspace_path(path_str: &str) -> String {
    if path_str.is_empty() {
        return String::new();
    }
    let decoded = decode_percent_encoding(path_str.trim());
    let mut clean = decoded.as_str();
    if let Some(stripped) = clean.strip_prefix("file:///") {
        clean = stripped;
    } else if let Some(stripped) = clean.strip_prefix("file://") {
        clean = stripped;
    }
    let mut s = clean.replace('\\', "/");
    while s.starts_with('/') {
        s.remove(0);
    }
    while s.ends_with('/') && !s.is_empty() {
        s.pop();
    }
    #[cfg(target_os = "windows")]
    {
        s = s.to_lowercase();
    }
    #[cfg(not(target_os = "windows"))]
    {
        if s.len() >= 2 && s.as_bytes()[0].is_ascii_alphabetic() && s.as_bytes()[1] == b':' {
            let mut chars: Vec<char> = s.chars().collect();
            chars[0] = chars[0].to_ascii_lowercase();
            s = chars.into_iter().collect();
        }
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

    // 1. Check .brain strictly in the target directory (no parent directory inheritance)
    let candidate = ws.join(".brain");
    if candidate.is_file() {
        if let Ok(content) = fs::read_to_string(&candidate) {
            let trimmed = content.trim();
            if trimmed.len() >= 30 && trimmed.contains('-') {
                return Some(trimmed.to_string());
            }
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

    pins.insert(
        norm_ws.clone(),
        serde_json::Value::String(brain_id.to_string()),
    );

    if let Some(parent) = pins_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&pins_file, serde_json::to_string_pretty(&pins)?)?;

    // Also write local .brain
    let _ = fs::write(ws.join(".brain"), format!("{}\n", brain_id.trim()));

    // Link workspace to session's workspace_uris in SQLite if present
    let target_db = get_db_path(app_dir);
    if target_db.is_file() {
        if let Ok(conn) = Connection::open(&target_db) {
            if let Ok(uris_raw) = conn.query_row(
                "SELECT workspace_uris FROM conversation_summaries WHERE conversation_id = ?1",
                rusqlite::params![brain_id.trim()],
                |r| r.get::<_, String>(0),
            ) {
                let mut uri_list: Vec<String> = serde_json::from_str(&uris_raw).unwrap_or_default();
                let already_linked = uri_list
                    .iter()
                    .any(|u| normalize_workspace_path(u) == norm_ws);
                if !already_linked {
                    let clean_slash = ws.to_string_lossy().replace('\\', "/");
                    let file_uri = if clean_slash.starts_with('/') {
                        format!("file://{}", clean_slash)
                    } else {
                        format!("file:///{}", clean_slash)
                    };
                    uri_list.push(file_uri);
                    if let Ok(new_json) = serde_json::to_string(&uri_list) {
                        let _ = conn.execute(
                            "UPDATE conversation_summaries SET workspace_uris = ?1 WHERE conversation_id = ?2",
                            rusqlite::params![new_json, brain_id.trim()],
                        );
                    }
                }
            }
        }
    }

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

    let dot_brain = ws.join(".brain");
    if dot_brain.is_file() {
        let _ = fs::remove_file(dot_brain);
        removed = true;
    }

    removed
}

pub fn truncate_str(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        return s.to_string();
    }
    if max_chars <= 3 {
        return s.chars().take(max_chars).collect();
    }
    let prefix: String = s.chars().take(max_chars - 3).collect();
    format!("{}...", prefix)
}

pub fn clean_single_line(s: &str, max_chars: usize) -> String {
    let first_line = s
        .lines()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let collapsed = first_line.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_str(&collapsed, max_chars)
}

pub fn extract_role_from_raw_summary(raw: &[u8], agent_name: &str) -> Option<String> {
    if !agent_name.is_empty() && agent_name.len() < 128 {
        let mut needle = Vec::with_capacity(agent_name.len() + 3);
        needle.push(b'\n');
        needle.push(agent_name.len() as u8);
        needle.extend_from_slice(agent_name.as_bytes());
        needle.push(0x12);

        if let Some(pos) = raw.windows(needle.len()).position(|w| w == needle) {
            let len_idx = pos + needle.len();
            if let Some(&role_len_byte) = raw.get(len_idx) {
                let role_len = role_len_byte as usize;
                if role_len > 0 && len_idx + 1 + role_len <= raw.len() {
                    if let Ok(role_str) =
                        std::str::from_utf8(&raw[len_idx + 1..len_idx + 1 + role_len])
                    {
                        let trimmed = role_str.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
        }
    }

    // Fallback scan for Protobuf field 8 (`B` + len + `\n` + ag_len + ag + `\x12` + role_len + role)
    for i in 0..raw.len().saturating_sub(6) {
        if raw[i] == b'B' && raw.get(i + 2) == Some(&b'\n') {
            let ag_len = raw[i + 3] as usize;
            let sep_idx = i + 4 + ag_len;
            if ag_len > 0 && ag_len < 64 && raw.get(sep_idx) == Some(&0x12) {
                if let Some(&role_len_byte) = raw.get(sep_idx + 1) {
                    let role_len = role_len_byte as usize;
                    let start = sep_idx + 2;
                    if role_len > 0 && role_len < 128 && start + role_len <= raw.len() {
                        if let Ok(role_str) = std::str::from_utf8(&raw[start..start + role_len]) {
                            let trimmed = role_str.trim();
                            if !trimmed.is_empty() && trimmed.chars().all(|c| !c.is_control()) {
                                return Some(trimmed.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn extract_title_from_transcript(brain_root: &Path, cid: &str) -> Option<String> {
    let trans_path = brain_root
        .join(cid)
        .join(".system_generated")
        .join("logs")
        .join("transcript.jsonl");
    let file = fs::File::open(trans_path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(40).flatten() {
        if !line.contains("USER_INPUT") {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
            if val.get("type").and_then(|v| v.as_str()) == Some("USER_INPUT") {
                if let Some(content) = val.get("content").and_then(|v| v.as_str()) {
                    let mut slice = content;
                    if let Some(start) = slice.find("<USER_REQUEST>") {
                        slice = &slice[start + "<USER_REQUEST>".len()..];
                        if let Some(end) = slice.find("</USER_REQUEST>") {
                            slice = &slice[..end];
                        }
                    }
                    let cleaned = clean_single_line(slice, 100);
                    if !cleaned.is_empty() {
                        return Some(cleaned);
                    }
                }
            }
        }
    }
    None
}

fn brain_has_files(brain_root: &Path, cid: &str) -> bool {
    let dir = brain_root.join(cid);
    if !dir.is_dir() {
        return false;
    }
    walkdir::WalkDir::new(dir)
        .max_depth(4)
        .into_iter()
        .flatten()
        .any(|e| e.file_type().is_file())
}

pub fn resolve_session_title(
    title: &str,
    preview: &str,
    raw_summary: Option<&[u8]>,
    agent_name: &str,
    brain_root: &Path,
    cid: &str,
) -> (String, String) {
    if !title.trim().is_empty() {
        return (clean_single_line(title, 100), "title".to_string());
    }
    if !preview.trim().is_empty() {
        return (clean_single_line(preview, 100), "preview".to_string());
    }
    if let Some(raw) = raw_summary {
        if let Some(role) = extract_role_from_raw_summary(raw, agent_name) {
            return (clean_single_line(&role, 100), "raw_summary".to_string());
        }
    }
    if let Some(trans_title) = extract_title_from_transcript(brain_root, cid) {
        return (trans_title, "transcript".to_string());
    }
    ("(sem titulo)".to_string(), "empty".to_string())
}

struct RawRow {
    cid: String,
    title: String,
    preview: String,
    steps: i64,
    mtime: String,
    uris_raw: String,
    status: String,
    agent_name: String,
    parent_id: String,
    nesting_depth: i64,
    raw_summary: Option<Vec<u8>>,
}

fn read_all_raw_rows(conn: &Connection) -> Vec<RawRow> {
    let full_query = "
        SELECT conversation_id, title, preview, step_count, last_modified_time, workspace_uris, status,
               agent_name, parent_conversation_id, nesting_depth, raw_summary
        FROM conversation_summaries
        ORDER BY step_count DESC, last_modified_time DESC
    ";

    if let Ok(mut stmt) = conn.prepare(full_query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok(RawRow {
                cid: row.get(0)?,
                title: row.get(1).unwrap_or_default(),
                preview: row.get(2).unwrap_or_default(),
                steps: row.get(3).unwrap_or(0),
                mtime: row.get(4).unwrap_or_default(),
                uris_raw: row.get(5).unwrap_or_default(),
                status: row.get(6).unwrap_or_default(),
                agent_name: row.get(7).unwrap_or_default(),
                parent_id: row.get(8).unwrap_or_default(),
                nesting_depth: row.get(9).unwrap_or(0),
                raw_summary: row.get(10).ok(),
            })
        }) {
            return rows.flatten().collect();
        }
    }

    let fallback_query = "
        SELECT conversation_id, title, preview, step_count, last_modified_time, workspace_uris, status
        FROM conversation_summaries
        ORDER BY step_count DESC, last_modified_time DESC
    ";

    if let Ok(mut stmt) = conn.prepare(fallback_query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok(RawRow {
                cid: row.get(0)?,
                title: row.get(1).unwrap_or_default(),
                preview: row.get(2).unwrap_or_default(),
                steps: row.get(3).unwrap_or(0),
                mtime: row.get(4).unwrap_or_default(),
                uris_raw: row.get(5).unwrap_or_default(),
                status: row.get(6).unwrap_or_default(),
                agent_name: String::new(),
                parent_id: String::new(),
                nesting_depth: 0,
                raw_summary: None,
            })
        }) {
            return rows.flatten().collect();
        }
    }

    Vec::new()
}

pub fn fetch_sessions(
    workspace_path: Option<&Path>,
    all_workspaces: bool,
    app_dir: Option<&Path>,
    db_path: Option<&Path>,
) -> Vec<SessionSummary> {
    fetch_sessions_ext(workspace_path, all_workspaces, false, app_dir, db_path)
}

pub fn fetch_sessions_ext(
    workspace_path: Option<&Path>,
    all_workspaces: bool,
    include_subagents: bool,
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
    let brain_root = get_brain_root(app_dir);

    let raw_rows = read_all_raw_rows(&conn);

    let mut subagent_counts: HashMap<String, usize> = HashMap::new();
    let mut title_map: HashMap<String, String> = HashMap::new();

    for r in &raw_rows {
        let is_sub = r.nesting_depth > 0 || !r.parent_id.is_empty();
        if is_sub && !r.parent_id.is_empty() {
            *subagent_counts.entry(r.parent_id.clone()).or_insert(0) += 1;
        }
        let (res_title, _) = resolve_session_title(
            &r.title,
            &r.preview,
            r.raw_summary.as_deref(),
            &r.agent_name,
            &brain_root,
            &r.cid,
        );
        title_map.insert(r.cid.clone(), res_title);
    }

    let mut results = Vec::new();
    for r in raw_rows {
        let is_subagent = r.nesting_depth > 0 || !r.parent_id.is_empty();
        if is_subagent && !include_subagents {
            continue;
        }

        let (resolved_title, title_source) = resolve_session_title(
            &r.title,
            &r.preview,
            r.raw_summary.as_deref(),
            &r.agent_name,
            &brain_root,
            &r.cid,
        );

        let is_pinned = pinned_id.as_deref() == Some(&r.cid);

        // Filter ghost sessions (0 steps or no title/content AND no files in brain folder)
        if !is_pinned
            && !brain_has_files(&brain_root, &r.cid)
            && (r.steps == 0 || title_source == "empty")
        {
            continue;
        }

        let uris: Vec<String> = if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&r.uris_raw)
        {
            parsed
                .into_iter()
                .map(|u| normalize_workspace_path(&u))
                .collect()
        } else if !r.uris_raw.is_empty() {
            vec![normalize_workspace_path(&r.uris_raw)]
        } else {
            Vec::new()
        };

        let primary_ws = uris.first().cloned().unwrap_or_default();

        let is_match = if all_workspaces || norm_ws.is_empty() {
            true
        } else {
            is_pinned || uris.contains(&norm_ws)
        };

        if is_match {
            let clean_mtime = if r.mtime.len() >= 19 {
                r.mtime[..19].replace('T', " ")
            } else {
                r.mtime
            };
            let parent_title = if !r.parent_id.is_empty() {
                title_map.get(&r.parent_id).cloned()
            } else {
                None
            };
            let subagent_count = subagent_counts.get(&r.cid).copied().unwrap_or(0);

            results.push(SessionSummary {
                id: r.cid,
                title: resolved_title,
                steps: r.steps,
                mtime: clean_mtime,
                uris,
                primary_workspace: primary_ws,
                status: r.status,
                pinned: is_pinned,
                is_subagent,
                agent_name: r.agent_name,
                parent_id: r.parent_id,
                parent_title,
                nesting_depth: r.nesting_depth,
                subagent_count,
                title_source,
            });
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

    let sessions = fetch_sessions(workspace_path, false, app_dir, None);

    if let Ok(idx) = t.parse::<usize>() {
        if idx < sessions.len() {
            return Some(sessions[idx].id.clone());
        }
        if t.len() < 4 {
            return None;
        }
    }

    if t == "default" {
        if let Some(pinned) = get_pinned_id_for_workspace(workspace_path, app_dir) {
            return Some(pinned);
        }
        if let Some(first) = sessions.first() {
            return Some(first.id.clone());
        }
        return None;
    }

    // Try prefix match on session ids (including subagents/global if explicit prefix given)
    for s in &sessions {
        if s.id.starts_with(t) {
            return Some(s.id.clone());
        }
    }
    let all_with_sub = fetch_sessions_ext(workspace_path, true, true, app_dir, None);
    for s in &all_with_sub {
        if s.id.starts_with(t) {
            return Some(s.id.clone());
        }
    }

    None
}

pub fn rename_session(
    target: &str,
    new_title: &str,
    workspace_path: Option<&Path>,
    app_dir: Option<&Path>,
    db_path: Option<&Path>,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let clean_title = clean_single_line(new_title, 120);
    if clean_title.is_empty() {
        return Err("O novo titulo nao pode ser vazio.".into());
    }

    let target_db = match db_path {
        Some(p) => p.to_path_buf(),
        None => get_db_path(app_dir),
    };

    let resolved_id = resolve_brain_target(Some(target), workspace_path, app_dir)
        .ok_or_else(|| format!("Sessao '{}' nao encontrada.", target))?;

    let conn = Connection::open(&target_db)?;
    let updated = conn.execute(
        "UPDATE conversation_summaries SET title = ?1 WHERE conversation_id = ?2",
        rusqlite::params![&clean_title, &resolved_id],
    )?;

    if updated == 0 {
        return Err(format!("Sessao '{}' nao encontrada no banco SQLite.", resolved_id).into());
    }

    let _ = conn.execute(
        "UPDATE conversations_fts SET title = ?1 WHERE conversation_id = ?2",
        rusqlite::params![&clean_title, &resolved_id],
    );

    Ok((resolved_id, clean_title))
}

pub fn prune_sessions(
    dry_run: bool,
    app_dir: Option<&Path>,
    db_path: Option<&Path>,
) -> Result<Vec<PrunedSession>, Box<dyn std::error::Error>> {
    let target_db = match db_path {
        Some(p) => p.to_path_buf(),
        None => get_db_path(app_dir),
    };

    if !target_db.is_file() {
        return Ok(Vec::new());
    }

    let conn = Connection::open(&target_db)?;
    let brain_root = get_brain_root(app_dir);
    let raw_rows = read_all_raw_rows(&conn);

    let mut pruned = Vec::new();
    for r in raw_rows {
        let (_, title_source) = resolve_session_title(
            &r.title,
            &r.preview,
            r.raw_summary.as_deref(),
            &r.agent_name,
            &brain_root,
            &r.cid,
        );

        if !brain_has_files(&brain_root, &r.cid) && (r.steps == 0 || title_source == "empty") {
            let clean_mtime = if r.mtime.len() >= 19 {
                r.mtime[..19].replace('T', " ")
            } else {
                r.mtime
            };
            if !dry_run {
                let _ = conn.execute(
                    "DELETE FROM conversation_summaries WHERE conversation_id = ?1",
                    rusqlite::params![&r.cid],
                );
                let _ = conn.execute(
                    "DELETE FROM conversations_fts WHERE conversation_id = ?1",
                    rusqlite::params![&r.cid],
                );
                let ghost_dir = brain_root.join(&r.cid);
                if ghost_dir.is_dir() {
                    let _ = fs::remove_dir_all(&ghost_dir);
                }
            }
            pruned.push(PrunedSession {
                id: r.cid,
                steps: r.steps,
                mtime: clean_mtime,
            });
        }
    }

    Ok(pruned)
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
