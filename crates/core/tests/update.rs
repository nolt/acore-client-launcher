//! End-to-end: a tiny HTTP server serves a patch folder, the updater syncs a
//! game folder from it.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use launcher_core::{
    Error, GenerateOptions, LocalState, Pause, Progress, UpdateOptions, Url, generate, update,
};

/// One GET the server received: path and Range header.
type Requests = Arc<Mutex<Vec<(String, Option<String>)>>>;

struct Server {
    url: String,
    requests: Requests,
}

impl Server {
    fn start(root: &Path) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/patch/version.json", listener.local_addr().unwrap());
        let requests = Requests::default();
        let (root, log) = (root.to_path_buf(), requests.clone());
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (root, log) = (root.clone(), log.clone());
                thread::spawn(move || serve(stream, &root, &log));
            }
        });
        Server { url, requests }
    }

    /// File downloads so far (the manifest itself not counted).
    fn file_gets(&self) -> Vec<(String, Option<String>)> {
        let all = self.requests.lock().unwrap();
        all.iter()
            .filter(|(p, _)| !p.ends_with("version.json"))
            .cloned()
            .collect()
    }
}

fn serve(stream: std::net::TcpStream, root: &Path, log: &Requests) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let mut range = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap() == 0 || header == "\r\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("range")
        {
            range = Some(value.trim().to_owned());
        }
    }
    log.lock().unwrap().push((path.clone(), range.clone()));

    let rel = path.trim_start_matches("/patch/").replace("%20", " ");
    let mut out = stream;
    let Ok(body) = fs::read(root.join(rel)) else {
        let _ = out.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return;
    };
    let start = range
        .as_deref()
        .and_then(|r| r.strip_prefix("bytes="))
        .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok());
    let head = match start {
        Some(s) => format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {s}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len() - 1,
            body.len(),
            body.len() - s
        ),
        None => format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        ),
    };
    let _ = out.write_all(head.as_bytes());
    let _ = out.write_all(&body[start.unwrap_or(0)..]);
}

/// Patch folder with a manifest, served over HTTP.
struct Fixture {
    _patch: tempfile::TempDir,
    patch_dir: PathBuf,
    game: tempfile::TempDir,
    server: Server,
}

impl Fixture {
    fn new(files: &[(&str, &[u8])]) -> Fixture {
        let patch = tempfile::tempdir().unwrap();
        let patch_dir = patch.path().to_path_buf();
        for (path, content) in files {
            write(&patch_dir.join(path), content);
        }
        let fixture = Fixture {
            server: Server::start(&patch_dir),
            _patch: patch,
            patch_dir,
            game: tempfile::tempdir().unwrap(),
        };
        fixture.publish();
        fixture
    }

    /// Regenerates version.json from the patch folder.
    fn publish(&self) {
        let generated = generate(&GenerateOptions {
            input: self.patch_dir.clone(),
            version: "test".into(),
            base_url: String::new(),
            exclude: vec![self.patch_dir.join("version.json")],
        })
        .unwrap();
        fs::write(
            self.patch_dir.join("version.json"),
            serde_json::to_vec(&generated.manifest).unwrap(),
        )
        .unwrap();
    }

    fn game(&self, path: &str) -> PathBuf {
        self.game.path().join(path)
    }

    async fn update(&self, full_check: bool) -> Result<launcher_core::UpdateSummary, Error> {
        let options = UpdateOptions {
            manifest_url: Url::parse(&self.server.url).unwrap(),
            game_dir: self.game.path().to_path_buf(),
            full_check,
            concurrency: 3,
            pause: None,
        };
        update(&options, |_| {}).await
    }
}

fn write(path: &Path, content: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn big(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 % 251) as u8).collect()
}

#[tokio::test]
async fn downloads_missing_files_then_skips_them() {
    let data = big(300_000);
    let fx = Fixture::new(&[
        ("Data/patch-4.MPQ", &data),
        ("Data/pl PL/realmlist.wtf", b"set realmlist x"),
        ("empty.txt", b""),
    ]);

    let summary = fx.update(false).await.unwrap();
    assert_eq!(summary.files_downloaded, 3);
    assert_eq!(fs::read(fx.game("Data/patch-4.MPQ")).unwrap(), data);
    assert_eq!(
        fs::read(fx.game("Data/pl PL/realmlist.wtf")).unwrap(),
        b"set realmlist x"
    );
    assert_eq!(fs::read(fx.game("empty.txt")).unwrap(), b"");
    assert!(LocalState::exists(fx.game.path()));
    // The empty file is created without a request.
    assert_eq!(fx.server.file_gets().len(), 2);

    // Second run: nothing changed, nothing downloaded.
    let summary = fx.update(false).await.unwrap();
    assert_eq!(summary.files_downloaded, 0);
    assert_eq!(fx.server.file_gets().len(), 2);
}

#[tokio::test]
async fn replaces_changed_files_and_leaves_the_rest_alone() {
    let fx = Fixture::new(&[("Data/patch-4.MPQ", b"new"), ("Wow.exe", b"exe")]);
    write(&fx.game("Data/patch-4.MPQ"), b"old");
    write(&fx.game("Data/patch-Z.MPQ"), b"player's own patch");
    write(&fx.game("Wow.exe"), b"exe");

    let summary = fx.update(false).await.unwrap();
    assert_eq!(summary.files_downloaded, 1);
    assert_eq!(fs::read(fx.game("Data/patch-4.MPQ")).unwrap(), b"new");
    // Not in the manifest: invisible to the launcher, never deleted.
    assert_eq!(
        fs::read(fx.game("Data/patch-Z.MPQ")).unwrap(),
        b"player's own patch"
    );
}

