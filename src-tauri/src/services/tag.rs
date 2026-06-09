use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};
use crate::models::{Tag, TagCategory};
use crate::services::cloud_sync::pipeline::{buckets, dirty};

pub struct TagService {
    conn: Arc<Mutex<Connection>>,
}

impl TagService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get_categories(&self) -> Result<Vec<TagCategory>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        // Get categories
        let mut stmt = conn.prepare(
            "SELECT id, name, color, sort_order FROM tag_categories ORDER BY sort_order, name",
        )?;

        let categories: Vec<TagCategory> = stmt
            .query_map([], |row| {
                Ok(TagCategory {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    sort_order: row.get(3)?,
                    tags: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Get all tags
        let mut stmt = conn.prepare(
            "SELECT id, category_id, name, color, sort_order FROM tags ORDER BY sort_order, name",
        )?;

        let tags: Vec<Tag> = stmt
            .query_map([], |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    category_id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    sort_order: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Group tags by category
        let mut result = categories;
        for tag in tags {
            if let Some(cat) = result.iter_mut().find(|c| c.id == tag.category_id) {
                cat.tags.push(tag);
            }
        }

        Ok(result)
    }

    pub fn create_category(&self, name: String, color: Option<String>) -> Result<TagCategory> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        // Check category count (max 4)
        let count: i32 =
            conn.query_row("SELECT COUNT(*) FROM tag_categories", [], |row| row.get(0))?;

        if count >= 4 {
            return Err(CrateError::InvalidOperation(
                "Maximum of 4 tag categories allowed".to_string(),
            ));
        }

        // Get next sort order
        let max_order: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) FROM tag_categories",
                [],
                |row| row.get(0),
            )
            .unwrap_or(-1);

        // Default color if not provided
        let category_color = color.or_else(|| Some("#6366f1".to_string()));

        let category = TagCategory {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            color: category_color,
            sort_order: max_order + 1,
            tags: Vec::new(),
        };

        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "INSERT INTO tag_categories (id, name, color, sort_order, _hlc) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                category.id,
                category.name,
                category.color,
                category.sort_order,
                hlc
            ],
        )?;
        dirty::mark_dirty(&conn, buckets::TAG_CATEGORIES)?;

        Ok(category)
    }

    pub fn update_category(
        &self,
        id: &str,
        name: Option<String>,
        color: Option<String>,
    ) -> Result<TagCategory> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        if let Some(ref n) = name {
            conn.execute(
                "UPDATE tag_categories SET name = ?1, _hlc = ?2 WHERE id = ?3",
                rusqlite::params![n, hlc, id],
            )?;
        }

        if let Some(ref c) = color {
            conn.execute(
                "UPDATE tag_categories SET color = ?1, _hlc = ?2 WHERE id = ?3",
                rusqlite::params![c, hlc, id],
            )?;
        }
        dirty::mark_dirty(&conn, buckets::TAG_CATEGORIES)?;

        drop(conn);
        self.get_category(id)
    }

    pub fn delete_category(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        dirty::record_tombstone(&conn, buckets::TAG_CATEGORIES, id, &hlc)?;
        conn.execute("DELETE FROM tag_categories WHERE id = ?1", [id])?;
        // Cascade removes this category's tags + their track/discovery links;
        // re-serialize those buckets so peers don't re-insert orphaned rows.
        dirty::mark_dirty(&conn, buckets::TAG_CATEGORIES)?;
        dirty::mark_dirty(&conn, buckets::TAGS)?;
        dirty::mark_dirty(&conn, buckets::TRACK_TAGS)?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASE_TAGS)?;
        Ok(())
    }

    fn get_category(&self, id: &str) -> Result<TagCategory> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let mut category = conn.query_row(
            "SELECT id, name, color, sort_order FROM tag_categories WHERE id = ?1",
            [id],
            |row| {
                Ok(TagCategory {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    sort_order: row.get(3)?,
                    tags: Vec::new(),
                })
            },
        )?;

        // Get tags for this category
        let mut stmt = conn.prepare(
            "SELECT id, category_id, name, color, sort_order FROM tags WHERE category_id = ?1 ORDER BY sort_order, name",
        )?;

        category.tags = stmt
            .query_map([id], |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    category_id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    sort_order: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(category)
    }

    pub fn create_tag(
        &self,
        category_id: String,
        name: String,
        color: Option<String>,
    ) -> Result<Tag> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        // Get next sort order
        let max_order: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) FROM tags WHERE category_id = ?1",
                [&category_id],
                |row| row.get(0),
            )
            .unwrap_or(-1);

        let tag = Tag {
            id: uuid::Uuid::new_v4().to_string(),
            category_id,
            name,
            color,
            sort_order: max_order + 1,
        };

        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "INSERT INTO tags (id, category_id, name, color, sort_order, _hlc) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![tag.id, tag.category_id, tag.name, tag.color, tag.sort_order, hlc],
        )?;
        dirty::mark_dirty(&conn, buckets::TAGS)?;

        Ok(tag)
    }

    pub fn update_tag(&self, id: &str, name: Option<String>, color: Option<String>) -> Result<Tag> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        if let Some(ref n) = name {
            conn.execute(
                "UPDATE tags SET name = ?1, _hlc = ?2 WHERE id = ?3",
                rusqlite::params![n, hlc, id],
            )?;
        }

        if let Some(ref c) = color {
            conn.execute(
                "UPDATE tags SET color = ?1, _hlc = ?2 WHERE id = ?3",
                rusqlite::params![c, hlc, id],
            )?;
        }
        dirty::mark_dirty(&conn, buckets::TAGS)?;

        conn.query_row(
            "SELECT id, category_id, name, color, sort_order FROM tags WHERE id = ?1",
            [id],
            |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    category_id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    sort_order: row.get(4)?,
                })
            },
        )
        .map_err(|e| e.into())
    }

    pub fn move_tag(&self, tag_id: &str, target_category_id: &str) -> Result<Tag> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        // Get current tag
        let (current_category_id, tag_name): (String, String) = conn.query_row(
            "SELECT category_id, name FROM tags WHERE id = ?1",
            [tag_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        // No-op if same category
        if current_category_id == target_category_id {
            return conn
                .query_row(
                    "SELECT id, category_id, name, color, sort_order FROM tags WHERE id = ?1",
                    [tag_id],
                    |row| {
                        Ok(Tag {
                            id: row.get(0)?,
                            category_id: row.get(1)?,
                            name: row.get(2)?,
                            color: row.get(3)?,
                            sort_order: row.get(4)?,
                        })
                    },
                )
                .map_err(|e| e.into());
        }

        // Check for name collision in target category
        let collision: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tags WHERE category_id = ?1 AND name = ?2)",
            rusqlite::params![target_category_id, tag_name],
            |row| row.get(0),
        )?;

        if collision {
            return Err(CrateError::InvalidOperation(
                "A tag with this name already exists in the target category".to_string(),
            ));
        }

        // Compute next sort_order in target category
        let max_order: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) FROM tags WHERE category_id = ?1",
                [target_category_id],
                |row| row.get(0),
            )
            .unwrap_or(-1);

        // Update the tag
        let hlc = dirty::next_hlc(&conn)?;
        conn.execute(
            "UPDATE tags SET category_id = ?1, sort_order = ?2, _hlc = ?3 WHERE id = ?4",
            rusqlite::params![target_category_id, max_order + 1, hlc, tag_id],
        )?;
        dirty::mark_dirty(&conn, buckets::TAGS)?;

        // Return updated tag
        conn.query_row(
            "SELECT id, category_id, name, color, sort_order FROM tags WHERE id = ?1",
            [tag_id],
            |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    category_id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    sort_order: row.get(4)?,
                })
            },
        )
        .map_err(|e| e.into())
    }

    pub fn delete_tag(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        dirty::record_tombstone(&conn, buckets::TAGS, id, &hlc)?;
        conn.execute("DELETE FROM tags WHERE id = ?1", [id])?;
        // Cascade removes this tag's track/discovery links; re-serialize them.
        dirty::mark_dirty(&conn, buckets::TAGS)?;
        dirty::mark_dirty(&conn, buckets::TRACK_TAGS)?;
        dirty::mark_dirty(&conn, buckets::DISCOVERY_RELEASE_TAGS)?;
        Ok(())
    }

    pub fn assign_tags(&self, track_ids: Vec<String>, tag_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        for track_id in &track_ids {
            for tag_id in &tag_ids {
                // OR IGNORE preserves an existing link's _hlc; new links are stamped.
                conn.execute(
                    "INSERT OR IGNORE INTO track_tags (track_id, tag_id, _hlc) VALUES (?1, ?2, ?3)",
                    rusqlite::params![track_id, tag_id, hlc],
                )?;
            }
        }
        dirty::mark_dirty(&conn, buckets::TRACK_TAGS)?;

        Ok(())
    }

    pub fn remove_tags(&self, track_ids: Vec<String>, tag_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let hlc = dirty::next_hlc(&conn)?;
        for track_id in &track_ids {
            for tag_id in &tag_ids {
                let deleted = conn.execute(
                    "DELETE FROM track_tags WHERE track_id = ?1 AND tag_id = ?2",
                    rusqlite::params![track_id, tag_id],
                )?;
                if deleted > 0 {
                    dirty::record_tombstone(
                        &conn,
                        buckets::TRACK_TAGS,
                        &dirty::junction_entity_id(track_id, tag_id),
                        &hlc,
                    )?;
                }
            }
        }
        dirty::mark_dirty(&conn, buckets::TRACK_TAGS)?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_tracks_by_tag(&self, tag_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().map_err(|_| CrateError::LockPoisoned)?;

        let mut stmt = conn.prepare("SELECT track_id FROM track_tags WHERE tag_id = ?1")?;

        let track_ids = stmt
            .query_map([tag_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;

        Ok(track_ids)
    }
}
