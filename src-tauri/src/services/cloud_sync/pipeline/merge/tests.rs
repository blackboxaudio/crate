//! Truth-table unit tests for the merge engine. Each test builds a precise local
//! state + a single remote row (with hand-chosen HLCs) and asserts the result.
//! HLCs are zero-padded so lexicographic order == numeric order.

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use super::merge_bucket;
use crate::services::cloud_sync::pipeline::buckets::Bucket;
use crate::services::cloud_sync::pipeline::rows::ParsedRow;

fn mem() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    for sql in crate::db::schema::get_migrations() {
        conn.execute_batch(sql).unwrap();
    }
    conn
}

fn parsed(value: serde_json::Value) -> ParsedRow {
    let hlc = value
        .get("_hlc")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let deleted = value
        .get("_deleted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    ParsedRow {
        value,
        hlc,
        deleted,
    }
}

// --- tag_categories (a simple single-PK entity) ---------------------------

fn insert_tag_category(conn: &Connection, id: &str, name: &str, hlc: &str) {
    conn.execute(
        "INSERT INTO tag_categories (id, name, sort_order, color, _hlc) VALUES (?1, ?2, 0, NULL, ?3)",
        params![id, name, hlc],
    )
    .unwrap();
}

fn tc_live(id: &str, name: &str, hlc: &str) -> ParsedRow {
    parsed(
        json!({"id": id, "name": name, "color": null, "sort_order": 0, "_hlc": hlc, "_deleted": false}),
    )
}

fn tc_tomb(id: &str, hlc: &str) -> ParsedRow {
    parsed(json!({"id": id, "_hlc": hlc, "_deleted": true}))
}

fn tc_name(conn: &Connection, id: &str) -> Option<String> {
    conn.query_row("SELECT name FROM tag_categories WHERE id = ?1", [id], |r| {
        r.get(0)
    })
    .optional()
    .unwrap()
}

fn tomb_hlc(conn: &Connection, etype: &str, eid: &str) -> Option<String> {
    conn.query_row(
        "SELECT _hlc FROM sync_tombstones WHERE entity_type = ?1 AND entity_id = ?2",
        params![etype, eid],
        |r| r.get(0),
    )
    .optional()
    .unwrap()
}

#[test]
fn entity_insert_when_absent() {
    let conn = mem();
    merge_bucket(
        &conn,
        &Bucket::TagCategories,
        &[tc_live("c1", "House", "0005")],
    )
    .unwrap();
    assert_eq!(tc_name(&conn, "c1").as_deref(), Some("House"));
}

#[test]
fn entity_replace_when_remote_newer() {
    let conn = mem();
    insert_tag_category(&conn, "c1", "House", "0005");
    merge_bucket(
        &conn,
        &Bucket::TagCategories,
        &[tc_live("c1", "Techno", "0009")],
    )
    .unwrap();
    assert_eq!(tc_name(&conn, "c1").as_deref(), Some("Techno"));
}

#[test]
fn entity_keep_when_remote_older() {
    let conn = mem();
    insert_tag_category(&conn, "c1", "House", "0009");
    merge_bucket(
        &conn,
        &Bucket::TagCategories,
        &[tc_live("c1", "Techno", "0005")],
    )
    .unwrap();
    assert_eq!(tc_name(&conn, "c1").as_deref(), Some("House"));
}

#[test]
fn entity_noop_when_equal_hlc() {
    let conn = mem();
    insert_tag_category(&conn, "c1", "House", "0005");
    merge_bucket(
        &conn,
        &Bucket::TagCategories,
        &[tc_live("c1", "House", "0005")],
    )
    .unwrap();
    assert_eq!(tc_name(&conn, "c1").as_deref(), Some("House"));
}

#[test]
fn entity_delete_when_remote_newer() {
    let conn = mem();
    insert_tag_category(&conn, "c1", "House", "0005");
    merge_bucket(&conn, &Bucket::TagCategories, &[tc_tomb("c1", "0009")]).unwrap();
    assert_eq!(tc_name(&conn, "c1"), None, "row deleted");
    assert_eq!(
        tomb_hlc(&conn, "tag_categories", "c1").as_deref(),
        Some("0009")
    );
}

#[test]
fn entity_keep_when_delete_older() {
    let conn = mem();
    insert_tag_category(&conn, "c1", "House", "0009");
    merge_bucket(&conn, &Bucket::TagCategories, &[tc_tomb("c1", "0005")]).unwrap();
    assert_eq!(
        tc_name(&conn, "c1").as_deref(),
        Some("House"),
        "kept (local newer)"
    );
    assert_eq!(tomb_hlc(&conn, "tag_categories", "c1"), None);
}

