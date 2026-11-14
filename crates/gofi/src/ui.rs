//! The Rune launcher window.
//!
//! The quiet skin: an 800×496 panel, Catppuccin Macchiato, JetBrains Mono,
//! 32 px icons, seven 48 px rows, a tinted selection pill and a scrollbar at
//! the list's edge. The compositor owns size and placement; this view fills
//! whatever it is given.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, channel};

use gofi_core::providers::{AppsProvider, ClipboardProvider, RunProvider};
use gofi_core::ranking::frecency::now_secs;
use gofi_core::ranking::{Ranked, Ranker};
use gofi_core::store::State;
use gofi_core::{Action, Env, Item, ItemId, ItemKind, Loaded, Mode, Provider, ProviderError};
use rune::tok::TextStyle;
use rune::{
    App, AppSpec, Color, Cx, El, Image, ImageCache, Key, KeyEvent, Span, TextEdit, Tx, WheelEvent,
    WindowOptions, col, el, row, text, text_input,
};

mod motion;

use motion::{Motion, SELECT, TAB, mode_transition};

const PANEL_W: f32 = 800.0;
const PANEL_H: f32 = 496.0;
/// The compositor fixes the height at 496 logical px, so the list viewport is
/// `496 - 10 - 59 - 20 - 10 = 397`.
const LIST_H: f32 = 397.0;
const TOP_H: f32 = 59.0;
const ROW_H: f32 = 48.0;
const ROW_GAP: f32 = 9.0;
const VISIBLE: usize = 7;
/// Tabs shrink-wrap their labels; the gap is the only space between them.
const TAB_GAP: f32 = 18.0;
const TOP_GAP: f32 = 20.0;
/// Six pixels under the text box: the underline is out of flow, so it never moves it.
const TAB_UNDERLINE_TOP: f32 = 45.5;
const TAB_UNDERLINE_H: f32 = 2.0;
const PANEL_RADIUS: f32 = 12.0;
const ROW_RADIUS: f32 = 8.0;
/// A tinted selection instead of rofi's filled bar.
const SELECTION_ALPHA: f32 = 0.16;
const ICON: f32 = 32.0;
const SCROLLBAR_W: f32 = 8.0;
/// Rofi/Pango draw matched text with a 2 px underline at 1x; Rune's default is 1 px.
const MATCH_UNDERLINE_W: f32 = 2.0;

/// rofi asks Pango for `JetBrainsMono 16`, i.e. 16 pt at 96 DPI = 21.33 px.
/// JetBrains Mono is embedded so startup does not pay a fontconfig lookup.
const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/JetBrainsMono-Medium.ttf"),
    include_bytes!("../assets/JetBrainsMono-Bold.ttf"),
];

const BODY: TextStyle = TextStyle::new(21.333, 400.0).lh(26.0);
const TAB_TEXT: TextStyle = TextStyle::new(15.0, 500.0).lh(20.0);

/// Catppuccin Macchiato, converted from sRGB to the linear space Rune blends in.
struct Palette {
    mantle: Color,
    base: Color,
    text: Color,
    blue: Color,
    overlay1: Color,
}

impl Palette {
    fn get() -> &'static Palette {
        static PALETTE: OnceLock<Palette> = OnceLock::new();
        PALETTE.get_or_init(|| Palette {
            mantle: Color {
                r: 0.012983,
                g: 0.0144438,
                b: 0.0295568,
                a: 1.0,
            },
            base: Color {
                r: 0.017642,
                g: 0.0202886,
                b: 0.0423114,
                a: 1.0,
            },
            text: Color {
                r: 0.590619,
                g: 0.651406,
                b: 0.913099,
                a: 1.0,
            },
            blue: Color {
                r: 0.254152,
                g: 0.417885,
                b: 0.904661,
                a: 1.0,
            },
            overlay1: Color {
                r: 0.215861,
                g: 0.242281,
                b: 0.361307,
                a: 1.0,
            },
        })
    }
}

pub struct Config {
    pub env: Env,
    pub mode: Mode,
}

