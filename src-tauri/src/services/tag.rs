use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{CrateError, Result};
use crate::models::{Tag, TagCategory};

pub struct TagService {
    conn: Arc<Mutex<Connection>>,
}

impl TagService {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get_categories(&self) -> Result<Vec<TagCategory>> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        // Get categories
        let mut stmt = conn.prepare(
            "SELECT id, name, sort_order FROM tag_categories ORDER BY sort_order, name",
        )?;

        let categories: Vec<TagCategory> = stmt
            .query_map([], |row| {
                Ok(TagCategory {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sort_order: row.get(2)?,
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

    pub fn create_category(&self, name: String) -> Result<TagCategory> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        // Check category count (max 4)
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tag_categories",
            [],
            |row| row.get(0),
        )?;

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

        let category = TagCategory {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            sort_order: max_order + 1,
            tags: Vec::new(),
        };

        conn.execute(
            "INSERT INTO tag_categories (id, name, sort_order) VALUES (?1, ?2, ?3)",
            rusqlite::params![category.id, category.name, category.sort_order],
        )?;

        Ok(category)
    }

    pub fn update_category(&self, id: &str, name: String) -> Result<TagCategory> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        conn.execute(
            "UPDATE tag_categories SET name = ?1 WHERE id = ?2",
            rusqlite::params![name, id],
        )?;

        drop(conn);
        self.get_category(id)
    }

    pub fn delete_category(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        conn.execute("DELETE FROM tag_categories WHERE id = ?1", [id])?;
        Ok(())
    }

    fn get_category(&self, id: &str) -> Result<TagCategory> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        let mut category = conn.query_row(
            "SELECT id, name, sort_order FROM tag_categories WHERE id = ?1",
            [id],
            |row| {
                Ok(TagCategory {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sort_order: row.get(2)?,
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

    pub fn create_tag(&self, category_id: String, name: String, color: Option<String>) -> Result<Tag> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

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

        conn.execute(
            "INSERT INTO tags (id, category_id, name, color, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![tag.id, tag.category_id, tag.name, tag.color, tag.sort_order],
        )?;

        Ok(tag)
    }

    pub fn update_tag(&self, id: &str, name: Option<String>, color: Option<String>) -> Result<Tag> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        if let Some(ref n) = name {
            conn.execute("UPDATE tags SET name = ?1 WHERE id = ?2", rusqlite::params![n, id])?;
        }

        if let Some(ref c) = color {
            conn.execute("UPDATE tags SET color = ?1 WHERE id = ?2", rusqlite::params![c, id])?;
        }

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

    pub fn delete_tag(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        conn.execute("DELETE FROM tags WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn assign_tags(&self, track_ids: Vec<String>, tag_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        for track_id in &track_ids {
            for tag_id in &tag_ids {
                conn.execute(
                    "INSERT OR IGNORE INTO track_tags (track_id, tag_id) VALUES (?1, ?2)",
                    rusqlite::params![track_id, tag_id],
                )?;
            }
        }

        Ok(())
    }

    pub fn remove_tags(&self, track_ids: Vec<String>, tag_ids: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        for track_id in &track_ids {
            for tag_id in &tag_ids {
                conn.execute(
                    "DELETE FROM track_tags WHERE track_id = ?1 AND tag_id = ?2",
                    rusqlite::params![track_id, tag_id],
                )?;
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_tracks_by_tag(&self, tag_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().map_err(|_| {
            CrateError::Database(rusqlite::Error::ExecuteReturnedResults)
        })?;

        let mut stmt = conn.prepare("SELECT track_id FROM track_tags WHERE tag_id = ?1")?;

        let track_ids = stmt
            .query_map([tag_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;

        Ok(track_ids)
    }
}
