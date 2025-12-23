//! Checkpoint service for resumable exports
//!
//! This module provides functionality to save and restore export checkpoints,
//! enabling users to resume failed or interrupted exports.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::{CrateError, Result};
use crate::models::export::{CheckpointState, ExportCheckpoint};

/// Service for managing export checkpoints
pub struct CheckpointService {
    conn: Arc<Mutex<Connection>>,
}

impl CheckpointService {
    /// Create a new checkpoint service
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Create a new checkpoint for an export operation
    pub fn create_checkpoint(
        &self,
        device_id: &str,
        device_name: &str,
        playlist_ids: &[String],
    ) -> Result<ExportCheckpoint> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let checkpoint = ExportCheckpoint {
            id: id.clone(),
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            started_at: now.clone(),
            state: CheckpointState::Copying {
                current_track_id: None,
                bytes_copied: 0,
            },
            playlist_ids: playlist_ids.to_vec(),
            tracks_completed: Vec::new(),
            tracks_failed: Vec::new(),
            last_updated_at: now,
        };

        self.save_checkpoint(&checkpoint)?;
        Ok(checkpoint)
    }

    /// Save a checkpoint to the database
    pub fn save_checkpoint(&self, checkpoint: &ExportCheckpoint) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CrateError::Export(format!("Failed to acquire database lock: {e}")))?;

        let state_json = serde_json::to_string(&checkpoint.state)
            .map_err(|e| CrateError::Export(format!("Failed to serialize state: {e}")))?;
        let playlist_ids_json = serde_json::to_string(&checkpoint.playlist_ids)
            .map_err(|e| CrateError::Export(format!("Failed to serialize playlist_ids: {e}")))?;
        let tracks_completed_json =
            serde_json::to_string(&checkpoint.tracks_completed).map_err(|e| {
                CrateError::Export(format!("Failed to serialize tracks_completed: {e}"))
            })?;
        let tracks_failed_json = serde_json::to_string(&checkpoint.tracks_failed)
            .map_err(|e| CrateError::Export(format!("Failed to serialize tracks_failed: {e}")))?;

        conn.execute(
            r#"
            INSERT INTO export_checkpoints (
                id, device_id, device_name, started_at, state,
                playlist_ids, tracks_completed, tracks_failed, last_updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                state = ?5,
                tracks_completed = ?7,
                tracks_failed = ?8,
                last_updated_at = ?9
            "#,
            rusqlite::params![
                checkpoint.id,
                checkpoint.device_id,
                checkpoint.device_name,
                checkpoint.started_at,
                state_json,
                playlist_ids_json,
                tracks_completed_json,
                tracks_failed_json,
                checkpoint.last_updated_at,
            ],
        )
        .map_err(|e| CrateError::Export(format!("Failed to save checkpoint: {e}")))?;

        Ok(())
    }

    /// Get a pending checkpoint for a device
    pub fn get_pending_checkpoint(&self, device_id: &str) -> Result<Option<ExportCheckpoint>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CrateError::Export(format!("Failed to acquire database lock: {e}")))?;

        let mut stmt = conn
            .prepare(
                r#"
            SELECT id, device_id, device_name, started_at, state,
                   playlist_ids, tracks_completed, tracks_failed, last_updated_at
            FROM export_checkpoints
            WHERE device_id = ?1
            ORDER BY started_at DESC
            LIMIT 1
            "#,
            )
            .map_err(|e| CrateError::Export(format!("Failed to prepare statement: {e}")))?;

        let checkpoint = stmt
            .query_row([device_id], |row| {
                let state_json: String = row.get(4)?;
                let playlist_ids_json: String = row.get(5)?;
                let tracks_completed_json: String = row.get(6)?;
                let tracks_failed_json: String = row.get(7)?;

                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    state_json,
                    playlist_ids_json,
                    tracks_completed_json,
                    tracks_failed_json,
                    row.get::<_, String>(8)?,
                ))
            })
            .optional()
            .map_err(|e| CrateError::Export(format!("Failed to query checkpoint: {e}")))?;

        match checkpoint {
            Some((
                id,
                device_id,
                device_name,
                started_at,
                state_json,
                playlist_ids_json,
                tracks_completed_json,
                tracks_failed_json,
                last_updated_at,
            )) => {
                let state: CheckpointState = serde_json::from_str(&state_json)
                    .map_err(|e| CrateError::Export(format!("Failed to parse state: {e}")))?;
                let playlist_ids: Vec<String> =
                    serde_json::from_str(&playlist_ids_json).map_err(|e| {
                        CrateError::Export(format!("Failed to parse playlist_ids: {e}"))
                    })?;
                let tracks_completed: Vec<String> = serde_json::from_str(&tracks_completed_json)
                    .map_err(|e| {
                        CrateError::Export(format!("Failed to parse tracks_completed: {e}"))
                    })?;
                let tracks_failed: Vec<(String, String)> =
                    serde_json::from_str(&tracks_failed_json).map_err(|e| {
                        CrateError::Export(format!("Failed to parse tracks_failed: {e}"))
                    })?;

                Ok(Some(ExportCheckpoint {
                    id,
                    device_id,
                    device_name,
                    started_at,
                    state,
                    playlist_ids,
                    tracks_completed,
                    tracks_failed,
                    last_updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// Mark a checkpoint as completed (delete it)
    pub fn complete_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CrateError::Export(format!("Failed to acquire database lock: {e}")))?;

        conn.execute(
            "DELETE FROM export_checkpoints WHERE id = ?1",
            [checkpoint_id],
        )
        .map_err(|e| CrateError::Export(format!("Failed to delete checkpoint: {e}")))?;

        Ok(())
    }

    /// Mark a track as completed in a checkpoint
    pub fn mark_track_completed(&self, checkpoint_id: &str, track_id: &str) -> Result<()> {
        let mut checkpoint = self
            .get_checkpoint_by_id(checkpoint_id)?
            .ok_or_else(|| CrateError::Export("Checkpoint not found".to_string()))?;

        if !checkpoint.tracks_completed.contains(&track_id.to_string()) {
            checkpoint.tracks_completed.push(track_id.to_string());
        }
        checkpoint.last_updated_at = Utc::now().to_rfc3339();

        self.save_checkpoint(&checkpoint)
    }

    /// Mark a track as failed in a checkpoint
    pub fn mark_track_failed(
        &self,
        checkpoint_id: &str,
        track_id: &str,
        error: &str,
    ) -> Result<()> {
        let mut checkpoint = self
            .get_checkpoint_by_id(checkpoint_id)?
            .ok_or_else(|| CrateError::Export("Checkpoint not found".to_string()))?;

        checkpoint
            .tracks_failed
            .push((track_id.to_string(), error.to_string()));
        checkpoint.last_updated_at = Utc::now().to_rfc3339();

        self.save_checkpoint(&checkpoint)
    }

    /// Update the checkpoint state
    pub fn update_state(&self, checkpoint_id: &str, state: CheckpointState) -> Result<()> {
        let mut checkpoint = self
            .get_checkpoint_by_id(checkpoint_id)?
            .ok_or_else(|| CrateError::Export("Checkpoint not found".to_string()))?;

        checkpoint.state = state;
        checkpoint.last_updated_at = Utc::now().to_rfc3339();

        self.save_checkpoint(&checkpoint)
    }

    /// Delete a checkpoint
    pub fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        self.complete_checkpoint(checkpoint_id)
    }

    /// Get a checkpoint by ID
    fn get_checkpoint_by_id(&self, checkpoint_id: &str) -> Result<Option<ExportCheckpoint>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CrateError::Export(format!("Failed to acquire database lock: {e}")))?;

        let mut stmt = conn
            .prepare(
                r#"
            SELECT id, device_id, device_name, started_at, state,
                   playlist_ids, tracks_completed, tracks_failed, last_updated_at
            FROM export_checkpoints
            WHERE id = ?1
            "#,
            )
            .map_err(|e| CrateError::Export(format!("Failed to prepare statement: {e}")))?;

        let checkpoint = stmt
            .query_row([checkpoint_id], |row| {
                let state_json: String = row.get(4)?;
                let playlist_ids_json: String = row.get(5)?;
                let tracks_completed_json: String = row.get(6)?;
                let tracks_failed_json: String = row.get(7)?;

                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    state_json,
                    playlist_ids_json,
                    tracks_completed_json,
                    tracks_failed_json,
                    row.get::<_, String>(8)?,
                ))
            })
            .optional()
            .map_err(|e| CrateError::Export(format!("Failed to query checkpoint: {e}")))?;

        match checkpoint {
            Some((
                id,
                device_id,
                device_name,
                started_at,
                state_json,
                playlist_ids_json,
                tracks_completed_json,
                tracks_failed_json,
                last_updated_at,
            )) => {
                let state: CheckpointState = serde_json::from_str(&state_json)
                    .map_err(|e| CrateError::Export(format!("Failed to parse state: {e}")))?;
                let playlist_ids: Vec<String> =
                    serde_json::from_str(&playlist_ids_json).map_err(|e| {
                        CrateError::Export(format!("Failed to parse playlist_ids: {e}"))
                    })?;
                let tracks_completed: Vec<String> = serde_json::from_str(&tracks_completed_json)
                    .map_err(|e| {
                        CrateError::Export(format!("Failed to parse tracks_completed: {e}"))
                    })?;
                let tracks_failed: Vec<(String, String)> =
                    serde_json::from_str(&tracks_failed_json).map_err(|e| {
                        CrateError::Export(format!("Failed to parse tracks_failed: {e}"))
                    })?;

                Ok(Some(ExportCheckpoint {
                    id,
                    device_id,
                    device_name,
                    started_at,
                    state,
                    playlist_ids,
                    tracks_completed,
                    tracks_failed,
                    last_updated_at,
                }))
            }
            None => Ok(None),
        }
    }
}