/// Items loaded off the UI thread, waiting for the first frame.
struct Preloaded {
    state: State,
    loaded: Result<Loaded, ProviderError>,
}

static CONFIG: OnceLock<Config> = OnceLock::new();
pub fn set_config(config: Config) {
    let _ = CONFIG.set(config);
}

pub fn run() {
    crate::trace("ui::run");
    let spec = AppSpec {
        fonts: FONTS,
        families: ("JetBrains Mono", "JetBrains Mono"),
        setup: Some(|state: &mut AppState, waker| state.setup_images(waker)),
        poll: Some(|state, _now, _tx| state.poll_preload()),
        ..AppSpec::new("gofi", (PANEL_W, PANEL_H), init, view)
    };
    App::new(spec)
        .configured_options(
            WindowOptions {
                transparent: true,
                borderless: true,
                resizable: false,
                min_size: Some((0.0, 0.0)),
                ..Default::default()
            }
            .into(),
        )
        .on_ready(|_| crate::trace("on_ready"))
        .on_focus(|_: &mut AppState, focused, tx| {
            if !focused {
                tx.close_window();
            }
        })
        .run();
}

fn init() -> AppState {
    crate::trace("init");
    let config = CONFIG.get().expect("ui::set_config runs before the app");
    let env = config.env.clone();
    let mode = config.mode;
    let state = AppState::loading(env.clone(), mode, preload(env, mode));
    crate::trace("init_done");
    state
}

/// Loads the selected mode on a thread as early as possible. `poll_preload`
/// takes the result on the first frame; if it is not ready yet, it waits there,
/// after the window and GPU have been set up, instead of in `init`.
fn preload(env: Env, mode: Mode) -> Receiver<Preloaded> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        crate::trace("preload_start");
        let state = State::load(&env);
        let loaded = load_items(&env, mode);
        let _ = tx.send(Preloaded { state, loaded });
        crate::trace("preload_ready");
    });
    rx
}

fn load_items(env: &Env, mode: Mode) -> Result<Loaded, ProviderError> {
    match mode {
        Mode::Apps => AppsProvider::new(env.clone()).load(),
        Mode::Run => RunProvider::new(env.clone()).load(),
        Mode::Clipboard => ClipboardProvider::new("cliphist").load(),
    }
}

pub struct AppState {
    env: Env,
    mode: Mode,
    /// Which way the last mode switch travelled: `1.0` forward, `-1.0` back.
    mode_dir: f32,
    query: TextEdit,
    selected: usize,
    offset: usize,
    items: HashMap<Mode, Vec<Item>>,
    ranked: Vec<Ranked>,
    frecency: State,
    icons: RefCell<HashMap<(PathBuf, u32), Image>>,
    cache: Option<ImageCache>,
    notice: Option<String>,
    ranker: Ranker,
    preload: Option<Receiver<Preloaded>>,
}

impl AppState {
    fn loading(env: Env, mode: Mode, preload: Receiver<Preloaded>) -> Self {
        let mut state = Self::empty(env, mode, State::default());
        state.preload = Some(preload);
        state
    }

    fn empty(env: Env, mode: Mode, frecency: State) -> Self {
        let mut query = TextEdit::new("");
        query.focused = true;
        Self {
            env,
            mode,
            mode_dir: 1.0,
            query,
            selected: 0,
            offset: 0,
            items: HashMap::new(),
            ranked: Vec::new(),
            frecency,
            icons: RefCell::new(HashMap::new()),
            cache: None,
            notice: None,
            ranker: Ranker::new(),
            preload: None,
        }
    }

    fn setup_images(&mut self, waker: rune::Waker) {
        self.cache = Some(ImageCache::new(waker));
    }

    fn load_mode(&mut self, mode: Mode) {
        if self.items.contains_key(&mode) {
            return;
        }
        let loaded = load_items(&self.env, mode);
        self.apply_loaded(mode, loaded);
    }

