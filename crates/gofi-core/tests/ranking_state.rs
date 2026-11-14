//! Behavior tests for ranking, frecency and persistent state.
//!
//! These pin observable behavior: the order of a fixed corpus, scores after a
//! known elapsed time, and what a real state file contains. They do not assert
//! matcher internals or exact scores, which ADR 0004 expects to move.

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use gofi_core::ranking::frecency::{self, Entry, Frecency, HALF_LIFE_SECS, NORMALIZE_K};
use gofi_core::ranking::{Ranked, Ranker, rank, rank_at};
use gofi_core::store::{MAX_ENTRIES_PER_MODE, STATE_VERSION, State};
use gofi_core::{Action, Env, Item, ItemId, ItemKind, Mode};

// ---------------------------------------------------------------- helpers

fn item(id: &str, title: &str) -> Item {
    Item {
        id: ItemId(id.to_string()),
        title: title.to_string(),
        subtitle: None,
        icon: None,
        search_text: String::new(),
        kind: ItemKind::App,
        action: Action::Launch {
            argv: Vec::new(),
            cwd: None,
        },
    }
}

fn clipboard_item(id: &str, title: &str) -> Item {
    Item {
        id: ItemId(id.to_string()),
        title: title.to_string(),
        subtitle: None,
        icon: None,
        search_text: String::new(),
        kind: ItemKind::Clipboard,
        action: Action::CopyClipboard { id: id.to_string() },
    }
}

fn corpus(titles: &[&str]) -> Vec<Item> {
    titles
        .iter()
        .map(|title| item(&format!("apps:{title}"), title))
        .collect()
}

fn order<'a>(items: &'a [Item], ranked: &[Ranked]) -> Vec<&'a str> {
    ranked
        .iter()
        .map(|ranked| items[ranked.index].title.as_str())
        .collect()
}

const NOW: u64 = 1_760_000_000;

// ---------------------------------------------------------------- ranking

#[test]
fn prefix_and_word_boundary_matches_beat_scattered_ones() {
    let items = corpus(&[
        "Firefox",
        "Files",
        "Profile Editor",
        "Config Editor",
        "fzf",
        "Spotify",
    ]);
    let ranked = rank_at(&items, "fi", Mode::Apps, &Frecency::default(), NOW);

    assert_eq!(
        order(&items, &ranked),
        ["Files", "Firefox", "Config Editor", "Profile Editor"],
        "prefix matches first, then mid-word matches, then nothing"
    );
    assert!(ranked.iter().all(|ranked| !ranked.positions.is_empty()));
}

#[test]
fn word_boundary_matches_beat_mid_word_matches() {
    let items = corpus(&["Terminal", "Determinate", "Xterm Settings Manager"]);
    let ranked = rank_at(&items, "term", Mode::Apps, &Frecency::default(), NOW);

    assert_eq!(
        order(&items, &ranked),
        ["Terminal", "Xterm Settings Manager", "Determinate"],
        "the word-boundary match wins; the two mid-word matches tie and the \
         earlier start (1 before 2) breaks it"
    );
}

#[test]
fn earlier_match_wins_a_score_tie() {
    // Both are consecutive matches at a word boundary, so they score the same;
    // the title would put "Kind Of Sort" first. The match closer to the start
    // must win instead.
    let items = corpus(&["Kind Of Sort", "Sort New"]);
    let ranked = rank_at(&items, "sort", Mode::Apps, &Frecency::default(), NOW);

    assert_eq!(
        ranked[0].score, ranked[1].score,
        "premise: the two matches score the same"
    );
    assert_eq!(
        order(&items, &ranked),
        ["Sort New", "Kind Of Sort"],
        "equal scores: the earlier match wins before the title"
    );
}

#[test]
fn earlier_match_does_not_beat_a_better_match() {
    // "Redshift" matches at 1, "Text Editor" at 5, but the word boundary makes
    // the later match clearly better. Position only breaks ties.
    let items = corpus(&["Redshift", "Text Editor"]);
    let ranked = rank_at(&items, "ed", Mode::Apps, &Frecency::default(), NOW);

    assert_eq!(
        order(&items, &ranked),
        ["Text Editor", "Redshift"],
        "a word-boundary match still beats an earlier mid-word match"
    );
}

