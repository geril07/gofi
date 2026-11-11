//! Behavior tests for the apps, run and clipboard providers and icon lookup.
//!
//! Every test builds a real temporary tree. The clipboard provider runs a real
//! script as its `cliphist` binary; nothing here mocks the filesystem or
//! `Command`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use gofi_core::icons::resolve_icon;
use gofi_core::providers::{AppsProvider, ClipboardProvider, RunProvider};
use gofi_core::{Action, Env, Item, ItemKind, Loaded, Provider};
use tempfile::TempDir;

fn env_for(root: &Path) -> Env {
    let mut env = Env::for_test(root);
    env.data_dirs = vec![root.join("system-share")];
    env.path = vec![root.join("bin")];
    env.current_desktop = Some("sway".into());
    env.lang = Some("en_US.UTF-8".into());
    env.terminal = Some("kitty".into());
    env
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn write_executable(path: &Path, contents: &str) {
    write_file(path, contents);
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn load_apps(env: &Env) -> Loaded {
    AppsProvider::new(env.clone()).load().unwrap()
}

fn find_item<'a>(loaded: &'a Loaded, id: &str) -> &'a Item {
    loaded
        .items
        .iter()
        .find(|item| item.id.as_str() == id)
        .unwrap_or_else(|| {
            panic!(
                "missing item {id}; got {:?}",
                loaded
                    .items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>()
            )
        })
}

fn has_item(loaded: &Loaded, id: &str) -> bool {
    loaded.items.iter().any(|item| item.id.as_str() == id)
}

fn launch_argv(item: &Item) -> Vec<String> {
    match &item.action {
        Action::Launch { argv, .. } => argv
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect(),
        other => panic!("expected a launch action, got {other:?}"),
    }
}

fn launch_cwd(item: &Item) -> Option<&Path> {
    match &item.action {
        Action::Launch { cwd, .. } => cwd.as_deref(),
        other => panic!("expected a launch action, got {other:?}"),
    }
}

// ---------------------------------------------------------------- apps

#[test]
fn apps_discover_name_keywords_and_exec() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    write_file(
        &env.data_home.join("applications/firefox.desktop"),
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Firefox\n\
         Name[de]=Feuerfuchs\n\
         GenericName=Web Browser\n\
         GenericName[de]=Webbrowser\n\
         Keywords=internet;browser;\n\
         Exec=firefox %u\n\
         Icon=firefox\n",
    );

    let loaded = load_apps(&env);
    let item = find_item(&loaded, "apps:firefox.desktop");
    assert_eq!(item.title, "Firefox");
    assert_eq!(item.subtitle.as_deref(), Some("Web Browser"));
    assert_eq!(item.search_text, "Firefox", "keywords are not matched");
    assert_eq!(item.kind, ItemKind::App);
    assert_eq!(launch_argv(item), ["firefox"]);
    assert!(item.icon.is_none());

    let mut german = env.clone();
    german.lang = Some("de_DE.UTF-8".into());
    let loaded = load_apps(&german);
    let item = find_item(&loaded, "apps:firefox.desktop");
    assert_eq!(item.title, "Feuerfuchs");
    assert_eq!(item.subtitle.as_deref(), Some("Webbrowser"));
}

#[test]
fn apps_first_data_dir_wins_and_user_hidden_suppresses_system() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    let system = &env.data_dirs[0];

    write_file(
        &env.data_home.join("applications/dup.desktop"),
        "[Desktop Entry]\nType=Application\nName=User Copy\nExec=user-copy\n",
    );
    write_file(
        &system.join("applications/dup.desktop"),
        "[Desktop Entry]\nType=Application\nName=System Copy\nExec=system-copy\n",
    );
    write_file(
        &env.data_home.join("applications/hidden.desktop"),
        "[Desktop Entry]\nType=Application\nName=User Hidden\nHidden=true\nExec=hidden\n",
    );
    write_file(
        &system.join("applications/hidden.desktop"),
        "[Desktop Entry]\nType=Application\nName=System Visible\nExec=system-visible\n",
    );
    write_file(
        &system.join("applications/system.desktop"),
        "[Desktop Entry]\nType=Application\nName=System Only\nExec=system-only\n",
    );

    let loaded = load_apps(&env);
    assert_eq!(find_item(&loaded, "apps:dup.desktop").title, "User Copy");
    assert_eq!(
        loaded
            .items
            .iter()
            .filter(|item| item.id.as_str() == "apps:dup.desktop")
            .count(),
        1
    );
    assert!(!has_item(&loaded, "apps:hidden.desktop"));
    assert_eq!(
        find_item(&loaded, "apps:system.desktop").title,
        "System Only"
    );
}

