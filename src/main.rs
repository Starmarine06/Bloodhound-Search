mod args;
mod detect;
mod engine;
mod index;
mod printer;
mod scan;
mod stats;
mod walker;

use std::io::{IsTerminal, Write};
use std::path::Path;
use std::sync::atomic::Ordering;

use clap::{CommandFactory, Parser};
use regex::RegexBuilder;

use args::Cli;
use engine::Engine;
use index::Index;
use printer::{Printer, PrinterConfig};
use stats::{render, Timer};
use walker::{Action, Walker};

fn main() {
    let cli = Cli::parse();
    // `bh` with no arguments prints the full help menu (like `bh --help`).
    if cli.is_bare() {
        print_help();
        return;
    }
    let code = run(&cli);
    std::process::exit(code);
}

fn run(cli: &Cli) -> i32 {
    // `bh help` prints the full help menu instead of searching for "help".
    if cli.is_help_query() {
        return print_help();
    }

    let timer = Timer::start();
    let color = Printer::color_from(&cli.color);

    // --index-dir: build/update an index for the given paths, then exit.
    if let Some(dir) = &cli.index_dir {
        return index_dir_mode(cli, dir);
    }

    let name_filter = cli.name.as_ref().or_else(|| {
        if cli.files && cli.pattern.is_some() {
            let pat = cli.pattern.as_ref().unwrap();
            if !cli.paths.is_empty() || !Path::new(pat).exists() {
                Some(pat)
            } else {
                None
            }
        } else {
            None
        }
    });

    let name_re = name_filter
        .map(|pat| {
            RegexBuilder::new(pat)
                .case_insensitive(cli.ignore_case || !pat.bytes().any(|b| b.is_ascii_uppercase()))
                .build()
        })
        .transpose()
        .map_err(|e| err_exit(&format!("invalid --name pattern: {e}")));

    let name_re = match name_re {
        Ok(Some(re)) => Some(re),
        Ok(None) => None,
        Err(code) => return code,
    };

    let patterns = cli.search_patterns();
    let patterns_empty = patterns.is_empty();

    // Determine the action.
    let name_mode = cli.files
        || (patterns_empty && (name_re.is_some() || !cli.globs.is_empty() || !cli.iglobs.is_empty()));
    let action = if cli.files {
        Action::ListFiles
    } else if name_mode {
        Action::ListFiles
    } else if patterns_empty {
        return err_exit("no pattern given: supply a PATTERN positional or -e/-f");
    } else if cli.files_with_matches {
        Action::ListWithMatches
    } else if cli.files_without_match {
        Action::ListWithoutMatches
    } else if cli.count || cli.count_matches {
        Action::Count
    } else if cli.quiet {
        Action::ListWithMatches
    } else {
        Action::SearchLines
    };

    let engine = if patterns.is_empty() {
        None
    } else {
        let ci = case_insensitive(cli, &patterns);
        match Engine::build(&patterns, ci) {
            Ok(e) => Some(e),
            Err(e) => return err_exit(&format!("invalid pattern: {e}")),
        }
    };

    let context_requested = cli.before_context() > 0 || cli.after_context() > 0;
    let line_numbers = if cli.no_line_number {
        false
    } else if cli.line_number {
        true
    } else {
        !cli.quiet && std::io::stdout().is_terminal() && !cli.json
    };
    let heading = if cli.no_heading {
        false
    } else if cli.heading {
        true
    } else {
        !cli.json && std::io::stdout().is_terminal() && !cli.count && !cli.files_with_matches
            && !cli.files_without_match
    };

    let out: Box<dyn Write + Send> = Box::new(std::io::stdout());
    let pr = Printer::new(PrinterConfig {
        color,
        json: cli.json && !cli.count && !cli.files_with_matches && !cli.files_without_match,
        line_numbers,
        only_matching: cli.only_matching,
        context_requested,
        heading,
        column: cli.column,
        out,
    });

    let paths = cli.resolved_paths();
    let max_filesize: Option<u64> = cli
        .max_filesize
        .as_deref()
        .and_then(|s| {
            if let Ok(Some(n)) = parse_size(s) {
                Some(n)
            } else {
                None
            }
        });

    let root_is_dir = paths[0].is_dir();
    let mut w = Walker::new(engine.as_ref(), &pr, paths.clone(), name_re);
    w.action = action;
    w.hidden = cli.hidden;
    w.follow = cli.follow;
    w.no_ignore = cli.no_ignore;
    w.max_depth = cli.max_depth;
    w.max_filesize = max_filesize;
    w.threads = cli.threads;
    w.globs = cli.globs.clone();
    w.iglobs = cli.iglobs.clone();
    w.invert = cli.invert_match;
    w.before = cli.before_context();
    w.after = cli.after_context();
    w.encoding = cli
        .encoding
        .as_deref()
        .and_then(detect::parse_encoding);
    w.force_text = cli.text;
    w.no_messages = cli.no_messages;
    w.quiet = cli.quiet;
    w.no_index_messages = cli.no_index_messages || cli.no_messages;

    // Index decision.
    let idx_off = cli.no_index;
    let idx_force = cli.index;
    let auto = if idx_off || idx_force {
        false
    } else {
        match std::env::var("BLOODHOUND_AUTO_INDEX") {
            Ok(v) => matches!(v.as_str(), "1" | "on" | "true" | "yes"),
            Err(_) => true, // default on
        }
    };
    let index_enabled = idx_force || auto;
    let single_word = engine
        .as_ref()
        .map(|e| e.is_index_queryable())
        .unwrap_or(false);
    let index_ok = index_enabled
        && single_word
        && !cli.files
        && root_is_dir
        && paths.len() == 1
        && !cli.no_ignore
        && !cli.hidden
        && cli.max_depth.is_none()
        && cli.globs.is_empty()
        && cli.iglobs.is_empty()
        && action != Action::ListWithoutMatches
        && !cli.invert_match
        && engine.as_ref().map(|e| !e.query_words().is_empty()).unwrap_or(false);

    if index_ok {
        w.index = Index::open(&paths[0]);
        w.store = w.index.load();
        w.index_query = true;
        w.update_index = true;
        w.force_index = idx_force;
    } else if index_enabled && !w.no_index_messages {
        eprintln!("index: skipped (query not index-friendly)");
    }

    if cli.count_matches {
        w.count_total = true; // stats.matched_lines accumulates the total
    }

    match w.walk(&|abs, display| w.process_file(abs, display)) {
        Ok(()) => {}
        Err(e) => {
            if !cli.no_messages {
                eprintln!("{e}");
            }
            return 2;
        }
    }
    pr.finish();

    if cli.count_matches {
        let total = w.stats.matched_lines.load(Ordering::Relaxed);
        pr.write_line(&total.to_string());
    }

    if cli.quiet {
        w.found.store(w.pr.matched_any() || w.found.load(Ordering::Relaxed), Ordering::Relaxed);
    }

    if cli.stats {
        let elapsed = timer.elapsed_secs();
        eprint!("{}", render(&w.stats, elapsed, &action_name(action)));
    }

    let found = w.found.load(Ordering::Relaxed) || w.pr.matched_any();
    if found {
        0
    } else {
        1
    }
}

