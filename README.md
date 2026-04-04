# zrom — deterministic zstd-compressed ROMs

A Rust CLI tool and library for compressing retro ROM files into a deterministic zstd-compressed format.

## Why

Many retro emulators can run compressed ROMs by decompressing archives (e.g. .zip) into memory.
However, archive formats cannot guarantee that identical ROM content produces identical file hashes — the compression algorithm, ROM file name, and modified date all affect the output hash.

zrom solves this by directly compressing the ROM file content with zstd using strict, fixed parameters. The result is:

- **Deterministic** — identical input always produces identical output
- **File name independent** — two identical inputs with different names will generate the same output and hash
- **Fast lossless compression** — uses the Zstandard (zstd) algorithm producing a single zstd frame

## Format

A compressed ROM file is a single zstd frame with `.zst` appended to the original extension:

| Extension | Compressed | Console |
| --- | --- | --- |
| `.gb` | `.gb.zst` | Game Boy |
| `.gbc` | `.gbc.zst` | Game Boy Color |
| `.gba` | `.gba.zst` | Game Boy Advance |
| `.nes` | `.nes.zst` | NES / Famicom |
| `.sfc` / `.smc` | `.sfc.zst` / `.smc.zst` | SNES / Super Famicom |
| `.z64` / `.v64` / `.n64` | `.z64.zst` / `.v64.zst` / `.n64.zst` | Nintendo 64 |
| `.nds` | `.nds.zst` | Nintendo DS |
| `.sms` | `.sms.zst` | Sega Master System |
| `.gg` | `.gg.zst` | Game Gear |
| `.md` / `.gen` | `.md.zst` / `.gen.zst` | Mega Drive / Genesis |

### Required zstd parameters

| Parameter | Value | Effect |
| --- | --- | --- |
| Compression level | 19 | Affects compressed bytes |
| Content size | enabled | Adds 8 bytes to frame header |
| Checksum | enabled | Appends 4-byte xxHash to frame footer |
| Single frame | enforced | No concatenated frames |

All four are set explicitly in library usage. Any deviation produces a different output hash.

## Installation

```bash
cargo install --path .
```

## Usage

### Compress a ROM

```bash
zrom pack "Game.ext"
# Output: "Game.ext.zst"
```

### Compress a directory tree

```bash
zrom pack ./roms/system/
```

### Decompress

```bash
zrom unpack "Game.ext.zst"
# Output: "Game.ext"
```

### Verify integrity

```bash
zrom verify "Game.ext.zst"
# Output: Game.ext.zst  OK  (1.2 MB compressed)
```

### Options

| Flag | Description |
| --- | --- |
| `-f`, `--force` | Overwrite existing output files |
| `--keep` | Keep source file after pack/unpack (default: remove) |
| `--dry-run` | Print what would happen, no writes |
| `-q`, `--quiet` | Suppress output except errors |

## Library

Add to your `Cargo.toml`:

```toml
[dependencies]
zrom = { path = "path/to/zrom" }
```

```rust
use zrom::{pack, unpack, ZromError};
use std::path::Path;

// Compress
let stats = pack(Path::new("game.ext"), Path::new("game.ext.zst"))?;
println!("Ratio: {:.2}x", stats.ratio());

// Decompress
let stats = unpack(Path::new("game.ext.zst"), Path::new("game.ext"))?;
```

## Blocked Extensions

- **Uncompressed 3DS Formats** (`.3ds` / `.cci`, `.cia`, `.cxi`, `.3dsx`)) — Compress with Azahar (Z3DS format)
- **Compressed 3DS Formats** (`.z3ds` / `.zcci`, `.zcia`, `.zcxi`, `.z3dsx`)) — Already compressed (Z3DS format)
- **Uncompressed disc images** (`.iso`, `.bin`, `.img`, `.chd`) — Compress with CHDMan (CHD format)
- **Compressed disc images** (`.chd`) — Already compressed (CHD format)
- **Archives** (`.zip`, `.7z`, etc) — Not a ROM, will be extracted

## Conformance Checklist

A conformant `zrom` file must satisfy:

- Uses the extension `.{ext}.zst` where {ext} is a known ROM extension
- First 4 bytes are `0x28 0xB5 0x2F 0xFD` (zstd magic number)
- Compressed with level 19
- Frame header Content Size Field is present and correct
- Frame footer xxHash Checksum is present and valid
- Has one zstd frame
- File mtime is the console's worldwide release date at midnight UTC as listed in [extensions.rs](src/extensions.rs)

Use `zrom verify` to check for conformancy.