#[test]
fn equal_scores_and_positions_fall_back_to_title() {
    let items = corpus(&["Sort Of", "Sort New"]);
    let ranked = rank_at(&items, "sort", Mode::Apps, &Frecency::default(), NOW);

    assert_eq!(
        order(&items, &ranked),
        ["Sort New", "Sort Of"],
        "both start at 0, so the title breaks the tie"
    );
}

#[test]
fn smart_case_ignores_case_only_for_lowercase_queries() {
    let items = corpus(&["Firefox", "firefox-dev", "Fire"]);

    let lowercase = rank_at(&items, "fire", Mode::Apps, &Frecency::default(), NOW);
    assert_eq!(order(&items, &lowercase).len(), 3, "all three match 'fire'");

    let capitalized = rank_at(&items, "Fire", Mode::Apps, &Frecency::default(), NOW);
    let titles = order(&items, &capitalized);
    assert_eq!(titles, ["Fire", "Firefox"], "uppercase F must match F");
    assert!(
        !titles.contains(&"firefox-dev"),
        "a capitalized query must not match lowercase text"
    );
}

#[test]
fn unicode_is_normalized_and_positions_are_character_indices() {
    let items = corpus(&["Café Ünïcode", "Plain"]);

    let ascii_query = rank_at(&items, "cafe", Mode::Apps, &Frecency::default(), NOW);
    assert_eq!(order(&items, &ascii_query), ["Café Ünïcode"]);

    let unicode_query = rank_at(&items, "ünï", Mode::Apps, &Frecency::default(), NOW);
    assert_eq!(order(&items, &unicode_query), ["Café Ünïcode"]);
    assert_eq!(
        unicode_query[0].positions,
        [5, 6, 7],
        "Ünï starts at character 5; byte offsets would differ"
    );
}

#[test]
fn positions_underline_the_title_and_skip_keyword_only_matches() {
    let items = corpus(&["Firefox"]);
    let ranked = rank_at(&items, "ff", Mode::Apps, &Frecency::default(), NOW);
    assert_eq!(ranked[0].positions, [0, 4]);

    let mut keyword = item("apps:firefox.desktop", "Firefox");
    keyword.search_text = "Firefox browser web".to_string();
    let items = vec![keyword];
    let ranked = rank_at(&items, "browser", Mode::Apps, &Frecency::default(), NOW);
    assert_eq!(
        order(&items, &ranked),
        ["Firefox"],
        "keywords are searchable"
    );
    assert!(
        ranked[0].positions.is_empty(),
        "a keyword match has nothing to underline in the title"
    );
}

#[test]
fn a_starting_match_beats_a_scattered_one() {
    let items = corpus(&["Telegram", "GNU Image Manipulation Program"]);
    let ranked = rank_at(&items, "tgram", Mode::Apps, &Frecency::default(), NOW);
    assert_eq!(
        order(&items, &ranked),
        ["Telegram", "GNU Image Manipulation Program"]
    );
    assert_eq!(ranked[0].positions, [0, 4, 5, 6, 7]);
}

#[test]
fn frecency_breaks_a_tie() {
    let items = corpus(&["Alpha One", "Alpha Two"]);
    let mut frecency = Frecency::default();
    frecency.record("apps:Alpha Two", NOW);

    let ranked = rank_at(&items, "alpha", Mode::Apps, &frecency, NOW);
    assert_eq!(
        order(&items, &ranked),
        ["Alpha Two", "Alpha One"],
        "equal match scores fall to frecency"
    );
}

#[test]
fn frecency_does_not_override_a_clearly_better_match() {
    let items = corpus(&["Terminal", "Xterm Settings Manager"]);
    let mut frecency = Frecency::default();
    for _ in 0..50 {
        frecency.record("apps:Xterm Settings Manager", NOW);
    }

    let ranked = rank_at(&items, "term", Mode::Apps, &frecency, NOW);
    assert_eq!(
        order(&items, &ranked),
        ["Terminal", "Xterm Settings Manager"],
        "a word-boundary match stays ahead of a heavily used mid-word match"
    );
}

#[test]
fn empty_query_title_order_ignores_case() {
    let items = corpus(&["Zed", "ark", "beta"]);
    let ranked = rank_at(&items, "", Mode::Apps, &Frecency::default(), NOW);
    assert_eq!(order(&items, &ranked), ["ark", "beta", "Zed"]);
}

