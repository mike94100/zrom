use std::path::{Path, PathBuf};
use filetime::{FileTime, set_file_times};
use walkdir::WalkDir;
use globset::{Glob, GlobSetBuilder};

#[derive(Debug, Clone)]
pub enum ZromError {
    AlreadyZROM,
    Blocked(String, &'static str),
    UnknownExtension(String),
    NoExtension,
    OutputExists(PathBuf),
    ChecksumMismatch,
    InvalidFile(String),
    Io(String),
    Zstd(String),
}


impl std::fmt::Display for ZromError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZromError::AlreadyZROM => write!(f, "file is already a zrom"),
            ZromError::Blocked(s, msg) => write!(f, "'{}' is blocked: {}", s, msg),
            ZromError::UnknownExtension(s) => write!(f, "unknown extension '.{}' — not in the supported format list", s),
            ZromError::NoExtension => write!(f, "file has no extension"),
            ZromError::OutputExists(p) => write!(f, "output already exists: {} (use --force to overwrite)", p.display()),
            ZromError::ChecksumMismatch => write!(f, "zstd checksum mismatch — file is corrupt"),
            ZromError::InvalidFile(s) => write!(f, "invalid file: {}", s),
            ZromError::Io(s) => write!(f, "I/O error: {}", s),
            ZromError::Zstd(s) => write!(f, "zstd error: {}", s),
        }
    }
}

impl From<std::io::Error> for ZromError {
    fn from(e: std::io::Error) -> Self {
        ZromError::Io(e.to_string())
    }
}

/// Apply the date as the file's mtime and atime.
pub fn set_date(path: &Path, date: (i32, u32, u32)) -> Result<(), ZromError> {
    let unix_secs = date_to_unix_secs(date.0, date.1, date.2);
    let ft = FileTime::from_unix_time(unix_secs, 0);
    set_file_times(path, ft, ft).map_err(|e| ZromError::Io(e.to_string()))
}

/// Proleptic Gregorian → Unix timestamp at midnight UTC.
/// Uses the standard Julian Day Number algorithm.
pub fn date_to_unix_secs(y: i32, m: u32, d: u32) -> i64 {
    let jdn = |y: i64, m: i64, d: i64| -> i64 {
        (1461 * (y + 4800 + (m - 14) / 12)) / 4
            + (367 * (m - 2 - 12 * ((m - 14) / 12))) / 12
            - (3 * ((y + 4900 + (m - 14) / 12) / 100)) / 4
            + d - 32075
    };
    let epoch = jdn(1970, 1, 1);
    (jdn(y as i64, m as i64, d as i64) - epoch) * 86_400
}

/// Find files in a directory matching specific extensions
pub fn scan_directory(path: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    if path.is_dir() {
        let mut builder = GlobSetBuilder::new();
        for ext in extensions {
            // Support case-insensitive globbing where possible
            builder.add(Glob::new(&format!("*.{}", ext)).unwrap());
        }
        let globset = builder.build().unwrap();
        
        WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| !globset.matches(e.path().file_name().unwrap()).is_empty())
            .map(|e| e.path().to_path_buf())
            .collect()
    } else if path.exists() {
        vec![path.to_path_buf()]
    } else {
        vec![]
    }
}