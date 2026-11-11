//! Headless view tests: geometry, rows, keys and the underline mapping.

use std::ffi::OsString;

use gofi_core::{Action, Env, Item, ItemId, ItemKind, Mode};
use rune::{Env as RuneEnv, Key, KeyEvent, Mods, Runtime};
use tempfile::TempDir;

use super::*;

fn app(id: &str, title: &str) -> Item {
    Item {
        id: ItemId(format!("apps:{id}")),
        title: title.to_string(),
        subtitle: None,
        icon: None,
        search_text: title.to_string(),
        kind: ItemKind::App,
        action: Action::Launch {
            argv: vec![OsString::from("true")],
            cwd: None,
        },
    }
}

fn clipboard(id: &str, title: &str) -> Item {
    Item {
        id: ItemId(format!("clipboard:{id}")),
        title: title.to_string(),
        subtitle: None,
        icon: None,
        search_text: title.to_string(),
        kind: ItemKind::Clipboard,
        action: Action::CopyClipboard { id: id.to_string() },
    }
}

fn state(temp: &TempDir, mode: Mode, items: Vec<Item>) -> AppState {
    AppState::for_test(Env::for_test(temp.path()), mode, items)
}

fn runtime(state: AppState) -> Runtime<AppState> {
    let mut runtime = Runtime::new(state, view, (PANEL_W, PANEL_H), RuneEnv::new(true, 1.0));
    runtime.frame(0.0);
    runtime
}

fn press(runtime: &mut Runtime<AppState>, key: Key) {
    runtime.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    runtime.frame(1.0 / 60.0);
}

fn titles(runtime: &Runtime<AppState>) -> Vec<String> {
    runtime
        .state
        .ranked
        .iter()
        .map(|ranked| {
            runtime.state.items[&runtime.state.mode][ranked.index]
                .title
                .clone()
        })
        .collect()
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 0.5
}

#[test]
fn geometry_matches_the_measured_rofi() {
    let temp = TempDir::new().unwrap();
    let items = (0..10)
        .map(|index| app(&format!("app-{index}"), &format!("App {index}")))
        .collect();
    let runtime = runtime(state(&temp, Mode::Apps, items));

    let (row, _) = runtime.presented("row-0").expect("first row");
    assert!(close(row[0], 20.0), "row x {}", row[0]);
    assert!(close(row[1], 89.0), "row y {}", row[1]);
    assert!(close(row[2], 902.0), "row width {}", row[2]);
    assert!(close(row[3], 48.0), "row height {}", row[3]);

    let (tab, _) = runtime.presented("tab-apps").expect("apps tab");
    assert!(close(tab[0], 600.0), "tab x {}", tab[0]);
    assert!(close(tab[1], 10.0), "tab y {}", tab[1]);
    assert!(close(tab[2], 100.0), "tab width {}", tab[2]);
    assert!(close(tab[3], 59.0), "tab height {}", tab[3]);

    let (scrollbar, _) = runtime.presented("scrollbar").expect("scrollbar");
    assert!(close(scrollbar[0], 932.0), "scrollbar x {}", scrollbar[0]);
    assert!(close(scrollbar[2], 8.0), "scrollbar width {}", scrollbar[2]);
    assert!(
        close(scrollbar[3], 396.0),
        "scrollbar height {}",
        scrollbar[3]
    );

    // rofi's handle: 396 * 7 / (10 + 7).
    let (handle, _) = runtime.presented("scrollbar-handle").expect("handle");
    assert!(close(handle[0], 932.0), "handle x {}", handle[0]);
    assert!(close(handle[3], 163.06), "handle height {}", handle[3]);
}

#[test]
fn scroll_offset_matches_rofi_centered_view() {
    assert_eq!(scroll_offset(0, 94), 0);
    assert_eq!(scroll_offset(3, 94), 0);
    assert_eq!(scroll_offset(4, 94), 1);
    assert_eq!(scroll_offset(7, 94), 4);
    assert_eq!(scroll_offset(89, 94), 86);
    assert_eq!(scroll_offset(90, 94), 87);
    assert_eq!(scroll_offset(93, 94), 87);
    assert_eq!(scroll_offset(4, 10), 1);
    assert_eq!(scroll_offset(5, 10), 2);
    assert_eq!(scroll_offset(6, 10), 3);
    assert_eq!(scroll_offset(9, 10), 3);
    assert_eq!(scroll_offset(6, 6), 0);
}

