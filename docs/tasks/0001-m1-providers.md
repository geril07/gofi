# Task 0001 — M1 providers

- **Status:** in progress
- **Owner:** worker (DeepSeek)
- **Worktree:** `.worktrees/m1-providers`

## Goal

Implement the three providers and icon resolution in `gofi-core`, with behavior
tests. The CLI `gofi --dump` must return real items for apps, run and clipboard.

## Scope

Only these files:

- `crates/gofi-core/src/providers/apps.rs`
- `crates/gofi-core/src/providers/run.rs`
- `crates/gofi-core/src/providers/clipboard.rs`
- `crates/gofi-core/src/icons.rs`
- `crates/gofi-core/tests/providers.rs` (new)

Do not change `model.rs`, `xdg.rs`, `ranking/`, `store.rs`, `crates/gofi/`,
`Cargo.toml`, `docs/`, or `references/`. If a dependency is genuinely required,
stop and report instead of adding it.

## Acceptance

### Apps

- Discover `.desktop` files from `Env::applications_dirs()` in precedence order.
- Parse INI-style entries; only `[Desktop Entry]`, `Type=Application`.
- Skip `Hidden=true`, `NoDisplay=true`, `OnlyShowIn`/`NotShowIn` mismatches, and
  missing `TryExec`.
- Deduplicate by desktop file ID (relative path with `/` → `-`); the first data
  dir wins. A user `Hidden=true` suppresses the system entry.
- Parse `Exec` into argv without a shell: quoted args, backslash escapes, field
  codes `%f/%F/%u/%U` (drop when no files), `%i`, `%c`, `%k`, `%%`; unknown codes
  are a warning, not a panic.
- `Terminal=true` prepends `$TERMINAL` (fallback `kitty`).
- `Path=` becomes the action cwd if it exists.
- `Name`, `GenericName`, `Comment`, `Keywords` with locale fallback via
  `Env::locale_candidates()`; `search_text` is Name + GenericName + Keywords.
- Resolve `Icon=` to a file path via `icons.rs`; missing icon is fine.
- `ItemId` is `apps:<desktop-file-id>`; `kind=App`; action is `Launch`.
- One bad file is a warning, not a failure.

### Run

- Scan `Env::path` directories in order; files with any executable bit; follow
  symlinks; skip directories, names starting with `.`, and `gofi` itself.
- Deduplicate by file name; the first PATH directory wins.
- `ItemId` is `run:exec:<name>`; `kind=Executable`; action argv is `[name]`; no icon.
- Missing PATH dirs are warnings, not failures.

### Clipboard

- Run the configured `cliphist` binary (default `cliphist`) with `list`.
- Parse `id<TAB>preview`; skip empty lines; cap at 750 entries.
- `ItemId` is `clipboard:<id>`; title/search_text is the preview; `kind=Clipboard`;
  action is `CopyClipboard { id }`.
- Command failure returns `ProviderError` with a short message; never print the
  previews or decoded content.

### Icons

- Absolute `Icon=` path wins if it exists.
- Otherwise search `Env::icons_dirs()` for `{name}.png` / `{name}.svg` in
  `32x32`, `48x48`, `64x64`, `128x128`, `scalable`, then the root; return the first.
- No icon is `None`, never an error.

## Verification

Run from `.worktrees/m1-providers`:

```sh
cargo fmt --check
cargo clippy -p gofi-core --all-targets -- -D warnings
cargo test -p gofi-core
```

Save the full output to `target/evidence/m1-providers/check.log`. Add focused
tests with real temp dirs and fake executables/scripts; do not mock the filesystem
or `Command`. Test observable behavior, not private fields.

## Report

Return: summary, files changed, commands + results, evidence path, risks,
follow-ups. Do not edit docs or decisions. Do not spawn subagents. Do not change
the model.
