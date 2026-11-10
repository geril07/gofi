//! Icon name → file path resolution.
//!
//! Resolution never fails a row: a missing icon is [`None`]. The quick path
//! probes the sizes a 32 px row actually uses; only a miss walks every directory
//! in the theme chain, which is what finds the legacy 16 px icons rofi shows.
//!
//! Themes follow GTK's lookup: the `gtk-icon-theme-name` from the user's GTK
//! settings, then that theme's `Inherits` chain, then the stock fallbacks, with
//! `hicolor` last because the spec makes it the required fallback.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::xdg::Env;

/// Sizes every theme is asked for first, smallest usable first.
const QUICK_SIZES: [&str; 7] = [
    "32x32", "48x48", "64x64", "128x128", "256x256", "512x512", "scalable",
];
/// Remaining sizes, for the full walk over a theme that has the icon nowhere
/// else. 16 px icons are upscaled; rofi does the same.
const EXTRA_SIZES: [&str; 5] = ["96x96", "192x192", "384x384", "24x24", "22x22"];
const SMALL_SIZES: [&str; 2] = ["16x16", "symbolic"];
const EXTENSIONS: [&str; 2] = ["png", "svg"];

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

    let dirs = env.icons_dirs();

    // Simple layouts: `<dir>/32x32/name.png` and `<dir>/name.png` (pixmaps).
    for dir in &dirs {
        for size in QUICK_SIZES {
            if let Some(found) = probe(&dir.join(size), name) {
                return Some(found);
            }
        }
        if let Some(found) = probe(dir, name) {
            return Some(found);
        }
    }

    for theme in theme_chain(env) {
        for dir in &dirs {
            let theme_dir = dir.join(&theme);
            if !theme_dir.is_dir() {
                continue;
            }
            if let Some(found) = find_in_theme(&theme_dir, name, false) {
                return Some(found);
            }
            if let Some(found) = find_in_theme(&theme_dir, name, true) {
                return Some(found);
            }
        }
    }

    None
}

fn probe(dir: &Path, name: &str) -> Option<PathBuf> {
    for extension in EXTENSIONS {
        let candidate = dir.join(format!("{name}.{extension}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_in_theme(theme_dir: &Path, name: &str, full: bool) -> Option<PathBuf> {
    if !full {
        for size in QUICK_SIZES {
            if let Some(found) = probe(&theme_dir.join(size).join("apps"), name) {
                return Some(found);
            }
        }
        return None;
    }

    for relative in theme_directories(theme_dir) {
        if let Some(found) = probe(&theme_dir.join(relative), name) {
            return Some(found);
        }
    }
    None
}

/// Every icon directory a theme declares, best size for a 32 px row first.
fn theme_directories(theme_dir: &Path) -> Vec<String> {
    let mut guard = cache();
    if let Some(directories) = guard.theme_dirs.get(theme_dir) {
        return directories.clone();
    }
    let directories = build_theme_directories(theme_dir);
    guard
        .theme_dirs
        .insert(theme_dir.to_path_buf(), directories.clone());
    directories
}

fn build_theme_directories(theme_dir: &Path) -> Vec<String> {
    let mut directories = index_value(theme_dir, "Directories").unwrap_or_default();
    directories.retain(|entry| !entry.is_empty());
    if directories.is_empty() {
        for size in QUICK_SIZES
            .into_iter()
            .chain(EXTRA_SIZES)
            .chain(SMALL_SIZES)
        {
            for category in ["apps", "legacy", ""] {
                directories.push(if category.is_empty() {
                    size.to_string()
                } else {
                    format!("{size}/{category}")
                });
            }
        }
    }

    directories.sort_by_key(|entry| directory_rank(entry));
    directories.dedup();
    directories
}

fn directory_rank(entry: &str) -> (u8, u8, String) {
    let mut parts = entry.split('/');
    let size = parts.next().unwrap_or_default();
    let category = parts.next().unwrap_or_default();
    let size_rank = match size {
        "32x32" => 0,
        "48x48" => 1,
        "64x64" => 2,
        "96x96" => 3,
        "128x128" => 4,
        "192x192" => 5,
        "256x256" => 6,
        "384x384" => 7,
        "512x512" => 8,
        "24x24" => 9,
        "22x22" => 10,
        "16x16" => 11,
        "scalable" => 12,
        "symbolic" => 13,
        _ => 20,
    };
    let category_rank = match category {
        "apps" => 0,
        "legacy" => 1,
        _ => 2,
    };
    (size_rank, category_rank, entry.to_string())
}

#[derive(Default)]
struct Cache {
    /// Theme chain keyed by the GTK config dir and the icon search path.
    themes: HashMap<(PathBuf, Vec<PathBuf>), Vec<String>>,
    /// `Directories=` per theme directory.
    theme_dirs: HashMap<PathBuf, Vec<String>>,
}

fn cache() -> MutexGuard<'static, Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE
        .get_or_init(|| Mutex::new(Cache::default()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn theme_chain(env: &Env) -> Vec<String> {
    let key = (env.config_home.clone(), env.icons_dirs());
    let guard = cache();
    if let Some(chain) = guard.themes.get(&key) {
        return chain.clone();
    }
    drop(guard);
    let chain = build_theme_chain(env);
    cache().themes.insert(key, chain.clone());
    chain
}

fn build_theme_chain(env: &Env) -> Vec<String> {
    let mut chain = Vec::new();
    push_theme(&mut chain, &configured_theme(env), env);
    // Keep the stock fallbacks even when a user theme names no parents.
    for fallback in ["Adwaita", "AdwaitaLegacy", "hicolor"] {
        push_theme(&mut chain, fallback, env);
    }
    chain
}

fn push_theme(chain: &mut Vec<String>, theme: &str, env: &Env) {
    let theme = theme.trim();
    if theme.is_empty() || chain.iter().any(|known| known.eq_ignore_ascii_case(theme)) {
        return;
    }
    chain.push(theme.to_string());
    if let Some(root) = theme_root(theme, env) {
        for parent in index_value(&root, "Inherits").unwrap_or_default() {
            push_theme(chain, &parent, env);
        }
    }
}

fn configured_theme(env: &Env) -> String {
    const SETTINGS: [&str; 2] = ["gtk-3.0/settings.ini", "gtk-4.0/settings.ini"];
    for settings in SETTINGS {
        let path = env.config_home.join(settings);
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if key.trim() == "gtk-icon-theme-name" && !value.trim().is_empty() {
                return value.trim().to_string();
            }
        }
    }
    "Adwaita".to_string()
}

/// The first theme directory found in the icon search path.
fn theme_root(theme: &str, env: &Env) -> Option<PathBuf> {
    env.icons_dirs()
        .into_iter()
        .map(|dir| dir.join(theme))
        .find(|dir| dir.is_dir())
}

fn index_value(theme_dir: &Path, key: &str) -> Option<Vec<String>> {
    let text = fs::read_to_string(theme_dir.join("index.theme")).ok()?;
    let mut values = Vec::new();
    for line in text.lines() {
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() != key {
            continue;
        }
        values.extend(
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_string),
        );
    }
    (!values.is_empty()).then_some(values)
}
