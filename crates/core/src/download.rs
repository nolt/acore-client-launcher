use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{CONTENT_RANGE, RANGE};
use reqwest::{Client, StatusCode, Url};
use sha2::{Digest, Sha256};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

use crate::error::{Error, Result};
use crate::hash::{sha256_prefix, to_hex};
use crate::manifest::ManifestFile;
use crate::state::FileRecord;
use crate::update::Shared;

const ATTEMPTS: u32 = 3;

/// Downloads one file, retrying transient failures. Progress bytes of a failed
/// attempt are taken back, so the bar never overshoots.
pub async fn download_with_retry(
    client: &Client,
    url: &Url,
    game_dir: &Path,
    file: &ManifestFile,
    shared: &Shared,
) -> Result<FileRecord> {
    let mut attempt = 1;
    loop {
        let mut counted = 0;
        match download(client, url, game_dir, file, shared, &mut counted).await {
            Ok(record) => return Ok(record),
            Err(e) => {
                shared.take_back(counted);
                if attempt >= ATTEMPTS || !e.is_transient() {
                    return Err(e);
                }
                tokio::time::sleep(Duration::from_secs(u64::from(attempt))).await;
                attempt += 1;
            }
        }
    }
}

/// Downloads into `<file>.part` (resuming what is already there), checks size
/// and hash, and only then replaces the real file.
async fn download(
    client: &Client,
    url: &Url,
    game_dir: &Path,
    file: &ManifestFile,
    shared: &Shared,
    counted: &mut u64,
) -> Result<FileRecord> {
    let target = file.local_path(game_dir);
    let part = part_path(&target);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| Error::io(parent, e))?;
    }

    let mut have = match fs::metadata(&part).await {
        Ok(meta) if meta.is_file() && meta.len() <= file.size => meta.len(),
        _ => 0,
    };
    let mut hasher = if have > 0 {
        let path = part.clone();
        tokio::task::spawn_blocking(move || sha256_prefix(&path, have))
            .await
            .expect("hashing task panicked")
            .map_err(|e| Error::io(&part, e))?
    } else {
        Sha256::new()
    };

    if have < file.size {
        let mut request = client.get(url.clone());
        if have > 0 {
            request = request.header(RANGE, format!("bytes={have}-"));
        }
        let response = request.send().await?;
        let status = response.status();
        let resumed = have > 0 && status == StatusCode::PARTIAL_CONTENT && range_starts_at(&response, have);
        if !resumed {
            if !status.is_success() || status == StatusCode::PARTIAL_CONTENT {
                return Err(Error::HttpStatus {
                    url: url.to_string(),
                    status: status.as_u16(),
                });
            }
            // The server ignored the range: start over.
            have = 0;
            hasher = Sha256::new();
        }

        let mut out = OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(&part)
            .await
            .map_err(|e| Error::io(&part, e))?;
        shared.add(have, false);
        *counted += have;

        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk?;
            have += chunk.len() as u64;
            if have > file.size {
                drop(out);
                let _ = fs::remove_file(&part).await;
                return Err(Error::SizeMismatch {
                    path: file.path.clone(),
                    expected: file.size,
                    actual: have,
                });
            }
            out.write_all(&chunk).await.map_err(|e| Error::io(&part, e))?;
            hasher.update(&chunk);
            shared.add(chunk.len() as u64, true);
            *counted += chunk.len() as u64;
        }
        out.flush().await.map_err(|e| Error::io(&part, e))?;
    } else if file.size == 0 {
        fs::write(&part, b"").await.map_err(|e| Error::io(&part, e))?;
    } else {
        // A previous run got the whole file but stopped before checking it.
        shared.add(have, false);
        *counted += have;
    }

    if have != file.size {
        // Keep the .part: the next attempt resumes from here.
        return Err(Error::SizeMismatch {
            path: file.path.clone(),
            expected: file.size,
            actual: have,
        });
    }
    if to_hex(&hasher.finalize()) != file.hash {
        let _ = fs::remove_file(&part).await;
        return Err(Error::HashMismatch {
            path: file.path.clone(),
        });
    }

    // Replaces the old file in one step (also on Windows).
    fs::rename(&part, &target)
        .await
        .map_err(|e| Error::io(&target, e))?;
    let meta = fs::metadata(&target).await.map_err(|e| Error::io(&target, e))?;
    Ok(FileRecord::new(&meta, &file.hash))
}

pub fn part_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    target.with_file_name(name)
}

/// `Content-Range: bytes <start>-...` must continue exactly where we stopped.
fn range_starts_at(response: &reqwest::Response, start: u64) -> bool {
    response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes "))
        .and_then(|v| v.split('-').next())
        .and_then(|v| v.parse::<u64>().ok())
        == Some(start)
}