    fn apply_loaded(&mut self, mode: Mode, loaded: Result<Loaded, ProviderError>) {
        match loaded {
            Ok(loaded) => {
                for warning in &loaded.warnings {
                    eprintln!("gofi: warning: {warning}");
                }
                self.items.insert(mode, loaded.items);
            }
            Err(error) => {
                eprintln!("gofi: {error}");
                self.notice = Some(error.to_string());
                self.items.insert(mode, Vec::new());
            }
        }
    }

    /// Takes the background load on the first frame, so the scan runs while
    /// Rune creates the window and the GPU. Returns whether state changed.
    fn poll_preload(&mut self) -> bool {
        let Some(preload) = self.preload.take() else {
            return false;
        };
        crate::trace("preload_wait");
        match preload.recv() {
            Ok(preloaded) => {
                self.frecency = preloaded.state;
                self.apply_loaded(self.mode, preloaded.loaded);
            }
            // The loader panicked; load here so the launcher still works.
            Err(_) => self.load_mode(self.mode),
        }
        self.rerank();
        crate::trace("preload_done");
        true
    }

    fn rerank(&mut self) {
        let items = self.items.get(&self.mode).map(Vec::as_slice).unwrap_or(&[]);
        self.ranked = self.ranker.rank(
            items,
            &self.query.text,
            self.mode,
            self.frecency.frecency(self.mode),
        );
        if self.selected >= self.ranked.len() {
            self.selected = self.ranked.len().saturating_sub(1);
        }
        self.offset = scroll_offset(self.selected, self.ranked.len());
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.ranked.len();
        if len == 0 {
            return;
        }
        let len = len as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(len) as usize;
        self.offset = scroll_offset(self.selected, self.ranked.len());
    }

    fn select(&mut self, index: usize) {
        if self.ranked.is_empty() {
            return;
        }
        self.selected = index.min(self.ranked.len() - 1);
        self.offset = scroll_offset(self.selected, self.ranked.len());
    }

    fn set_mode(&mut self, mode: Mode, direction: f32) {
        if self.mode == mode {
            return;
        }
        self.mode_dir = if direction < 0.0 { -1.0 } else { 1.0 };
        self.mode = mode;
        self.query.clear();
        self.query.focused = true;
        self.notice = None;
        self.load_mode(mode);
        self.selected = 0;
        self.offset = 0;
        self.rerank();
    }

    fn cycle_mode(&mut self, direction: i32) {
        let next = match (self.mode, direction.signum()) {
            (Mode::Apps, 1) => Mode::Run,
            (Mode::Run, 1) => Mode::Clipboard,
            (Mode::Clipboard, 1) => Mode::Apps,
            (Mode::Apps, -1) => Mode::Clipboard,
            (Mode::Run, -1) => Mode::Apps,
            (Mode::Clipboard, -1) => Mode::Run,
            (_, _) => self.mode,
        };
        self.set_mode(next, if direction < 0 { -1.0 } else { 1.0 });
    }

    fn on_query_changed(&mut self) {
        self.selected = 0;
        self.offset = 0;
        self.notice = None;
        self.rerank();
    }

    fn activate(&mut self, index: usize, tx: &mut Tx) {
        let Some(ranked) = self.ranked.get(index) else {
            return;
        };
        let Some(item) = self
            .items
            .get(&self.mode)
            .and_then(|items| items.get(ranked.index))
        else {
            return;
        };
        let id = item.id.clone();
        let action = item.action.clone();

        match action {
            Action::Launch { argv, cwd } => match crate::launch::spawn(&argv, cwd.as_deref()) {
                Ok(()) => {
                    self.record_use(&id);
                    tx.close_window();
                }
                Err(error) => self.notice = Some(error),
            },
            Action::CopyClipboard { id } => {
                let result = crate::launch::copy_clipboard(
                    OsStr::new("cliphist"),
                    &id,
                    OsStr::new("wl-copy"),
                );
                match result {
                    Ok(()) => tx.close_window(),
                    Err(error) => self.notice = Some(error),
                }
            }
        }
    }

    fn record_use(&mut self, id: &ItemId) {
        let now = now_secs();
        if let Some(frecency) = self.frecency.frecency_mut(self.mode) {
            frecency.record(id.as_str(), now);
        }
        if let Err(error) = self.frecency.save(&self.env) {
            eprintln!("gofi: warning: {error}");
        }
    }