#[test]
fn entity_delete_wins_tie() {
    let conn = mem();
    insert_tag_category(&conn, "c1", "House", "0007");
    merge_bucket(&conn, &Bucket::TagCategories, &[tc_tomb("c1", "0007")]).unwrap();
    assert_eq!(tc_name(&conn, "c1"), None, "delete wins the tie");
}

#[test]
fn entity_resurrect_when_update_newer_than_tombstone() {
    let conn = mem();
    conn.execute(
        "INSERT INTO sync_tombstones (entity_type, entity_id, _hlc) VALUES ('tag_categories','c1','0010')",
        [],
    )
    .unwrap();
    merge_bucket(
        &conn,
        &Bucket::TagCategories,
        &[tc_live("c1", "House", "0020")],
    )
    .unwrap();
    assert_eq!(
        tc_name(&conn, "c1").as_deref(),
        Some("House"),
        "resurrected"
    );
    assert_eq!(
        tomb_hlc(&conn, "tag_categories", "c1"),
        None,
        "tombstone dropped"
    );
}

#[test]
fn entity_stays_deleted_when_update_older_than_tombstone() {
    let conn = mem();
    conn.execute(
        "INSERT INTO sync_tombstones (entity_type, entity_id, _hlc) VALUES ('tag_categories','c1','0020')",
        [],
    )
    .unwrap();
    merge_bucket(
        &conn,
        &Bucket::TagCategories,
        &[tc_live("c1", "House", "0010")],
    )
    .unwrap();
    assert_eq!(tc_name(&conn, "c1"), None, "stays deleted");
}

#[test]
fn entity_records_tombstone_when_never_seen() {
    let conn = mem();
    merge_bucket(&conn, &Bucket::TagCategories, &[tc_tomb("c1", "0009")]).unwrap();
    assert_eq!(
        tomb_hlc(&conn, "tag_categories", "c1").as_deref(),
        Some("0009")
    );
}

#[test]
fn entity_merge_is_idempotent() {
    let conn = mem();
    let rows = [tc_live("c1", "House", "0005"), tc_tomb("c2", "0006")];
    merge_bucket(&conn, &Bucket::TagCategories, &rows).unwrap();
    let after_once = (
        tc_name(&conn, "c1"),
        tomb_hlc(&conn, "tag_categories", "c2"),
    );
    merge_bucket(&conn, &Bucket::TagCategories, &rows).unwrap();
    let after_twice = (
        tc_name(&conn, "c1"),
        tomb_hlc(&conn, "tag_categories", "c2"),
    );
    assert_eq!(after_once, after_twice);
}

#[test]
fn entity_unique_collision_is_skipped_not_fatal() {
    let conn = mem();
    // Local "House" with id c-local; remote "House" with a different id c-remote.
    insert_tag_category(&conn, "c-local", "House", "0005");
    let res = merge_bucket(
        &conn,
        &Bucket::TagCategories,
        &[
            tc_live("c-remote", "House", "0009"), // UNIQUE(name) collision -> skipped
            tc_live("c-other", "Techno", "0009"), // still merges
        ],
    );
    assert!(res.is_ok(), "bucket must not abort on a UNIQUE collision");
    assert_eq!(tc_name(&conn, "c-local").as_deref(), Some("House"));
    assert_eq!(tc_name(&conn, "c-remote"), None, "duplicate skipped");
    assert_eq!(tc_name(&conn, "c-other").as_deref(), Some("Techno"));
}

// --- track_tags (a tag junction: ADD-WINS-TIE, no ordering) ----------------

fn setup_junction_endpoints(conn: &Connection) {
    conn.execute(
        "INSERT INTO tag_categories (id, name, sort_order, _hlc) VALUES ('c1','Cat',0,'0001')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tags (id, category_id, name, sort_order, _hlc) VALUES ('g1','c1','Tag',0,'0001')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tracks (id, file_path, duration_ms, date_added, date_modified, _hlc) \
         VALUES ('t1','/x',1000,'2020-01-01T00:00:00Z','2020-01-01T00:00:00Z','0001')",
        [],
    )
    .unwrap();
}

fn tt_live(hlc: &str) -> ParsedRow {
    parsed(json!({"track_id": "t1", "tag_id": "g1", "_hlc": hlc, "_deleted": false}))
}

fn tt_tomb(hlc: &str) -> ParsedRow {
    parsed(json!({"track_id": "t1", "tag_id": "g1", "_hlc": hlc, "_deleted": true}))
}

