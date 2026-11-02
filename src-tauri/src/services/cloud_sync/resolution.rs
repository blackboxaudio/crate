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
    // Longest prefix wins: the row order is otherwise unspecified, so a broad root
    // could shadow a nested root arbitrarily. Preferring the most specific mapping
    // (e.g. `<root>/inner` over `<root>`) makes nested roots resolve deterministically.
    let mut stmt = conn.prepare(
        "SELECT library_root_id, local_absolute_path FROM sync_root_mappings
         ORDER BY length(local_absolute_path) DESC",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Canonicalize the candidate the same way `set_root_mapping` canonicalizes the
    // stored prefix. Canonicalizing only the mapping side would make a file reached
    // through a symlink fail to match its own root. A deleted or not-yet-existing file
    // falls back to the raw path, keeping the previous best-effort behaviour.
    let canonical_file = std::fs::canonicalize(file_path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file_path.to_string());

    for (root_id, local_root) in rows {
        if let Ok(rel) = Path::new(&canonical_file).strip_prefix(&local_root) {
            return Ok((Some(root_id), Some(rel.to_string_lossy().to_string())));
        }
    }
    Ok((None, None))
}

// --- library_roots CRUD (synced entity) ---

/// Ensure a `library_roots` row exists for `id`, inserting it only when absent.
///
/// The id is caller-supplied because the id is what keeps one logical root stable
/// across devices: `sync_root_mappings` is keyed by `library_root_id`, so the same
/// synced root can map to a different local folder on each device. A generated id
/// would create a new logical root on every call.
pub fn ensure_root(conn: &Connection, id: &str, name: &str) -> Result<()> {
    let hlc = dirty::next_hlc(conn)?;
    let inserted = conn.execute(
        "INSERT INTO library_roots (id, name, _hlc) VALUES (?1, ?2, ?3)
         ON CONFLICT(id) DO NOTHING",
        rusqlite::params![id, name, hlc],
    )?;
    // Only a real insert changes the synced entity; an idempotent no-op must not mark
    // the bucket dirty.
    if inserted > 0 {
        dirty::mark_dirty(conn, buckets::LIBRARY_ROOTS)?;
    }
    Ok(())
}

/// Register a new library root (synced). Returns its id.
pub fn register_root(conn: &Connection, name: &str) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    ensure_root(conn, &id, name)?;
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

/// Read the local absolute folder mapped to `root_id` on this device, if any.
pub fn root_mapping(conn: &Connection, root_id: &str) -> Result<Option<String>> {
    let path = conn
        .query_row(
            "SELECT local_absolute_path FROM sync_root_mappings WHERE library_root_id = ?1",
            [root_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(path)
}

/// Map a library root to a local absolute folder on this device. Device-local —
/// NOT synced, so no HLC and no dirty mark.
pub fn set_root_mapping(conn: &Connection, root_id: &str, local_path: &str) -> Result<()> {
    // The walk uses the stored value as its root prefix and relies on `strip_prefix`
    // to derive relative paths, so the stored form must be canonical; otherwise a path
    // reached through a symlink or a `..` segment would not match its own root. Fall
    // back to the raw string when canonicalization fails (e.g. the folder does not
    // exist yet while restoring a mapping on a new device).
    let canonical = std::fs::canonicalize(local_path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| local_path.to_string());
    conn.execute(
        "INSERT INTO sync_root_mappings (library_root_id, local_absolute_path) VALUES (?1, ?2)
         ON CONFLICT(library_root_id) DO UPDATE SET local_absolute_path = excluded.local_absolute_path",
        rusqlite::params![root_id, canonical],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fresh in-memory device with the real schema applied and FKs enforced.
    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        for sql in crate::db::schema::get_migrations() {
            conn.execute_batch(sql).unwrap();
        }
        conn
    }

    /// Unique temp directory; callers remove it with `remove_dir_all`.
    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("crate-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn nested_roots_resolve_to_most_specific() {
        let conn = test_conn();
        let tmp = temp_dir();
        let inner = tmp.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        let file = inner.join("song.mp3");
        std::fs::write(&file, b"audio").unwrap();

        ensure_root(&conn, "outer", "Outer").unwrap();
        ensure_root(&conn, "inner", "Inner").unwrap();
        set_root_mapping(&conn, "outer", &tmp.to_string_lossy()).unwrap();
        set_root_mapping(&conn, "inner", &inner.to_string_lossy()).unwrap();

        let (root_id, rel) = assign_root_for_import(&conn, &file.to_string_lossy()).unwrap();
        assert_eq!(root_id.as_deref(), Some("inner"));
        assert_eq!(rel.as_deref(), Some("song.mp3"));

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn file_outside_every_mapping_resolves_to_none() {
        let conn = test_conn();
        let tmp = temp_dir();
        let mapped = tmp.join("mapped");
        let outside = tmp.join("outside");
        std::fs::create_dir_all(&mapped).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let file = outside.join("song.mp3");
        std::fs::write(&file, b"audio").unwrap();

        ensure_root(&conn, "root", "Root").unwrap();
        set_root_mapping(&conn, "root", &mapped.to_string_lossy()).unwrap();

        let (root_id, rel) = assign_root_for_import(&conn, &file.to_string_lossy()).unwrap();
        assert_eq!(root_id, None);
        assert_eq!(rel, None);

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn sibling_prefix_does_not_match() {
        let conn = test_conn();
        let tmp = temp_dir();
        let sibling = tmp.with_file_name(format!(
            "{}-other",
            tmp.file_name().unwrap().to_string_lossy()
        ));
        std::fs::create_dir_all(&sibling).unwrap();
        let file = sibling.join("song.mp3");
        std::fs::write(&file, b"audio").unwrap();

        ensure_root(&conn, "root", "Root").unwrap();
        set_root_mapping(&conn, "root", &tmp.to_string_lossy()).unwrap();

        // A naive string prefix would match `<tmp>` against `<tmp>-other`; `strip_prefix`
        // compares path components and must reject it.
        let (root_id, rel) = assign_root_for_import(&conn, &file.to_string_lossy()).unwrap();
        assert_eq!(root_id, None);
        assert_eq!(rel, None);

        std::fs::remove_dir_all(&tmp).unwrap();
        std::fs::remove_dir_all(&sibling).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_path_resolves_to_mapped_root() {
        let conn = test_conn();
        let tmp = temp_dir();
        let real = tmp.join("real");
        std::fs::create_dir_all(&real).unwrap();
        let link = tmp.join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        std::fs::write(real.join("song.mp3"), b"audio").unwrap();

        ensure_root(&conn, "real-root", "Real").unwrap();
        set_root_mapping(&conn, "real-root", &real.to_string_lossy()).unwrap();

        // The file is reached through the symlink; matching its own root requires the
        // candidate to be canonicalized just like the stored mapping was.
        let linked_file = link.join("song.mp3");
        let (root_id, rel) = assign_root_for_import(&conn, &linked_file.to_string_lossy()).unwrap();
        assert_eq!(root_id.as_deref(), Some("real-root"));
        assert_eq!(rel.as_deref(), Some("song.mp3"));

        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