#[test]
fn apps_skip_non_applications_hidden_and_desktop_mismatches() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    let dir = env.data_home.join("applications");

    let entries = [
        (
            "link.desktop",
            "Type=Link\nName=Link\nURL=https://example.com\n",
        ),
        (
            "nodisplay.desktop",
            "Type=Application\nName=No Display\nNoDisplay=true\nExec=nodisplay\n",
        ),
        (
            "only.desktop",
            "Type=Application\nName=Only Gnome\nOnlyShowIn=GNOME;\nExec=only\n",
        ),
        (
            "notshow.desktop",
            "Type=Application\nName=Not Sway\nNotShowIn=sway;\nExec=notshow\n",
        ),
        (
            "tryexec-abs.desktop",
            "Type=Application\nName=Missing TryExec\nTryExec=/definitely/not/here\nExec=missing\n",
        ),
        (
            "tryexec-path.desktop",
            "Type=Application\nName=Missing TryExec Path\nTryExec=definitely-not-on-path\nExec=missing\n",
        ),
        (
            "keep.desktop",
            "Type=Application\nName=Keep Me\nExec=keep\n",
        ),
        (
            "sway-only.desktop",
            "Type=Application\nName=Sway Only\nOnlyShowIn=sway;\nExec=sway-only\n",
        ),
        (
            "not-gnome.desktop",
            "Type=Application\nName=Not Gnome\nNotShowIn=GNOME;\nExec=not-gnome\n",
        ),
    ];
    for (name, contents) in entries {
        write_file(&dir.join(name), &format!("[Desktop Entry]\n{contents}"));
    }

    let loaded = load_apps(&env);
    assert!(has_item(&loaded, "apps:keep.desktop"));
    assert!(has_item(&loaded, "apps:sway-only.desktop"));
    assert!(has_item(&loaded, "apps:not-gnome.desktop"));
    for id in [
        "apps:link.desktop",
        "apps:nodisplay.desktop",
        "apps:only.desktop",
        "apps:notshow.desktop",
        "apps:tryexec-abs.desktop",
        "apps:tryexec-path.desktop",
    ] {
        assert!(!has_item(&loaded, id), "{id} should have been skipped");
    }
}

#[test]
fn apps_bad_entry_is_a_warning_not_a_failure() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    let dir = env.data_home.join("applications");

    write_file(
        &dir.join("bad.desktop"),
        "[Desktop Entry]\nType=Application\nName=Bad Quote\nExec=broken \"unterminated\n",
    );
    write_file(
        &dir.join("good.desktop"),
        "[Desktop Entry]\nType=Application\nName=Good Entry\nExec=good\n",
    );

    let loaded = load_apps(&env);
    assert!(has_item(&loaded, "apps:good.desktop"));
    assert!(!has_item(&loaded, "apps:bad.desktop"));
    assert!(
        loaded
            .warnings
            .iter()
            .any(|warning| warning.contains("bad.desktop")),
        "expected a warning for bad.desktop, got {:?}",
        loaded.warnings
    );
}

#[test]
fn apps_expand_exec_field_codes_without_a_shell() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    let icon = temp.path().join("icon.png");
    write_file(&icon, "png");
    let desktop = env.data_home.join("applications/fields.desktop");
    write_file(
        &desktop,
        &format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Field Test\n\
             Icon={}\n\
             Exec=/usr/bin/app --name \"Hello World\" \"quote\\\"inside\" %f %F %u %U %i %c %k %% %z\n",
            icon.display()
        ),
    );

    let loaded = load_apps(&env);
    let item = find_item(&loaded, "apps:fields.desktop");
    assert_eq!(
        launch_argv(item),
        [
            "/usr/bin/app",
            "--name",
            "Hello World",
            "quote\"inside",
            "--icon",
            icon.to_str().unwrap(),
            "Field Test",
            desktop.to_str().unwrap(),
            "%",
        ]
    );
    assert!(
        loaded.warnings.iter().any(|warning| warning.contains("%z")),
        "unknown field code should warn, got {:?}",
        loaded.warnings
    );
}

