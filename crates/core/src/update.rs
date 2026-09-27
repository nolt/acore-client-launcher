use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use futures_util::{StreamExt, stream};
use reqwest::header::CACHE_CONTROL;
use reqwest::{Client, Url};

use crate::download::download_with_retry;
use crate::error::{Error, Result};
use crate::hash::sha256_file;
use crate::manifest::{Manifest, ManifestFile};
use crate::progress::{Progress, RateMeter};
use crate::state::{FileRecord, LocalState};

pub struct UpdateOptions {
    /// Full URL of `version.json`.
    pub manifest_url: Url,
    pub game_dir: PathBuf,
    /// Hash every file instead of trusting unchanged size + modification time.
    pub full_check: bool,
    /// Files downloaded at the same time.
    pub concurrency: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateSummary {
    pub manifest_version: String,
    pub files_downloaded: usize,
    pub bytes_downloaded: u64,
}

pub(crate) type Report = Arc<dyn Fn(Progress) + Send + Sync>;

/// Brings the game folder in line with the manifest: downloads missing and
/// outdated files, never deletes anything, ignores files the manifest does not list.
pub async fn update(
    options: &UpdateOptions,
    report: impl Fn(Progress) + Send + Sync + 'static,
) -> Result<UpdateSummary> {
    let report: Report = Arc::new(report);
    let client = http_client()?;
    let game_dir = &options.game_dir;
    std::fs::create_dir_all(game_dir).map_err(|e| Error::io(game_dir, e))?;

    report(Progress::FetchingManifest);
    let manifest = fetch_manifest(&client, &options.manifest_url).await?;

    let mut state = LocalState::load(game_dir);
    let wanted = check_files(&manifest, game_dir, &mut state, options.full_check, &report).await?;
    state.save(game_dir)?;

    let bytes_total = wanted.iter().map(|f| f.size).sum();
    let files_total = wanted.len();
    let shared = Shared::new(files_total, bytes_total, report.clone());
    // Owned items: a stream of references trips rustc's Send check when the
    // update runs on a spawned task.
    let (manifest_ref, shared_ref, client_ref) = (&manifest, &shared, &client);
    let mut downloads = stream::iter(wanted)
        .map(|file: ManifestFile| async move {
            let url = manifest_ref.file_url(&options.manifest_url, &file)?;
            shared_ref.start_file(&file.path);
            let record = download_with_retry(client_ref, &url, game_dir, &file, shared_ref).await?;
            shared_ref.finish_file(&file.path);
            Ok::<_, Error>((file.path, record))
        })
        .buffer_unordered(options.concurrency.max(1));

    let mut failure = None;
    while let Some(result) = downloads.next().await {
        match result {
            Ok((path, record)) => {
                state.files.insert(path, record);
            }
            Err(e) => {
                // Dropping the stream cancels the other downloads; their
                // .part files stay and the next run resumes them.
                failure = Some(e);
                break;
            }
        }
    }
    drop(downloads);

    if failure.is_none() {
        state.manifest_version = manifest.version.clone();
    }
    state.save(game_dir)?;
    match failure {
        Some(e) => Err(e),
        None => Ok(UpdateSummary {
            manifest_version: manifest.version,
            files_downloaded: files_total,
            bytes_downloaded: bytes_total,
        }),
    }
}

pub(crate) fn http_client() -> Result<Client> {
    // reqwest is built without a bundled crypto provider; ring cross-compiles
    // to Windows without extra tools.
    static PROVIDER: Once = Once::new();
    PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    Ok(Client::builder()
        .user_agent(concat!("acore-launcher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .build()?)
}

async fn fetch_manifest(client: &Client, url: &Url) -> Result<Manifest> {
    let response = client
        .get(url.clone())
        .header(CACHE_CONTROL, "no-cache")
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::HttpStatus {
            url: url.to_string(),
            status: status.as_u16(),
        });
    }
    Manifest::parse(&response.bytes().await?)
}

/// Returns the files that have to be downloaded, refreshing `state` for the rest.
async fn check_files(
    manifest: &Manifest,
    game_dir: &Path,
    state: &mut LocalState,
    full_check: bool,
    report: &Report,
) -> Result<Vec<ManifestFile>> {
    let total = manifest.files.len();
    let mut wanted = Vec::new();
    for (i, file) in manifest.files.iter().enumerate() {
        report(Progress::Checking { done: i, total });
        let path = file.local_path(game_dir);
        let known = if full_check {
            None
        } else {
            state.files.get(&file.path).cloned()
        };
        let expected = file.clone();
        let verified = tokio::task::spawn_blocking(move || verify(&path, &expected, known))
            .await
            .expect("check task panicked")?;
        match verified {
            Some(record) => {
                state.files.insert(file.path.clone(), record);
            }
            None => {
                state.files.remove(&file.path);
                wanted.push(file.clone());
            }
        }
    }
    report(Progress::Checking { done: total, total });
    Ok(wanted)
}

/// `Some(record)` when the file on disk matches the manifest.
fn verify(path: &Path, file: &ManifestFile, known: Option<FileRecord>) -> Result<Option<FileRecord>> {
    let meta = match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => meta,
        _ => return Ok(None),
    };
    if meta.len() != file.size {
        return Ok(None);
    }
    if let Some(record) = known
        && record.hash == file.hash
        && record.matches(&meta)
    {
        return Ok(Some(record));
    }
    let hash = sha256_file(path).map_err(|e| Error::io(path, e))?;
    Ok((hash == file.hash).then(|| FileRecord::new(&meta, &hash)))
}

/// Progress shared by the concurrent downloads.
pub(crate) struct Shared {
    report: Report,
    files_total: usize,
    bytes_total: u64,
    finished: AtomicUsize,
    bytes_done: AtomicU64,
    /// Bytes that actually came over the network (the speed ignores resumed parts).
    received: AtomicU64,
    inner: Mutex<SharedInner>,
}

struct SharedInner {
    /// Downloads in progress, oldest first.
    active: Vec<String>,
    meter: RateMeter,
    last_report: Option<Instant>,
}

impl Shared {
    pub(crate) fn new(files_total: usize, bytes_total: u64, report: Report) -> Self {
        Shared {
            report,
            files_total,
            bytes_total,
            finished: AtomicUsize::new(0),
            bytes_done: AtomicU64::new(0),
            received: AtomicU64::new(0),
            inner: Mutex::new(SharedInner {
                active: Vec::new(),
                meter: RateMeter::new(Duration::from_secs(5)),
                last_report: None,
            }),
        }
    }