fn index_dir_mode(cli: &Cli, dir: &Path) -> i32 {
    let index = Index::open(dir);
    let binding = Printer::color_from_bare();
    let mut w = Walker::new(None, &binding, vec![dir.to_path_buf()], None);
    w.threads = cli.threads;
    let files = w.collect_files();
    match index.update(dir, &files) {
        Ok(r) => {
            if !cli.no_index_messages {
                eprintln!(
                    "index: {} files indexed ({} tokenized, {} reused) -> {}",
                    r.total_files,
                    r.files_tokenized,
                    r.files_reused,
                    index.path().display()
                );
            }
            0
        }
        Err(e) => {
            if !cli.no_messages {
                eprintln!("index build failed: {e}");
            }
            2
        }
    }
}

/// A throwaway printer used by modes that never print matches (index-dir).
impl Printer {
    fn color_from_bare() -> Printer {
        Printer::new(PrinterConfig {
            color: false,
            json: false,
            line_numbers: false,
            only_matching: false,
            context_requested: false,
            heading: false,
            column: false,
            out: Box::new(std::io::sink()),
        })
    }
}

fn case_insensitive(cli: &Cli, patterns: &[String]) -> bool {
    if cli.case_sensitive {
        return false;
    }
    if cli.ignore_case {
        return true;
    }
    // smart case: case-insensitive unless a pattern contains an uppercase letter.
    !patterns.iter().any(|p| p.bytes().any(|b| b.is_ascii_uppercase()))
}

fn parse_size(s: &str) -> Result<Option<u64>, &'static str> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty --max-filesize");
    }
    let (num, mult) = match s.as_bytes()[s.len() - 1] {
        b'K' | b'k' => (&s[..s.len() - 1], 1u64 << 10),
        b'M' | b'm' => (&s[..s.len() - 1], 1u64 << 20),
        b'G' | b'g' => (&s[..s.len() - 1], 1u64 << 30),
        b'B' | b'b' => (&s[..s.len() - 1], 1),
        _ => (s, 1),
    };
    let n: u64 = num
        .trim()
        .parse()
        .map_err(|_| "invalid --max-filesize (use a number, optionally with K/M/G)")?;
    Ok(Some(n.saturating_mul(mult)))
}

fn err_exit(msg: &str) -> i32 {
    eprintln!("error: {msg}");
    eprintln!("usage: bh [OPTIONS] PATTERN [PATH ...]");
    2
}

/// Print the full clap help menu to stdout and exit successfully.
fn print_help() -> i32 {
    let _ = Cli::command().print_help();
    println!();
    0
}

fn action_name(a: Action) -> &'static str {
    match a {
        Action::SearchLines => "line",
        Action::Count => "count",
        Action::ListWithMatches => "files-with-matches",
        Action::ListWithoutMatches => "files-without-match",
        Action::ListFiles => "files",
    }
}