use std::path::Path;

/// Extension that can be zrom-compressed
#[derive(Debug, Clone, Copy)]
pub struct ROMExtension {
    pub extension: &'static str,
    pub name: &'static str,
    pub release_date: (i32, u32, u32),
}

/// Extensions that are allowed to be zrom-compressed
pub static ROM_EXTENSIONS: &[ROMExtension] = &[
    // Nintendo
    ROMExtension { extension: "gb",  name: "Nintendo Game Boy",
        release_date: (1989, 4, 21) },
    ROMExtension { extension: "gbc", name: "Nintendo Game Boy Color",
        release_date: (1998, 10, 21) },
    ROMExtension { extension: "gba", name: "Nintendo Game Boy Advance",
        release_date: (2001, 3, 21) },
    ROMExtension { extension: "nds", name: "Nintendo DS",
        release_date: (2004, 11, 21) },
    ROMExtension { extension: "nes", name: "Nintendo Entertainment System",
        release_date: (1983, 7, 15) },
    ROMExtension { extension: "sfc", name: "Super Nintendo Entertainment System",
        release_date: (1990, 11, 21) },
    ROMExtension { extension: "smc", name: "Super Nintendo Entertainment System",
        release_date: (1990, 11, 21) },
    ROMExtension { extension: "z64", name: "Nintendo 64",
        release_date: (1996, 6, 23) },
    ROMExtension { extension: "n64", name: "Nintendo 64",
        release_date: (1996, 6, 23) },
    ROMExtension { extension: "v64", name: "Nintendo 64",
        release_date: (1996, 6, 23) },
    // Sega
    ROMExtension { extension: "sms", name: "Sega Master System",
        release_date: (1985, 10, 20) },
    ROMExtension { extension: "gg",  name: "Sega Game Gear",
        release_date: (1990, 10, 6) },
    ROMExtension { extension: "md",  name: "Sega Mega Drive",
        release_date: (1988, 10, 29) },
    ROMExtension { extension: "gen", name: "Sega Mega Drive",
        release_date: (1988, 10, 29) },
];

/// Extension that should not be zrom-compressed
#[derive(Debug, Clone, Copy)]
pub struct BlockedExtension{
    pub extension: &'static str,
    pub reason: &'static str,
}

/// Extensions that are blocked from zrom-compressionwith reason
pub static BLOCKED_EXTENSIONS: &[BlockedExtension] = &[
    // 3DS
    BlockedExtension{ extension: "3ds",
        reason: "use Azahar to compress (Z3DS format)" },
    BlockedExtension{ extension: "cci",
        reason: "use Azahar to compress (Z3DS format)" },
    BlockedExtension{ extension: "cia",
        reason: "use Azahar to compress (Z3DS format)" },
    BlockedExtension{ extension: "cxi",
        reason: "use Azahar to compress (Z3DS format)" },
    BlockedExtension{ extension: "3dsx",
        reason: "use Azahar to compress (Z3DS format)" },
    BlockedExtension{ extension: "z3ds",
        reason: "already compressed (Z3DS format)" },
    BlockedExtension{ extension: "zcci",
        reason: "already compressed (Z3DS format)" },
    BlockedExtension{ extension: "zcia",
        reason: "already compressed (Z3DS format)" },
    BlockedExtension{ extension: "zcxi",
        reason: "already compressed (Z3DS format)" },
    // Disk images
    BlockedExtension{ extension: "iso",
        reason: "use chdman to compress (CHD format)" },
    BlockedExtension{ extension: "cue",
        reason: "use chdman to compress (CHD format)" },
    BlockedExtension{ extension: "bin",
        reason: "use chdman to compress (CHD format)" },
    BlockedExtension{ extension: "img",
        reason: "use chdman to compress (CHD format)" },
    BlockedExtension{ extension: "chd",
        reason: "already compressed (CHD format)" },
];

/// Array of allowed ROM extensions
pub fn get_allowed_rom_ext() -> Vec<&'static str> {
    ROM_EXTENSIONS.iter().map(|rom| rom.extension).collect()
}

/// Array of blocked ROM extensions
pub fn get_blocked_rom_ext() -> Vec<&'static str> {
    BLOCKED_EXTENSIONS.iter().map(|rom| rom.extension).collect()
}

/// Allowed archive extensions
pub static ARCHIVE_EXTENSIONS: &[&str] = &["zip", "7z", "rar"];

/// Returns the ROM extension data for a ROM_Extension if the file matches
/// *.{ROM_Extension}.zst
pub fn get_rom_ext_data(filepath: &Path) -> Option<&'static ROMExtension> {
    let ext = get_rom_ext(filepath)?;   
    // Check if this extension is a valid ROM extension
    ROM_EXTENSIONS.iter().find(|rom| rom.extension == ext)
}
/// Returns the blocked extension data for a Blocked_Extension if the file matches
/// *.{Blocked_Extension}
pub fn get_blocked_ext_data(filepath: &Path) -> Option<&'static BlockedExtension> {
    let ext = get_rom_ext(filepath)?;   
    // Check if this extension is a valid ROM extension
    BLOCKED_EXTENSIONS.iter().find(|rom| rom.extension == ext)
}

/// Get the extension of a file path -> *.{ext}
fn get_rom_ext(filepath: &Path) -> Option<&str> {
    let filename = filepath.file_name()?.to_str()?;
    let base_name = filename.strip_suffix(".zst")?;
    
    // Return the extension from the base name
    let dot_index = base_name.rfind('.')?;
    Some(&base_name[dot_index + 1..])
}

/// Returns true if the extension is an allowed archive
pub fn is_archive(filepath: &Path) -> Option<bool> {
    let fname = filepath.file_name()?.to_str()?;
    let dot_index = fname.rfind('.')?;
    let ext = &fname[dot_index + 1..];
    Some(ARCHIVE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Returns true if file matches *.{ROM_Extension}.zst
pub fn is_zrom(filepath: &Path) -> bool {
    get_rom_ext_data(filepath).is_some()
}