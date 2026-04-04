use std::path::{Path, PathBuf};
use clap::{Parser, Subcommand};
use rayon::{prelude::*};
use walkdir::WalkDir;
use globset::{Glob, GlobSetBuilder};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tracing::{error, info, warn};

use zrom::benchmark::{benchmark_files};
use zrom::compression::{zrom_pack, compressed_path};
use zrom::decompression::{unpack, decompressed_path, extract_archive};
use zrom::extensions::{get_allowed_rom_ext, get_rom_ext_data, is_archive};
use zrom::core::set_date;
use zrom::{ZromError};

#[derive(Parser)]
#[command(name = "zrom", about = "zstd-compressed ROMs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Overwrite existing files
    #[arg(short, long)]
    force: bool,

    /// Remove source files
    #[arg(short, long)]
    remove: bool,

    /// Print what would happen without changes
    #[arg(long)]
    dry_run: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Compress ROM(s) to zrom format
    Pack {
        /// File or directory to compress
        inputs: Vec<PathBuf>,
    },
    /// Decompress zrom format ROM(s)
    Unpack {
        /// File or directory to decompress
        inputs: Vec<PathBuf>,
    },
    /// Validate zrom formatting without decompressing
    Validate {
        /// File or directory to validate
        inputs: Vec<PathBuf>,
    },
    /// Benchmark compression and decompression performance
    Benchmark {
        /// File or directory to benchmark
        inputs: Vec<PathBuf>,
    },
}

struct JobResult {
    input: PathBuf,
    output: Option<PathBuf>,
    error: Option<ZromError>,
    stats: Option<(u64, u64)>,
    is_dry_run: bool,
}

