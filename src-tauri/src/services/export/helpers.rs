use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::models::Track;

use rusqlite::Connection;

/// Get all descendant playlist IDs (recursive implementation)
pub(super) fn get_all_descendant_playlist_ids_impl(
    conn: &Connection,
    parent_id: &str,
) -> Result<Vec<String>> {
    let mut result = Vec::new();

    let mut stmt = conn
        .prepare("SELECT id, is_folder FROM playlists WHERE parent_id = ?1 ORDER BY sort_order")?;

    let children: Vec<(String, bool)> = stmt
        .query_map([parent_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for (child_id, is_folder) in children {
        result.push(child_id.clone());
        if is_folder {
            result.extend(get_all_descendant_playlist_ids_impl(conn, &child_id)?);
        }
    }

    Ok(result)
}

/// Sanitize a path component by replacing invalid characters
pub(super) fn sanitize_path_component(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    if sanitized.is_empty() || sanitized.trim().is_empty() {
        "Unknown".to_string()
    } else {
        sanitized.trim().to_string()
    }
}

/// Build the USB path for a track: Contents/{Artist}/{Album}/{filename}
pub(super) fn build_usb_path(track: &Track) -> String {
    let artist = sanitize_path_component(track.artist.as_deref().unwrap_or("Unknown Artist"));
    let album = sanitize_path_component(track.album.as_deref().unwrap_or("Unknown Album"));
    let filename = Path::new(&track.file_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    format!("{artist}/{album}/{filename}")
}

/// Recursively remove empty directories
pub(super) fn cleanup_empty_dirs(path: &Path) {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                cleanup_empty_dirs(&entry_path);
                // Try to remove if empty
                let _ = fs::remove_dir(&entry_path);
            }
        }
    }
}