#[test]
fn empty_query_orders_by_frecency_then_title() {
    let items = corpus(&["Zed", "Alpha", "Midnight"]);

    let fresh = rank_at(&items, "", Mode::Apps, &Frecency::default(), NOW);
    assert_eq!(order(&items, &fresh), ["Alpha", "Midnight", "Zed"]);

    let mut frecency = Frecency::default();
    frecency.record("apps:Midnight", NOW);
    for _ in 0..3 {
        frecency.record("apps:Zed", NOW);
    }
    let used = rank_at(&items, "", Mode::Apps, &frecency, NOW);
    assert_eq!(order(&items, &used), ["Zed", "Midnight", "Alpha"]);
    assert!(
        used.iter().all(|ranked| ranked.positions.is_empty()),
        "nothing is underlined without a query"
    );
}

#[test]
fn clipboard_best_match_first_then_recency() {
    let items = vec![
        clipboard_item("clipboard:3", "the config editor is open"),
        clipboard_item("clipboard:2", "find the files here"),
        clipboard_item("clipboard:1", "release notes for the week"),
    ];

    let ranked = rank_at(&items, "fi", Mode::Clipboard, &Frecency::default(), NOW);
    assert_eq!(
        order(&items, &ranked),
        ["find the files here", "the config editor is open"],
        "the prefix match beats the mid-word match even though it is older"
    );

    // Frecency is not consulted, even for a heavily used entry.
    let mut frecency = Frecency::default();
    for _ in 0..100 {
        frecency.record("clipboard:3", NOW);
    }
    let used = rank_at(&items, "fi", Mode::Clipboard, &frecency, NOW);
    assert_eq!(order(&items, &used), order(&items, &ranked));
}

#[test]
fn clipboard_equal_scores_prefer_the_earlier_match() {
    // A prefix and a word-boundary match score the same. The newer entry matches
    // at 3, the older at 0, so the earlier match must win before recency.
    let items = vec![
        clipboard_item("clipboard:2", "xx git"),
        clipboard_item("clipboard:1", "git status"),
    ];

    let ranked = rank_at(&items, "git", Mode::Clipboard, &Frecency::default(), NOW);
    assert_eq!(
        ranked[0].score, ranked[1].score,
        "premise: same match score"
    );
    assert_eq!(
        order(&items, &ranked),
        ["git status", "xx git"],
        "equal scores: the earlier match wins before recency"
    );
}

#[test]
fn clipboard_equal_scores_and_positions_fall_back_to_recency() {
    // Structural twins: same score, both start at 0. Provider order is newest
    // first, so the newer entry wins the last tie.
    let items = vec![
        clipboard_item("clipboard:2", "git status"),
        clipboard_item("clipboard:1", "git rebase -i"),
    ];

    let ranked = rank_at(&items, "git", Mode::Clipboard, &Frecency::default(), NOW);
    assert_eq!(
        ranked[0].score, ranked[1].score,
        "premise: same match score"
    );
    assert_eq!(
        order(&items, &ranked),
        ["git status", "git rebase -i"],
        "equal scores and positions: the newer entry wins"
    );
}

#[test]
fn clipboard_empty_query_keeps_every_entry_in_provider_order() {
    let items = vec![
        clipboard_item("clipboard:3", "third"),
        clipboard_item("clipboard:2", "first"),
        clipboard_item("clipboard:1", "second"),
    ];
    let ranked = rank_at(&items, "", Mode::Clipboard, &Frecency::default(), NOW);
    assert_eq!(order(&items, &ranked), ["third", "first", "second"]);
}

#[test]
fn reusing_a_ranker_matches_one_shot_ranking() {
    let items = corpus(&["Firefox", "Files", "Profile Editor"]);
    let frecency = Frecency::default();
    let mut ranker = Ranker::new();

    for query in ["fi", "profile", "", "fi"] {
        let reused = ranker.rank_at(&items, query, Mode::Apps, &frecency, NOW);
        let one_shot = rank_at(&items, query, Mode::Apps, &frecency, NOW);
        assert_eq!(
            order(&items, &reused),
            order(&items, &one_shot),
            "a reused ranker must not leak the previous query ({query:?})"
        );
        assert_eq!(reused, one_shot);
    }
}

#[test]
fn rank_uses_the_process_clock() {
    let items = corpus(&["Alpha One", "Alpha Two"]);
    let mut frecency = Frecency::default();
    frecency.record("apps:Alpha Two", frecency::now_secs());

    let ranked = rank(&items, "", Mode::Apps, &frecency);
    assert_eq!(
        order(&items, &ranked),
        ["Alpha Two", "Alpha One"],
        "the convenience entry point still applies frecency"
    );
}

