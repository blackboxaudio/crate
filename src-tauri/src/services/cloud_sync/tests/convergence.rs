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
use crate::services::cloud_sync::backend::types::{BucketEntry, GcEntry, Manifest};
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

/// The sweep must never delete a blob the CURRENT manifest references: keys are
/// content-addressed, so a key enqueued at supersession can re-enter the manifest later
/// (a bucket serializing back to earlier bytes), and deleting it would dangle the
/// manifest and wedge every pull. The entry is acked without deleting.
#[tokio::test]
async fn gc_sweep_spares_still_referenced_blobs() -> Result<()> {
    let backend: Arc<dyn CloudBackend> = Arc::new(MockCloudBackend::new());
    let session = test_session();
    let relative = "playlists-cafebabe.jsonl.gz";
    let key = format!("users/test-uid/vault/{relative}");

    backend
        .blobs()
        .upload(
            &session,
            &key,
            Bytes::from_static(b"live"),
            "application/x-ndjson",
        )
        .await?;

    // The manifest references the key while a stale, past-due GC entry names it too.
    let mut manifest = Manifest::empty("A");
    manifest.buckets.insert(
        "playlists".to_string(),
        BucketEntry {
            blob_hash: "cafebabe".into(),
            object_key: relative.to_string(),
            count: 1,
            hlc: String::new(),
        },
    );
    let past = SystemTime::now() - Duration::from_secs(120);
    backend
        .manifest()
        .write(
            &session,
            &manifest,
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
        "entry processed (acked)"
    );
    assert!(
        backend.blobs().download(&session, &key).await.is_ok(),
        "still-referenced blob spared"
    );
    assert!(
        backend
            .manifest()
            .dequeue_gc(&session, SystemTime::now() + Duration::from_secs(3600), 100)
            .await?
            .is_empty(),
        "stale entry still drained from the queue"
    );
    Ok(())
}

/// A manifest entry whose blob is gone (a dangling reference — e.g. reclaimed by a
/// pre-guard GC sweep) must not wedge sync. The pull skips the bucket and marks it
/// dirty; the next push re-uploads local content and rewrites the manifest; and the
/// device still holding the lost rows re-pushes its union after merging the repaired
/// (older) blob via the post-merge anti-entropy mark. Nothing is lost, all converge.
#[tokio::test]
async fn dangling_manifest_reference_self_heals() -> Result<()> {
    let backend: Arc<dyn CloudBackend> = Arc::new(MockCloudBackend::new());
    let session = test_session();
    let a_conn = new_device(0x0A);
    create_playlist(&a_conn, 0x0A, 5, "p", "Lost");
    let a = Arc::new(Mutex::new(a_conn));
    let b = Arc::new(Mutex::new(new_device(0x0B)));

    // A pushes, then the playlists blob vanishes out from under the manifest.
    push_remote(a.clone(), &backend, &session, "A").await?;
    let (manifest, _) = backend.manifest().read(&session).await?.expect("manifest");
    let dead = manifest
        .bucket("playlists")
        .expect("playlists entry")
        .clone();
    backend
        .blobs()
        .delete(
            &session,
            &format!("users/{}/vault/{}", session.uid, dead.object_key),
        )
        .await?;

    // B's pull survives: the bucket is skipped and scheduled for re-upload.
    let outcome = pull_remote(b.clone(), &backend, &session, "B").await?;
    assert_eq!(
        outcome.skipped,
        vec!["playlists".to_string()],
        "dangling bucket skipped, not fatal"
    );
    let dirty_count = |conn: &Connection| -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM sync_dirty_buckets WHERE bucket = 'playlists'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(
        dirty_count(&b.lock().unwrap()),
        1,
        "skipped bucket marked dirty on B"
    );

    // B's push repairs the manifest: playlists points at a live blob again.
    push_remote(b.clone(), &backend, &session, "B").await?;
    let (manifest, _) = backend.manifest().read(&session).await?.expect("manifest");
    let repaired = manifest.bucket("playlists").expect("playlists entry");
    assert_ne!(
        repaired.blob_hash, dead.blob_hash,
        "manifest entry rewritten"
    );
    assert!(
        backend
            .blobs()
            .download(
                &session,
                &format!("users/{}/vault/{}", session.uid, repaired.object_key),
            )
            .await
            .is_ok(),
        "repaired blob is live"
    );

    // A merges the repaired (empty) blob, keeps its playlist, and the anti-entropy
    // mark schedules the union push that restores the lost rows to the cloud.
    pull_remote(a.clone(), &backend, &session, "A").await?;
    assert_eq!(
        dirty_count(&a.lock().unwrap()),
        1,
        "A marked dirty after merging a remote that lacks its rows"
    );
    push_remote(a.clone(), &backend, &session, "A").await?;
    assert!(
        pull_remote(b.clone(), &backend, &session, "B")
            .await?
            .merged,
        "B merges the restored rows"
    );
    assert_eq!(
        playlist_name(&b.lock().unwrap(), "p").as_deref(),
        Some("Lost"),
        "lost rows recovered from their authoring device"
    );
    assert_eq!(
        state_hash(&a.lock().unwrap())?,
        state_hash(&b.lock().unwrap())?,
        "A and B converged"
    );
    Ok(())
}

