use std::path::{PathBuf};
use std::time::Instant;
use std::fs::File;
use std::io::Write;

use tracing::{info};

use crate::compression::pack;
use crate::decompression::unpack;
use crate::core::ZromError;

#[derive(Debug)]
pub struct BenchmarkResult {
    pub level: i32,
    pub compression_time: f64,
    pub decompression_time: f64,
    pub input_bytes: u64,
    pub output_bytes: u64,
}

impl BenchmarkResult {
    // Stats
    pub fn compression_ratio(&self) -> f64 {
        if self.input_bytes == 0 { 0.0 }
        else { self.output_bytes as f64 / self.input_bytes as f64 } 
    }

    pub fn compression_efficiency(&self) -> f64 {
        if self.compression_time == 0.0 { 0.0 }
        else { self.compression_ratio() / self.compression_time }
    }

    pub fn decompression_efficiency(&self) -> f64 {
        if self.decompression_time == 0.0 { 0.0 }
        else { self.compression_ratio() / self.decompression_time }
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
                result.level,
                result.input_bytes,
                result.output_bytes,
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
    let mut level_results = Vec::new();

    let zstd_levels = [1, 4, 9, 19];

    for level in zstd_levels {
        let mut comp_times = Vec::new();
        let mut decomp_times = Vec::new();
        let mut in_bytes = Vec::new();
        let mut out_bytes = Vec::new();

        for input in inputs {
            let temp_comp = tempfile::NamedTempFile::new().expect("Failed to create temp file");
            let temp_decomp = tempfile::NamedTempFile::new().expect("Failed to create temp file");

            info!("ZSTD Level {}: {}", level, input.display());

            // Compression
            let start = Instant::now();
            let stats = pack(input, temp_comp.path(), level, true, true)?;
            comp_times.push(start.elapsed().as_secs_f64());
            in_bytes.push(stats.input_bytes);
            out_bytes.push(stats.output_bytes);

            // Decompression
            let start = Instant::now();
            unpack(temp_comp.path(), temp_decomp.path())?;
            decomp_times.push(start.elapsed().as_secs_f64());
        }

        level_results.push(BenchmarkResult {
            level,
            compression_time: comp_times.iter().sum::<f64>(),
            decompression_time: decomp_times.iter().sum::<f64>(),
            input_bytes: in_bytes.iter().sum::<u64>(),
            output_bytes: out_bytes.iter().sum::<u64>(),
        });
    }

    BenchmarkResult::save_benchmark(&level_results)?;
    Ok(())
}
