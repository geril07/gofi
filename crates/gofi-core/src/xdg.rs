//! Environment and XDG paths, injectable so tests never touch the real home.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Env {
    pub home: PathBuf,
    pub data_home: PathBuf,
    pub data_dirs: Vec<PathBuf>,
    pub state_home: PathBuf,
    pub config_home: PathBuf,
    pub runtime_dir: Option<PathBuf>,
    pub path: Vec<PathBuf>,
    pub current_desktop: Option<String>,
    pub lang: Option<String>,
    pub terminal: Option<String>,
}

impl Env {
    pub fn from_process() -> Self {
        let home = env_path("HOME").unwrap_or_else(|| PathBuf::from("/"));
        let data_home = env_path("XDG_DATA_HOME")
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| home.join(".local/share"));
        let data_dirs = env_paths("XDG_DATA_DIRS")
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| {
                vec![
                    PathBuf::from("/usr/local/share"),
                    PathBuf::from("/usr/share"),
                ]
            });
        let state_home = env_path("XDG_STATE_HOME")
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| home.join(".local/state"));
        let config_home = env_path("XDG_CONFIG_HOME")
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| home.join(".config"));
        let runtime_dir = env_path("XDG_RUNTIME_DIR").filter(|p| !p.as_os_str().is_empty());
        let path = env_paths("PATH")
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| {
                vec![
                    PathBuf::from("/usr/local/bin"),
                    PathBuf::from("/usr/bin"),
                    PathBuf::from("/bin"),
                ]
            });
        let current_desktop = env_string("XDG_CURRENT_DESKTOP")
            .or_else(|| env_string("DESKTOP_SESSION"))
            .filter(|s| !s.is_empty());
        let lang = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|key| env_string(key))
            .filter(|s| !s.is_empty());
        let terminal = env_string("TERMINAL").filter(|s| !s.is_empty());
        Self {
            home,
            data_home,
            data_dirs,
            state_home,
            config_home,
            runtime_dir,
            path,
            current_desktop,
            lang,
            terminal,
        }
    }

    /// A self-contained environment for tests. All paths live under `root`.
    pub fn for_test(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let data_home = root.join("data");
        let state_home = root.join("state");
        let config_home = root.join("config");
        Self {
            home: root.clone(),
            data_home: data_home.clone(),
            data_dirs: vec![data_home.clone()],
            state_home,
            config_home,
            runtime_dir: Some(root.join("run")),
            path: vec![root.join("bin")],
            current_desktop: Some("sway".into()),
            lang: Some("en_US.UTF-8".into()),
            terminal: Some("kitty".into()),
        }
    }

    pub fn applications_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        push_unique(&mut dirs, self.data_home.join("applications"));
        for dir in &self.data_dirs {
            push_unique(&mut dirs, dir.join("applications"));
        }
        push_unique(
            &mut dirs,
            self.home
                .join(".local/share/flatpak/exports/share/applications"),
        );
        push_unique(
            &mut dirs,
            PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        );
        for dir in &self.data_dirs {
            push_unique(&mut dirs, dir.join("flatpak/exports/share/applications"));
        }
        dirs
    }

    pub fn icons_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        push_unique(&mut dirs, self.data_home.join("icons"));
        push_unique(&mut dirs, self.home.join(".icons"));
        for dir in &self.data_dirs {
            push_unique(&mut dirs, dir.join("icons"));
        }
        push_unique(&mut dirs, PathBuf::from("/usr/share/pixmaps"));
        dirs
    }

    pub fn state_file(&self) -> PathBuf {
        self.state_home.join("gofi/state.json")
    }

    pub fn terminal(&self) -> &str {
        self.terminal.as_deref().unwrap_or("kitty")
    }

    /// Locale fallbacks for `Name[xx]`: `en_GB.UTF-8` → `en_GB`, `en`.
    pub fn locale_candidates(&self) -> Vec<String> {
        let Some(raw) = &self.lang else {
            return Vec::new();
        };
        let base = raw.split('.').next().unwrap_or(raw);
        let base = base.split('@').next().unwrap_or(base);
        let mut out = Vec::new();
        if !base.is_empty() {
            out.push(base.to_string());
        }
        if let Some(language) = base.split(['_', '-']).next()
            && language != base
            && !language.is_empty()
        {
            out.push(language.to_string());
        }
        out
    }

    /// Whether `OnlyShowIn` / `NotShowIn` accept this desktop.
    pub fn desktop_matches(&self, only_show_in: Option<&str>, not_show_in: Option<&str>) -> bool {
        let current: Vec<String> = self
            .current_desktop
            .as_deref()
            .unwrap_or_default()
            .split(':')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let listed = |value: &str| {
            value
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_ascii_lowercase())
                .any(|entry| current.iter().any(|desktop| desktop == &entry))
        };
        if let Some(only) = only_show_in
            && !only.trim().is_empty()
            && !listed(only)
        {
            return false;
        }
        if let Some(not) = not_show_in
            && listed(not)
        {
            return false;
        }
        true
    }
}

fn env_string(key: &str) -> Option<String> {
    std::env::var_os(key).map(|value| value.to_string_lossy().into_owned())
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).map(PathBuf::from)
}

fn env_paths(key: &str) -> Option<Vec<PathBuf>> {
    std::env::var_os(key).map(|value| std::env::split_paths(&value).collect())
}

fn push_unique(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if !dir.as_os_str().is_empty() && !dirs.contains(&dir) {
        dirs.push(dir);
    }
}

/// `~` expansion for the rare absolute-ish paths in desktop entries.
pub fn expand_home(path: &str, env: &Env) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        env.home.join(rest)
    } else if path == "~" {
        env.home.clone()
    } else {
        PathBuf::from(path)
    }
}

pub fn is_absolute_or_home(path: &str) -> bool {
    Path::new(path).is_absolute() || path.starts_with("~/") || path == "~"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_candidates_strip_encoding_and_modifier() {
        let mut env = Env::for_test("/tmp/gofi-test");
        env.lang = Some("en_GB.UTF-8@euro".into());
        assert_eq!(env.locale_candidates(), ["en_GB", "en"]);
    }

    #[test]
    fn desktop_matches_respects_only_and_not() {
        let mut env = Env::for_test("/tmp/gofi-test");
        env.current_desktop = Some("sway:wlroots".into());
        assert!(env.desktop_matches(Some("sway;GNOME;"), None));
        assert!(!env.desktop_matches(Some("GNOME;KDE;"), None));
        assert!(!env.desktop_matches(None, Some("wlroots;")));
        assert!(env.desktop_matches(None, Some("GNOME;")));
    }

    #[test]
    fn applications_dirs_are_deduplicated() {
        let env = Env::for_test("/tmp/gofi-test");
        let dirs = env.applications_dirs();
        let mut sorted = dirs.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(dirs.len(), sorted.len(), "duplicate application dir");
    }
}
