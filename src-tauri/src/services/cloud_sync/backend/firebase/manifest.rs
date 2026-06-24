//! Firestore REST for the single manifest document + the GC queue collection.
//!
//! Layout (confirmed): manifest at `users/{uid}/vault/manifest`, GC queue at
//! `users/{uid}/gc_queue/{autoId}`. The manifest write is a compare-and-swap: a
//! `:commit` with a `currentDocument.updateTime == expected` precondition, with the
//! GC enqueues written in the SAME commit so a superseded blob is queued atomically
//! with the swap. A precondition failure maps to [`CrateError::CloudSyncConflict`].

use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use futures::stream::BoxStream;
use reqwest::StatusCode;
use serde_json::{json, Value};

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::backend::types::{
    AuthSession, GcEntry, GcEntryId, Manifest, ManifestEtag,
};
use crate::services::cloud_sync::backend::ManifestStore;

use super::{listener, rest, FirebaseInner};

pub(crate) struct FirebaseManifest {
    inner: Arc<FirebaseInner>,
}

impl FirebaseManifest {
    pub(crate) fn new(inner: Arc<FirebaseInner>) -> Self {
        Self { inner }
    }

    fn manifest_doc_path(uid: &str) -> String {
        format!("users/{uid}/vault/manifest")
    }
}

/// Pull the opaque server token out of a [`ManifestEtag`] (Firebase always uses the
/// token variant). A counter etag here means a backend mix-up.
fn etag_token(etag: &ManifestEtag) -> Result<&str> {
    etag.as_token()
        .ok_or_else(|| CrateError::CloudSync("expected a server etag token".into()))
}

/// Whether a failed `:commit` is a CAS precondition failure (vs. a real error).
fn is_conflict(status: StatusCode, body: &str) -> bool {
    status == StatusCode::CONFLICT
        || status == StatusCode::PRECONDITION_FAILED
        || (status == StatusCode::BAD_REQUEST && body.contains("FAILED_PRECONDITION"))
        || body.contains("\"FAILED_PRECONDITION\"")
        || body.contains("\"ABORTED\"")
}