    /// Map rofi's line-editing keys onto the query. `TextEdit` is the source of
    /// truth for the field, so mutating it here and reranking is enough.
    fn clear_query(&mut self) {
        if self.query.text.is_empty() {
            return;
        }
        self.query.replace_text("");
        self.on_query_changed();
    }

    fn delete_to_start(&mut self) {
        let (anchor, focus) = self.query.range();
        if anchor != focus {
            self.replace_query_range(anchor, focus);
        } else {
            self.replace_query_range(0, focus);
        }
    }

    fn delete_to_end(&mut self) {
        let (anchor, focus) = self.query.range();
        if anchor != focus {
            self.replace_query_range(anchor, focus);
        } else {
            self.replace_query_range(focus, self.query.text.len());
        }
    }

    fn delete_backward(&mut self) {
        let (anchor, focus) = self.query.range();
        if anchor != focus {
            self.replace_query_range(anchor, focus);
        } else if focus > 0 {
            let start = self.query.text[..focus]
                .chars()
                .next_back()
                .map_or(focus, |ch| focus - ch.len_utf8());
            self.replace_query_range(start, focus);
        }
    }

    fn delete_forward(&mut self) {
        let (anchor, focus) = self.query.range();
        if anchor != focus {
            self.replace_query_range(anchor, focus);
        } else {
            let end = self.query.text[focus..]
                .chars()
                .next()
                .map_or(focus, |ch| focus + ch.len_utf8());
            self.replace_query_range(focus, end);
        }
    }

    fn replace_query_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        self.query.text.replace_range(start..end, "");
        self.query.set_caret(start);
        self.on_query_changed();
    }

    fn move_caret(&mut self, delta: isize) {
        let focus = self.query.focus;
        let to = if delta < 0 {
            self.query.text[..focus]
                .chars()
                .next_back()
                .map_or(focus, |ch| focus - ch.len_utf8())
        } else {
            self.query.text[focus..]
                .chars()
                .next()
                .map_or(focus, |ch| focus + ch.len_utf8())
        };
        self.query.set_caret(to);
    }

    #[cfg(test)]
    fn for_test(env: Env, mode: Mode, items: Vec<Item>) -> Self {
        let mut state = Self::empty(env, mode, State::default());
        state.items.insert(mode, items);
        state.rerank();
        state
    }
}

/// Rofi's `scroll-method: 1`: the selected row walks down to the middle of the
/// seven rows, then the list follows it; near the end the list stops at a full
/// page so the selection moves to the bottom. A short list keeps a gap below.
fn scroll_offset(selected: usize, total: usize) -> usize {
    const MIDDLE: usize = (VISIBLE - 1) / 2;
    if selected <= MIDDLE || total <= VISIBLE {
        return 0;
    }
    if selected < total.saturating_sub(VISIBLE - MIDDLE) {
        selected - MIDDLE
    } else {
        total - VISIBLE
    }
}

fn view(state: &AppState, cx: &mut Cx<AppState>) -> El {
    let palette = Palette::get();
    let motion = Motion::get();
    let top = row()
        .h(TOP_H)
        .w_full()
        .gap(TOP_GAP)
        .child(search_field(state, palette))
        .child(tabs(state, palette, motion));

    let mut root = col()
        .w_full()
        .h_full()
        .fill(palette.mantle)
        .radius(PANEL_RADIUS)
        .border(1.0, palette.overlay1.alpha(0.35))
        .pt(10.0)
        .pb(10.0)
        .px(20.0)
        .child(top)
        .child(
            row()
                .mt(20.0)
                .w_full()
                .grow(1.0)
                .gap(10.0)
                .child(list_view(state, palette, cx.env().scale, motion))
                .child(scrollbar(state, palette)),
        );

    // The off switch also lands every default layout spring inside the panel.
    if !motion.enabled() {
        root = root.instant_changes();
    }
    root
}