fn tt_exists(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT 1 FROM track_tags WHERE track_id='t1' AND tag_id='g1'",
        [],
        |_| Ok(()),
    )
    .optional()
    .unwrap()
    .is_some()
}

#[test]
fn junction_insert_when_endpoints_exist() {
    let conn = mem();
    setup_junction_endpoints(&conn);
    merge_bucket(&conn, &Bucket::TrackTags, &[tt_live("0005")]).unwrap();
    assert!(tt_exists(&conn));
}

#[test]
fn junction_orphan_insert_is_skipped() {
    let conn = mem();
    // No endpoints inserted -> the add references missing track/tag.
    merge_bucket(&conn, &Bucket::TrackTags, &[tt_live("0005")]).unwrap();
    assert!(!tt_exists(&conn), "orphan membership skipped");
}

#[test]
fn junction_add_wins_over_older_delete() {
    let conn = mem();
    setup_junction_endpoints(&conn);
    conn.execute(
        "INSERT INTO sync_tombstones (entity_type, entity_id, _hlc) VALUES ('track_tags','t1|g1','0010')",
        [],
    )
    .unwrap();
    merge_bucket(&conn, &Bucket::TrackTags, &[tt_live("0020")]).unwrap();
    assert!(tt_exists(&conn), "add newer than removal resurrects");
}

#[test]
fn junction_delete_wins_only_when_strictly_newer() {
    let conn = mem();
    setup_junction_endpoints(&conn);
    conn.execute(
        "INSERT INTO track_tags (track_id, tag_id, _hlc) VALUES ('t1','g1','0010')",
        [],
    )
    .unwrap();
    // delete older -> keep
    merge_bucket(&conn, &Bucket::TrackTags, &[tt_tomb("0005")]).unwrap();
    assert!(tt_exists(&conn), "older delete keeps the add");
    // delete newer -> remove
    merge_bucket(&conn, &Bucket::TrackTags, &[tt_tomb("0020")]).unwrap();
    assert!(!tt_exists(&conn), "newer delete removes the add");
}

#[test]
fn junction_delete_tie_keeps_add() {
    let conn = mem();
    setup_junction_endpoints(&conn);
    conn.execute(
        "INSERT INTO track_tags (track_id, tag_id, _hlc) VALUES ('t1','g1','0010')",
        [],
    )
    .unwrap();
    merge_bucket(&conn, &Bucket::TrackTags, &[tt_tomb("0010")]).unwrap();
    assert!(tt_exists(&conn), "add wins the tie");
}

// --- settings (LWW per whitelisted key) -----------------------------------

fn setting_value(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
        r.get(0)
    })
    .optional()
    .unwrap()
}

#[test]
fn settings_apply_whitelisted_newer() {
    let conn = mem();
    merge_bucket(
        &conn,
        &Bucket::Settings,
        &[parsed(
            json!({"key": "theme", "value": "dark", "_hlc": "0005"}),
        )],
    )
    .unwrap();
    assert_eq!(setting_value(&conn, "theme").as_deref(), Some("dark"));
    let hlc: String = conn
        .query_row(
            "SELECT value FROM sync_state WHERE key = 'setting_hlc:theme'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hlc, "0005");
}

#[test]
fn settings_drop_non_whitelisted() {
    let conn = mem();
    merge_bucket(
        &conn,
        &Bucket::Settings,
        &[parsed(
            json!({"key": "audio_device", "value": "X", "_hlc": "0009"}),
        )],
    )
    .unwrap();
    assert_eq!(
        setting_value(&conn, "audio_device"),
        None,
        "device-local key dropped"
    );
}

#[test]
fn settings_keep_local_when_older() {
    let conn = mem();
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('theme','dark')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES ('setting_hlc:theme','0009')",
        [],
    )
    .unwrap();
    merge_bucket(
        &conn,
        &Bucket::Settings,
        &[parsed(
            json!({"key": "theme", "value": "light", "_hlc": "0005"}),
        )],
    )
    .unwrap();
    assert_eq!(
        setting_value(&conn, "theme").as_deref(),
        Some("dark"),
        "older remote ignored"
    );
}

// --- override reporting (observational — must not change merge outcomes) ---

fn set_node_id(conn: &Connection, node: &str) {
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES ('node_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [node],
    )
    .unwrap();
}

