use super::*;
use crate::services::cloud_sync::pipeline::{buckets, dirty};

impl LibraryService {
    pub fn update_track(&self, id: &str, update: TrackUpdate) -> Result<Track> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();

        // Build update query dynamically based on provided fields
        let mut updates: Vec<String> = vec!["date_modified = ?1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        let mut param_idx = 2;

        if let Some(ref title) = update.title {
            updates.push(format!("title = ?{param_idx}"));
            params.push(Box::new(title.clone()));
            param_idx += 1;
        }
        if let Some(ref artist) = update.artist {
            updates.push(format!("artist = ?{param_idx}"));
            params.push(Box::new(artist.clone()));
            param_idx += 1;
        }
        if let Some(ref album) = update.album {
            updates.push(format!("album = ?{param_idx}"));
            params.push(Box::new(album.clone()));
            param_idx += 1;
        }
        if let Some(year) = update.year {
            updates.push(format!("year = ?{param_idx}"));
            params.push(Box::new(year));
            param_idx += 1;
        }
        if let Some(ref genre) = update.genre {
            updates.push(format!("genre = ?{param_idx}"));
            params.push(Box::new(genre.clone()));
            param_idx += 1;
        }
        if let Some(ref label) = update.label {
            updates.push(format!("label = ?{param_idx}"));
            params.push(Box::new(label.clone()));
            param_idx += 1;
        }
        if let Some(bpm) = update.bpm {
            updates.push(format!("bpm = ?{param_idx}"));
            params.push(Box::new(bpm));
            param_idx += 1;
        }
        if let Some(ref key) = update.key {
            updates.push(format!("key = ?{param_idx}"));
            params.push(Box::new(key.clone()));
            param_idx += 1;
        }
        if let Some(rating) = update.rating {
            updates.push(format!("rating = ?{param_idx}"));
            params.push(Box::new(rating));
            param_idx += 1;
        }

        let hlc = dirty::next_hlc(&conn)?;
        updates.push(format!("_hlc = ?{param_idx}"));
        params.push(Box::new(hlc));
        param_idx += 1;

        params.push(Box::new(id.to_string()));

        let sql = format!(
            "UPDATE tracks SET {} WHERE id = ?{}",
            updates.join(", "),
            param_idx
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        conn.execute(&sql, params_refs.as_slice())?;
        dirty::mark_dirty(&conn, &buckets::bucket_for_track_id(id))?;

        drop(conn);
        self.get_track(id)
    }

    /// Update multiple tracks with the same update data (bulk operation)
    pub fn update_tracks(&self, ids: Vec<String>, update: TrackUpdate) -> Result<Vec<Track>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();

        // Build update query dynamically based on provided fields
        let mut updates: Vec<String> = vec!["date_modified = ?1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        let mut param_idx = 2;

        if let Some(ref title) = update.title {
            updates.push(format!("title = ?{param_idx}"));
            params.push(Box::new(title.clone()));
            param_idx += 1;
        }
        if let Some(ref artist) = update.artist {
            updates.push(format!("artist = ?{param_idx}"));
            params.push(Box::new(artist.clone()));
            param_idx += 1;
        }
        if let Some(ref album) = update.album {
            updates.push(format!("album = ?{param_idx}"));
            params.push(Box::new(album.clone()));
            param_idx += 1;
        }
        if let Some(year) = update.year {
            updates.push(format!("year = ?{param_idx}"));
            params.push(Box::new(year));
            param_idx += 1;
        }
        if let Some(ref genre) = update.genre {
            updates.push(format!("genre = ?{param_idx}"));
            params.push(Box::new(genre.clone()));
            param_idx += 1;
        }
        if let Some(ref label) = update.label {
            updates.push(format!("label = ?{param_idx}"));
            params.push(Box::new(label.clone()));
            param_idx += 1;
        }
        if let Some(bpm) = update.bpm {
            updates.push(format!("bpm = ?{param_idx}"));
            params.push(Box::new(bpm));
            param_idx += 1;
        }
        if let Some(ref key) = update.key {
            updates.push(format!("key = ?{param_idx}"));
            params.push(Box::new(key.clone()));
            param_idx += 1;
        }
        if let Some(rating) = update.rating {
            updates.push(format!("rating = ?{param_idx}"));
            params.push(Box::new(rating));
            param_idx += 1;
        }

        let hlc = dirty::next_hlc(&conn)?;
        updates.push(format!("_hlc = ?{param_idx}"));
        params.push(Box::new(hlc));
        param_idx += 1;

        // Build placeholders for track IDs
        let placeholders: Vec<String> = ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", param_idx + i))
            .collect();

        for id in &ids {
            params.push(Box::new(id.clone()));
        }

        let sql = format!(
            "UPDATE tracks SET {} WHERE id IN ({})",
            updates.join(", "),
            placeholders.join(", ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())?;

        drop(conn);

        // Return all updated tracks
        let mut updated_tracks = Vec::new();
        for id in ids {
            if let Ok(track) = self.get_track(&id) {
                updated_tracks.push(track);
            }
        }

        Ok(updated_tracks)
    }

    /// Set color for multiple tracks (bulk operation)
    pub fn set_track_colors(&self, track_ids: Vec<String>, color: Option<String>) -> Result<()> {
        if track_ids.is_empty() {
            return Ok(());
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let now = chrono::Utc::now().to_rfc3339();
        let hlc = dirty::next_hlc(&conn)?;

        let placeholders: Vec<String> = track_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 4))
            .collect();

        let sql = format!(
            "UPDATE tracks SET color = ?1, date_modified = ?2, _hlc = ?3 WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(color), Box::new(now), Box::new(hlc)];

        for id in &track_ids {
            params.push(Box::new(id.clone()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())?;
        dirty::mark_dirty_track_shards(&conn, &track_ids)?;

        Ok(())
    }

    pub fn delete_tracks(&self, ids: Vec<String>) -> Result<()> {
        // Delete artwork files for each track
        for id in &ids {
            self.artwork_service.delete(id);
        }

        let conn = self
            .conn
            .lock()
            .map_err(|_| CrateError::LockPoisoned)?;

        let placeholders: Vec<String> = ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();

        let sql = format!(
            "DELETE FROM tracks WHERE id IN ({})",
            placeholders.join(", ")
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let hlc = dirty::next_hlc(&conn)?;
        for id in &ids {
            dirty::record_tombstone(&conn, buckets::TRACKS_ENTITY, id, &hlc)?;
        }

        conn.execute(&sql, params_refs.as_slice())?;

        // The deleted tracks' shards, plus the cascade-deleted child buckets
        // (playlist memberships, tag links, cues).
        dirty::mark_dirty_track_shards(&conn, &ids)?;
        dirty::mark_dirty(&conn, buckets::PLAYLIST_TRACKS)?;
        dirty::mark_dirty(&conn, buckets::TRACK_TAGS)?;
        dirty::mark_dirty(&conn, buckets::CUES)?;

        Ok(())
    }
}