#[test]
fn apps_terminal_entries_are_wrapped() {
    let temp = TempDir::new().unwrap();
    let mut env = env_for(temp.path());
    write_file(
        &env.data_home.join("applications/htop.desktop"),
        "[Desktop Entry]\nType=Application\nName=htop\nTerminal=true\nExec=htop\n",
    );

    let loaded = load_apps(&env);
    assert_eq!(
        launch_argv(find_item(&loaded, "apps:htop.desktop")),
        ["kitty", "htop"]
    );

    env.terminal = Some("foot --hold".into());
    let loaded = load_apps(&env);
    assert_eq!(
        launch_argv(find_item(&loaded, "apps:htop.desktop")),
        ["foot", "--hold", "htop"]
    );

    env.terminal = None;
    let loaded = load_apps(&env);
    assert_eq!(
        launch_argv(find_item(&loaded, "apps:htop.desktop")),
        ["kitty", "htop"]
    );
}

#[test]
fn apps_path_becomes_cwd_only_when_it_exists() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    let work = temp.path().join("work");
    fs::create_dir_all(&work).unwrap();

    write_file(
        &env.data_home.join("applications/with-path.desktop"),
        &format!(
            "[Desktop Entry]\nType=Application\nName=With Path\nExec=app\nPath={}\n",
            work.display()
        ),
    );
    write_file(
        &env.data_home.join("applications/missing-path.desktop"),
        &format!(
            "[Desktop Entry]\nType=Application\nName=Missing Path\nExec=app\nPath={}\n",
            temp.path().join("missing").display()
        ),
    );

    let loaded = load_apps(&env);
    assert_eq!(
        launch_cwd(find_item(&loaded, "apps:with-path.desktop")),
        Some(work.as_path())
    );
    assert_eq!(
        launch_cwd(find_item(&loaded, "apps:missing-path.desktop")),
        None
    );
}

#[test]
fn apps_subdirectory_ids_use_dashes_and_dedupe_across_dirs() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    let system = &env.data_dirs[0];

    write_file(
        &env.data_home.join("applications/kde/foo.desktop"),
        "[Desktop Entry]\nType=Application\nName=User Nested\nExec=user-nested\n",
    );
    write_file(
        &system.join("applications/kde/foo.desktop"),
        "[Desktop Entry]\nType=Application\nName=System Nested\nExec=system-nested\n",
    );

    let loaded = load_apps(&env);
    assert_eq!(
        find_item(&loaded, "apps:kde-foo.desktop").title,
        "User Nested"
    );
    assert_eq!(
        loaded
            .items
            .iter()
            .filter(|item| item.id.as_str() == "apps:kde-foo.desktop")
            .count(),
        1
    );
}

#[test]
fn apps_resolve_icons_to_theme_files() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    let themed = env.data_home.join("icons/48x48/myapp.png");
    write_file(&themed, "png");
    let absolute = temp.path().join("absolute.png");
    write_file(&absolute, "png");

    write_file(
        &env.data_home.join("applications/themed.desktop"),
        "[Desktop Entry]\nType=Application\nName=Themed\nIcon=myapp\nExec=themed\n",
    );
    write_file(
        &env.data_home.join("applications/absolute.desktop"),
        &format!(
            "[Desktop Entry]\nType=Application\nName=Absolute\nIcon={}\nExec=absolute\n",
            absolute.display()
        ),
    );
    write_file(
        &env.data_home.join("applications/no-icon.desktop"),
        "[Desktop Entry]\nType=Application\nName=No Icon\nIcon=definitely-missing-icon\nExec=no-icon\n",
    );

    let loaded = load_apps(&env);
    assert_eq!(
        find_item(&loaded, "apps:themed.desktop").icon.as_deref(),
        Some(themed.as_path())
    );
    assert_eq!(
        find_item(&loaded, "apps:absolute.desktop").icon.as_deref(),
        Some(absolute.as_path())
    );
    assert!(find_item(&loaded, "apps:no-icon.desktop").icon.is_none());
}