// --- discovery duplicate collapse -----------------------------------------
//
// Discovery tracks/releases historically minted random v4 ids with no natural-key
// uniqueness, so two devices independently materializing the same content produced
// duplicate rows (tracks) or a UNIQUE(url)-skip id split-brain (releases). These
// cases exercise the content-based collapse in the merge engine plus the startup
// dedupe sweep. Seeding helpers mirror the real mutation sites (stamp + dirty).

fn create_discovery_release(conn: &Connection, node: u32, wall: u64, id: &str, url: &str) {
    conn.execute(
        "INSERT INTO discovery_releases (id, url, source_type, date_added, date_modified, _hlc) \
         VALUES (?1, ?2, 'bandcamp', ?3, ?3, ?4)",
        params![id, url, DATE, hlc(node, wall)],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::DISCOVERY_RELEASES).unwrap();
}

fn create_discovery_track(
    conn: &Connection,
    node: u32,
    wall: u64,
    id: &str,
    release_id: &str,
    name: &str,
    position: i32,
) {
    conn.execute(
        "INSERT INTO discovery_tracks (id, release_id, name, position, _hlc) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, release_id, name, position, hlc(node, wall)],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::DISCOVERY_TRACKS).unwrap();
}

fn like_discovery_track(conn: &Connection, node: u32, wall: u64, id: &str) {
    conn.execute(
        "UPDATE discovery_tracks SET is_liked = 1, _hlc = ?1 WHERE id = ?2",
        params![hlc(node, wall), id],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::DISCOVERY_TRACKS).unwrap();
}

fn add_discovery_release_tag(conn: &Connection, node: u32, wall: u64, release: &str, tag: &str) {
    conn.execute(
        "INSERT INTO discovery_release_tags (release_id, tag_id, _hlc) VALUES (?1, ?2, ?3) \
         ON CONFLICT(release_id, tag_id) DO UPDATE SET _hlc = excluded._hlc",
        params![release, tag, hlc(node, wall)],
    )
    .unwrap();
    dirty::mark_dirty(conn, buckets::DISCOVERY_RELEASE_TAGS).unwrap();
}

/// `(id, name, is_liked)` of a release's live tracks, ordered by id.
fn discovery_tracks_for(conn: &Connection, release: &str) -> Vec<(String, String, bool)> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, is_liked FROM discovery_tracks WHERE release_id = ?1 ORDER BY id",
        )
        .unwrap();
    stmt.query_map([release], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i32>(2)? != 0,
        ))
    })
    .unwrap()
    .collect::<std::result::Result<Vec<_>, _>>()
    .unwrap()
}

fn discovery_release_ids(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT id FROM discovery_releases ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap()
}

