//! Builds `version.json` from a folder of game files.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};
use crate::hash::sha256_file;
use crate::manifest::{Manifest, ManifestFile};

pub struct GenerateOptions {
    pub input: PathBuf,
    pub version: String,
    pub base_url: String,
    /// Files to leave out, e.g. the manifest being written into the same folder.
    pub exclude: Vec<PathBuf>,
}

pub struct Generated {
    pub manifest: Manifest,
    /// Hidden files and folders that were left out (web servers usually
    /// refuse to serve them, which would break every update).
    pub skipped: Vec<String>,
}

pub fn generate(options: &GenerateOptions) -> Result<Generated> {
    let exclude: Vec<PathBuf> = options
        .exclude
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .collect();
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    walk(&options.input, "", &exclude, &mut files, &mut skipped)?;
    // Ordinal order: an unchanged folder gives a byte-identical manifest.
    files.sort_by(|a: &ManifestFile, b| a.path.cmp(&b.path));
    Ok(Generated {
        manifest: Manifest {
            version: options.version.clone(),
            generated_at_utc: utc_now_iso(),
            base_url: options.base_url.clone(),
            files,
        },
        skipped,
    })
}

fn walk(
    dir: &Path,
    prefix: &str,
    exclude: &[PathBuf],
    files: &mut Vec<ManifestFile>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let entries = fs::read_dir(dir).map_err(|e| Error::io(dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::io(dir, e))?;
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            skipped.push(format!(
                "{prefix}{} (name is not UTF-8)",
                entry.file_name().to_string_lossy()
            ));
            continue;
        };
        let rel = format!("{prefix}{name}");
        // Follows symlinks, so a linked folder is published like a real one.
        let meta = fs::metadata(&path).map_err(|e| Error::io(&path, e))?;
        if name.starts_with('.') {
            skipped.push(if meta.is_dir() { format!("{rel}/") } else { rel });
            continue;
        }
        if meta.is_dir() {
            walk(&path, &format!("{rel}/"), exclude, files, skipped)?;
        } else if meta.is_file() {
            if name.to_ascii_lowercase().ends_with(".part")
                || path.canonicalize().is_ok_and(|p| exclude.contains(&p))
            {
                continue;
            }
            let hash = sha256_file(&path).map_err(|e| Error::io(&path, e))?;
            files.push(ManifestFile {
                path: rel,
                hash,
                size: meta.len(),
            });
        }
    }
    Ok(())
}

/// Default release version: today's UTC date, "2026.09.27".
pub fn today_version() -> String {
    let (y, m, d, ..) = utc_now();
    format!("{y:04}.{m:02}.{d:02}")
}

fn utc_now_iso() -> String {
    let (y, mo, d, h, mi, s) = utc_now();
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn utc_now() -> (i64, u32, u32, u32, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400) as u32);
    let (y, m, d) = civil_from_days(days);
    (y, m, d, rem / 3600, rem / 60 % 60, rem % 60)
}

/// Days since 1970-01-01 to a Gregorian date (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_days_to_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_723), (2026, 9, 27));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    #[test]
    fn lists_files_and_skips_hidden_ones() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("Data/plPL")).unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join("Data/plPL/patch-plPL-4.MPQ"), b"pl").unwrap();
        fs::write(dir.path().join("Wow.exe"), b"exe").unwrap();
        fs::write(dir.path().join(".stamp"), b"x").unwrap();
        fs::write(dir.path().join("version.json"), b"{}").unwrap();

        let generated = generate(&GenerateOptions {
            input: dir.path().to_path_buf(),
            version: "1".into(),
            base_url: String::new(),
            exclude: vec![dir.path().join("version.json")],
        })
        .unwrap();

        let paths: Vec<_> = generated.manifest.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["Data/plPL/patch-plPL-4.MPQ", "Wow.exe"]);
        let mut skipped = generated.skipped;
        skipped.sort();
        assert_eq!(skipped, [".git/", ".stamp"]);
    }
}
