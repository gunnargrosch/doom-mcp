use std::path::PathBuf;

/// Search for a DOOM WAD file across standard locations.
///
/// Search order:
/// 1. DOOM_WAD_PATH environment variable
/// 2. Relative to executable: ../../wad/, ../wad/, wad/
/// 3. Relative to cwd: wad/
/// 4. System paths: ~/.local/share/doom, /usr/share/doom, etc.
pub fn find_wad() -> Option<String> {
    if let Ok(path) = std::env::var("DOOM_WAD_PATH") {
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    let search_dirs: Vec<PathBuf> = [
        exe_dir.as_ref().map(|d| d.join("../../wad")),
        exe_dir.as_ref().map(|d| d.join("../wad")),
        exe_dir.as_ref().map(|d| d.join("wad")),
        Some(PathBuf::from("wad")),
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".local/share/doom")),
        Some(PathBuf::from("/usr/share/doom")),
        Some(PathBuf::from("/usr/local/share/doom")),
        Some(PathBuf::from("/usr/share/games/doom")),
    ]
    .into_iter()
    .flatten()
    .collect();

    let wad_names = [
        "freedoom1.wad",
        "freedoom2.wad",
        "DOOM.WAD",
        "DOOM2.WAD",
        "doom.wad",
        "doom2.wad",
    ];

    for dir in &search_dirs {
        for name in &wad_names {
            let path = dir.join(name);
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }

    None
}
