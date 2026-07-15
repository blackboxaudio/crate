//! Cloud Storage (Firebase Storage) REST for content-addressed bucket blobs.
//!
//! Gzip lives HERE, not in the pipeline: `upload` receives raw JSONL, gzips it, and
//! stores it with content-type `application/gzip` — deliberately NOT
//! `Content-Encoding: gzip`, which would make GCS transparently decompress on download
//! and break our manual gunzip. `download` gunzips before returning, so the trait
//! always carries UNCOMPRESSED bytes (the manifest's blake3 is over the uncompressed
//! JSONL, computed by the pipeline). The mock stores raw and is unaffected.

use std::io::{Read, Write};
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::StatusCode;

use crate::error::{CrateError, Result};
use crate::services::cloud_sync::backend::types::AuthSession;
use crate::services::cloud_sync::backend::BlobStore;

use super::{rest, FirebaseInner};

pub(crate) struct FirebaseBlobs {
    inner: Arc<FirebaseInner>,
}

impl FirebaseBlobs {
    pub(crate) fn new(inner: Arc<FirebaseInner>) -> Self {
        Self { inner }
    }
}

fn gzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data)
        .map_err(|e| CrateError::CloudSync(format!("gzip: {e}")))?;
    enc.finish()
        .map_err(|e| CrateError::CloudSync(format!("gzip finish: {e}")))
}

fn gunzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut dec = GzDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out)
        .map_err(|e| CrateError::CloudSync(format!("gunzip: {e}")))?;
    Ok(out)
}

/// Overall bound for one blob upload, scaled to its size (assume ≥ 32 KiB/s sustained,
/// floor 120s — an 8 MiB blob gets ~4.5 min). The shared client deliberately has no
/// total timeout (a large blob on a slow link must be allowed to finish; its
/// `read_timeout` kills stalls), so this is the explicit ceiling for the send path,
/// which an inactivity timeout can't cover.
fn transfer_timeout(bytes: usize) -> std::time::Duration {
    std::time::Duration::from_secs(120.max(30 + (bytes / (32 * 1024)) as u64))
}

/// Overall bound for one blob download (size unknown up front; stalls are already
/// killed by the client's `read_timeout`).
const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// The blob's basename (e.g. `discovery_releases-ab12….jsonl.gz`) for logging — the
/// full key embeds the uid, which doesn't belong in logs.
fn basename(key: &str) -> &str {
    key.rsplit('/').next().unwrap_or(key)
}

#[async_trait]
impl BlobStore for FirebaseBlobs {
    async fn upload(
        &self,
        s: &AuthSession,
        key: &str,
        data: Bytes,
        _content_type: &str,
    ) -> Result<()> {
        let compressed = gzip(&data)?;
        log::info!(
            "cloud_sync: uploading blob {} ({:.1} KiB gz / {:.1} KiB raw)",
            basename(key),
            compressed.len() as f64 / 1024.0,
            data.len() as f64 / 1024.0
        );
        let url = format!(
            "{}?name={}",
            self.inner.storage_base(),
            rest::percent_encode(key)
        );
        let deadline = transfer_timeout(compressed.len());
        let request = self
            .inner
            .authed(reqwest::Method::POST, &url, s)
            .await
            .header(reqwest::header::CONTENT_TYPE, "application/gzip")
            .body(compressed);
        let resp = tokio::time::timeout(deadline, request.send())
            .await
            .map_err(|_| CrateError::CloudSyncNetwork("blob upload timed out".into()))?
            .map_err(|e| rest::send_error("blob upload request", e))?;
        if !resp.status().is_success() {
            return Err(rest::http_error("blob upload", resp).await);
        }
        Ok(())
    }

    async fn download(&self, s: &AuthSession, key: &str) -> Result<Bytes> {
        let url = format!(
            "{}/{}?alt=media",
            self.inner.storage_base(),
            rest::percent_encode(key)
        );
        let request = self.inner.authed(reqwest::Method::GET, &url, s).await;
        let compressed = tokio::time::timeout(DOWNLOAD_TIMEOUT, async {
            let resp = request
                .send()
                .await
                .map_err(|e| rest::send_error("blob download request", e))?;
            if resp.status() == StatusCode::NOT_FOUND {
                return Err(CrateError::CloudSyncBlobNotFound(key.to_string()));
            }
            if !resp.status().is_success() {
                return Err(rest::http_error("blob download", resp).await);
            }
            resp.bytes()
                .await
                .map_err(|e| CrateError::CloudSyncNetwork(format!("blob download body: {e}")))
        })
        .await
        .map_err(|_| CrateError::CloudSyncNetwork("blob download timed out".into()))??;
        log::info!(
            "cloud_sync: downloaded blob {} ({:.1} KiB gz)",
            basename(key),
            compressed.len() as f64 / 1024.0
        );
        Ok(Bytes::from(gunzip(&compressed)?))
    }

    async fn delete(&self, s: &AuthSession, key: &str) -> Result<()> {
        let url = format!(
            "{}/{}",
            self.inner.storage_base(),
            rest::percent_encode(key)
        );
        let resp = self
            .inner
            .authed(reqwest::Method::DELETE, &url, s)
            .await
            .send()
            .await
            .map_err(|e| rest::send_error("blob delete request", e))?;
        if !resp.status().is_success() && resp.status() != StatusCode::NOT_FOUND {
            return Err(rest::http_error("blob delete", resp).await);
        }
        Ok(())
    }

    async fn list_prefix(&self, s: &AuthSession, prefix: &str) -> Result<Vec<String>> {
        #[derive(serde::Deserialize)]
        struct ListResponse {
            #[serde(default)]
            items: Vec<ListItem>,
            #[serde(rename = "nextPageToken", default)]
            next_page_token: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct ListItem {
            name: String,
        }

        let mut names = Vec::new();
        let mut page_token: Option<String> = None;
        // Pagination backstop: 1000 pages of 1000 ≫ any real vault.
        for _ in 0..1000 {
            let mut url = format!(
                "{}?prefix={}&maxResults=1000",
                self.inner.storage_base(),
                rest::percent_encode(prefix)
            );
            if let Some(tok) = &page_token {
                url.push_str("&pageToken=");
                url.push_str(&rest::percent_encode(tok));
            }
            let resp = self
                .inner
                .authed(reqwest::Method::GET, &url, s)
                .await
                .send()
                .await
                .map_err(|e| rest::send_error("blob list request", e))?;
            if resp.status() == StatusCode::NOT_FOUND {
                break;
            }
            if !resp.status().is_success() {
                return Err(rest::http_error("blob list", resp).await);
            }
            let page: ListResponse = resp
                .json()
                .await
                .map_err(|e| CrateError::CloudSync(format!("blob list decode: {e}")))?;
            names.extend(page.items.into_iter().map(|i| i.name));
            match page.next_page_token {
                Some(tok) if !tok.is_empty() => page_token = Some(tok),
                _ => break,
            }
        }
        Ok(names)
    }
}
