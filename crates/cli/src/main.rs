//! Command-line companion of the launcher: builds manifests for the patch host
//! and runs an update without the GUI.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use launcher_core::{
    GenerateOptions, LauncherInfo, ManifestFile, Progress, UpdateOptions, Url, format, generate, sha256_file,
    today_version, update,
};

const USAGE: &str = "\
Usage:
  acore-launcher-cli manifest <game-files-dir> [--output FILE] [--version V] [--base-url URL]
      Writes version.json (default: <game-files-dir>/version.json) listing every
      file with its SHA-256. Hidden files and folders are left out.
  acore-launcher-cli launcher <version> [--linux FILE] [--windows FILE] [--output FILE]
      Writes launcher.json (default: next to the first binary) announcing launcher
      <version> for self-update. Binary paths are stored relative to launcher.json;
      upload them with the same layout, next to version.json.
  acore-launcher-cli update <manifest-url> <game-dir> [--full-check]
      Downloads missing and outdated files into <game-dir>, like the launcher does.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("manifest") => manifest(&args[1..]),
        Some("launcher") => launcher_info(&args[1..]),
        Some("update") => run_update(&args[1..]),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// Splits `args` into positional arguments and `--name value` / `--flag` options.
struct Args {
    positional: Vec<String>,
    options: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(args: &[String], with_value: &[&str], flags: &[&str]) -> Result<Args, String> {
        let mut parsed = Args {
            positional: Vec::new(),
            options: Vec::new(),
        };
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            if with_value.contains(&arg.as_str()) {
                let value = it.next().ok_or_else(|| format!("{arg} needs a value"))?;
                parsed.options.push((arg.clone(), Some(value.clone())));
            } else if flags.contains(&arg.as_str()) {
                parsed.options.push((arg.clone(), None));
            } else if arg.starts_with("--") {
                return Err(format!("unknown option {arg}\n\n{USAGE}"));
            } else {
                parsed.positional.push(arg.clone());
            }
        }
        Ok(parsed)
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, v)| v.as_deref())
    }

    fn flag(&self, name: &str) -> bool {
        self.options.iter().any(|(n, _)| n == name)
    }
}

fn manifest(args: &[String]) -> Result<(), String> {
    let args = Args::parse(args, &["--output", "--version", "--base-url"], &[])?;
    let [input] = args.positional.as_slice() else {
        return Err(USAGE.to_owned());
    };
    let input = PathBuf::from(input);
    let output = args
        .value("--output")
        .map_or_else(|| input.join("version.json"), PathBuf::from);

    let generated = generate(&GenerateOptions {
        input,
        version: args.value("--version").map_or_else(today_version, str::to_owned),
        base_url: args.value("--base-url").unwrap_or_default().to_owned(),
        exclude: vec![output.clone()],
    })
    .map_err(|e| e.to_string())?;

    for skipped in &generated.skipped {
        eprintln!("skipped hidden: {skipped}");
    }
    let json = serde_json::to_string_pretty(&generated.manifest).map_err(|e| e.to_string())?;
    std::fs::write(&output, json + "\n").map_err(|e| format!("{}: {e}", output.display()))?;
    let total: u64 = generated.manifest.files.iter().map(|f| f.size).sum();
    println!(
        "{}: version {}, {} files, {}",
        output.display(),
        generated.manifest.version,
        generated.manifest.files.len(),
        format::bytes(total)
    );
    Ok(())
}

fn launcher_info(args: &[String]) -> Result<(), String> {
    let args = Args::parse(args, &["--linux", "--windows", "--output"], &[])?;
    let [version] = args.positional.as_slice() else {
        return Err(USAGE.to_owned());
    };
    let binaries: Vec<(&str, PathBuf)> = [("linux-x64", "--linux"), ("windows-x64", "--windows")]
        .into_iter()
        .filter_map(|(platform, option)| args.value(option).map(|p| (platform, PathBuf::from(p))))
        .collect();
    let Some((_, first)) = binaries.first() else {
        return Err(format!("give at least one of --linux, --windows\n\n{USAGE}"));
    };
    let output = args
        .value("--output")
        .map_or_else(|| first.with_file_name("launcher.json"), PathBuf::from);

    let mut files = std::collections::BTreeMap::new();
    for (platform, path) in &binaries {
        let name = relative_url_path(path, &output)?;
        let size = std::fs::metadata(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .len();
        let hash = sha256_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
        files.insert(
            platform.to_string(),
            ManifestFile {
                path: name,
                hash,
                size,
            },
        );
    }
    let info = LauncherInfo {
        version: version.clone(),
        files,
    };
    let json = serde_json::to_string_pretty(&info).map_err(|e| e.to_string())?;
    std::fs::write(&output, json + "\n").map_err(|e| format!("{}: {e}", output.display()))?;
    println!(
        "{}: launcher {version} for {}",
        output.display(),
        info.files.keys().cloned().collect::<Vec<_>>().join(", ")
    );
    Ok(())
}

/// `binary` as a URL path relative to `output`: "launcher/acore-launcher.exe"
/// when it sits in a subfolder, else just the file name.
fn relative_url_path(binary: &Path, output: &Path) -> Result<String, String> {
    let base = output.parent().unwrap_or(Path::new(""));
    let relative = std::path::absolute(binary)
        .ok()
        .zip(std::path::absolute(base).ok())
        .and_then(|(b, o)| b.strip_prefix(o).ok().map(Path::to_path_buf))
        .or_else(|| binary.file_name().map(PathBuf::from))
        .ok_or_else(|| format!("{}: not a file", binary.display()))?;
    let parts: Option<Vec<&str>> = relative.components().map(|c| c.as_os_str().to_str()).collect();
    parts
        .map(|p| p.join("/"))
        .ok_or_else(|| format!("{}: not a UTF-8 path", binary.display()))
}

fn run_update(args: &[String]) -> Result<(), String> {
    let args = Args::parse(args, &[], &["--full-check"])?;
    let [url, dir] = args.positional.as_slice() else {
        return Err(USAGE.to_owned());
    };
    let options = UpdateOptions {
        manifest_url: Url::parse(url).map_err(|e| format!("{url}: {e}"))?,
        game_dir: PathBuf::from(dir),
        full_check: args.flag("--full-check"),
        concurrency: 1,
        pause: None,
    };

    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let summary = runtime
        .block_on(update(&options, print_progress))
        .map_err(|e| e.to_string())?;
    eprintln!();
    println!(
        "version {}: {} files downloaded ({})",
        summary.manifest_version,
        summary.files_downloaded,
        format::bytes(summary.bytes_downloaded)
    );
    Ok(())
}

fn print_progress(progress: Progress) {
    let line = match progress {
        Progress::FetchingManifest => "Fetching manifest…".to_owned(),
        Progress::Checking { done, total } => format!("Checking files {done}/{total}"),
        Progress::Downloading {
            file,
            files_total,
            bytes_done,
            bytes_total,
            current,
            bytes_per_sec,
            eta,
        } => {
            let mut line = format!(
                "Downloading {file}/{files_total}  ({} / {})  {current}",
                format::bytes(bytes_done),
                format::bytes(bytes_total)
            );
            if bytes_per_sec > 0.0 {
                line.push_str("  ");
                line.push_str(&format::rate(bytes_per_sec));
            }
            if let Some(eta) = eta {
                line.push_str("  ");
                line.push_str(&format::eta(eta));
            }
            line
        }
    };
    // One status line, rewritten in place.
    eprint!("\r\x1b[2K{line}");
}