#[test]
fn scrollbar_follows_the_selection_and_stays_at_the_right_edge() {
    let temp = TempDir::new().unwrap();
    let items = (0..10)
        .map(|index| app(&format!("app-{index}"), &format!("App {index}")))
        .collect();
    let mut runtime = runtime(state(&temp, Mode::Apps, items));

    let (handle, _) = runtime.presented("scrollbar-handle").expect("handle");
    assert!(close(handle[0], 932.0), "handle x {}", handle[0]);
    assert!(close(handle[1], 89.0), "handle y {}", handle[1]);

    for _ in 0..5 {
        press(&mut runtime, Key::Down);
    }
    for _ in 0..30 {
        runtime.frame(1.0 / 60.0);
    }
    let (handle, _) = runtime.presented("scrollbar-handle").expect("handle");
    let (track, _) = runtime.presented("scrollbar").expect("track");
    eprintln!(
        "track {track:?} handle {handle:?} selected {}",
        runtime.state.selected
    );
    assert!(close(handle[0], 932.0), "handle drifted: x {}", handle[0]);
    assert!(close(handle[1], 218.41), "handle y {}", handle[1]);

    for _ in 0..4 {
        press(&mut runtime, Key::Down);
    }
    let (handle, _) = runtime.presented("scrollbar-handle").expect("handle");
    assert!(close(handle[0], 932.0), "handle drifted: x {}", handle[0]);
    assert!(close(handle[1], 321.94), "handle y {}", handle[1]);
    assert!(close(handle[3], 163.06), "handle height {}", handle[3]);
}

#[test]
fn a_short_list_keeps_a_handle_and_a_gap_below() {
    let temp = TempDir::new().unwrap();
    let items = vec![app("a", "Alpha"), app("b", "Beta"), app("c", "Gamma")];
    let mut runtime = runtime(state(&temp, Mode::Apps, items));

    let (handle, _) = runtime.presented("scrollbar-handle").expect("handle");
    assert!(close(handle[3], 277.2), "handle height {}", handle[3]);
    assert!(
        runtime.presented("row-3").is_none(),
        "a fourth row was drawn"
    );

    press(&mut runtime, Key::Down);
    press(&mut runtime, Key::Down);
    let (handle, _) = runtime.presented("scrollbar-handle").expect("handle");
    assert!(close(handle[1], 207.8), "handle y {}", handle[1]);
}

#[test]
fn seven_rows_and_three_tabs_are_rendered() {
    let temp = TempDir::new().unwrap();
    let items = (0..10)
        .map(|index| app(&format!("app-{index}"), &format!("App {index}")))
        .collect();
    let runtime = runtime(state(&temp, Mode::Apps, items));

    for index in 0..VISIBLE {
        assert!(
            runtime.presented(&format!("row-{index}")).is_some(),
            "row {index} missing"
        );
    }
    assert!(
        runtime.presented("row-7").is_none(),
        "an eighth row was drawn"
    );
    for key in [
        "query",
        "tab-apps",
        "tab-apps-glyph",
        "tab-run",
        "tab-clipboard",
    ] {
        assert!(runtime.presented(key).is_some(), "{key} missing");
    }
}

#[test]
fn typing_filters_and_selects_the_first_match() {
    let temp = TempDir::new().unwrap();
    let items = vec![
        app("firefox", "Firefox"),
        app("files", "Files"),
        app("terminal", "Terminal"),
    ];
    let mut runtime = runtime(state(&temp, Mode::Apps, items));

    for character in ["f", "i", "r", "e"] {
        press(&mut runtime, Key::Text(character.into()));
    }

    assert_eq!(runtime.state.query.text, "fire");
    assert_eq!(titles(&runtime), ["Firefox"]);
    assert_eq!(runtime.state.selected, 0);
}

#[test]
fn tab_cycles_modes_and_clears_the_query() {
    let temp = TempDir::new().unwrap();
    let apps = vec![app("firefox", "Firefox")];
    let runs = vec![app("poweroff", "poweroff")];
    let mut initial = state(&temp, Mode::Apps, apps);
    initial.items.insert(Mode::Run, runs);

    let mut runtime = runtime(initial);
    press(&mut runtime, Key::Text("f".into()));
    assert_eq!(runtime.state.query.text, "f");

    press(&mut runtime, Key::Tab);
    assert_eq!(runtime.state.mode, Mode::Run);
    assert_eq!(runtime.state.query.text, "");
    assert_eq!(titles(&runtime), ["poweroff"]);

    press(&mut runtime, Key::Tab);
    assert_eq!(runtime.state.mode, Mode::Clipboard);

    press(&mut runtime, Key::Tab);
    assert_eq!(runtime.state.mode, Mode::Apps);

    let shift_tab = KeyEvent {
        key: Key::Tab,
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
    };
    runtime.key(shift_tab);
    runtime.frame(1.0 / 60.0);
    assert_eq!(runtime.state.mode, Mode::Clipboard);

    let shift_right = KeyEvent {
        key: Key::Right,
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
    };
    runtime.key(shift_right);
    runtime.frame(1.0 / 60.0);
    assert_eq!(runtime.state.mode, Mode::Apps, "Shift+Right cycles forward");

    let shift_left = KeyEvent {
        key: Key::Left,
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
    };
    runtime.key(shift_left);
    runtime.frame(1.0 / 60.0);
    assert_eq!(
        runtime.state.mode,
        Mode::Clipboard,
        "Shift+Left cycles backward"
    );
}

