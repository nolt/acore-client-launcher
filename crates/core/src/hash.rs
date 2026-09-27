use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

const BUFFER: usize = 1 << 20;

/// Lowercase hex SHA-256 of a whole file.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    Ok(to_hex(&sha256_prefix(path, u64::MAX)?.finalize()))
}

/// Hasher fed with the first `len` bytes of a file (to resume a download).
pub fn sha256_prefix(path: &Path, len: u64) -> io::Result<Sha256> {
    let mut reader = File::open(path)?.take(len);
    let mut hasher = Sha256::new();
    let mut buf = vec![0; BUFFER];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            return Ok(hasher);
        }
        hasher.update(&buf[..n]);
    }
}

pub fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut hex = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        hex.push(DIGITS[usize::from(b >> 4)].into());
        hex.push(DIGITS[usize::from(b & 0xf)].into());
    }
    hex
}