// ---------------------------------------------------------------- frecency

#[test]
fn score_halves_every_half_life() {
    let mut frecency = Frecency::default();
    frecency.record("apps:firefox.desktop", NOW);
    frecency.record("apps:firefox.desktop", NOW);

    assert!((frecency.score("apps:firefox.desktop", NOW) - 2.0).abs() < 1e-9);
    assert!((frecency.score("apps:firefox.desktop", NOW + HALF_LIFE_SECS) - 1.0).abs() < 1e-6);
    assert!((frecency.score("apps:firefox.desktop", NOW + 2 * HALF_LIFE_SECS) - 0.5).abs() < 1e-6);
}

#[test]
fn reading_a_score_does_not_write() {
    let mut frecency = Frecency::default();
    frecency.record("apps:firefox.desktop", NOW);
    frecency.record("apps:firefox.desktop", NOW);

    let _ = frecency.score("apps:firefox.desktop", NOW + 10 * HALF_LIFE_SECS);
    assert!(
        (frecency.score("apps:firefox.desktop", NOW) - 2.0).abs() < 1e-9,
        "decay on read must not mutate the stored score"
    );
    assert_eq!(frecency.entries["apps:firefox.desktop"].last_used, NOW);
}

#[test]
fn record_decays_before_adding() {
    let mut frecency = Frecency::default();
    frecency.record("apps:firefox.desktop", NOW);
    frecency.record("apps:firefox.desktop", NOW + HALF_LIFE_SECS);

    assert!(
        (frecency.score("apps:firefox.desktop", NOW + HALF_LIFE_SECS) - 1.5).abs() < 1e-6,
        "one use decays to 0.5, then the new use adds 1.0"
    );
    assert_eq!(
        frecency.entries["apps:firefox.desktop"].last_used,
        NOW + HALF_LIFE_SECS
    );
}

#[test]
fn unknown_zero_and_ancient_entries() {
    let mut frecency = Frecency::default();
    assert_eq!(frecency.score("apps:never-used", NOW), 0.0);

    frecency.entries.insert(
        "apps:zero".to_string(),
        Entry {
            score: 0.0,
            last_used: NOW,
        },
    );
    assert_eq!(frecency.score("apps:zero", NOW), 0.0);

    frecency.entries.insert(
        "apps:ancient".to_string(),
        Entry {
            score: 1.0,
            last_used: 0,
        },
    );
    let ancient = frecency.score("apps:ancient", 100 * HALF_LIFE_SECS);
    assert!(
        ancient > 0.0 && ancient < 1e-6,
        "ancient use is not gone but is tiny"
    );

    frecency.record("apps:never-used", NOW);
    assert!((frecency.score("apps:never-used", NOW) - 1.0).abs() < 1e-9);
    assert_eq!(frecency.entries["apps:never-used"].last_used, NOW);
}

#[test]
fn a_future_timestamp_does_not_decay() {
    let mut frecency = Frecency::default();
    frecency.record("apps:firefox.desktop", NOW + 5_000);
    assert!(
        (frecency.score("apps:firefox.desktop", NOW) - 1.0).abs() < 1e-9,
        "clock skew must not amplify or erase a score"
    );
}

#[test]
fn normalization_saturates_below_one() {
    assert_eq!(frecency::normalize(0.0), 0.0);
    assert_eq!(frecency::normalize(-3.0), 0.0);
    assert!((frecency::normalize(NORMALIZE_K) - 0.5).abs() < 1e-9);
    let saturated = frecency::normalize(1e9);
    assert!(saturated > 0.999 && saturated < 1.0);
}

// ---------------------------------------------------------------- state

fn temp_env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().expect("temp dir");
    let env = Env::for_test(dir.path());
    (dir, env)
}

fn read_state_json(env: &Env) -> serde_json::Value {
    let bytes = fs::read(env.state_file()).expect("state file exists");
    serde_json::from_slice(&bytes).expect("state file is valid JSON")
}

