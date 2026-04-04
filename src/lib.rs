pub mod compression;
pub mod decompression;
pub mod validation;
pub mod core;
pub mod extensions;
pub mod benchmark;

pub use compression::{pack, compressed_path, Stats};
pub use decompression::{unpack, decompressed_path};
pub use validation::{validate_zroms};
pub use extensions::{is_zrom, is_archive};
pub use core::{ZromError, set_date, date_to_unix_secs};
pub use benchmark::{benchmark_files, BenchmarkResult};