#[async_trait]
impl ManifestStore for FirebaseManifest {
    async fn read(&self, s: &AuthSession) -> Result<Option<(Manifest, ManifestEtag)>> {
        let url = format!(
            "{}/{}",
            self.inner.firestore_base(),
            Self::manifest_doc_path(&s.uid)
        );
        let resp = self
            .inner
            .authed(reqwest::Method::GET, &url, s)
            .await
            .send()
            .await
            .map_err(|e| rest::send_error("manifest read request", e))?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(rest::http_error("manifest read", resp).await);
        }
        let doc: Value = resp
            .json()
            .await
            .map_err(|e| CrateError::CloudSync(format!("manifest read decode: {e}")))?;
        let manifest: Manifest = rest::parse_json_field(&doc)?;
        let etag = rest::update_time(&doc)
            .map(ManifestEtag::token)
            .ok_or_else(|| CrateError::CloudSync("manifest doc missing updateTime".into()))?;
        Ok(Some((manifest, etag)))
    }

    async fn write(
        &self,
        s: &AuthSession,
        manifest: &Manifest,
        expected: Option<&ManifestEtag>,
        gc_enqueue: &[GcEntry],
    ) -> Result<ManifestEtag> {
        let manifest_name = self.inner.doc_name(&Self::manifest_doc_path(&s.uid));

        let precondition = match expected {
            Some(etag) => {
                let token = etag_token(etag)?;
                json!({ "updateTime": token })
            }
            None => json!({ "exists": false }),
        };

        let manifest_fields = rest::json_fields(manifest)?;
        let mut writes = vec![json!({
            "update": {
                "name": manifest_name,
                "fields": manifest_fields,
            },
            "currentDocument": precondition,
        })];

        for entry in gc_enqueue {
            let gc_id = uuid::Uuid::new_v4().to_string();
            let gc_name = self
                .inner
                .doc_name(&format!("users/{}/gc_queue/{gc_id}", s.uid));
            let gc_fields = rest::json_fields(entry)?;
            writes.push(json!({
                "update": {
                    "name": gc_name,
                    "fields": gc_fields,
                }
            }));
        }

        let commit_url = format!("{}:commit", self.inner.firestore_base());
        let resp = self
            .inner
            .authed(reqwest::Method::POST, &commit_url, s)
            .await
            .json(&json!({ "writes": writes }))
            .send()
            .await
            .map_err(|e| rest::send_error("manifest commit request", e))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            if is_conflict(status, &text) {
                return Err(CrateError::CloudSyncConflict);
            }
            return Err(CrateError::CloudSync(format!(
                "manifest commit HTTP {status}: {text}"
            )));
        }

        let body: Value = resp
            .json()
            .await
            .map_err(|e| CrateError::CloudSync(format!("manifest commit decode: {e}")))?;
        // writeResults[0] is the manifest write; its updateTime is the new CAS token.
        let new_time = body
            .get("writeResults")
            .and_then(|w| w.as_array())
            .and_then(|a| a.first())
            .and_then(|r| r.get("updateTime"))
            .and_then(|t| t.as_str())
            .or_else(|| body.get("commitTime").and_then(|t| t.as_str()))
            .ok_or_else(|| CrateError::CloudSync("commit response missing updateTime".into()))?;
        Ok(ManifestEtag::token(new_time))
    }

    async fn subscribe(
        &self,
        _s: &AuthSession,
    ) -> Result<BoxStream<'static, (Manifest, ManifestEtag)>> {
        // Phase 2: no live updates yet. Phase 3 implements the Firestore Listen stream.
        Ok(listener::empty_stream())
    }

    async fn dequeue_gc(
        &self,
        s: &AuthSession,
        due_before: SystemTime,
        limit: usize,
    ) -> Result<Vec<(GcEntryId, GcEntry)>> {
        // List the gc_queue collection and filter client-side by `delete_after`.
        let url = format!(
            "{}/users/{}/gc_queue?pageSize=300",
            self.inner.firestore_base(),
            s.uid
        );
        let resp = self
            .inner
            .authed(reqwest::Method::GET, &url, s)
            .await
            .send()
            .await
            .map_err(|e| rest::send_error("gc list request", e))?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(vec![]);
        }
        if !resp.status().is_success() {
            return Err(rest::http_error("gc list", resp).await);
        }
        let body: Value = resp
            .json()
            .await
            .map_err(|e| CrateError::CloudSync(format!("gc list decode: {e}")))?;

        let mut out = Vec::new();
        if let Some(docs) = body.get("documents").and_then(|d| d.as_array()) {
            for doc in docs {
                let entry: GcEntry = match rest::parse_json_field(doc) {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                if entry.delete_after <= due_before {
                    if let Some(name) = doc.get("name").and_then(|n| n.as_str()) {
                        let id = name.rsplit('/').next().unwrap_or(name).to_string();
                        out.push((GcEntryId(id), entry));
                        if out.len() >= limit {
                            break;
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    async fn ack_gc(&self, s: &AuthSession, id: GcEntryId) -> Result<()> {
        let url = format!(
            "{}/users/{}/gc_queue/{}",
            self.inner.firestore_base(),
            s.uid,
            rest::percent_encode(&id.0)
        );
        let resp = self
            .inner
            .authed(reqwest::Method::DELETE, &url, s)
            .await
            .send()
            .await
            .map_err(|e| rest::send_error("gc ack request", e))?;
        if !resp.status().is_success() && resp.status() != StatusCode::NOT_FOUND {
            return Err(rest::http_error("gc ack", resp).await);
        }
        Ok(())
    }

    async fn delete(&self, s: &AuthSession) -> Result<()> {
        let url = format!(
            "{}/{}",
            self.inner.firestore_base(),
            Self::manifest_doc_path(&s.uid)
        );
        let resp = self
            .inner
            .authed(reqwest::Method::DELETE, &url, s)
            .await
            .send()
            .await
            .map_err(|e| rest::send_error("manifest delete request", e))?;
        if !resp.status().is_success() && resp.status() != StatusCode::NOT_FOUND {
            return Err(rest::http_error("manifest delete", resp).await);
        }
        Ok(())
    }
}
