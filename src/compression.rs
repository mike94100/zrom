use std::{fs::{File},
    io::{self, BufReader, BufWriter},
    path::{Path, PathBuf}};
use zstd::Encoder;
use flate2;
use tar;
use sevenz_rust2;
use zip;
use xz2;
use crate::core::{ZromError, set_date};
use crate::extensions::{get_rom_ext_data};

#[derive(Debug, Clone)]
pub struct Stats {
    pub input_bytes: u64,
    pub output_bytes: u64,
}

impl Stats {
    pub fn ratio(&self) -> f64 {
        if self.input_bytes == 0 {
            return 0.0;
        }
        self.output_bytes as f64 / self.input_bytes as f64
    }
}

/// Compress with configurable options
pub fn pack(input: &Path, output: &Path, level: i32, include_contentsize: bool, include_checksum: bool) -> Result<Stats, ZromError> {
    let input_bytes = input.metadata()?.len();
    let data = get_rom_ext_data(input).ok_or(ZromError::NoExtension)?;
    let date = data.release_date;
    let src = File::open(input)?;
    let dst = File::create(output)?;

    let mut enc = Encoder::new(BufWriter::new(dst), level)
        .map_err(|e| ZromError::Zstd(e.to_string()))?;

    if include_contentsize {
        enc.include_contentsize(true)
            .map_err(|e| ZromError::Zstd(e.to_string()))?;
        enc.set_pledged_src_size(Some(input_bytes))  // ← tell the encoder the size upfront
            .map_err(|e| ZromError::Zstd(e.to_string()))?;
    }

    if include_checksum {
        enc.include_checksum(true)
            .map_err(|e| ZromError::Zstd(e.to_string()))?;
    }

    io::copy(&mut BufReader::new(src), &mut enc)
        .map_err(|e| ZromError::Io(e.to_string()))?;
    enc.finish().map_err(|e| ZromError::Zstd(e.to_string()))?;
    
    set_date(output, date)?;

    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}

/// Compress with conformant zrom settings.
pub fn zrom_pack(input: &Path, output: &Path) -> Result<Stats, ZromError> {
    pack(input, output, 19, true, true)
}

/// Compress using Gzip
pub fn pack_gzip(input: &Path, output: &Path, level: u32) -> Result<Stats, ZromError> {
    let input_bytes = input.metadata()?.len();
    let mut encoder = flate2::write::GzEncoder::new(File::create(output)?, flate2::Compression::new(level));
    io::copy(&mut File::open(input)?, &mut encoder)?;
    encoder.finish()?;
    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}

/// Compress using XZ
pub fn pack_xz(input: &Path, output: &Path, level: u32) -> Result<Stats, ZromError> {
    let input_bytes = input.metadata()?.len();
    let mut encoder = xz2::write::XzEncoder::new(File::create(output)?, level);
    io::copy(&mut File::open(input)?, &mut encoder)?;
    encoder.finish()?;
    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}

/// Compress using Zip (Deflate)
pub fn pack_zip(input: &Path, output: &Path, level: i32) -> Result<Stats, ZromError> {
    let input_bytes = input.metadata()?.len();
    let file = File::create(output)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(level as i64)); // Ensure i64 cast for modern zip crate versions

    
    let file_name = input.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| ZromError::InvalidFile("Invalid input filename".to_string()))?;

    zip.start_file(file_name, options).map_err(|e| ZromError::Io(e.to_string()))?;
    io::copy(&mut File::open(input)?, &mut zip)?;
    zip.finish().map_err(|e| ZromError::Io(e.to_string()))?;
    
    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}

/// Returns path of the compressed file: "game.ext" → "game.ext.zst"
pub fn compressed_path(input: &Path) -> PathBuf {
    let filename = input.file_name().unwrap().to_str().unwrap();
    input.with_file_name(format!("{}.zst", filename))
}

/// Compress entire directory as single tar.zst
pub fn pack_tar_zst_dir(inputs: &[PathBuf], output: &Path, level: i32) -> Result<Stats, ZromError> {
    let mut input_bytes = 0;
    for input in inputs {
        input_bytes += input.metadata()?.len();
    }

    let dst = File::create(output)?;
    let enc = Encoder::new(dst, level)?.auto_finish();
    let mut tar = tar::Builder::new(enc);

    for input in inputs {
        let file_name = input.file_name()
            .ok_or_else(|| ZromError::InvalidFile("Invalid input filename".to_string()))?;
        tar.append_path_with_name(input, file_name)?;
    }
    tar.finish()?;
    drop(tar);

    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}

/// Compress entire directory as single zip
pub fn pack_zip_dir(inputs: &[PathBuf], output: &Path, level: i32) -> Result<Stats, ZromError> {
    let mut input_bytes = 0;
    for input in inputs {
        input_bytes += input.metadata()?.len();
    }

    let file = File::create(output)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(level as i64));

    for input in inputs {
        let file_name = input.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| ZromError::InvalidFile("Invalid input filename".to_string()))?;
        zip.start_file(file_name, options).map_err(|e| ZromError::Io(e.to_string()))?;
        io::copy(&mut File::open(input)?, &mut zip)?;
    }
    zip.finish().map_err(|e| ZromError::Io(e.to_string()))?;

    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}

/// Compress entire directory as single 7z
pub fn pack_7z_dir(inputs: &[PathBuf], output: &Path) -> Result<Stats, ZromError> {
    let mut input_bytes = 0;
    for input in inputs {
        input_bytes += input.metadata()?.len();
    }

    let temp_dir = tempfile::tempdir().map_err(|e| ZromError::Io(e.to_string()))?;

    for input in inputs {
        let file_name = input.file_name()
            .ok_or_else(|| ZromError::InvalidFile("Invalid input filename".to_string()))?;
        let dest_path = temp_dir.path().join(file_name);
        std::fs::copy(input, &dest_path)
            .map_err(|e| ZromError::Io(e.to_string()))?;
    }

    // Compress the temporary directory with 7z
    sevenz_rust2::compress_to_path(temp_dir.path(), output)
        .map_err(|e| ZromError::Io(format!("7z error: {}", e)))?;

    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}