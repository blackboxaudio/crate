//! Library-root path resolution.
//!
//! A synced track carries a logical `(library_root_id, relative_path)` pair. Each
//! device maps a `library_root_id` to a local absolute folder in the device-local
//! `sync_root_mappings` table. Resolution joins the two; when no mapping resolves
//! (or the file is missing), the track is `Unavailable` on this device and the UI
//! renders it dimmed with a "Locate" affordance.
//!
//! Tracks with no root association fall back to their absolute `file_path` — which
//! resolves on the device that imported them and shows `Unavailable` elsewhere
//! (the "sync as Unavailable" behaviour).

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

use crate::error::Result;

use super::pipeline::{buckets, dirty};

/// Where a track resolves on THIS device.
pub enum ResolvedPath {
    /// Present and playable at this absolute path.
    Playable(PathBuf),
    /// No local file resolves (unmapped root, or missing file). Show dimmed.
    Unavailable,
}

/// Resolve a track to a playable absolute path on this device.
pub fn resolve_track_path(
    conn: &Connection,
    library_root_id: Option<&str>,
    relative_path: Option<&str>,
    file_path: &str,
) -> Result<ResolvedPath> {
    if let (Some(root_id), Some(rel)) = (library_root_id, relative_path) {
        let local_root: Option<String> = conn
            .query_row(
                "SELECT local_absolute_path FROM sync_root_mappings WHERE library_root_id = ?1",
                [root_id],
                |r| r.get(0),
            )
            .optional()?;
        return Ok(match local_root {
            Some(root) => {
                let abs = Path::new(&root).join(rel);
                if abs.exists() {
                    ResolvedPath::Playable(abs)
                } else {
                    ResolvedPath::Unavailable
                }
            }
            None => ResolvedPath::Unavailable,
        });
    }

    let p = PathBuf::from(file_path);
    Ok(if p.exists() {
        ResolvedPath::Playable(p)
    } else {
        ResolvedPath::Unavailable
    })
}

/// At import time, decide whether `file_path` falls under a registered library
/// root (via its local mapping). Returns `(library_root_id, relative_path)`, or
/// `(None, None)` when no root matches (a device-local track). Until a user sets
/// up roots in the wizard (Phase 4) there are no mappings, so this returns
/// `(None, None)` and tracks are stored with their absolute path only.
pub fn assign_root_for_import(
    conn: &Connection,
    file_path: &str,
) -> Result<(Option<String>, Option<String>)> {
    // Best-effort: never fail an import because root resolution is unavailable.
    Ok(
        try_assign_root_for_import(conn, file_path).unwrap_or_else(|e| {
            log::warn!("cloud_sync: root assignment skipped, track stored device-local: {e}");
            (None, None)
        }),
    )
}

fn try_assign_root_for_import(
    conn: &Connection,
    file_path: &str,
) -> Result<(Option<String>, Option<String>)> {
    let mut stmt =
        conn.prepare("SELECT library_root_id, local_absolute_path FROM sync_root_mappings")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for (root_id, local_root) in rows {
        if let Ok(rel) = Path::new(file_path).strip_prefix(&local_root) {
            return Ok((Some(root_id), Some(rel.to_string_lossy().to_string())));
        }
    }
    Ok((None, None))
}

// --- library_roots CRUD (synced entity) ---

/// Register a new library root (synced). Returns its id.
pub fn register_root(conn: &Connection, name: &str) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let hlc = dirty::next_hlc(conn)?;
    conn.execute(
        "INSERT INTO library_roots (id, name, _hlc) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, name, hlc],
    )?;
    dirty::mark_dirty(conn, buckets::LIBRARY_ROOTS)?;
    Ok(id)
}

/// Rename a library root (synced).
pub fn rename_root(conn: &Connection, id: &str, name: &str) -> Result<()> {
    let hlc = dirty::next_hlc(conn)?;
    conn.execute(
        "UPDATE library_roots SET name = ?1, _hlc = ?2 WHERE id = ?3",
        rusqlite::params![name, hlc, id],
    )?;
    dirty::mark_dirty(conn, buckets::LIBRARY_ROOTS)?;
    Ok(())
}

/// Delete a library root (synced) — records a tombstone.
pub fn remove_root(conn: &Connection, id: &str) -> Result<()> {
    let hlc = dirty::next_hlc(conn)?;
    dirty::record_tombstone(conn, buckets::LIBRARY_ROOTS, id, &hlc)?;
    conn.execute("DELETE FROM library_roots WHERE id = ?1", [id])?;
    dirty::mark_dirty(conn, buckets::LIBRARY_ROOTS)?;
    Ok(())
}

/// Map a library root to a local absolute folder on this device. Device-local —
/// NOT synced, so no HLC and no dirty mark.
pub fn set_root_mapping(conn: &Connection, root_id: &str, local_path: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_root_mappings (library_root_id, local_absolute_path) VALUES (?1, ?2)
         ON CONFLICT(library_root_id) DO UPDATE SET local_absolute_path = excluded.local_absolute_path",
        rusqlite::params![root_id, local_path],
    )?;
    Ok(())
}
