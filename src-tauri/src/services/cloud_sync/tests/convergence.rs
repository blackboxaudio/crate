//! End-to-end convergence cases: two devices make offline edits, then sync through
//! the mock cloud and must converge to byte-identical state.
//!
//! Mutations are applied via small helpers that stamp `_hlc` with an EXPLICIT
//! wall-clock (`wall`) so the truth-table outcome is deterministic: a higher `wall`
//! is a strictly newer HLC (the format puts `wall_ms` in the most significant
//! position). Each helper also marks the bucket dirty / records a tombstone exactly
//! as a real mutation site would.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::backend::mock::MockCloudBackend;
use crate::services::cloud_sync::backend::types::{GcEntry, Manifest};
use crate::services::cloud_sync::backend::CloudBackend;
use crate::services::cloud_sync::hlc::Hlc;
use crate::services::cloud_sync::pipeline::buckets::{self, Bucket};
use crate::services::cloud_sync::pipeline::dirty;
use crate::services::cloud_sync::pipeline::pull::pull as pull_remote;
use crate::services::cloud_sync::pipeline::push::push as push_remote;
use crate::services::cloud_sync::pipeline::{gc, rows};

use super::{new_device, pull, push, state_hash, test_session};

const DATE: &str = "2020-01-01T00:00:00Z";

fn hlc(node: u32, wall: u64) -> String {
    Hlc::new(wall, 0, node).format()
}

// --- mutation helpers ------------------------------------------------------

fn create_track(conn: &Connection, node: u32, wall: u64, id: &str, title: &str) {
    conn.execute(
        "INSERT INTO tracks (id, file_path, duration_ms, title, date_added, date_modified, _hlc) \
         VALUES (?1, ?2, 1000, ?3, ?4, ?4, ?5)",
        params![id, format!("/music/{id}.mp3"), title, DATE, hlc(node, wall)],
    )
    .unwrap();
    dirty::mark_dirty(conn, &buckets::bucket_for_track_id(id)).unwrap();
}

fn update_track_title(conn: &Connection, node: u32, wall: u64, id: &str, title: &str) {
    conn.execute(
        "UPDATE tracks SET title = ?1, date_modified = ?2, _hlc = ?3 WHERE id = ?4",
        params![title, DATE, hlc(node, wall), id],
    )
    .unwrap();
    dirty::mark_dirty(conn, &buckets::bucket_for_track_id(id)).unwrap();
}

fn delete_track(conn: &Connection, node: u32, wall: u64, id: &str) {
    dirty::record_tombstone(conn, buckets::TRACKS_ENTITY, id, &hlc(node, wall)).unwrap();
    conn.execute("DELETE FROM tracks WHERE id = ?1", [id])
        .unwrap();
    dirty::mark_dirty(conn, &buckets::bucket_for_track_id(id)).unwrap();
}

fn create_playlist(conn: &Connection, node: u32, wall: u64, id: &str, name: &str) {
    conn.execute(
        "INSERT INTO playlists \
            (id, name, parent_id, is_folder, is_smart, smart_rules, sort_order, context, \
             date_created, date_modified, _hlc) \
         VALUES (?1, ?2, NULL, 0, 0, NULL, 0, 'library', ?3, ?3, ?4)",
        params![id, name, DATE, hlc(node, wall)],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::PLAYLISTS).unwrap();
}

fn rename_playlist(conn: &Connection, node: u32, wall: u64, id: &str, name: &str) {
    conn.execute(
        "UPDATE playlists SET name = ?1, _hlc = ?2 WHERE id = ?3",
        params![name, hlc(node, wall), id],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::PLAYLISTS).unwrap();
}

fn add_track_to_playlist(conn: &Connection, node: u32, wall: u64, pl: &str, tr: &str, pos: i32) {
    conn.execute(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position, date_added, _hlc) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(playlist_id, track_id) DO UPDATE SET position = excluded.position, _hlc = excluded._hlc",
        params![pl, tr, pos, DATE, hlc(node, wall)],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::PLAYLIST_TRACKS).unwrap();
}

fn create_tag_category(conn: &Connection, node: u32, wall: u64, id: &str, name: &str) {
    conn.execute(
        "INSERT INTO tag_categories (id, name, sort_order, _hlc) VALUES (?1, ?2, 0, ?3)",
        params![id, name, hlc(node, wall)],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::TAG_CATEGORIES).unwrap();
}

