use std::path::{PathBuf};
use std::time::Instant;
use std::fs::File;
use std::io::Write;

use tracing::{info};
use bytesize;

use crate::compression::*;
use crate::decompression::*;
use crate::core::ZromError;

#[derive(Debug)]
pub struct BenchmarkResult {
    pub label: String,
    pub compression_time: f64,
    pub decompression_time: f64,
    pub input_bytes: u64,
    pub output_bytes: u64,
}

impl BenchmarkResult {
    // Stats
    pub fn compression_ratio(&self) -> f64 {
        if self.input_bytes == 0 { 0.0 }
        else { self.input_bytes as f64 / self.output_bytes as f64 } 
    }

    pub fn compression_efficiency(&self) -> f64 {
        if self.compression_time == 0.0 { 0.0 }
        else { self.compression_ratio().powf(0.7) * self.compression_time.powf(-0.3) }
    }

    pub fn decompression_efficiency(&self) -> f64 {
        if self.decompression_time == 0.0 { 0.0 }
        else { self.compression_ratio().powf(0.7) * self.decompression_time.powf(-0.3) }
    }

    // Results
    fn format_md(results: &[BenchmarkResult]) -> String {
        let mut output = String::new();
        output.push_str("# Benchmark Results\n\n");
        let headers = ["Level", "In Size", "Out Size", "Ratio",
            "Comp Time (s)", "Decomp Time (s)", "Comp Efficiency", "Decomp Efficiency"];

        // Create header row
        let header_row = headers.join(" | ");
        let separator_row = headers.iter().map(|h| "-".repeat(h.len())).collect::<Vec<_>>().join(" | ");
        output.push_str(&format!("| {} |\n", header_row));
        output.push_str(&format!("| {} |\n", separator_row));

        for result in results {
            output.push_str(&format!(
                "| {} | {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
                result.label,
                bytesize::ByteSize::b(result.input_bytes),
                bytesize::ByteSize::b(result.output_bytes),
                result.compression_ratio(),
                result.compression_time,
                result.decompression_time,
                result.compression_efficiency(),
                result.decompression_efficiency()
            ));
        }
        output
    }

    pub fn save_benchmark(results: &[BenchmarkResult]) -> Result<(), ZromError> {
        // Save as markdown
        let md_path = "BENCHMARK.md";
        let mut file = File::create(md_path)
            .map_err(|e| ZromError::Io(format!("Failed to create file {}: {}", md_path, e)))?;
        file.write_all(Self::format_md(results).as_bytes())
            .map_err(|e| ZromError::Io(format!("Failed to write to file {}: {}", md_path, e)))?;
        info!("Benchmark results save to: {}", md_path);
        Ok(())
    }
}

pub fn benchmark_files(inputs: &[PathBuf]) -> Result<(), ZromError> {
    let mut all_results = Vec::new();

    let single_modes = vec![
        //("Zstd (L1)",   1),
        ("Zstd (L4)",   4),
        ("Zstd (L9)",   9),
        ("Zstd (L19)", 19),
        ("Gzip (L1)",   1),
        ("Gzip (L9)",   9),
        ("XZ (L1)",     1),
        ("XZ (L9)",     9),
        //("Zip (L1)",    1),
        //("Zip (L9)",    9),
    ];

    for (label, level) in single_modes {
        let mut comp_times = 0.0;
        let mut decomp_times = 0.0;
        let mut total_in = 0;
        let mut total_out = 0;

        for input in inputs {
            let temp_comp = tempfile::NamedTempFile::new().map_err(|e| ZromError::Io(e.to_string()))?;
            let temp_decomp = tempfile::NamedTempFile::new().map_err(|e| ZromError::Io(e.to_string()))?;
            let comp_path = temp_comp.path();
            let decomp_path = temp_decomp.path();

            info!("Benchmarking {}: {}", label, input.display());

            // COMPRESSION
            let start = Instant::now();
            let stats = match label {
                l if l.starts_with("Zstd") => pack(input, comp_path, level, true, true)?,
                l if l.starts_with("Gzip") => pack_gzip(input, comp_path, level as u32)?,
                l if l.starts_with("XZ")   => pack_xz(input, comp_path, level as u32)?,
                l if l.starts_with("Zip")  => pack_zip(input, comp_path, level)?,
                _ => return Err(ZromError::Zstd("Unknown benchmark mode".to_string())),
            };
            comp_times += start.elapsed().as_secs_f64();
            let out_size = stats.output_bytes;

            // DECOMPRESSION
            let start = Instant::now();
            match label {
                l if l.starts_with("Zstd")      => { unpack(comp_path, decomp_path)?; }
                l if l.starts_with("Gzip")      => { unpack_gzip(comp_path, decomp_path)?; }
                l if l.starts_with("XZ")        => { unpack_xz(comp_path, decomp_path)?; }
                l if l.starts_with("Zip")       => { unpack_zip(comp_path, decomp_path)?; }
                _ => {}
            };
            decomp_times += start.elapsed().as_secs_f64();

            total_in += input.metadata()?.len();
            total_out += out_size;
        }

        all_results.push(BenchmarkResult {
            label: label.to_string(),
            compression_time: comp_times,
            decompression_time: decomp_times,
            input_bytes: total_in,
            output_bytes: total_out,
        });
    }

    // Directory / Multi-file modes
    let dir_modes = vec![
        //("Zip (Dir) (L9)", 9),
        //("7z (Dir)", 0),
        //("Tar.zst (Dir) (L9)", 9),
        ("Tar.zst (Dir) (L19)", 19),
    ];

    for (label, level) in dir_modes {
        let temp_comp = tempfile::NamedTempFile::new().map_err(|e| ZromError::Io(e.to_string()))?;
        let comp_path = temp_comp.path();
        
        info!("Benchmarking {}: {} files", label, inputs.len());

        let start = Instant::now();
        let stats = match label {
            l if l.starts_with("Zip (Dir)")     => pack_zip_dir(inputs, comp_path, level)?,
            l if l.starts_with("7z (Dir)")      => pack_7z_dir(inputs, comp_path)?,
            l if l.starts_with("Tar.zst (Dir)") => pack_tar_zst_dir(inputs, comp_path, level)?,
            _ => unreachable!(),
        };
        let compression_time = start.elapsed().as_secs_f64();

        let temp_dir = tempfile::tempdir().map_err(|e| ZromError::Io(e.to_string()))?;
        let start = Instant::now();
        match label {
            l if l.starts_with("Zip (Dir)")     => { unpack_zip_dir(comp_path, temp_dir.path())?; },
            l if l.starts_with("7z (Dir)")      => { unpack_7z(comp_path, temp_dir.path())?; },
            l if l.starts_with("Tar.zst (Dir)") => { unpack_tar_zst_dir(comp_path, temp_dir.path())?; },
            _ => return Err(ZromError::Zstd("Unknown directory benchmark mode".to_string())),
        }
        let decompression_time = start.elapsed().as_secs_f64();

        all_results.push(BenchmarkResult {
            label: label.to_string(),
            compression_time,
            decompression_time,
            input_bytes: stats.input_bytes,
            output_bytes: stats.output_bytes,
        });
    }

    BenchmarkResult::save_benchmark(&all_results)?;
    Ok(())
}