    pub(crate) fn start_file(&self, path: &str) {
        self.inner.lock().unwrap().active.push(path.to_owned());
        self.emit(true);
    }

    pub(crate) fn finish_file(&self, path: &str) {
        self.finished.fetch_add(1, Ordering::Relaxed);
        self.inner.lock().unwrap().active.retain(|p| p != path);
        self.emit(true);
    }

    /// `network`: false for bytes already on disk from an interrupted run.
    pub(crate) fn add(&self, bytes: u64, network: bool) {
        self.bytes_done.fetch_add(bytes, Ordering::Relaxed);
        if network {
            self.received.fetch_add(bytes, Ordering::Relaxed);
        }
        self.emit(false);
    }

    pub(crate) fn take_back(&self, bytes: u64) {
        self.bytes_done.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Reports at most ten times a second unless `force`.
    fn emit(&self, force: bool) {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        inner.meter.record(now, self.received.load(Ordering::Relaxed));
        if !force
            && inner
                .last_report
                .is_some_and(|t| now.duration_since(t) < Duration::from_millis(100))
        {
            return;
        }
        inner.last_report = Some(now);

        let bytes_done = self.bytes_done.load(Ordering::Relaxed).min(self.bytes_total);
        let bytes_per_sec = inner.meter.bytes_per_sec();
        let eta = (bytes_per_sec > 0.0)
            .then(|| Duration::from_secs_f64((self.bytes_total - bytes_done) as f64 / bytes_per_sec));
        let progress = Progress::Downloading {
            // With parallel downloads, "file N" means N-1 are finished.
            file: (self.finished.load(Ordering::Relaxed) + 1).min(self.files_total),
            files_total: self.files_total,
            bytes_done,
            bytes_total: self.bytes_total,
            // The longest-running download: the big file the player waits for,
            // not the small ones that flash by.
            current: inner.active.first().cloned().unwrap_or_default(),
            bytes_per_sec,
            eta,
        };
        drop(inner);
        (self.report)(progress);
    }
}
