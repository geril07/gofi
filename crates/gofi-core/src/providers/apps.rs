//! XDG `.desktop` application discovery.
//!
//! Files are discovered in `Env::applications_dirs()` precedence order, parsed
//! without ever executing a shell, and deduplicated by desktop file ID. The
//! first directory wins, so a user `Hidden=true` entry also suppresses the
//! system entry with the same ID.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::icons::Icons;
use crate::model::{Action, Item, ItemId, ItemKind, Loaded, Provider, ProviderError};
use crate::xdg::{Env, expand_home};

pub struct AppsProvider {
    pub env: Env,
}

impl AppsProvider {
    pub fn new(env: Env) -> Self {
        Self { env }
    }
}

impl Provider for AppsProvider {
    fn load(&self) -> Result<Loaded, ProviderError> {
        let mut loaded = Loaded::default();
        let mut seen: HashSet<String> = HashSet::new();
        // One index for the whole scan: a per-name walk costs a hundred times
        // more on a real icon theme.
        let icons = Icons::new(&self.env);

        for dir in self.env.applications_dirs() {
            if !dir.is_dir() {
                continue;
            }

            let mut files = Vec::new();
            collect_desktop_files(&dir, &mut files, &mut loaded.warnings);

            for path in files {
                let Ok(relative) = path.strip_prefix(&dir) else {
                    continue;
                };
                let id = desktop_file_id(relative);
                if !seen.insert(id.clone()) {
                    continue;
                }
                match parse_desktop_file(&path, &id, &self.env, &icons, &mut loaded.warnings) {
                    Ok(Some(item)) => loaded.items.push(item),
                    Ok(None) => {}
                    Err(warning) => loaded.warnings.push(warning),
                }
            }
        }

        Ok(loaded)
    }
}

/// Recursively collect `.desktop` files, sorted so discovery is deterministic.
fn collect_desktop_files(dir: &Path, out: &mut Vec<PathBuf>, warnings: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(format!("applications directory {}: {error}", dir.display()));
            return;
        }
    };

    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(error) => {
                warnings.push(format!("applications directory {}: {error}", dir.display()))
            }
        }
    }
    paths.sort();

    for path in paths {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => collect_desktop_files(&path, out, warnings),
            Ok(_) if path.extension().is_some_and(|ext| ext == "desktop") => out.push(path),
            _ => {}
        }
    }
}

/// Desktop file ID: the path below the applications directory with `/` → `-`.
fn desktop_file_id(relative: &Path) -> String {
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("-")
}

fn parse_desktop_file(
    path: &Path,
    id: &str,
    env: &Env,
    icons: &Icons,
    warnings: &mut Vec<String>,
) -> Result<Option<Item>, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let entry = parse_entry(&String::from_utf8_lossy(&bytes));

    if entry.get("Type").map(String::as_str) != Some("Application") {
        return Ok(None);
    }
    if is_true(&entry, "Hidden") || is_true(&entry, "NoDisplay") {
        return Ok(None);
    }
    if !env.desktop_matches(
        entry.get("OnlyShowIn").map(String::as_str),
        entry.get("NotShowIn").map(String::as_str),
    ) {
        return Ok(None);
    }
    if let Some(try_exec) = entry.get("TryExec").filter(|value| !value.is_empty())
        && !try_exec_exists(try_exec, env)
    {
        return Ok(None);
    }

    let Some(name) = localized(&entry, "Name", env) else {
        return Err(format!("{}: missing Name", path.display()));
    };
    let Some(exec) = entry.get("Exec").filter(|value| !value.is_empty()) else {
        return Err(format!("{}: missing Exec", path.display()));
    };

    let icon_name = entry
        .get("Icon")
        .map(String::as_str)
        .filter(|value| !value.is_empty());

    let mut argv = build_argv(exec, &name, icon_name, path, warnings)?;

    if is_true(&entry, "Terminal") {
        let terminal = env.terminal();
        let mut prefix: Vec<OsString> = shell_words::split(terminal)
            .unwrap_or_else(|_| vec![terminal.to_string()])
            .into_iter()
            .map(OsString::from)
            .collect();
        prefix.append(&mut argv);
        argv = prefix;
    }

    let cwd = entry
        .get("Path")
        .filter(|value| !value.is_empty())
        .map(|value| expand_home(value, env))
        .filter(|path| path.is_dir());

    let generic = localized(&entry, "GenericName", env);
    let comment = localized(&entry, "Comment", env);
    let keywords = localized(&entry, "Keywords", env).map(|value| {
        value
            .split(';')
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    });

    let search_text = [Some(name.as_str()), generic.as_deref(), keywords.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");

    let icon = icon_name.and_then(|icon_name| icons.resolve(icon_name));

    Ok(Some(Item {
        id: ItemId(format!("apps:{id}")),
        title: name,
        // The generic name is the intended subtitle; Comment is the fallback for
        // entries that do not provide one.
        subtitle: generic.or(comment),
        icon,
        search_text,
        kind: ItemKind::App,
        action: Action::Launch { argv, cwd },
    }))
}

