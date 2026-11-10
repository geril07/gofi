//! Icon name → file path resolution.
//!
//! Resolution never fails a row: a missing icon is [`None`]. Sizes are tried
//! smallest first so a 32 px row does not pay for a 128 px file.

use std::path::{Path, PathBuf};

use crate::xdg::Env;

/// Sizes in the order they are probed, matching the icon theme directory names.
/// `scalable` comes last: SVG rasterizes crisply but costs more than a PNG, and
/// many real themes ship only 256 px icons for large app logos.
const SIZES: [&str; 7] = [
    "32x32", "48x48", "64x64", "128x128", "256x256", "512x512", "scalable",
];
const EXTENSIONS: [&str; 2] = ["png", "svg"];
/// Themes present on a stock freedesktop system. `hicolor` is the required
/// fallback theme; Adwaita ships with GTK and covers most applications.
const THEMES: [&str; 2] = ["hicolor", "Adwaita"];

pub fn resolve_icon(name: &str, env: &Env) -> Option<PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }

    let path = Path::new(name);
    if path.is_absolute() {
        return path.is_file().then(|| path.to_path_buf());
    }

    // A relative path is not a valid icon name; the spec only allows an absolute
    // path or a name looked up in the theme.
    if name.contains('/') {
        return None;
    }

    env.icons_dirs()
        .iter()
        .find_map(|dir| find_in_dir(dir, name))
}

fn find_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    for size in SIZES {
        for extension in EXTENSIONS {
            let candidate = dir.join(size).join(format!("{name}.{extension}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    for extension in EXTENSIONS {
        let candidate = dir.join(format!("{name}.{extension}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // Real themes nest icons one level deeper, under a category directory.
    for theme in THEMES {
        for size in SIZES {
            for category in ["apps", ""] {
                for extension in EXTENSIONS {
                    let candidate = dir
                        .join(theme)
                        .join(size)
                        .join(category)
                        .join(format!("{name}.{extension}"));
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    None
}