fn create_tag(conn: &Connection, node: u32, wall: u64, id: &str, cat: &str, name: &str) {
    conn.execute(
        "INSERT INTO tags (id, category_id, name, sort_order, _hlc) VALUES (?1, ?2, ?3, 0, ?4)",
        params![id, cat, name, hlc(node, wall)],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::TAGS).unwrap();
}

fn add_track_tag(conn: &Connection, node: u32, wall: u64, tr: &str, tag: &str) {
    conn.execute(
        "INSERT INTO track_tags (track_id, tag_id, _hlc) VALUES (?1, ?2, ?3) \
         ON CONFLICT(track_id, tag_id) DO UPDATE SET _hlc = excluded._hlc",
        params![tr, tag, hlc(node, wall)],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::TRACK_TAGS).unwrap();
}

fn remove_track_tag(conn: &Connection, node: u32, wall: u64, tr: &str, tag: &str) {
    let cid = dirty::junction_entity_id(tr, tag);
    dirty::record_tombstone(conn, buckets::TRACK_TAGS, &cid, &hlc(node, wall)).unwrap();
    conn.execute(
        "DELETE FROM track_tags WHERE track_id = ?1 AND tag_id = ?2",
        params![tr, tag],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::TRACK_TAGS).unwrap();
}

fn set_setting_synced(conn: &Connection, node: u32, wall: u64, key: &str, value: &str) {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![format!("setting_hlc:{key}"), hlc(node, wall)],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::SETTINGS).unwrap();
}

fn set_setting_local(conn: &Connection, key: &str, value: &str) {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .unwrap();
}

// --- assertions / queries --------------------------------------------------

fn playlist_name(conn: &Connection, id: &str) -> Option<String> {
    conn.query_row("SELECT name FROM playlists WHERE id = ?1", [id], |r| {
        r.get(0)
    })
    .optional()
    .unwrap()
}

fn track_title(conn: &Connection, id: &str) -> Option<String> {
    conn.query_row("SELECT title FROM tracks WHERE id = ?1", [id], |r| r.get(0))
        .optional()
        .unwrap()
}

fn track_exists(conn: &Connection, id: &str) -> bool {
    conn.query_row("SELECT 1 FROM tracks WHERE id = ?1", [id], |_| Ok(()))
        .optional()
        .unwrap()
        .is_some()
}

fn track_tag_exists(conn: &Connection, tr: &str, tag: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM track_tags WHERE track_id = ?1 AND tag_id = ?2",
        params![tr, tag],
        |_| Ok(()),
    )
    .optional()
    .unwrap()
    .is_some()
}

fn playlist_track_exists(conn: &Connection, pl: &str, tr: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
        params![pl, tr],
        |_| Ok(()),
    )
    .optional()
    .unwrap()
    .is_some()
}

fn setting_value(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
        r.get(0)
    })
    .optional()
    .unwrap()
}

/// Drive both devices to a fixed point (A→B→A) and assert convergence.
async fn assert_converged(a: &Connection, b: &Connection, cloud: &MockCloudBackend) -> Result<()> {
    push(a, cloud, "A").await?;
    pull(b, cloud).await?;
    push(b, cloud, "B").await?;
    pull(a, cloud).await?;
    push(a, cloud, "A").await?;
    pull(b, cloud).await?;
    assert_eq!(
        state_hash(a)?,
        state_hash(b)?,
        "devices did not converge to identical state"
    );
    Ok(())
}

// --- cases -----------------------------------------------------------------

#[tokio::test]
async fn disjoint_creates_converge() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    create_playlist(&a, 0x0A, 5, "p-a", "A-list");
    create_track(&a, 0x0A, 5, "t-a", "A-song");
    create_playlist(&b, 0x0B, 5, "p-b", "B-list");
    create_track(&b, 0x0B, 5, "t-b", "B-song");

    assert_converged(&a, &b, &cloud).await?;

    for conn in [&a, &b] {
        assert_eq!(playlist_name(conn, "p-a").as_deref(), Some("A-list"));
        assert_eq!(playlist_name(conn, "p-b").as_deref(), Some("B-list"));
        assert!(track_exists(conn, "t-a"));
        assert!(track_exists(conn, "t-b"));
    }
    Ok(())
}

#[tokio::test]
async fn rename_vs_rename_higher_hlc_wins() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    create_playlist(&a, 0x0A, 5, "p", "Base");
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;

    // Concurrent offline renames; B's HLC is strictly higher.
    rename_playlist(&a, 0x0A, 10, "p", "Laptop");
    rename_playlist(&b, 0x0B, 20, "p", "Studio");

    assert_converged(&a, &b, &cloud).await?;
    assert_eq!(playlist_name(&a, "p").as_deref(), Some("Studio"));
    assert_eq!(playlist_name(&b, "p").as_deref(), Some("Studio"));
    Ok(())
}

