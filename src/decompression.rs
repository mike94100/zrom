use std::{fs::File, io::{self, BufReader, BufWriter}, path::{Path, PathBuf}};
use zstd::Decoder;
use walkdir::WalkDir;
use crate::core::ZromError;
use crate::compression::Stats;
use zip;
use sevenz_rust2;
use unrar;

/// Decompress a conformant zstd frame.
pub fn unpack(input: &Path, output: &Path) -> Result<Stats, ZromError> {
    let input_bytes = input.metadata()?.len();

    let src = File::open(input)?;
    let dst = File::create(output)?;

    let mut dec = Decoder::new(BufReader::new(src))
        .map_err(|e| ZromError::Zstd(e.to_string()))?;

    io::copy(&mut dec, &mut BufWriter::new(dst))
        .map_err(|e| {
            if e.to_string().contains("checksum") {
                ZromError::ChecksumMismatch
            } else {
                ZromError::Io(e.to_string())
            }
        })?;

    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<Vec<PathBuf>, ZromError> {
    let file = std::fs::File::open(archive).map_err(|e| ZromError::Io(e.to_string()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| ZromError::Io(e.to_string()))?;
    let mut files = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| ZromError::Io(e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let out_path = dest.join(entry.name());
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ZromError::Io(e.to_string()))?;
        }
        let mut out_file = std::fs::File::create(&out_path).map_err(|e| ZromError::Io(e.to_string()))?;
        io::copy(&mut entry, &mut out_file).map_err(|e| ZromError::Io(e.to_string()))?;
        files.push(out_path);
    }
    Ok(files)
}

fn extract_7z(archive: &Path, dest: &Path) -> Result<Vec<PathBuf>, ZromError> {
    sevenz_rust2::decompress_file(archive, dest)
        .map_err(|e| ZromError::Io(format!("7z error: {}", e)))?;
    // Walk the destination to find extracted files
    let mut files = Vec::new();
    for entry in WalkDir::new(dest).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }
    Ok(files)
}

fn extract_rar(archive: &Path, dest: &Path) -> Result<Vec<PathBuf>, ZromError> {
    let mut archive = unrar::Archive::new(archive.to_str().unwrap_or(""))
        .open_for_processing()
        .map_err(|e| ZromError::Io(format!("rar error: {}", e)))?;

    let mut files = Vec::new();

    loop {
        let entry = archive.read_header()
            .map_err(|e| ZromError::Io(format!("rar error: {}", e)))?;

        match entry {
            Some(open_archive) => {
                let filename = open_archive.entry().filename.clone();
                let out_path = dest.join(&filename);

                if !open_archive.entry().is_directory() {
                    archive = open_archive.extract_to(dest)
                        .map_err(|e| ZromError::Io(format!("rar error: {}", e)))?;
                    files.push(out_path);
                } else {
                    archive = open_archive.skip()
                        .map_err(|e| ZromError::Io(format!("rar error: {}", e)))?;
                }
            }
            None => break,
        }
    }

    Ok(files)
}

/// Decompress Gzip
pub fn unpack_gzip(input: &Path, output: &Path) -> Result<Stats, ZromError> {
    let input_bytes = input.metadata()?.len();
    let mut decoder = flate2::read::GzDecoder::new(File::open(input)?);
    io::copy(&mut decoder, &mut File::create(output)?)?;
    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}

/// Decompress XZ
pub fn unpack_xz(input: &Path, output: &Path) -> Result<Stats, ZromError> {
    let input_bytes = input.metadata()?.len();
    let mut decoder = xz2::read::XzDecoder::new(File::open(input)?);
    io::copy(&mut decoder, &mut File::create(output)?)?;
    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}

/// Decompress Zip archive to directory
pub fn unpack_zip_dir(input: &Path, output_dir: &Path) -> Result<(), ZromError> {
    let file = File::open(input)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| ZromError::Io(e.to_string()))?;
    archive.extract(output_dir).map_err(|e| ZromError::Io(e.to_string()))?;
    Ok(())
}

/// Decompress 7z
pub fn unpack_7z(input: &Path, output_dir: &Path) -> Result<Stats, ZromError> {
    let input_bytes = input.metadata()?.len();
    sevenz_rust2::decompress_file(input, output_dir).map_err(|e| ZromError::Io(format!("7z error: {}", e)))?;
    let output_bytes = 0; // Folder-based decompression makes size calculation tricky for benchmark stats
    Ok(Stats { input_bytes, output_bytes })
}

/// Decompress Tar.zst
pub fn unpack_tar_zst(input: &Path, output: &Path) -> Result<Stats, ZromError> {
    let input_bytes = input.metadata()?.len();
    let mut archive = tar::Archive::new(zstd::Decoder::new(File::open(input)?)?);
    let mut entry = archive.entries()?.next().ok_or(ZromError::Zstd("Tar empty".to_string()))??;
    io::copy(&mut entry, &mut File::create(output)?)?;
    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}

/// Decompress Tar.zst archive to directory
pub fn unpack_tar_zst_dir(input: &Path, output_dir: &Path) -> Result<(), ZromError> {
    let mut archive = tar::Archive::new(zstd::Decoder::new(File::open(input)?)?);
    archive.unpack(output_dir).map_err(|e| ZromError::Io(e.to_string()))?;
    Ok(())
}

pub fn extract_archive(archive: &Path, dest: &Path) -> Result<Vec<PathBuf>, ZromError> {
    let ext = archive.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "zip" => extract_zip(archive, dest),
        "7z" => extract_7z(archive, dest),
        "rar" => extract_rar(archive, dest),
        _ => Err(ZromError::UnknownExtension(ext)),
    }
}

/// Returns path of the decompressed file: "game.ext.zst" → "game.ext"
pub fn decompressed_path(input: &Path) -> PathBuf {
    let filename = input.file_name().unwrap().to_str().unwrap();
    let original = filename.strip_suffix(".zst").unwrap();
    input.with_file_name(original)
}