#[tokio::test]
async fn full_check_catches_a_file_changed_behind_our_back() {
    let fx = Fixture::new(&[("a.bin", b"aaaa")]);
    fx.update(false).await.unwrap();

    // Same size and, after restoring it, same modification time: only a full
    // check can notice.
    let path = fx.game("a.bin");
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    fs::write(&path, b"bbbb").unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(modified)
        .unwrap();

    assert_eq!(fx.update(false).await.unwrap().files_downloaded, 0);
    assert_eq!(fx.update(true).await.unwrap().files_downloaded, 1);
    assert_eq!(fs::read(&path).unwrap(), b"aaaa");
}

#[tokio::test]
async fn resumes_an_interrupted_download() {
    let data = big(200_000);
    let fx = Fixture::new(&[("big.bin", &data)]);
    write(&fx.game("big.bin.part"), &data[..120_000]);

    fx.update(false).await.unwrap();
    assert_eq!(fs::read(fx.game("big.bin")).unwrap(), data);
    assert!(!fx.game("big.bin.part").exists());
    assert_eq!(
        fx.server.file_gets(),
        [("/patch/big.bin".to_owned(), Some("bytes=120000-".to_owned()))]
    );
}

#[tokio::test]
async fn pause_stops_the_downloads_and_the_next_run_finishes_them() {
    let (a, b) = (big(200_000), big(100_000));
    let fx = Fixture::new(&[("a.bin", &a), ("b.bin", &b)]);
    let pause = Pause::default();
    let on_download = pause.clone();
    let options = UpdateOptions {
        manifest_url: Url::parse(&fx.server.url).unwrap(),
        game_dir: fx.game.path().to_path_buf(),
        full_check: false,
        concurrency: 1,
        pause: Some(pause),
    };
    let err = update(&options, move |p| {
        if matches!(p, Progress::Downloading { .. }) {
            on_download.request();
        }
    })
    .await
    .unwrap_err();
    assert!(matches!(err, Error::Paused), "{err}");
    // One file at a time: the second one never started.
    assert!(fx.server.file_gets().iter().all(|(p, _)| !p.ends_with("b.bin")));

    fx.update(false).await.unwrap();
    assert_eq!(fs::read(fx.game("a.bin")).unwrap(), a);
    assert_eq!(fs::read(fx.game("b.bin")).unwrap(), b);
}

#[tokio::test]
async fn a_corrupt_download_never_replaces_the_file() {
    let fx = Fixture::new(&[("a.bin", b"good")]);
    write(&fx.game("a.bin"), b"mine");
    // The server now serves different bytes than the manifest promises.
    fs::write(fx.patch_dir.join("a.bin"), b"evil").unwrap();

    let err = fx.update(false).await.unwrap_err();
    assert!(matches!(err, Error::HashMismatch { .. }), "{err}");
    assert_eq!(fs::read(fx.game("a.bin")).unwrap(), b"mine");
    assert!(!fx.game("a.bin.part").exists());
    assert_eq!(fx.server.file_gets().len(), 3, "retried");
}

#[tokio::test]
async fn a_missing_file_on_the_server_is_an_error() {
    let fx = Fixture::new(&[("a.bin", b"a")]);
    fs::remove_file(fx.patch_dir.join("a.bin")).unwrap();
    let err = fx.update(false).await.unwrap_err();
    assert!(matches!(err, Error::HttpStatus { status: 404, .. }), "{err}");
    assert_eq!(fx.server.file_gets().len(), 1, "404 is not retried");
}

#[tokio::test]
async fn refuses_a_manifest_that_escapes_the_game_folder() {
    let fx = Fixture::new(&[]);
    fs::write(
        fx.patch_dir.join("version.json"),
        r#"{"version":"x","files":[{"path":"../evil","hash":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","size":0}]}"#,
    )
    .unwrap();
    let err = fx.update(false).await.unwrap_err();
    assert!(matches!(err, Error::Manifest(_)), "{err}");
}

#[tokio::test]
async fn reports_progress() {
    let fx = Fixture::new(&[("a.bin", &big(50_000)), ("b.bin", &big(70_000))]);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let options = UpdateOptions {
        manifest_url: Url::parse(&fx.server.url).unwrap(),
        game_dir: fx.game.path().to_path_buf(),
        full_check: false,
        concurrency: 1,
        pause: None,
    };
    update(&options, move |p| log.lock().unwrap().push(p))
        .await
        .unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(seen[0], Progress::FetchingManifest);
    assert!(seen.contains(&Progress::Checking { done: 2, total: 2 }));
    let Some(Progress::Downloading {
        file,
        files_total,
        bytes_done,
        bytes_total,
        ..
    }) = seen.last()
    else {
        panic!("last event should be a download: {:?}", seen.last());
    };
    assert_eq!(
        (*file, *files_total, *bytes_done, *bytes_total),
        (2, 2, 120_000, 120_000)
    );
}