fn scan_directory(path: &Path, extensions: &[&str]) -> Vec<PathBuf> {
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

fn run_pack(cli: &Cli, inputs: &[PathBuf]) {
    let known_exts= get_allowed_rom_ext();
    let files: Vec<PathBuf> = inputs.iter()
        .flat_map(|p| scan_directory(p, &known_exts))
        .collect();

    if files.is_empty() {
        error!("No supported ROM files found. Supported: {:?}", known_exts);
        return;
    }
    let results: Vec<_> = files.par_iter().flat_map(|input| {
        let roms = if is_archive(&input).expect("Failed to determine if file is archive.") {
            let temp_dir = match tempfile::tempdir() {
                Ok(d) => d,
                Err(e) => return vec![JobResult {
                    input: input.to_path_buf(), output: None,
                    error: Some(ZromError::Io(e.to_string())), stats: None, is_dry_run: false
                }],
            };

            match extract_archive(input, temp_dir.path()) {
                Ok(f) => f,
                Err(e) => return vec![JobResult {
                    input: input.to_path_buf(), output: None,
                    error: Some(e), stats: None, is_dry_run: false
                }],
            }
        } else { vec![input.clone()] };

        roms.iter().map(|r| {
            let rom = r.clone();
            let Some(data) = get_rom_ext_data(r) else {
                return JobResult { input: rom, output: None, error: Some(ZromError::NoExtension), stats: None, is_dry_run: false };
            };

            let out_path = compressed_path(r);

            if cli.dry_run {
                return JobResult { input: rom, output: Some(out_path), error: None, stats: None, is_dry_run: true };
            }
            
            if !cli.force && out_path.exists() {
                return JobResult { input: rom, output: Some(out_path.clone()), error: Some(ZromError::OutputExists(out_path)), stats: None, is_dry_run: false };
            }

            match zrom_pack(&r, &out_path) {
                Ok(stats) => {
                    if cli.remove {
                        if let Err(e) = std::fs::remove_file(r) {
                            return JobResult { input: rom, output: Some(out_path), error: Some(ZromError::Io(e.to_string())), stats: None, is_dry_run: false };
                        }
                    }

                    let (name, date) = (data.name, data.release_date);
                    if let Err(e) = set_date(&out_path, date) {
                        ZromError::Io(e.to_string());
                    }

                    info!("Packed: {} [{}]", out_path.display(), name);
                    JobResult { input: rom, output: Some(out_path), error: None, stats: Some((stats.input_bytes, stats.output_bytes)), is_dry_run: false }
                }
                Err(e) => JobResult { input: rom, output: Some(out_path), error: Some(e), stats: None, is_dry_run: false },
            }
        }).collect()
    }).collect();
    print_results(results);
}

fn run_unpack(cli: &Cli, inputs: &[PathBuf]) {
let files: Vec<PathBuf> = inputs.iter()
        .flat_map(|p| scan_directory(p, &["zst"]))
        .collect();

    if files.is_empty() {
        error!("No files found");
        return;
    }

    let results: Vec<_> = files.par_iter().map(|input| {
        let output = decompressed_path(input);

        if !cli.force && output.exists() {
            return JobResult { input: input.clone(), output: Some(output.clone()), error: Some(ZromError::OutputExists(output)), stats: None, is_dry_run: false };
        }

        if cli.dry_run {
            return JobResult { input: input.clone(), output: Some(output), error: None, stats: None, is_dry_run: true };
        }

        match unpack(input, &output) {
            Ok(stats) => {
                if cli.remove { let _ = std::fs::remove_file(input); }
                JobResult { input: input.clone(), output: Some(output), error: None, stats: Some((stats.input_bytes, stats.output_bytes)), is_dry_run: false }
            }
            Err(e) => JobResult { input: input.clone(), output: Some(output), error: Some(e), stats: None, is_dry_run: false },
        }
    }).collect();

    print_results(results);
}

fn run_validate(inputs: &[PathBuf]) {
    let files: Vec<PathBuf> = inputs.iter()
        .flat_map(|p| scan_directory(p, &["zst"]))
        .collect();

    if files.is_empty() {
        warn!("No files found");
        return;
    }

    let results = zrom::validation::validate_zroms(&files);
    zrom::validation::print_results(&results);
    if results.iter().any(|r| r.status.is_err()) {
        error!("Validation completed with errors.");
    } else {
        info!("All files validated.");
    }
}

fn run_benchmark(inputs: &[PathBuf]) {
    let exts: Vec<&str> = get_allowed_rom_ext();
    let files: Vec<PathBuf> = inputs.iter()
        .flat_map(|p| scan_directory(p, &exts))
        .collect();

    if files.is_empty() {
        error!("No supported ROM files found");
        return;
    }

    match benchmark_files(&files) {
        Ok(_) => { info!("Benchmark completed successfully") }
        Err(e) => { error!("Benchmark failed: {}", e) }
    }
}

fn print_results(results: Vec<JobResult>) {
    let mut success_count = 0;
    let mut error_count = 0;

    for r in results {
        if let Some(e) = r.error {
            error!("  {}: {}", r.input.display(), e);
            error_count += 1;
            continue;
        }

        if r.is_dry_run {
            if let Some(out) = r.output {
                info!("(dry-run) {} → {}", r.input.display(), out.display());
            }
            continue;
        }

        if let Some((in_sz, out_sz)) = r.stats {
            success_count += 1;
            let ratio = if in_sz > 0 { out_sz as f64 / in_sz as f64 } else { 0.0 };
            println!(
                "{} → {}  {} → {} ({:.2}x)",
                r.input.display(),
                r.output.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                bytesize::ByteSize::b(in_sz),
                bytesize::ByteSize::b(out_sz),
                ratio
            );
        }
    }

    info!("Summary: {} successful, {} failed", success_count, error_count);
}

fn init_logging() -> tracing_appender::non_blocking::WorkerGuard {
    let file_appender = tracing_appender::rolling::daily("logs", "zrom.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking);

    let terminal_layer = fmt::layer()
        .with_ansi(true);

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(file_layer)
        .with(terminal_layer)
        .init();
    
    guard
}

fn main() {
    let _guard = init_logging();
    let cli = Cli::parse();
    
    match &cli.command {
        Commands::Pack { inputs } => run_pack(&cli, inputs),
        Commands::Unpack { inputs } => run_unpack(&cli, inputs),
        Commands::Validate { inputs } => run_validate(inputs),
        Commands::Benchmark { inputs} => run_benchmark(inputs),
    }
}
