use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone, Copy)]
pub enum CompressionFormat { Zstd, Xz, Zip, SevenZip, TarZst }

#[derive(Debug, Clone, Copy)]
pub enum BenchmarkMode { Single, Directory }

pub struct BenchmarkTask {
    pub format: CompressionFormat,
    pub mode: BenchmarkMode,
    pub level: i32,
}

impl BenchmarkTask {
    pub fn label(&self) -> String {
        let mode_str = match self.mode {
            BenchmarkMode::Single => "",
            BenchmarkMode::Directory => " (Dir)",
        };
        format!("{:?}{} (L{})", self.format, mode_str, self.level)
    }

    fn compress(&self, inputs: &[PathBuf], output: &Path) -> Result<Stats, ZromError> {
        match (self.format, self.mode) {
            (CompressionFormat::Zstd, BenchmarkMode::Single) => pack_zst(&inputs[0], output, self.level, true, true),
            (CompressionFormat::Xz, BenchmarkMode::Single)   => pack_xz(&inputs[0], output, self.level as u32),
            (CompressionFormat::Zip, _)                      => pack_zip_dir(inputs, output, self.level),
            (CompressionFormat::SevenZip, BenchmarkMode::Directory) => pack_7z_dir(inputs, output, self.level),
            (CompressionFormat::TarZst, BenchmarkMode::Directory)   => pack_tar_zst_dir(inputs, output, self.level),
            _ => Err(ZromError::InvalidFile(format!("Format {:?} does not support {:?} mode", self.format, self.mode))),
        }
    }

    fn decompress(&self, input: &Path, output: &Path) -> Result<(), ZromError> {
        match (self.format, self.mode) {
            (CompressionFormat::Zstd, BenchmarkMode::Single) => { unpack(input, output)?; Ok(()) },
            (CompressionFormat::Xz, BenchmarkMode::Single)   => { unpack_xz(input, output)?; Ok(()) },
            (CompressionFormat::Zip, _) => unpack_zip_dir(input, output),
            (CompressionFormat::SevenZip, BenchmarkMode::Directory) => { unpack_7z(input, output)?; Ok(()) },
            (CompressionFormat::TarZst, BenchmarkMode::Directory)   => unpack_tar_zst_dir(input, output),
            _ => unreachable!(),
        }
    }
}

pub struct BenchmarkSuite {
    pub tasks: Vec<BenchmarkTask>,
}

impl BenchmarkSuite {
    pub fn default_suite() -> Self {
        let mut tasks = Vec::new();
        tasks.push(BenchmarkTask { format: CompressionFormat::Zstd, mode: BenchmarkMode::Single, level: 1 });
        tasks.push(BenchmarkTask { format: CompressionFormat::Zstd, mode: BenchmarkMode::Single, level: 4 });
        tasks.push(BenchmarkTask { format: CompressionFormat::Zstd, mode: BenchmarkMode::Single, level: 9 });
        tasks.push(BenchmarkTask { format: CompressionFormat::Zstd, mode: BenchmarkMode::Single, level: 14 });
        tasks.push(BenchmarkTask { format: CompressionFormat::Zstd, mode: BenchmarkMode::Single, level: 17 });
        tasks.push(BenchmarkTask { format: CompressionFormat::Zstd, mode: BenchmarkMode::Single, level: 19 });

        tasks.push(BenchmarkTask { format: CompressionFormat::Xz, mode: BenchmarkMode::Single, level: 1 });
        tasks.push(BenchmarkTask { format: CompressionFormat::Xz, mode: BenchmarkMode::Single, level: 9 });
        tasks.push(BenchmarkTask { format: CompressionFormat::Zip, mode: BenchmarkMode::Single, level: 1 });
        tasks.push(BenchmarkTask { format: CompressionFormat::Zip, mode: BenchmarkMode::Single, level: 9 });
        
        tasks.push(BenchmarkTask { format: CompressionFormat::Zip, mode: BenchmarkMode::Directory, level: 9 });
        tasks.push(BenchmarkTask { format: CompressionFormat::SevenZip, mode: BenchmarkMode::Directory, level: 9 });
        tasks.push(BenchmarkTask { format: CompressionFormat::TarZst, mode: BenchmarkMode::Directory, level: 9 });
        tasks.push(BenchmarkTask { format: CompressionFormat::TarZst, mode: BenchmarkMode::Directory, level: 19 });
        
        Self { tasks }
    }

    pub fn run_benchmark(&self, inputs: &[PathBuf]) -> Result<Vec<BenchmarkResult>, ZromError> {
        let mut results = Vec::new();
        for task in &self.tasks {
            let result = match task.mode {
                BenchmarkMode::Single => self.benchmark_single(task, inputs)?,
                BenchmarkMode::Directory => self.benchmark_dir(task, inputs)?,
            };
            results.push(result);
        }
        Ok(results)
    }

    fn benchmark_single(&self, task: &BenchmarkTask, inputs: &[PathBuf]) -> Result<BenchmarkResult, ZromError> {
        let mut comp_times = 0.0;
        let mut decomp_times = 0.0;
        let mut total_in = 0;
        let mut total_out = 0;

        for input in inputs {
            let temp_comp = tempfile::NamedTempFile::new()?;
            let temp_decomp = tempfile::NamedTempFile::new()?;
            
            info!("Benchmarking {}: {}", task.label(), input.display());

            let start = Instant::now();
            let stats = task.compress(&[input.clone()], temp_comp.path())?;
            comp_times += start.elapsed().as_secs_f64();

            let start = Instant::now();
            task.decompress(temp_comp.path(), temp_decomp.path())?;
            decomp_times += start.elapsed().as_secs_f64();

            total_in += stats.input_bytes;
            total_out += stats.output_bytes;
        }

        Ok(BenchmarkResult {
            label: task.label(),
            compression_time: comp_times,
            decompression_time: decomp_times,
            input_bytes: total_in,
            output_bytes: total_out,
        })
    }

    fn benchmark_dir(&self, task: &BenchmarkTask, inputs: &[PathBuf]) -> Result<BenchmarkResult, ZromError> {
        let temp_comp = tempfile::NamedTempFile::new()?;
        let temp_dir = tempfile::tempdir()?;

        info!("Benchmarking {}: {} files", task.label(), inputs.len());

        let start = Instant::now();
        let stats = task.compress(inputs, temp_comp.path())?;
        let compression_time = start.elapsed().as_secs_f64();

        let start = Instant::now();
        task.decompress(temp_comp.path(), temp_dir.path())?;
        let decompression_time = start.elapsed().as_secs_f64();

        Ok(BenchmarkResult {
            label: task.label(),
            compression_time,
            decompression_time,
            input_bytes: stats.input_bytes,
            output_bytes: stats.output_bytes,
        })
    }
}

pub fn benchmark_files(inputs: &[PathBuf]) -> Result<Vec<BenchmarkResult>, ZromError> {
    let suite = BenchmarkSuite::default_suite();
    suite.run_benchmark(inputs)
}
