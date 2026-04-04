use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    collections::BTreeMap,
};
use thiserror::Error;
use tracing::{info, error};

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("I/O error: {source} on {path}")]
    Io {source: std::io::Error, path: PathBuf },

    #[error("Invalid magic number {magic:#08x}: '{path}'")]
    InvalidMagicNumber { magic: u32, path: PathBuf },

    #[error("Single_Segment_Flag not set: '{path}'")]
    SingleSegmentFlagUnset { path: PathBuf },

    #[error("Invalid FCS_Flag value {value}: '{path}'")]
    InvalidFcsFlag { value: u8, path: PathBuf },

    #[error("Invalid Dictionary_ID flag value {value}: '{path}'")]
    InvalidDictIdFlag { value: u8, path: PathBuf },

    #[error("Content_Checksum_Flag unset: '{path}'")]
    ContentChecksumFlagUnset { path: PathBuf },
}

impl ValidationError {
    fn io(source: std::io::Error, path: &Path) -> Self {
        Self::Io { source: source, path: path.to_path_buf()}
    }
}

struct FrameHeaderDescriptor {
    fcs_bytes: usize,
    dict_id_bytes: u64,
    window_descriptor_bytes: u64,
}

impl FrameHeaderDescriptor {
    fn parse(descriptor: u8, path: &Path) -> Result<Self, ValidationError> {
        let fcs_flag = (descriptor >> 6) & 0x03;
        let single_segment_flag = descriptor & 0x20 != 0;

        let (window_descriptor_bytes, fcs_bytes) = if !single_segment_flag {
            //let window_descriptor_bytes = 1;
            //let fcs_bytes = 0;
            return Err(ValidationError::SingleSegmentFlagUnset { path: path.to_path_buf() });
        } else {
            let wd_bytes = 0;
            let fcs_bytes = match fcs_flag {
                0 => 1,
                1 => 2,
                2 => 4,
                3 => 8,
                v => return Err(ValidationError::InvalidFcsFlag { value: v, path: path.to_path_buf() }),
            };
            (wd_bytes, fcs_bytes)
        };

        let dict_id_flag = descriptor & 0x03;
        let dict_id_bytes = match dict_id_flag {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 4,
            v => return Err(ValidationError::InvalidDictIdFlag { value: v, path: path.to_path_buf() }),
        };

        Ok(Self { fcs_bytes, dict_id_bytes, window_descriptor_bytes })
    }
}

pub struct FileResult {
    pub path: PathBuf,
    pub status: Result<(), ValidationError>,
}

pub fn validate_zroms (inputs: &[PathBuf]) -> Vec<FileResult> {
    inputs.iter().map(|path| {
        let result = (|| {
            let file = File::open(path).map_err(|e| ValidationError::io(e, path))?;
            let mut reader = BufReader::new(file);

            let descriptor = read_frame_header_descriptor(&mut reader, path)?;
            let fhd = FrameHeaderDescriptor::parse(descriptor, path)?;

            validate_magic_number(&mut reader, path)?;
            validate_content_size(&mut reader, &fhd, path)?;
            validate_xxhash(&mut reader, descriptor, path)?;

            Ok(())
        })();

        FileResult {
            path: path.to_path_buf(),
            status: result,
        }
    }).collect()
}

pub fn validate_magic_number(
    reader: &mut BufReader<File>,
    path: &Path
) -> Result<(), ValidationError> {
    reader.seek(SeekFrom::Start(0)).map_err(|e| ValidationError::io(e, path))?;
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).map_err(|e| ValidationError::io(e, path))?;

    if magic == [0x28, 0xB5, 0x2F, 0xFD] {
        info!("Valid magic number: {}", path.display());
        Ok(())
    } else {
        return Err(ValidationError::InvalidMagicNumber {
            magic: u32::from_le_bytes(magic),
            path: path.to_path_buf(),
        })
    }
}