fn tombstone_exists(conn: &Connection, entity_type: &str, id: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sync_tombstones WHERE entity_type = ?1 AND entity_id = ?2",
        params![entity_type, id],
        |_| Ok(()),
    )
    .optional()
    .unwrap()
    .is_some()
}

fn release_tag_exists(conn: &Connection, release: &str, tag: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM discovery_release_tags WHERE release_id = ?1 AND tag_id = ?2",
        params![release, tag],
        |_| Ok(()),
    )
    .optional()
    .unwrap()
    .is_some()
}

/// Both devices independently populate the same (synced) release's tracks — the
/// pre-deterministic-id bug shape. The merge must collapse the two id sets into
/// one row per track, keeping the like and tombstoning the losing id.
#[tokio::test]
async fn independent_track_fetches_collapse_to_one_row() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    create_discovery_release(&a, 0x0A, 5, "rel-1", "https://x.bandcamp.com/album/y");
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;

    // Same logical track, different ids, different name case; B also likes its copy.
    create_discovery_track(&a, 0x0A, 10, "aaaa-track", "rel-1", "Intro", 1);
    create_discovery_track(&b, 0x0B, 20, "bbbb-track", "rel-1", "intro", 1);
    like_discovery_track(&b, 0x0B, 25, "bbbb-track");

    assert_converged(&a, &b, &cloud).await?;

    for conn in [&a, &b] {
        let tracks = discovery_tracks_for(conn, "rel-1");
        assert_eq!(tracks.len(), 1, "duplicates collapsed to one row");
        let (id, _, liked) = &tracks[0];
        assert_eq!(id, "aaaa-track", "smaller id survives");
        assert!(*liked, "the like on the losing copy was folded in");
        assert!(tombstone_exists(conn, "discovery_tracks", "bbbb-track"));
    }
    Ok(())
}

/// Same as above with the push order mirrored — the collapse rule must be
/// direction-independent (same survivor, byte-identical buckets).
#[tokio::test]
async fn independent_track_fetches_collapse_mirrored_order() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    create_discovery_release(&a, 0x0A, 5, "rel-1", "https://x.bandcamp.com/album/y");
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;

    create_discovery_track(&a, 0x0A, 10, "aaaa-track", "rel-1", "Intro", 1);
    create_discovery_track(&b, 0x0B, 20, "bbbb-track", "rel-1", "intro", 1);
    like_discovery_track(&b, 0x0B, 25, "bbbb-track");

    // B pushes first this time.
    push(&b, &cloud, "B").await?;
    pull(&a, &cloud).await?;
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;
    push(&b, &cloud, "B").await?;
    pull(&a, &cloud).await?;
    assert_eq!(state_hash(&a)?, state_hash(&b)?);

    for conn in [&a, &b] {
        let tracks = discovery_tracks_for(conn, "rel-1");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].0, "aaaa-track", "same survivor in both directions");
        assert!(tracks[0].2);
    }
    Ok(())
}

/// Existing (already-synced) duplicates, swept on one device while the OTHER
/// device concurrently likes the copy the sweep removed. The like must not be
/// destroyed by the sweep's tombstone — it outranks it, survives the pull, and
/// the merge collapse folds it into the survivor.
#[tokio::test]
async fn sweep_preserves_concurrent_like_on_removed_copy() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    // Seed identical (converged) duplicate state on both devices, as if it had
    // synced before the fix shipped. Same node in the stamps → same bytes.
    for conn in [&a, &b] {
        create_discovery_release(conn, 0x0A, 5, "rel-1", "https://x.bandcamp.com/album/y");
        create_discovery_track(conn, 0x0A, 10, "xxxx-track", "rel-1", "Intro", 1);
        create_discovery_track(conn, 0x0A, 20, "yyyy-track", "rel-1", "Intro", 1);
    }
    assert_eq!(state_hash(&a)?, state_hash(&b)?, "seeded converged");

    // B likes the copy A's sweep is about to remove.
    like_discovery_track(&b, 0x0B, 30, "yyyy-track");

    let removed = crate::services::discovery::dedupe_discovery_tracks(&a)?;
    assert_eq!(removed, 1, "A's sweep removed the duplicate");
    assert_eq!(
        discovery_tracks_for(&a, "rel-1").len(),
        1,
        "one row left on A"
    );

    assert_converged(&a, &b, &cloud).await?;

    for conn in [&a, &b] {
        let tracks = discovery_tracks_for(conn, "rel-1");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].0, "xxxx-track", "smallest id survives everywhere");
        assert!(tracks[0].2, "B's like on the removed copy was preserved");
    }

    // A later sweep on B is a no-op — nothing left to collapse.
    assert_eq!(crate::services::discovery::dedupe_discovery_tracks(&b)?, 0);
    assert_eq!(state_hash(&a)?, state_hash(&b)?);
    Ok(())
}