#[test]
fn state_round_trips_through_a_real_file() {
    let (_dir, env) = temp_env();
    let mut state = State::default();
    state.apps.record("apps:firefox.desktop", NOW);
    state.apps.record("apps:firefox.desktop", NOW);
    state.run.record("run:exec:code", NOW);
    state.save(&env).expect("save");

    let json = read_state_json(&env);
    assert_eq!(json["version"], STATE_VERSION);
    assert_eq!(json["modes"]["apps"]["apps:firefox.desktop"]["score"], 2.0);
    assert_eq!(json["modes"]["run"]["run:exec:code"]["score"], 1.0);
    assert_eq!(
        json["modes"]["apps"]["apps:firefox.desktop"]["last_used"],
        NOW
    );

    let loaded = State::load(&env);
    assert!((loaded.apps.score("apps:firefox.desktop", NOW) - 2.0).abs() < 1e-9);
    assert!((loaded.run.score("run:exec:code", NOW) - 1.0).abs() < 1e-9);
    assert_eq!(loaded.apps.entries.len(), 1);
    assert_eq!(loaded.run.entries.len(), 1);
}

#[test]
fn missing_state_file_loads_empty_without_a_warning() {
    let (_dir, env) = temp_env();
    let (state, warnings) = State::load_with_warnings(&env);
    assert!(state.apps.is_empty() && state.run.is_empty());
    assert!(warnings.is_empty(), "a first run is not an error");
}

#[test]
fn corrupt_state_file_warns_and_loads_empty() {
    let (_dir, env) = temp_env();
    let path = env.state_file();
    fs::create_dir_all(path.parent().expect("parent")).expect("create state dir");
    fs::write(&path, b"{ this is not json").expect("write");

    let (state, warnings) = State::load_with_warnings(&env);
    assert!(state.apps.is_empty() && state.run.is_empty());
    assert_eq!(warnings.len(), 1, "one warning: {warnings:?}");
    assert!(warnings[0].contains("corrupt"), "{}", warnings[0]);
}

#[test]
fn unknown_version_warns_and_loads_empty() {
    let (_dir, env) = temp_env();
    let path = env.state_file();
    fs::create_dir_all(path.parent().expect("parent")).expect("create state dir");
    fs::write(
        &path,
        br#"{"version": 99, "modes": {"apps": {"apps:x": {"score": 5.0, "last_used": 1}}}}"#,
    )
    .expect("write");

    let (state, warnings) = State::load_with_warnings(&env);
    assert!(
        state.apps.is_empty(),
        "an unknown version is not interpreted as v1"
    );
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("version"), "{}", warnings[0]);
}

#[test]
fn save_replaces_the_file_and_leaves_no_temp_file() {
    let (_dir, env) = temp_env();
    let mut first = State::default();
    first.apps.record("apps:first.desktop", NOW);
    first.save(&env).expect("first save");

    let mut second = State::default();
    second.apps.record("apps:second.desktop", NOW);
    second.save(&env).expect("second save");

    let json = read_state_json(&env);
    assert!(json["modes"]["apps"]["apps:second.desktop"].is_object());
    assert!(
        json["modes"]["apps"]["apps:first.desktop"].is_null(),
        "a save replaces the whole file"
    );
    let tmp = env.state_file().with_file_name("state.json.tmp");
    assert!(!tmp.exists(), "the temp file must be renamed away");
}

#[test]
fn concurrent_readers_never_see_a_partial_file() {
    let (_dir, env) = temp_env();
    let mut seed = State::default();
    seed.apps.record("apps:seed.desktop", NOW);
    seed.save(&env).expect("seed save");

    let stop = Arc::new(AtomicBool::new(false));
    let reader_path = env.state_file();
    let reader_stop = Arc::clone(&stop);
    let reader = thread::spawn(move || {
        let mut reads = 0u32;
        while !reader_stop.load(Ordering::Relaxed) {
            if let Ok(bytes) = fs::read(&reader_path) {
                let json: serde_json::Value =
                    serde_json::from_slice(&bytes).expect("reader saw a partial state file");
                assert_eq!(json["version"], STATE_VERSION);
                reads += 1;
            }
        }
        reads
    });

    for round in 0..50 {
        let mut state = State::default();
        for index in 0..=round {
            state
                .apps
                .record(&format!("apps:item{index}.desktop"), NOW + index);
        }
        state.save(&env).expect("save");
    }
    stop.store(true, Ordering::Relaxed);
    let reads = reader.join().expect("reader thread");
    assert!(reads > 0, "the reader never observed the state file");
}