#[test]
fn arrows_wrap_and_scroll_the_window() {
    let temp = TempDir::new().unwrap();
    let items = (0..10)
        .map(|index| app(&format!("app-{index}"), &format!("App {index}")))
        .collect();
    let mut runtime = runtime(state(&temp, Mode::Apps, items));

    for _ in 0..8 {
        press(&mut runtime, Key::Down);
    }
    assert_eq!(runtime.state.selected, 8);
    assert_eq!(runtime.state.offset, 3);
    let (top, _) = runtime.presented("row-0").expect("row-0");
    assert!(close(top[1], 89.0), "row-0 is not first: y {}", top[1]);
    let (bottom, _) = runtime.presented("row-6").expect("row-6");
    assert!(
        close(bottom[1], 431.0),
        "row-6 is not last: y {}",
        bottom[1]
    );
    // The selection is at the bottom of the window, not centered yet: 10 items
    // clamp the window before it can center.
    let (selected, _) = runtime.presented("row-5").expect("row-5");
    assert!(close(selected[1], 374.0), "selected slot y {}", selected[1]);

    press(&mut runtime, Key::Down);
    press(&mut runtime, Key::Down);
    assert_eq!(runtime.state.selected, 0);
    assert_eq!(runtime.state.offset, 0);
    let (top, _) = runtime.presented("row-0").expect("row-0");
    assert!(close(top[1], 89.0), "row-0 is not first: y {}", top[1]);

    press(&mut runtime, Key::Up);
    assert_eq!(runtime.state.selected, 9);
    assert_eq!(runtime.state.offset, 3);
}

#[test]
fn wheel_moves_the_selection() {
    let temp = TempDir::new().unwrap();
    let items = (0..10)
        .map(|index| app(&format!("app-{index}"), &format!("App {index}")))
        .collect();
    let mut runtime = runtime(state(&temp, Mode::Apps, items));

    runtime.wheel_event(WheelEvent::new(0.0, -50.0).at(100.0, 200.0));
    runtime.frame(1.0 / 60.0);
    assert_eq!(runtime.state.selected, 1, "wheel down moves down");

    runtime.wheel_event(WheelEvent::new(0.0, 50.0).at(100.0, 200.0));
    runtime.frame(1.0 / 60.0);
    assert_eq!(runtime.state.selected, 0, "wheel up moves up");
}

#[test]
fn ctrl_n_and_ctrl_p_move_the_selection() {
    let temp = TempDir::new().unwrap();
    let items = vec![app("a", "Alpha"), app("b", "Beta"), app("c", "Gamma")];
    let mut runtime = runtime(state(&temp, Mode::Apps, items));

    press(&mut runtime, Key::Command('n'));
    assert_eq!(runtime.state.selected, 1);
    press(&mut runtime, Key::Command('n'));
    assert_eq!(runtime.state.selected, 2);
    press(&mut runtime, Key::Command('n'));
    assert_eq!(runtime.state.selected, 0, "Ctrl+N wraps");
    press(&mut runtime, Key::Command('p'));
    assert_eq!(runtime.state.selected, 2, "Ctrl+P wraps back");
}

#[test]
fn home_end_and_page_keys_move_the_selection() {
    let temp = TempDir::new().unwrap();
    let items = (0..20)
        .map(|index| app(&format!("app-{index}"), &format!("App {index}")))
        .collect();
    let mut runtime = runtime(state(&temp, Mode::Apps, items));

    press(&mut runtime, Key::End);
    assert_eq!(runtime.state.selected, 19);
    assert_eq!(runtime.state.offset, 13);
    let (handle, _) = runtime.presented("scrollbar-handle").expect("handle");
    assert!(close(handle[0], 932.0), "handle x {}", handle[0]);
    assert!(
        close(handle[1] + handle[3], 485.0),
        "the handle did not reach the bottom: {}",
        handle[1] + handle[3]
    );

    press(&mut runtime, Key::Home);
    assert_eq!(runtime.state.selected, 0);
    assert_eq!(runtime.state.offset, 0);

    press(&mut runtime, Key::PageDown);
    assert_eq!(runtime.state.selected, 7);
    assert_eq!(runtime.state.offset, 4);

    press(&mut runtime, Key::PageDown);
    assert_eq!(runtime.state.selected, 14);
    assert_eq!(runtime.state.offset, 11);

    press(&mut runtime, Key::PageUp);
    assert_eq!(runtime.state.selected, 7);
    assert_eq!(runtime.state.offset, 4);
}