/// A new-build device minting a content-derived (v5) id meets an old device's
/// legacy random id for the same track: they collapse like any other pair.
#[tokio::test]
async fn deterministic_id_meets_legacy_v4_id() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    create_discovery_release(&a, 0x0A, 5, "rel-1", "https://x.bandcamp.com/album/y");
    push(&a, &cloud, "A").await?;
    pull(&b, &cloud).await?;

    let det_id = crate::models::deterministic_track_id("rel-1", "Intro");
    create_discovery_track(&a, 0x0A, 10, "zzzz-legacy", "rel-1", "Intro", 1);
    create_discovery_track(&b, 0x0B, 20, &det_id, "rel-1", "Intro", 1);
    like_discovery_track(&b, 0x0B, 25, &det_id);

    assert_converged(&a, &b, &cloud).await?;

    // Hex uuids sort below "zzzz-…", so the deterministic id is the survivor.
    for conn in [&a, &b] {
        let tracks = discovery_tracks_for(conn, "rel-1");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].0, det_id);
        assert!(tracks[0].2);
        assert!(tombstone_exists(conn, "discovery_tracks", "zzzz-legacy"));
    }
    Ok(())
}

/// Two devices independently add the SAME release URL before syncing (the release
/// split-brain the UNIQUE(url) merge-skip used to cause). The merge must collapse
/// the two release ids into one, re-parent children (tracks, tag links) onto the
/// survivor, and collapse the now-sibling duplicate tracks — no permanent id
/// divergence, no lost likes or tags.
#[tokio::test]
async fn same_url_releases_collapse_and_reparent_children() -> Result<()> {
    let cloud = MockCloudBackend::new();
    let a = new_device(0x0A);
    let b = new_device(0x0B);

    let url = "https://x.bandcamp.com/album/split-brain";
    create_discovery_release(&a, 0x0A, 5, "p-rel", url);
    create_discovery_track(&a, 0x0A, 6, "aaaa-track", "p-rel", "Intro", 1);

    create_discovery_release(&b, 0x0B, 7, "q-rel", url);
    create_discovery_track(&b, 0x0B, 8, "bbbb-track", "q-rel", "Intro", 1);
    like_discovery_track(&b, 0x0B, 9, "bbbb-track");
    create_tag_category(&b, 0x0B, 9, "cat", "Genre");
    create_tag(&b, 0x0B, 9, "tag-house", "cat", "House");
    add_discovery_release_tag(&b, 0x0B, 9, "q-rel", "tag-house");

    assert_converged(&a, &b, &cloud).await?;

    for conn in [&a, &b] {
        assert_eq!(
            discovery_release_ids(conn),
            vec!["p-rel".to_string()],
            "one release, smaller id"
        );
        assert!(tombstone_exists(conn, "discovery_releases", "q-rel"));
        let tracks = discovery_tracks_for(conn, "p-rel");
        assert_eq!(tracks.len(), 1, "re-parented duplicate tracks collapsed");
        assert_eq!(tracks[0].0, "aaaa-track");
        assert!(tracks[0].2, "like survived the re-parent + collapse");
        assert!(
            release_tag_exists(conn, "p-rel", "tag-house"),
            "tag link re-parented onto the survivor"
        );
    }
    Ok(())
}