#[tokio::test]
async fn edit_loses_to_newer_delete() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    create_track(&a, 0x0A, 5, "t", "Song");
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;

    update_track_title(&a, 0x0A, 10, "t", "Edited"); // older
    delete_track(&b, 0x0B, 20, "t"); // newer

    assert_converged(&a, &b, &cloud).await?;
    assert!(!track_exists(&a, "t"), "delete wins on A");
    assert!(!track_exists(&b, "t"), "delete wins on B");
    Ok(())
}

#[tokio::test]
async fn edit_beats_older_delete() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    create_track(&a, 0x0A, 5, "t", "Song");
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;

    update_track_title(&a, 0x0A, 20, "t", "Edited"); // newer
    delete_track(&b, 0x0B, 10, "t"); // older

    assert_converged(&a, &b, &cloud).await?;
    assert_eq!(
        track_title(&a, "t").as_deref(),
        Some("Edited"),
        "edit wins on A"
    );
    assert_eq!(
        track_title(&b, "t").as_deref(),
        Some("Edited"),
        "edit wins on B"
    );
    Ok(())
}

async fn setup_tag_endpoints(
    a: &Connection,
    b: &Connection,
    cloud: &MockCloudBackend,
) -> Result<()> {
    create_track(a, 0x0A, 1, "t1", "Song");
    create_tag_category(a, 0x0A, 1, "c1", "Genre");
    create_tag(a, 0x0A, 1, "g1", "c1", "House");
    add_track_tag(a, 0x0A, 5, "t1", "g1"); // baseline membership @5
    push(a, cloud, "A").await?;
    pull(b, cloud).await?;
    Ok(())
}

#[tokio::test]
async fn add_wins_over_older_delete() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);
    setup_tag_endpoints(&a, &b, &cloud).await?;

    add_track_tag(&a, 0x0A, 20, "t1", "g1"); // re-touch, newer
    remove_track_tag(&b, 0x0B, 10, "t1", "g1"); // remove, older

    assert_converged(&a, &b, &cloud).await?;
    assert!(track_tag_exists(&a, "t1", "g1"), "add wins on A");
    assert!(track_tag_exists(&b, "t1", "g1"), "add wins on B");
    Ok(())
}

#[tokio::test]
async fn delete_wins_over_older_add() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);
    setup_tag_endpoints(&a, &b, &cloud).await?;

    add_track_tag(&a, 0x0A, 10, "t1", "g1"); // re-touch, older
    remove_track_tag(&b, 0x0B, 20, "t1", "g1"); // remove, newer

    assert_converged(&a, &b, &cloud).await?;
    assert!(!track_tag_exists(&a, "t1", "g1"), "delete wins on A");
    assert!(!track_tag_exists(&b, "t1", "g1"), "delete wins on B");
    Ok(())
}

#[tokio::test]
async fn delete_propagates_and_double_pull_is_idempotent() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    create_track(&a, 0x0A, 5, "t", "Song");
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;
    assert!(track_exists(&b, "t"));

    delete_track(&a, 0x0A, 10, "t");
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;
    assert!(!track_exists(&b, "t"), "delete propagated");

    let after_first = state_hash(&b)?;
    pull(&b, &cloud).await?; // redundant pull, no new remote writes
    assert_eq!(state_hash(&b)?, after_first, "second pull is a no-op");
    Ok(())
}

#[tokio::test]
async fn resurrection_update_outvotes_earlier_delete() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    create_track(&a, 0x0A, 5, "t", "Song");
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;

    // A deletes (older) and pushes the tombstone; B updates (newer) without seeing it.
    delete_track(&a, 0x0A, 10, "t");
    push(&a, &cloud, "A").await?;
    update_track_title(&b, 0x0B, 20, "t", "Revived");

    assert_converged(&a, &b, &cloud).await?;
    assert_eq!(
        track_title(&a, "t").as_deref(),
        Some("Revived"),
        "update wins on A"
    );
    assert_eq!(
        track_title(&b, "t").as_deref(),
        Some("Revived"),
        "update wins on B"
    );
    Ok(())
}