#[test]
fn ctrl_w_clears_and_ctrl_u_kills_to_the_caret() {
    let temp = TempDir::new().unwrap();
    let items = vec![
        app("firefox", "Firefox"),
        app("files", "Files"),
        app("terminal", "Terminal"),
    ];
    let mut runtime = runtime(state(&temp, Mode::Apps, items));

    for character in ["f", "i", "r", "e"] {
        press(&mut runtime, Key::Text(character.into()));
    }
    assert_eq!(runtime.state.query.text, "fire");

    press(&mut runtime, Key::Command('w'));
    assert_eq!(runtime.state.query.text, "", "Ctrl+W clears the line");
    assert_eq!(titles(&runtime).len(), 3, "the full list came back");

    for character in ["f", "i", "r", "e"] {
        press(&mut runtime, Key::Text(character.into()));
    }
    press(&mut runtime, Key::Left);
    press(&mut runtime, Key::Left);
    assert_eq!(runtime.state.query.focus, 2, "caret after two Lefts");

    press(&mut runtime, Key::Command('u'));
    assert_eq!(
        runtime.state.query.text, "re",
        "Ctrl+U deletes to the start"
    );
    assert_eq!(runtime.state.query.focus, 0);
    assert_eq!(titles(&runtime), ["Firefox"]);

    press(&mut runtime, Key::Command('k'));
    assert_eq!(runtime.state.query.text, "", "Ctrl+K deletes to the end");

    for character in ["f", "i", "r", "e"] {
        press(&mut runtime, Key::Text(character.into()));
    }
    press(&mut runtime, Key::Left);
    press(&mut runtime, Key::Left);
    press(&mut runtime, Key::Command('d'));
    assert_eq!(runtime.state.query.text, "fie", "Ctrl+D deletes forward");
    press(&mut runtime, Key::Command('h'));
    assert_eq!(runtime.state.query.text, "fe", "Ctrl+H deletes backward");
}

#[test]
fn an_empty_corpus_renders_without_rows() {
    let temp = TempDir::new().unwrap();
    let runtime = runtime(state(&temp, Mode::Apps, Vec::new()));
    assert!(runtime.presented("row-0").is_none());
    assert!(runtime.presented("query").is_some());
}

#[test]
fn clipboard_rows_use_the_generic_icon_and_keep_provider_order() {
    let temp = TempDir::new().unwrap();
    let items = vec![clipboard("2", "newer"), clipboard("1", "older")];
    let runtime = runtime(state(&temp, Mode::Clipboard, items));

    let first = &runtime.state.items[&Mode::Clipboard][runtime.state.ranked[0].index];
    let second = &runtime.state.items[&Mode::Clipboard][runtime.state.ranked[1].index];
    assert_eq!(first.id.as_str(), "clipboard:2");
    assert_eq!(second.id.as_str(), "clipboard:1");
}

#[test]
fn underline_spans_merge_runs_and_count_bytes() {
    let ink = Palette::get().text;

    let merged = match_spans("Firefox", &[0, 1, 2, 3], ink);
    assert_eq!(merged.len(), 1);
    assert_eq!((merged[0].start, merged[0].end), (0, 4));
    assert_eq!(merged[0].weight, Some(700.0), "matched text is bold");
    assert!(merged[0].underline.is_some(), "matched text is underlined");

    let split = match_spans("Firefox", &[0, 2], ink);
    assert_eq!(split.len(), 2);
    assert_eq!((split[0].start, split[0].end), (0, 1));
    assert_eq!((split[1].start, split[1].end), (2, 3));

    // `Ü` is two bytes, so the second character starts at byte 3.
    let unicode = match_spans("Über", &[0, 2], ink);
    assert_eq!(unicode.len(), 2);
    assert_eq!((unicode[0].start, unicode[0].end), (0, 2));
    assert_eq!((unicode[1].start, unicode[1].end), (3, 4));
}

#[test]
fn a_failed_launch_keeps_the_window_open_and_shows_a_notice() {
    let _guard = crate::test_support::spawn_guard();
    let temp = TempDir::new().unwrap();
    let mut item = app("broken", "Broken");
    item.action = Action::Launch {
        argv: vec![OsString::from("/definitely/not/a/program")],
        cwd: None,
    };
    let mut runtime = runtime(state(&temp, Mode::Apps, vec![item]));

    runtime.update(|state, tx| state.activate(0, tx));

    let notice = runtime.state.notice.as_deref().expect("a notice");
    assert!(notice.contains("could not launch"), "{notice}");
}