fn validate_content_size(
    reader: &mut BufReader<File>, 
    fhd: &FrameHeaderDescriptor, 
    path: &Path
) -> Result<(), ValidationError> {
    // Magic + Frame_Header = 5 Bytes
    let fcs_offset = 5 + fhd.dict_id_bytes + fhd.window_descriptor_bytes;
    
    reader.seek(SeekFrom::Start(fcs_offset)).map_err(|e| ValidationError::io(e, path))?;
    
    let mut content_size_bytes = [0u8; 8];
    reader.read_exact(&mut content_size_bytes[..fhd.fcs_bytes]).map_err(|e| ValidationError::io(e, path))?;

    let content_size = u64::from_le_bytes(content_size_bytes);
    info!("Content size: {}, {}", content_size, path.display());
    Ok(())
}

pub fn validate_xxhash(
    reader: &mut BufReader<File>, 
    descriptor: u8, 
    path: &Path
) -> Result<(), ValidationError> {
    let file_size = reader.get_ref().metadata().map_err(|e| ValidationError::io(e, path))?.len();

    // Check Content_Checksum_Flag (bit 2)
    if (descriptor & 0x04) == 0 {
        return Err(ValidationError::ContentChecksumFlagUnset { path: path.to_path_buf() });
    }

    let mut xxhash_bytes = [0u8; 4];
    reader.seek(SeekFrom::Start(file_size - 4)).map_err(|e| ValidationError::io(e, path))?;
    reader.read_exact(&mut xxhash_bytes).map_err(|e| ValidationError::io(e, path))?;

    info!("xxHash checksum: {:08x}, {}", u32::from_le_bytes(xxhash_bytes), path.display());
    Ok(())
}

pub fn read_frame_header_descriptor(
    reader: &mut BufReader<File>,
    path: &Path,
) -> Result<u8, ValidationError> {
    reader
        .seek(SeekFrom::Start(4))
        .map_err(|e| ValidationError::io(e, path))?;

    let mut buf = [0u8; 1 as usize];
    reader.read_exact(&mut buf).map_err(|e| ValidationError::io(e, path))?;
    Ok(buf[0])
}

pub fn print_results(results: &[FileResult]) {
    // Markdown
    let md_path = "VALIDATION.md";
    let stats = match std::fs::File::create(md_path) {
        Ok(mut file) => {
            match print_results_md(results, &mut file) {
                Ok(counts) => Some(counts),
                Err(e) => {
                    error!("Failed to write to {}: {}", md_path, e);
                    None
                }
            }
        }
        Err(e) => {
            error!("Could not create {}: {}", md_path, e);
            None
        }
    };

    // Summary
    if let Some((passed, failed)) = stats {
        println!("\n--- Validation Summary ---");
        println!("Total Files:  {}", passed + failed);
        println!("Passed:       {}", passed);
        println!("Failed:       {}", failed);
        println!("--------------------------\n");
    }
}

fn print_results_md(results: &[FileResult], writer: &mut dyn Write) -> std::io::Result<(usize, usize)> {
    let mut total_passed = 0;
    let mut total_failed = 0;

    // Grouping results by directory
    let mut tree: BTreeMap<PathBuf, Vec<&FileResult>> = BTreeMap::new();
    for res in results {
        let parent = res.path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
        tree.entry(parent).or_default().push(res);
    }

    writeln!(writer, "# ZROM Validation Report\n")?;

    for (dir, files) in tree {
        writeln!(writer, "## Directory: `{}/`\n", dir.display())?;
        writeln!(writer, "| Status | File Name | Details |")?;
        writeln!(writer, "| :---: | :--- | :--- |")?;
        
        for res in files {
            let filename = res.path.file_name().unwrap_or_default().to_string_lossy();
            match &res.status {
                Ok(_) => {
                    total_passed += 1;
                    writeln!(writer, "| `pass` | `{}` | `-` |", filename)?;
                }
                Err(e) => {
                    total_failed += 1;
                    let error_msg = format!("{e}").replace('|', "\\|");
                    writeln!(writer, "| `***fail***` | `{}` | `{}` |", filename, error_msg)?;
                }
            }
        }
        writeln!(writer)?; // Space between directory tables
    }

    writeln!(writer, "## Summary\n")?;
    writeln!(writer, "- **Total Passed:** {}", total_passed)?;
    writeln!(writer, "- **Total Failed:** {}", total_failed)?;

    Ok((total_passed, total_failed))
}