fn search_field(state: &AppState, palette: &Palette) -> El {
    let field = text_input(&state.query, BODY)
        .key("query")
        .single_line()
        // Static width: a long query scrolls inside the field instead of
        // widening it and pushing the tabs off the panel.
        .shrink()
        .grow(1.0)
        .h(26.0)
        .px(16.0)
        .color(palette.text)
        .placeholder("Search...", palette.overlay1)
        .on_edit(|state: &mut AppState| &mut state.query)
        .on_input_key(handle_key)
        .on_input_change(|state: &mut AppState, _tx| state.on_query_changed())
        .on_submit(|state: &mut AppState, tx| state.activate(state.selected, tx));

    row().h_full().grow(1.0).shrink().center_y().child(field)
}

fn handle_key(state: &mut AppState, event: &KeyEvent, tx: &mut Tx) -> bool {
    // Typing must land at once; moving the selection and switching modes are
    // the motion layer's business. With motion off everything lands at once.
    let animated = match &event.key {
        Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::Home | Key::End | Key::Tab => true,
        Key::Command('p' | 'n') => true,
        Key::Left | Key::Right => event.mods.shift,
        _ => false,
    };
    if !animated || !Motion::get().enabled() {
        tx.instant();
    }
    match &event.key {
        Key::Escape => {
            tx.close_window();
            true
        }
        Key::Up => {
            state.move_selection(-1);
            true
        }
        Key::Down => {
            state.move_selection(1);
            true
        }
        // rofi's default row bindings and paging.
        Key::Command('p') => {
            state.move_selection(-1);
            true
        }
        Key::Command('n') => {
            state.move_selection(1);
            true
        }
        Key::PageUp => {
            state.move_selection(-(VISIBLE as isize));
            true
        }
        Key::PageDown => {
            state.move_selection(VISIBLE as isize);
            true
        }
        Key::Home => {
            state.select(0);
            true
        }
        Key::End => {
            state.select(state.ranked.len().saturating_sub(1));
            true
        }
        Key::Tab => {
            state.cycle_mode(if event.mods.shift { -1 } else { 1 });
            true
        }
        // rofi's kb-mode-next / kb-mode-previous on Shift+Right / Shift+Left.
        Key::Right if event.mods.shift => {
            state.cycle_mode(1);
            true
        }
        Key::Left if event.mods.shift => {
            state.cycle_mode(-1);
            true
        }
        Key::Enter | Key::Command('j' | 'm') => {
            state.activate(state.selected, tx);
            true
        }
        Key::Command('g' | 'c' | '[') => {
            tx.close_window();
            true
        }
        // rofi's line editing: Ctrl+A/E, B/F, K/U/W.
        Key::Command('a') => {
            state.query.set_caret(0);
            true
        }
        Key::Command('e') => {
            let end = state.query.text.len();
            state.query.set_caret(end);
            true
        }
        Key::Command('b') => {
            state.move_caret(-1);
            true
        }
        Key::Command('f') => {
            state.move_caret(1);
            true
        }
        Key::Command('k') => {
            state.delete_to_end();
            true
        }
        Key::Command('d') => {
            state.delete_forward();
            true
        }
        Key::Command('h') => {
            state.delete_backward();
            true
        }
        Key::Command('u') => {
            state.delete_to_start();
            true
        }
        Key::Command('w') => {
            state.clear_query();
            true
        }
        _ => false,
    }
}

fn tabs(state: &AppState, palette: &Palette, motion: &Motion) -> El {
    let mut bar = row().h_full().gap(TAB_GAP);
    for mode in [Mode::Apps, Mode::Run, Mode::Clipboard] {
        bar = bar.child(tab_button(state, palette, mode, motion));
    }
    bar
}

