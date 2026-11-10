//! gofi CLI. M1 exposes `--dump`; the Rune UI arrives in M2.

use std::process::ExitCode;

use gofi_core::providers::{AppsProvider, ClipboardProvider, RunProvider};
use gofi_core::ranking::rank;
use gofi_core::store::State;
use gofi_core::{Env, ItemKind, Mode, Provider};

#[derive(Debug)]
struct Args {
    dump: bool,
    mode: Mode,
    query: String,
    limit: usize,
    help: bool,
    version: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            dump: false,
            mode: Mode::Apps,
            query: String::new(),
            limit: 50,
            help: false,
            version: false,
        }
    }
}

impl Args {
    fn parse(argv: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut args = Args::default();
        let mut argv = argv.into_iter();
        while let Some(arg) = argv.next() {
            match arg.as_str() {
                "--dump" => args.dump = true,
                "--help" | "-h" => args.help = true,
                "--version" | "-V" => args.version = true,
                "--mode" | "-m" => {
                    let value = argv.next().ok_or("--mode needs a value")?;
                    args.mode =
                        Mode::parse(&value).ok_or_else(|| format!("unknown mode: {value}"))?;
                }
                "--query" | "-q" => {
                    args.query = argv.next().ok_or("--query needs a value")?;
                }
                "--limit" | "-n" => {
                    let value = argv.next().ok_or("--limit needs a value")?;
                    args.limit = value
                        .parse()
                        .map_err(|_| format!("invalid limit: {value}"))?;
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        Ok(args)
    }
}

#[derive(serde::Serialize)]
struct DumpItem {
    id: String,
    title: String,
    subtitle: Option<String>,
    kind: ItemKind,
    score: i64,
    positions: Vec<u32>,
    icon: Option<String>,
}

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("gofi: {error}");
            eprintln!("Try `gofi --help`.");
            return ExitCode::from(2);
        }
    };

    if args.help {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.version {
        println!("gofi {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if !args.dump {
        eprintln!("gofi: the UI is not implemented yet (M2). Use `gofi --dump`.");
        return ExitCode::from(2);
    }

    let env = Env::from_process();
    let loaded = match args.mode {
        Mode::Apps => AppsProvider::new(env.clone()).load(),
        Mode::Run => RunProvider::new(env.clone()).load(),
        Mode::Clipboard => ClipboardProvider::new("cliphist").load(),
    };
    let loaded = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("gofi: {}", error);
            return ExitCode::FAILURE;
        }
    };
    for warning in &loaded.warnings {
        eprintln!("gofi: warning: {warning}");
    }

    let state = State::load(&env);
    let ranked = rank(
        &loaded.items,
        &args.query,
        args.mode,
        state.frecency(args.mode),
    );
    let out: Vec<DumpItem> = ranked
        .iter()
        .take(args.limit)
        .map(|ranked| {
            let item = &loaded.items[ranked.index];
            DumpItem {
                id: item.id.0.clone(),
                title: item.title.clone(),
                subtitle: item.subtitle.clone(),
                kind: item.kind,
                score: ranked.score,
                positions: ranked.positions.clone(),
                icon: item.icon.as_ref().map(|p| p.to_string_lossy().into_owned()),
            }
        })
        .collect();

    match serde_json::to_string_pretty(&out) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gofi: could not serialize results: {error}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "\
gofi {version}

Usage:
  gofi [--dump] [--mode apps|run|clipboard] [--query TEXT] [--limit N]

M1 exposes the headless dump; the Rune UI arrives in M2.

Options:
  --dump              print ranked items as JSON and exit
  -m, --mode MODE     apps (default), run, or clipboard
  -q, --query TEXT    fuzzy query (default: empty)
  -n, --limit N       maximum results (default: 50)
  -h, --help          show this help
  -V, --version       show the version",
        version = env!("CARGO_PKG_VERSION")
    );
}