// ---------------------------------------------------------------- run

#[test]
fn run_scans_path_in_order_and_dedupes_by_name() {
    let temp = TempDir::new().unwrap();
    let mut env = env_for(temp.path());
    let first = env.path[0].clone();
    let second = temp.path().join("bin2");

    write_executable(&first.join("alpha"), "#!/bin/sh\n");
    write_executable(&second.join("alpha"), "#!/bin/sh\n");
    write_file(&first.join("not-executable"), "plain text\n");
    write_executable(&first.join(".hidden"), "#!/bin/sh\n");
    write_executable(&first.join("gofi"), "#!/bin/sh\n");
    write_executable(&second.join("beta"), "#!/bin/sh\n");
    fs::create_dir_all(first.join("dir-exec")).unwrap();
    let mut permissions = fs::metadata(first.join("dir-exec")).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(first.join("dir-exec"), permissions).unwrap();
    std::os::unix::fs::symlink(second.join("beta"), first.join("delta")).unwrap();

    env.path = vec![first, second];
    let loaded = RunProvider::new(env).load().unwrap();

    let mut ids: Vec<&str> = loaded.items.iter().map(|item| item.id.as_str()).collect();
    ids.sort();
    assert_eq!(
        ids,
        ["run:exec:alpha", "run:exec:beta", "run:exec:delta"],
        "warnings: {:?}",
        loaded.warnings
    );

    let alpha = find_item(&loaded, "run:exec:alpha");
    assert_eq!(alpha.title, "alpha");
    assert_eq!(alpha.kind, ItemKind::Executable);
    assert_eq!(launch_argv(alpha), ["alpha"]);
    assert!(alpha.icon.is_none());
    assert_eq!(alpha.subtitle, None);
}

#[test]
fn run_missing_path_directory_is_a_warning() {
    let temp = TempDir::new().unwrap();
    let mut env = env_for(temp.path());
    let missing = temp.path().join("missing-bin");
    env.path = vec![missing.clone(), missing.clone()];

    let loaded = RunProvider::new(env).load().unwrap();
    assert!(loaded.items.is_empty());
    assert_eq!(loaded.warnings.len(), 1, "{:?}", loaded.warnings);
    assert!(loaded.warnings[0].contains("missing-bin"));
}

// ---------------------------------------------------------------- clipboard

fn cliphist(temp: &TempDir, body: &str) -> PathBuf {
    let path = temp.path().join("cliphist");
    write_executable(&path, body);
    path
}

#[test]
fn clipboard_parses_cliphist_list() {
    let temp = TempDir::new().unwrap();
    let bin = cliphist(
        &temp,
        "#!/bin/sh\nprintf '12\\tfirst entry\\n13\\tsecond entry\\n\\n'\n",
    );

    let loaded = ClipboardProvider::new(bin).load().unwrap();
    assert_eq!(loaded.items.len(), 2);

    let first = find_item(&loaded, "clipboard:12");
    assert_eq!(first.title, "first entry");
    assert_eq!(first.search_text, "first entry");
    assert_eq!(first.kind, ItemKind::Clipboard);
    assert!(first.icon.is_none());
    match &first.action {
        Action::CopyClipboard { id } => assert_eq!(id, "12"),
        other => panic!("expected a clipboard action, got {other:?}"),
    }

    assert_eq!(find_item(&loaded, "clipboard:13").title, "second entry");
}

#[test]
fn clipboard_caps_entries_at_750() {
    let temp = TempDir::new().unwrap();
    let mut lines = String::new();
    for id in 1..=800 {
        lines.push_str(&format!("{id}\tentry {id}\n"));
    }
    let data = temp.path().join("entries.txt");
    write_file(&data, &lines);
    let bin = cliphist(&temp, &format!("#!/bin/sh\ncat '{}'\n", data.display()));

    let loaded = ClipboardProvider::new(bin).load().unwrap();
    assert_eq!(loaded.items.len(), 750);
    assert_eq!(loaded.items.last().unwrap().id.as_str(), "clipboard:750");
}