fn tab_button(state: &AppState, palette: &Palette, mode: Mode, motion: &Motion) -> El {
    let selected = state.mode == mode;
    let ink = if selected {
        palette.text
    } else {
        palette.overlay1
    };

    let mut tab = col()
        .key(format!("tab-{}", mode.as_str()))
        .h(TOP_H)
        .center()
        .child(
            text(tab_label(mode))
                .key(format!("tab-{}-label", mode.as_str()))
                .font(TAB_TEXT)
                .color(ink),
        );

    if selected {
        // Out of flow so selecting a tab does not shift its label; width follows
        // the label, which also gives the shared-element flight its new size.
        let mut indicator = el()
            .key("tab-indicator")
            .absolute(0.0, TAB_UNDERLINE_TOP)
            .w_full()
            .h(TAB_UNDERLINE_H)
            .radius(1.0)
            .fill(palette.blue);
        if motion.enabled() {
            indicator = indicator
                .matched("tab-indicator")
                .motion(motion.spring(TAB));
        }
        tab = tab.child(indicator);
    }

    tab.on_press_tx(move |state: &mut AppState, _tx| {
        state.set_mode(mode, mode_direction(state.mode, mode))
    })
}

fn mode_rank(mode: Mode) -> u8 {
    match mode {
        Mode::Apps => 0,
        Mode::Run => 1,
        Mode::Clipboard => 2,
    }
}

fn mode_direction(from: Mode, to: Mode) -> f32 {
    if mode_rank(to) >= mode_rank(from) {
        1.0
    } else {
        -1.0
    }
}

fn tab_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Apps => "Apps",
        Mode::Run => "Run",
        Mode::Clipboard => "Clipboard",
    }
}

fn clipboard_glyph(size: f32, ink: Color) -> El {
    let scale = size / 24.0;
    el().w(size)
        .h(size)
        .child(
            el().absolute(4.0 * scale, 3.0 * scale)
                .w(16.0 * scale)
                .h(18.0 * scale)
                .radius(3.0 * scale)
                .border(2.0 * scale, ink),
        )
        .child(
            el().absolute(8.0 * scale, 1.0 * scale)
                .w(8.0 * scale)
                .h(5.0 * scale)
                .radius(2.0 * scale)
                .fill(ink),
        )
}

fn list_view(state: &AppState, palette: &Palette, scale: f32, motion: &Motion) -> El {
    let mut list = col()
        .key(format!("list-{}", state.mode.as_str()))
        .transition(motion.transition(mode_transition(state.mode_dir)))
        .w_full()
        .grow(1.0)
        .shrink()
        .gap(ROW_GAP)
        .on_wheel(|state: &mut AppState, event: WheelEvent, _tx| {
            if event.dy == 0.0 {
                return false;
            }
            // Wheel down (negative dy in Rune's content direction) moves down.
            state.move_selection(if event.dy > 0.0 { -1 } else { 1 });
            true
        });

    if state.ranked.is_empty() {
        if let Some(notice) = &state.notice {
            list = list.child(
                text(notice.clone())
                    .font(BODY)
                    .color(palette.overlay1)
                    .px(15.0)
                    .mt(8.0),
            );
        }
        return list;
    }

    let slot = state.selected.saturating_sub(state.offset);
    if slot < VISIBLE {
        list = list.child(selection_pill(palette, slot, motion));
    }

    let end = (state.offset + VISIBLE).min(state.ranked.len());
    for (slot, index) in (state.offset..end).enumerate() {
        let ranked = &state.ranked[index];
        let item = &state.items[&state.mode][ranked.index];
        list = list.child(row_view(
            state,
            palette,
            scale,
            RowSpec {
                slot,
                index,
                item,
                ranked,
            },
        ));
    }
    list
}

fn selection_pill(palette: &Palette, slot: usize, motion: &Motion) -> El {
    el().key("selection")
        .absolute(0.0, slot as f32 * (ROW_H + ROW_GAP))
        .motion(motion.spring(SELECT))
        .w_full()
        .h(ROW_H)
        .radius(ROW_RADIUS)
        .fill(palette.blue.alpha(SELECTION_ALPHA))
}

struct RowSpec<'a> {
    slot: usize,
    index: usize,
    item: &'a Item,
    ranked: &'a Ranked,
}

