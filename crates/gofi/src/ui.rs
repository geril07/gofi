//! The Rune launcher window.
//!
//! Geometry and colours are measured from the owner's rofi (Catppuccin Macchiato,
//! JetBrains Mono 16, 32 px icons, seven 48 px rows, 50% × 495 window). The
//! compositor owns size and placement; this view fills whatever it is given.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::OnceLock;

use gofi_core::providers::{AppsProvider, ClipboardProvider, RunProvider};
use gofi_core::ranking::frecency::now_secs;
use gofi_core::ranking::{Ranked, Ranker};
use gofi_core::store::State;
use gofi_core::{Action, Env, Item, ItemId, ItemKind, Mode, Provider};
use rune::tok::TextStyle;
use rune::{
    App, AppSpec, Color, Cx, El, Image, ImageCache, Key, KeyEvent, Span, TextEdit, Tx, WheelEvent,
    WindowOptions, col, el, row, text, text_input,
};

const PANEL_W: f32 = 960.0;
const PANEL_H: f32 = 495.0;
/// The compositor fixes the height at 495 logical px, so the list viewport is
/// `495 - 10 - 59 - 20 - 10 = 396`.
const LIST_H: f32 = 396.0;
const TOP_H: f32 = 59.0;
const ROW_H: f32 = 48.0;
const ROW_GAP: f32 = 9.0;
const VISIBLE: usize = 7;
const TAB_W: f32 = 100.0;
const TAB_GAP: f32 = 20.0;
const ICON: f32 = 32.0;
const SCROLLBAR_W: f32 = 8.0;

/// rofi asks Pango for `JetBrainsMono 16`, i.e. 16 pt at 96 DPI = 21.33 px.
/// JetBrains Mono is embedded so startup does not pay a fontconfig lookup.
const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/JetBrainsMono-Medium.ttf"),
];

const BODY: TextStyle = TextStyle::new(21.333, 400.0).lh(26.0);
const TAB_RUN: TextStyle = TextStyle::new(17.0, 500.0).lh(20.0);

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

#[derive(Clone)]
pub struct Config {
    pub env: Env,
    pub mode: Mode,
}

static CONFIG: OnceLock<Config> = OnceLock::new();
pub fn set_config(config: Config) {
    let _ = CONFIG.set(config);
}

pub fn run() {
    let spec = AppSpec {
        fonts: FONTS,
        families: ("JetBrains Mono", "JetBrains Mono"),
        setup: Some(|state: &mut AppState, waker| state.setup_images(waker)),
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
        .on_focus(|_: &mut AppState, focused, tx| {
            if !focused {
                tx.close_window();
            }
        })
        .run();
}

fn init() -> AppState {
    let config = CONFIG.get().expect("ui::set_config runs before the app");
    AppState::new(config.env.clone(), config.mode)
}

pub struct AppState {
    env: Env,
    mode: Mode,
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
}

impl AppState {
    pub fn new(env: Env, mode: Mode) -> Self {
        let mut state = Self::empty(env, mode);
        state.load_mode(mode);
        state.rerank();
        state
    }

    fn empty(env: Env, mode: Mode) -> Self {
        let frecency = State::load(&env);
        let mut query = TextEdit::new("");
        query.focused = true;
        Self {
            env,
            mode,
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
        }
    }

    fn setup_images(&mut self, waker: rune::Waker) {
        self.cache = Some(ImageCache::new(waker));
    }

    fn load_mode(&mut self, mode: Mode) {
        if self.items.contains_key(&mode) {
            return;
        }
        let loaded = match mode {
            Mode::Apps => AppsProvider::new(self.env.clone()).load(),
            Mode::Run => RunProvider::new(self.env.clone()).load(),
            Mode::Clipboard => ClipboardProvider::new("cliphist").load(),
        };
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
        self.clamp_offset();
    }

    fn clamp_offset(&mut self) {
        let len = self.ranked.len();
        if len <= VISIBLE {
            self.offset = 0;
            return;
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        }
        if self.selected >= self.offset + VISIBLE {
            self.offset = self.selected + 1 - VISIBLE;
        }
        self.offset = self.offset.min(len - VISIBLE);
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.ranked.len();
        if len == 0 {
            return;
        }
        let len = len as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(len) as usize;
        self.clamp_offset();
    }

    fn set_mode(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
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
        self.set_mode(next);
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

    #[cfg(test)]
    fn for_test(env: Env, mode: Mode, items: Vec<Item>) -> Self {
        let mut state = Self::empty(env, mode);
        state.items.insert(mode, items);
        state.rerank();
        state
    }
}

fn view(state: &AppState, cx: &mut Cx<AppState>) -> El {
    let palette = Palette::get();
    let top = row()
        .h(TOP_H)
        .w_full()
        .gap(TAB_GAP)
        .child(search_field(state, palette))
        .child(tabs(state, palette));

    col()
        .w_full()
        .h_full()
        .fill(palette.mantle)
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
                .child(list_view(state, palette, cx.env().scale))
                .child(scrollbar(state, palette)),
        )
}

fn search_field(state: &AppState, palette: &Palette) -> El {
    let field = text_input(&state.query, BODY)
        .key("query")
        .single_line()
        .grow(1.0)
        .h(26.0)
        .px(16.0)
        .color(palette.text)
        .placeholder("Search...", palette.text)
        .on_edit(|state: &mut AppState| &mut state.query)
        .on_input_key(handle_key)
        .on_input_change(|state: &mut AppState, _tx| state.on_query_changed())
        .on_submit(|state: &mut AppState, tx| state.activate(state.selected, tx));

    row().h_full().grow(1.0).center_y().child(field)
}

fn handle_key(state: &mut AppState, event: &KeyEvent, tx: &mut Tx) -> bool {
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
        Key::Tab => {
            state.cycle_mode(if event.mods.shift { -1 } else { 1 });
            true
        }
        Key::Enter => {
            state.activate(state.selected, tx);
            true
        }
        Key::Command('g' | 'c' | '[') => {
            tx.close_window();
            true
        }
        _ => false,
    }
}

