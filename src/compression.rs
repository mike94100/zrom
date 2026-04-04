use std::{fs::{File},
    io::{self, BufReader, BufWriter},
    path::{Path, PathBuf}};
use zstd::{Encoder};
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
    let date = get_rom_ext_data(input).unwrap().release_date;
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
    
    if let Err(e) = set_date(output, date) {
        ZromError::Io(e.to_string());
    }

    let output_bytes = output.metadata()?.len();
    Ok(Stats { input_bytes, output_bytes })
}

/// Compress with conformant zrom settings.
pub fn zrom_pack(input: &Path, output: &Path) -> Result<Stats, ZromError> {
    pack(input, output, 19, true, true)
}

/// Returns path of the compressed file: "game.ext" → "game.ext.zst"
pub fn compressed_path(input: &Path) -> PathBuf {
    let filename = input.file_name().unwrap().to_str().unwrap();
    input.with_file_name(format!("{}.zst", filename))
}