#[test]
fn save_waits_for_the_advisory_lock() {
    use fs2::FileExt;

    let (_dir, env) = temp_env();
    let path = env.state_file();
    fs::create_dir_all(path.parent().expect("parent")).expect("create state dir");
    let lock_path = path.parent().expect("parent").join("state.json.lock");
    let held = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .expect("open lock");
    held.lock_exclusive().expect("hold lock");

    let mut state = State::default();
    state.apps.record("apps:firefox.desktop", NOW);
    let (done, finished) = mpsc::channel();
    let worker_env = env.clone();
    let worker = thread::spawn(move || {
        let saved = state.save(&worker_env);
        let _ = done.send(saved.map_err(|error| error.to_string()));
    });

    assert!(
        matches!(
            finished.recv_timeout(Duration::from_millis(300)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "save must block while another process holds the lock"
    );

    drop(held);
    let result = finished
        .recv_timeout(Duration::from_secs(10))
        .expect("save finished after the lock was released");
    worker.join().expect("worker thread");
    assert_eq!(result, Ok(()));
    assert!(read_state_json(&env)["modes"]["apps"]["apps:firefox.desktop"].is_object());
}

#[test]
fn save_creates_the_state_directory() {
    let (_dir, env) = temp_env();
    assert!(!env.state_file().parent().expect("parent").exists());

    let mut state = State::default();
    state.run.record("run:exec:code", NOW);
    state.save(&env).expect("save");

    assert!(env.state_file().is_file());
}

#[test]
fn clipboard_is_never_persisted() {
    let (_dir, env) = temp_env();
    let mut state = State::default();
    state.apps.record("apps:firefox.desktop", NOW);
    state.run.record("run:exec:code", NOW);
    assert!(
        state.frecency_mut(Mode::Clipboard).is_none(),
        "clipboard has no frecency slot"
    );
    state.save(&env).expect("save");

    let json = read_state_json(&env);
    let modes = json["modes"].as_object().expect("modes object");
    let mut names: Vec<&str> = modes.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, ["apps", "run"]);

    let raw = fs::read_to_string(env.state_file()).expect("read state");
    assert!(
        !raw.contains("clipboard"),
        "no clipboard ID or preview may reach the state file"
    );

    // A hand-written clipboard block is ignored rather than trusted.
    fs::write(
        env.state_file(),
        br#"{"version": 1, "modes": {"clipboard": {"clipboard:7": {"score": 9.0, "last_used": 1}}}}"#,
    )
    .expect("write");
    let (loaded, warnings) = State::load_with_warnings(&env);
    assert!(warnings.is_empty());
    assert!(loaded.apps.is_empty() && loaded.run.is_empty());
    assert!(loaded.frecency(Mode::Clipboard).is_empty());
}

#[test]
fn state_caps_entries_and_evicts_the_lowest_score() {
    let (_dir, env) = temp_env();
    let mut state = State::default();
    for index in 0..MAX_ENTRIES_PER_MODE + 5 {
        state.apps.entries.insert(
            format!("apps:item{index:05}.desktop"),
            Entry {
                score: index as f64 + 1.0,
                last_used: NOW,
            },
        );
    }
    state.save(&env).expect("save");

    let loaded = State::load(&env);
    assert_eq!(loaded.apps.entries.len(), MAX_ENTRIES_PER_MODE);
    for index in 0..5 {
        assert!(
            !loaded
                .apps
                .entries
                .contains_key(&format!("apps:item{index:05}.desktop")),
            "the lowest score is evicted"
        );
    }
    assert!(
        loaded
            .apps
            .entries
            .contains_key(&format!("apps:item{:05}.desktop", MAX_ENTRIES_PER_MODE + 4))
    );
}

#[test]
fn unknown_mode_names_in_the_file_are_ignored() {
    let (_dir, env) = temp_env();
    let path = env.state_file();
    fs::create_dir_all(path.parent().expect("parent")).expect("create state dir");
    fs::write(
        &path,
        br#"{"version": 1, "modes": {
            "apps": {"apps:firefox.desktop": {"score": 3.0, "last_used": 1000}},
            "windows": {"win:1": {"score": 4.0, "last_used": 1000}}
        }}"#,
    )
    .expect("write");

    let (state, warnings) = State::load_with_warnings(&env);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!((state.apps.score("apps:firefox.desktop", 1_000) - 3.0).abs() < 1e-9);
    assert!(state.run.is_empty());
}