#[tokio::test]
async fn settings_whitelist_crosses_but_device_local_does_not() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    set_setting_synced(&a, 0x0A, 10, "theme", "dark"); // whitelisted
    set_setting_local(&a, "audio_device", "Scarlett 2i2"); // device-local

    assert_converged(&a, &b, &cloud).await?;
    assert_eq!(
        setting_value(&b, "theme").as_deref(),
        Some("dark"),
        "synced key crossed"
    );
    assert_eq!(
        setting_value(&b, "audio_device"),
        None,
        "device-local key stayed home"
    );
    Ok(())
}

#[tokio::test]
async fn order_independence_commutativity() -> Result<()> {
    async fn run(a_first: bool) -> Result<String> {
        let cloud = MockCloudBackend::new();
        let a = new_device(0x0A);
        let b = new_device(0x0B);
        create_playlist(&a, 0x0A, 5, "p", "Base");
        push(&a, &cloud, "A").await?;
        pull(&b, &cloud).await?;
        rename_playlist(&a, 0x0A, 10, "p", "Laptop");
        rename_playlist(&b, 0x0B, 20, "p", "Studio");
        if a_first {
            push(&a, &cloud, "A").await?;
            pull(&b, &cloud).await?;
            push(&b, &cloud, "B").await?;
            pull(&a, &cloud).await?;
        } else {
            push(&b, &cloud, "B").await?;
            pull(&a, &cloud).await?;
            push(&a, &cloud, "A").await?;
            pull(&b, &cloud).await?;
        }
        push(&a, &cloud, "A").await?;
        pull(&b, &cloud).await?;
        assert_eq!(state_hash(&a)?, state_hash(&b)?);
        assert_eq!(playlist_name(&a, "p").as_deref(), Some("Studio"));
        state_hash(&a)
    }

    assert_eq!(
        run(true).await?,
        run(false).await?,
        "sync order must not matter"
    );
    Ok(())
}

#[tokio::test]
async fn first_sync_stamps_empty_hlc_library() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    // A pre-sync library: rows carry the '' sentinel, and initial_stamp_done is unset.
    a.execute(
        "INSERT INTO playlists \
            (id, name, parent_id, is_folder, is_smart, smart_rules, sort_order, context, \
             date_created, date_modified, _hlc) \
         VALUES ('p1', 'Old', NULL, 0, 0, NULL, 0, 'library', '2021-05-01T10:00:00Z', \
                 '2021-05-02T10:00:00Z', '')",
        [],
    )
    .unwrap();
    a.execute(
        "INSERT INTO tracks (id, file_path, duration_ms, title, date_added, date_modified, _hlc) \
         VALUES ('t1', '/music/t1.mp3', 1000, 'Song', '2021-05-01T10:00:00Z', '2021-05-02T10:00:00Z', '')",
        [],
    )
    .unwrap();

    push(&a, &cloud, "A").await?; // triggers stamp_unstamped_rows
    pull(&b, &cloud).await?;

    let unstamped_playlists: i64 = a
        .query_row("SELECT COUNT(*) FROM playlists WHERE _hlc = ''", [], |r| {
            r.get(0)
        })
        .unwrap();
    let unstamped_tracks: i64 = a
        .query_row("SELECT COUNT(*) FROM tracks WHERE _hlc = ''", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(unstamped_playlists, 0, "playlists got real HLCs");
    assert_eq!(unstamped_tracks, 0, "tracks got real HLCs");
    assert_eq!(
        state_hash(&a)?,
        state_hash(&b)?,
        "stamped library converges"
    );
    assert!(track_exists(&b, "t1"));
    assert_eq!(playlist_name(&b, "p1").as_deref(), Some("Old"));
    Ok(())
}

#[tokio::test]
async fn cascade_deleted_parent_skips_orphan_junction() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    create_track(&a, 0x0A, 5, "t", "Song");
    create_playlist(&a, 0x0A, 5, "p", "Set");
    add_track_to_playlist(&a, 0x0A, 5, "p", "t", 0);
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;

    // A deletes the track (cascades away its playlist_tracks row, no junction
    // tombstone); B concurrently re-adds the membership at a higher HLC.
    delete_track(&a, 0x0A, 20, "t");
    add_track_to_playlist(&b, 0x0B, 30, "p", "t", 1);

    // Sync B first so A sees a live junction whose endpoint it has deleted.
    push(&b, &cloud, "B").await?;
    pull(&a, &cloud).await?;
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;
    push(&b, &cloud, "B").await?;
    pull(&a, &cloud).await?;

    assert_eq!(
        state_hash(&a)?,
        state_hash(&b)?,
        "converged despite cascade race"
    );
    assert!(!track_exists(&a, "t"), "track deleted");
    assert!(
        !playlist_track_exists(&a, "p", "t"),
        "orphan membership not resurrected"
    );
    assert!(!playlist_track_exists(&b, "p", "t"));
    Ok(())
}