fn tabs(state: &AppState, palette: &Palette) -> El {
    let mut bar = row().h_full().gap(TAB_GAP);
    for mode in [Mode::Apps, Mode::Run, Mode::Clipboard] {
        bar = bar.child(tab_button(state, palette, mode));
    }
    bar
}

fn tab_button(state: &AppState, palette: &Palette, mode: Mode) -> El {
    let selected = state.mode == mode;
    let background = if selected { palette.blue } else { palette.base };
    let ink = if selected {
        palette.mantle
    } else {
        palette.text
    };

    el().key(format!("tab-{}", mode.as_str()))
        .w(TAB_W)
        .h(TOP_H)
        .radius(8.0)
        .fill(background)
        .center()
        .child(tab_glyph(mode, ink))
        .on_press_tx(move |state: &mut AppState, _tx| state.set_mode(mode))
}

fn tab_glyph(mode: Mode, ink: Color) -> El {
    match mode {
        Mode::Apps => magnifier(ink),
        Mode::Run => text("</>").font(TAB_RUN).color(ink),
        Mode::Clipboard => clipboard_glyph(24.0, ink),
    }
}

fn magnifier(ink: Color) -> El {
    el().w(24.0)
        .h(24.0)
        .child(
            el().absolute(3.0, 2.0)
                .w(14.0)
                .h(14.0)
                .radius(7.0)
                .border(2.0, ink),
        )
        .child(
            el().absolute(14.0, 15.0)
                .w(2.0)
                .h(9.0)
                .radius(1.0)
                .fill(ink)
                .rotate(0.785),
        )
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

fn list_view(state: &AppState, palette: &Palette, scale: f32) -> El {
    let mut list = col().key("list").w_full().grow(1.0).gap(ROW_GAP).on_wheel(
        |state: &mut AppState, event: WheelEvent, _tx| {
            if event.dy == 0.0 {
                return false;
            }
            // Wheel down (negative dy in Rune's content direction) moves down.
            state.move_selection(if event.dy > 0.0 { -1 } else { 1 });
            true
        },
    );

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
                selected: index == state.selected,
            },
        ));
    }
    list
}

struct RowSpec<'a> {
    slot: usize,
    index: usize,
    item: &'a Item,
    ranked: &'a Ranked,
    selected: bool,
}

fn row_view(state: &AppState, palette: &Palette, scale: f32, spec: RowSpec<'_>) -> El {
    let RowSpec {
        slot,
        index,
        item,
        ranked,
        selected,
    } = spec;
    let ink = if selected {
        palette.mantle
    } else {
        palette.text
    };
    let icon = icon_view(state, palette, scale, item, selected);
    let has_icon = icon.is_some();

    let mut row = row()
        .key(format!("row-{slot}"))
        .w_full()
        .h(ROW_H)
        .radius(8.0)
        .clip_rounded(8.0)
        .fill(if selected {
            palette.blue
        } else {
            Color::TRANSPARENT
        })
        .center_y()
        .gap(16.0)
        .px(if has_icon { 9.0 } else { 15.0 })
        .on_press_tx(move |state: &mut AppState, tx| state.activate(index, tx));

    if let Some(icon) = icon {
        row = row.child(icon);
    }
    row.child(title_text(item, &ranked.positions, ink))
}

fn icon_view(
    state: &AppState,
    palette: &Palette,
    scale: f32,
    item: &Item,
    selected: bool,
) -> Option<El> {
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
        ItemKind::Clipboard => Some(el().w(ICON).h(ICON).center().child(clipboard_glyph(
            20.0,
            if selected {
                palette.mantle
            } else {
                palette.overlay1
            },
        ))),
        ItemKind::Executable => None,
    }
}

fn title_text(item: &Item, positions: &[u32], ink: Color) -> El {
    let mut title = text(item.title.clone()).font(BODY).color(ink);
    let spans = underline_spans(&item.title, positions, ink);
    if !spans.is_empty() {
        title = title.spans(spans);
    }
    title
}

/// Merge the matcher's character indices into byte-range underline spans. The
/// matcher counts characters; Rune spans count UTF-8 bytes.
fn underline_spans(title: &str, positions: &[u32], ink: Color) -> Vec<Span> {
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
                spans.push(Span::new(start, end).underline(ink));
                run = None;
            }
            (false, None) => {}
        }
    }
    if let Some((start, end)) = run {
        spans.push(Span::new(start, end).underline(ink));
    }
    spans
}

fn scrollbar(state: &AppState, palette: &Palette) -> El {
    let total = state.ranked.len();
    let track = el()
        .key("scrollbar")
        .w(SCROLLBAR_W)
        .h_full()
        .radius(8.0)
        .fill(palette.base);

    if total <= VISIBLE {
        return track;
    }

    // rofi's handle: `track * visible / (total + visible)`.
    let handle = (LIST_H * VISIBLE as f32 / (total + VISIBLE) as f32).max(8.0);
    let remaining = (LIST_H - handle).max(0.0);
    let top = remaining * state.offset as f32 / (total - VISIBLE) as f32;
    let bottom = (remaining - top).max(0.0);

    track
        .child(el().w_full().grow(top.max(0.001)))
        .child(
            el().key("scrollbar-handle")
                .w(SCROLLBAR_W)
                .h(handle)
                .radius(8.0)
                .fill(palette.blue),
        )
        .child(el().w_full().grow(bottom.max(0.001)))
}

#[cfg(test)]
mod tests;