#[test]
fn clipboard_failure_is_an_error_without_leaking_previews() {
    let temp = TempDir::new().unwrap();
    let secret = "super-secret-clipboard-preview";
    let bin = cliphist(&temp, &format!("#!/bin/sh\nprintf '{secret}\\n'\nexit 3\n"));

    let error = ClipboardProvider::new(bin).load().unwrap_err();
    let message = error.to_string();
    assert!(message.contains("failed"), "{message}");
    assert!(!message.contains(secret), "error leaked preview: {message}");

    let missing = temp.path().join("missing-cliphist");
    assert!(ClipboardProvider::new(missing).load().is_err());
}

// ---------------------------------------------------------------- icons

#[test]
fn icons_use_absolute_paths_and_sizes_in_order() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());

    let absolute = temp.path().join("absolute.png");
    write_file(&absolute, "png");
    assert_eq!(
        resolve_icon(absolute.to_str().unwrap(), &env),
        Some(absolute.clone())
    );

    let icons = env.data_home.join("icons");
    write_file(&icons.join("32x32/foo.png"), "png");
    write_file(&icons.join("48x48/foo.png"), "png");
    assert_eq!(resolve_icon("foo", &env), Some(icons.join("32x32/foo.png")));

    write_file(&icons.join("64x64/bar.svg"), "svg");
    write_file(&icons.join("64x64/bar.png"), "png");
    assert_eq!(resolve_icon("bar", &env), Some(icons.join("64x64/bar.png")));

    assert_eq!(resolve_icon("unknown-icon-name", &env), None);
    assert_eq!(
        resolve_icon(temp.path().join("missing.png").to_str().unwrap(), &env),
        None
    );
}

#[test]
fn icons_fall_back_to_root_theme_and_system_dirs() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    let icons = env.data_home.join("icons");

    write_file(&icons.join("plain.png"), "png");
    assert_eq!(resolve_icon("plain", &env), Some(icons.join("plain.png")));

    let themed = icons.join("hicolor/48x48/apps/themed.png");
    write_file(&themed, "png");
    assert_eq!(resolve_icon("themed", &env), Some(themed));

    let system = env.data_dirs[0].join("icons/64x64/system.png");
    write_file(&system, "png");
    assert_eq!(resolve_icon("system", &env), Some(system));
}

#[test]
fn icons_follow_the_gtk_theme_and_its_inherited_legacy_dirs() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    let icons = env.data_home.join("icons");

    write_file(
        &icons.join("MyTheme/index.theme"),
        "[Icon Theme]\nInherits=MyLegacy,hicolor\nDirectories=32x32/apps\n",
    );
    write_file(
        &icons.join("MyLegacy/index.theme"),
        "[Icon Theme]\nDirectories=16x16/legacy\n",
    );
    let legacy = icons.join("MyLegacy/16x16/legacy/legacy-only.png");
    write_file(&legacy, "png");
    write_file(
        &env.config_home.join("gtk-3.0/settings.ini"),
        "[Settings]\ngtk-icon-theme-name = MyTheme\n",
    );

    assert_eq!(resolve_icon("legacy-only", &env), Some(legacy));
}

#[test]
fn icons_prefer_the_selected_theme_over_a_better_hicolor_size() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    let icons = env.data_home.join("icons");

    let selected = icons.join("MyTheme/48x48/apps/shared.png");
    write_file(&selected, "png");
    write_file(
        &icons.join("MyTheme/index.theme"),
        "[Icon Theme]\nDirectories=48x48/apps\n",
    );
    write_file(&icons.join("hicolor/32x32/apps/shared.png"), "png");
    write_file(
        &env.config_home.join("gtk-4.0/settings.ini"),
        "[Settings]\ngtk-icon-theme-name=MyTheme\n",
    );

    assert_eq!(resolve_icon("shared", &env), Some(selected));
}

#[test]
fn icons_fall_back_to_adwaita_legacy_without_gtk_settings() {
    let temp = TempDir::new().unwrap();
    let env = env_for(temp.path());
    let icons = env.data_home.join("icons");

    write_file(
        &icons.join("Adwaita/index.theme"),
        "[Icon Theme]\nInherits=AdwaitaLegacy,hicolor\nDirectories=16x16/legacy\n",
    );
    let legacy = icons.join("AdwaitaLegacy/16x16/legacy/stock-only.png");
    write_file(&legacy, "png");

    assert_eq!(resolve_icon("stock-only", &env), Some(legacy));
}