fn row_view(state: &AppState, palette: &Palette, scale: f32, spec: RowSpec<'_>) -> El {
    let RowSpec {
        slot,
        index,
        item,
        ranked,
    } = spec;
    let icon = icon_view(state, palette, scale, item);
    let has_icon = icon.is_some();

    let mut row = row()
        .key(format!("row-{slot}"))
        .w_full()
        .h(ROW_H)
        .center_y()
        .gap(16.0)
        .px(if has_icon { 9.0 } else { 15.0 })
        .on_press_tx(move |state: &mut AppState, tx| state.activate(index, tx));

    if let Some(icon) = icon {
        row = row.child(icon);
    }
    row.child(title_text(item, &ranked.positions, palette.text))
}

fn icon_view(state: &AppState, palette: &Palette, scale: f32, item: &Item) -> Option<El> {
    match item.kind {
        ItemKind::App => {
            let Some(path) = &item.icon else {
                // Keep every app row's text aligned even when the icon is missing.
                return Some(el().w(ICON).h(ICON));
            };
            let Some(cache) = &state.cache else {
                return Some(el().w(ICON).h(ICON));
            };
            let image = state
                .icons
                .borrow_mut()
                .entry((path.clone(), scale.to_bits()))
                .or_insert_with(|| cache.file(path, ICON, scale))
                .clone();
            Some(el().w(ICON).h(ICON).pixels(image.pixels()))
        }
        ItemKind::Clipboard => Some(
            el().w(ICON)
                .h(ICON)
                .center()
                .child(clipboard_glyph(20.0, palette.overlay1)),
        ),
        ItemKind::Executable => None,
    }
}

fn title_text(item: &Item, positions: &[u32], ink: Color) -> El {
    let mut title = text(item.title.clone()).font(BODY).color(ink).truncate();
    let spans = match_spans(&item.title, positions, ink);
    if !spans.is_empty() {
        title = title.spans(spans);
    }
    title
}

/// Merge the matcher's character indices into byte-range spans. Rofi's default
/// highlight is bold plus a single underline (`ROFI_HL_BOLD | ROFI_HL_UNDERLINE`);
/// the matcher counts characters while Rune spans count UTF-8 bytes.
fn match_spans(title: &str, positions: &[u32], ink: Color) -> Vec<Span> {
    if positions.is_empty() {
        return Vec::new();
    }
    let mut spans = Vec::new();
    let mut next = positions.iter().copied().peekable();
    let mut run: Option<(usize, usize)> = None;

    for (char_index, (byte, ch)) in title.char_indices().enumerate() {
        let matched = next.peek() == Some(&(char_index as u32));
        if matched {
            next.next();
        }
        let end = byte + ch.len_utf8();
        match (matched, run) {
            (true, Some((start, _))) => run = Some((start, end)),
            (true, None) => run = Some((byte, end)),
            (false, Some((start, end))) => {
                spans.push(highlighted(start, end, ink));
                run = None;
            }
            (false, None) => {}
        }
    }
    if let Some((start, end)) = run {
        spans.push(highlighted(start, end, ink));
    }
    spans
}

fn highlighted(start: usize, end: usize, ink: Color) -> Span {
    Span::new(start, end)
        .weight(700.0)
        .underline(ink)
        .underline_width(MATCH_UNDERLINE_W)
}

fn scrollbar(state: &AppState, palette: &Palette) -> El {
    let total = state.ranked.len();
    let track = el()
        .key("scrollbar")
        .w(SCROLLBAR_W)
        .h_full()
        .radius(8.0)
        .fill(palette.base);

    if total == 0 {
        return track;
    }

    // rofi's handle length, and a position that follows the selected row: the
    // highlight is the state a launcher's scrollbar shows.
    let handle = (LIST_H * VISIBLE as f32 / (total + VISIBLE) as f32).max(8.0);
    let remaining = (LIST_H - handle).max(0.0);
    let top = if total > 1 {
        (remaining * state.selected as f32 / (total - 1) as f32).min(remaining)
    } else {
        0.0
    };

    track.child(
        el().key("scrollbar-handle")
            .absolute(0.0, top)
            .w(SCROLLBAR_W)
            .h(handle)
            .radius(8.0)
            .fill(palette.blue),
    )
}

#[cfg(test)]
mod tests;