#[test]
fn override_reported_when_local_authored_value_is_replaced() {
    let conn = mem();
    set_node_id(&conn, "aabbccdd");
    // Local "House" authored by THIS device (node aabbccdd).
    insert_tag_category(&conn, "c1", "House", "0000000000000005-00000000-aabbccdd");
    // Remote rename with a higher HLC from ANOTHER device.
    let overrides = merge_bucket(
        &conn,
        &Bucket::TagCategories,
        &[tc_live(
            "c1",
            "Techno",
            "0000000000000006-00000000-11223344",
        )],
    )
    .unwrap();
    // Outcome is unchanged (remote wins); the override is reported alongside.
    assert_eq!(tc_name(&conn, "c1").as_deref(), Some("Techno"));
    assert_eq!(overrides.len(), 1);
    assert_eq!(
        overrides[0].label, "House",
        "names the discarded local value"
    );
}

#[test]
fn no_override_when_local_value_authored_elsewhere() {
    let conn = mem();
    set_node_id(&conn, "aabbccdd");
    // Local value was authored by a DIFFERENT device (node 99999999) — plain
    // propagation, not a conflict this device should be toasted about.
    insert_tag_category(&conn, "c1", "House", "0000000000000005-00000000-99999999");
    let overrides = merge_bucket(
        &conn,
        &Bucket::TagCategories,
        &[tc_live(
            "c1",
            "Techno",
            "0000000000000006-00000000-11223344",
        )],
    )
    .unwrap();
    assert_eq!(tc_name(&conn, "c1").as_deref(), Some("Techno"));
    assert!(overrides.is_empty());
}

// --- orphaned child entities (parent deleted locally) ----------------------
//
// Regression: a release deleted locally (tombstone wins over the remote release
// row) must NOT wedge sync when the remote's discovery_tracks rows still
// reference it — with deferred FKs the violation surfaced at COMMIT and aborted
// the whole bucket on every pull ("Cloud sync merge error (discovery_tracks):
// FOREIGN KEY constraint failed").

fn insert_discovery_release(conn: &Connection, id: &str, hlc: &str) {
    conn.execute(
        "INSERT INTO discovery_releases (id, url, source_type, date_added, date_modified, _hlc)
         VALUES (?1, ?2, 'bandcamp', '2026-01-01', '2026-01-01', ?3)",
        params![id, format!("https://x.bandcamp.com/album/{id}"), hlc],
    )
    .unwrap();
}

fn dt_live(id: &str, release_id: &str, position: i32, hlc: &str) -> ParsedRow {
    parsed(json!({
        "id": id, "release_id": release_id, "name": format!("Track {position}"),
        "position": position, "duration_ms": null, "video_id": null, "is_liked": false,
        "_hlc": hlc, "_deleted": false
    }))
}

#[test]
fn orphan_child_entity_is_skipped_not_fatal() {
    let conn = mem();
    // "r1" exists locally; "r-deleted" was deleted locally (tombstone, no live row).
    insert_discovery_release(&conn, "r1", "0005");
    conn.execute(
        "INSERT INTO sync_tombstones (entity_type, entity_id, _hlc) VALUES ('discovery_release', 'r-deleted', '0009')",
        [],
    )
    .unwrap();

    // Remote tracks bucket carries rows for BOTH releases (the peer hasn't seen the
    // delete yet). The orphan must be skipped; the valid row must still apply.
    merge_bucket(
        &conn,
        &Bucket::DiscoveryTracks,
        &[
            dt_live("t-orphan", "r-deleted", 1, "0005"),
            dt_live("t-ok", "r1", 1, "0005"),
        ],
    )
    .unwrap();

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM discovery_tracks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "only the track with a live parent applies");
    let ok: Option<String> = conn
        .query_row(
            "SELECT release_id FROM discovery_tracks WHERE id = 't-ok'",
            [],
            |r| r.get(0),
        )
        .optional()
        .unwrap();
    assert_eq!(ok.as_deref(), Some("r1"));
}

#[test]
fn orphan_child_update_pointing_at_missing_parent_is_skipped() {
    let conn = mem();
    insert_discovery_release(&conn, "r1", "0005");
    // Track exists locally under r1.
    merge_bucket(
        &conn,
        &Bucket::DiscoveryTracks,
        &[dt_live("t1", "r1", 1, "0005")],
    )
    .unwrap();
    // Remote update re-parents it onto a release that doesn't exist locally — must
    // be skipped (not applied, not fatal), keeping the local row intact.
    merge_bucket(
        &conn,
        &Bucket::DiscoveryTracks,
        &[dt_live("t1", "r-gone", 1, "0009")],
    )
    .unwrap();
    let release_id: String = conn
        .query_row(
            "SELECT release_id FROM discovery_tracks WHERE id = 't1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(release_id, "r1", "local row untouched by the orphan update");
}