/// Parse the `[Desktop Entry]` group. Other groups (actions, desktop action
/// sections) are ignored.
fn parse_entry(text: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    let mut in_desktop_entry = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    values
}

fn localized(values: &HashMap<String, String>, key: &str, env: &Env) -> Option<String> {
    for candidate in env.locale_candidates() {
        if let Some(value) = values.get(&format!("{key}[{candidate}]"))
            && !value.is_empty()
        {
            return Some(value.clone());
        }
    }
    values.get(key).filter(|value| !value.is_empty()).cloned()
}

fn is_true(values: &HashMap<String, String>, key: &str) -> bool {
    values
        .get(key)
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

/// `TryExec` names a binary that must exist; otherwise the entry is not shown.
fn try_exec_exists(try_exec: &str, env: &Env) -> bool {
    let is_executable = |path: &Path| {
        fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    };

    if try_exec.contains('/') {
        return is_executable(&expand_home(try_exec, env));
    }

    env.path
        .iter()
        .any(|dir| is_executable(&dir.join(try_exec)))
}

/// Build argv from `Exec` without a shell, expanding desktop entry field codes.
fn build_argv(
    exec: &str,
    name: &str,
    icon: Option<&str>,
    desktop_path: &Path,
    warnings: &mut Vec<String>,
) -> Result<Vec<OsString>, String> {
    let tokens = shell_words::split(exec)
        .map_err(|error| format!("{}: could not parse Exec: {error}", desktop_path.display()))?;

    let mut argv = Vec::new();
    for token in tokens {
        for argument in expand_token(&token, name, icon, desktop_path, warnings) {
            argv.push(OsString::from(argument));
        }
    }

    if argv.is_empty() {
        return Err(format!(
            "{}: Exec produced no arguments",
            desktop_path.display()
        ));
    }
    Ok(argv)
}

fn expand_token(
    token: &str,
    name: &str,
    icon: Option<&str>,
    desktop_path: &Path,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    // A quoted empty argument is a real argument; a token that only holds field
    // codes disappears when those codes expand to nothing.
    if token.is_empty() {
        return vec![String::new()];
    }

    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut chars = token.chars();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            current.push(ch);
            continue;
        }

        let Some(code) = chars.next() else {
            warnings.push(format!(
                "unknown field code % at end of Exec in {}",
                desktop_path.display()
            ));
            break;
        };

        match code {
            // No file arguments are ever passed, so file/URL codes vanish.
            'f' | 'F' | 'u' | 'U' => {}
            // Deprecated codes are removed per the desktop entry spec.
            'd' | 'D' | 'n' | 'N' | 'v' | 'm' => {}
            'i' => {
                if let Some(icon) = icon {
                    if !current.is_empty() {
                        arguments.push(std::mem::take(&mut current));
                    }
                    arguments.push("--icon".to_string());
                    arguments.push(icon.to_string());
                }
            }
            'c' => current.push_str(name),
            'k' => current.push_str(&desktop_path.to_string_lossy()),
            '%' => current.push('%'),
            other => warnings.push(format!(
                "unknown field code %{other} in {}",
                desktop_path.display()
            )),
        }
    }

    if !current.is_empty() {
        arguments.push(current);
    }
    arguments
}