#[tokio::test]
async fn byte_identical_buckets_after_convergence() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    create_track(&a, 0x0A, 5, "t1", "One");
    create_playlist(&a, 0x0A, 5, "p1", "Set");
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;

    update_track_title(&a, 0x0A, 10, "t1", "One-A");
    update_track_title(&b, 0x0B, 20, "t1", "One-B");
    create_track(&b, 0x0B, 8, "t2", "Two");

    assert_converged(&a, &b, &cloud).await?;

    // The strong guard: every bucket's serialized bytes must be identical.
    for bucket in Bucket::all() {
        let ba = rows::serialize_bucket(&a, &bucket)?;
        let bb = rows::serialize_bucket(&b, &bucket)?;
        assert_eq!(ba, bb, "bucket {} diverged byte-for-byte", bucket.as_str());
    }
    Ok(())
}

// --- Phase 3: production pull + GC ------------------------------------------
//
// These drive the REAL `pull::pull` / `push::push` (Arc<Mutex<Connection>> +
// Arc<dyn CloudBackend>), unlike the simplified `super::{push, pull}` helpers above.

/// Device A pushes, Device B pulls and converges; a redundant pull is a no-op (etag
/// gate) and A never merges its own manifest back (self-echo skip).
#[tokio::test]
async fn production_pull_converges_then_short_circuits() -> Result<()> {
    let backend: Arc<dyn CloudBackend> = Arc::new(MockCloudBackend::new());
    let session = test_session();

    let a_conn = new_device(0x0A);
    create_playlist(&a_conn, 0x0A, 5, "p", "Set");
    let a = Arc::new(Mutex::new(a_conn));
    let b = Arc::new(Mutex::new(new_device(0x0B)));

    // A pushes; B pulls and converges.
    push_remote(a.clone(), &backend, &session, "A").await?;
    assert!(
        pull_remote(b.clone(), &backend, &session, "B")
            .await?
            .merged,
        "B merges A's push"
    );
    assert_eq!(
        playlist_name(&b.lock().unwrap(), "p").as_deref(),
        Some("Set"),
        "playlist crossed to B"
    );
    assert_eq!(
        state_hash(&a.lock().unwrap())?,
        state_hash(&b.lock().unwrap())?,
        "A and B converged"
    );

    // A redundant pull on B does nothing — the manifest etag is unchanged.
    assert!(
        !pull_remote(b.clone(), &backend, &session, "B")
            .await?
            .merged,
        "second pull is a no-op (etag gate)"
    );
    // A never merges its own manifest back.
    assert!(
        !pull_remote(a.clone(), &backend, &session, "A")
            .await?
            .merged,
        "A skips its own write (self-echo)"
    );
    Ok(())
}

/// `gc_sweep` deletes blobs whose grace window has elapsed and drains the queue.
#[tokio::test]
async fn gc_sweep_reclaims_due_blobs() -> Result<()> {
    let backend: Arc<dyn CloudBackend> = Arc::new(MockCloudBackend::new());
    let session = test_session();
    let key = "users/test-uid/vault/superseded-deadbeef.jsonl.gz".to_string();

    // Stage a stale blob and enqueue it for GC with a past-due delete_after.
    backend
        .blobs()
        .upload(
            &session,
            &key,
            Bytes::from_static(b"stale"),
            "application/x-ndjson",
        )
        .await?;
    let past = SystemTime::now() - Duration::from_secs(120);
    backend
        .manifest()
        .write(
            &session,
            &Manifest::empty("A"),
            None,
            &[GcEntry {
                object_key: key.clone(),
                delete_after: past,
            }],
        )
        .await?;

    assert_eq!(
        gc::gc_sweep(&backend, &session).await?,
        1,
        "one entry processed"
    );

    // The blob is gone and the queue is drained.
    assert!(
        matches!(
            backend.blobs().download(&session, &key).await,
            Err(CrateError::CloudSyncBlobNotFound(_))
        ),
        "blob deleted"
    );
    assert!(
        backend
            .manifest()
            .dequeue_gc(&session, SystemTime::now() + Duration::from_secs(3600), 100)
            .await?
            .is_empty(),
        "queue drained"
    );
    Ok(())
}
