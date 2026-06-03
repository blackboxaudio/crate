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
        let url = format!(
            "{}?name={}",
            self.inner.storage_base(),
            rest::percent_encode(key)
        );
        let resp = self
            .inner
            .client
            .post(&url)
            .bearer_auth(&s.access_token)
            .header(reqwest::header::CONTENT_TYPE, "application/gzip")
            .body(compressed)
            .send()
            .await
            .map_err(|e| CrateError::CloudSync(format!("blob upload request: {e}")))?;
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
        let resp = self
            .inner
            .client
            .get(&url)
            .bearer_auth(&s.access_token)
            .send()
            .await
            .map_err(|e| CrateError::CloudSync(format!("blob download request: {e}")))?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Err(CrateError::CloudSyncBlobNotFound(key.to_string()));
        }
        if !resp.status().is_success() {
            return Err(rest::http_error("blob download", resp).await);
        }
        let compressed = resp
            .bytes()
            .await
            .map_err(|e| CrateError::CloudSync(format!("blob download body: {e}")))?;
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
            .client
            .delete(&url)
            .bearer_auth(&s.access_token)
            .send()
            .await
            .map_err(|e| CrateError::CloudSync(format!("blob delete request: {e}")))?;
        if !resp.status().is_success() && resp.status() != StatusCode::NOT_FOUND {
            return Err(rest::http_error("blob delete", resp).await);
        }
        Ok(())
    }
}
