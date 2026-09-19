use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "bh",
    version,
    about = "Bloodhound - a fast search tool for file names and file contents."
)]
pub struct Cli {
    /// The pattern to search for (or a file name pattern with --name).
    ///
    /// When -e/--regexp or -f/--file is given, this positional is treated as a
    /// path instead.
    pub pattern: Option<String>,

    /// Paths to search. Defaults to the current directory.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Additional patterns; search matches ANY pattern (logical OR).
    #[arg(short = 'e', long = "regexp", value_name = "PATTERN")]
    pub regexp: Vec<String>,

    /// Read patterns from a file, one per line.
    #[arg(short = 'f', long = "file", value_name = "PATTERNFILE")]
    pub pattern_files: Vec<PathBuf>,

    /// Match case-insensitively.
    #[arg(short = 'i', long = "ignore-case")]
    pub ignore_case: bool,

    /// Force case-sensitive matching even when the pattern has no uppercase.
    #[arg(short = 's', long = "case-sensitive")]
    pub case_sensitive: bool,

    /// Only print the paths of files that contain a match.
    #[arg(short = 'l', long = "files-with-matches")]
    pub files_with_matches: bool,

    /// Only print the paths of files that do NOT contain a match.
    #[arg(long = "files-without-match")]
    pub files_without_match: bool,

    /// Print a count of matching lines per file.
    #[arg(short = 'c', long = "count")]
    pub count: bool,

    /// Print the total number of matches instead of per-file counts.
    #[arg(long = "count-matches")]
    pub count_matches: bool,

    /// Print only the matched (non-empty) parts of a matching line.
    #[arg(short = 'o', long = "only-matching")]
    pub only_matching: bool,

    /// Invert the match: print lines that do NOT match.
    #[arg(short = 'v', long = "invert-match")]
    pub invert_match: bool,

    /// Do not print anything to stdout; only set the exit code.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Print the line numbers of matches. On by default when stdout is a TTY.
    #[arg(short = 'n', long = "line-number")]
    pub line_number: bool,

    /// Show each file path as its own heading line instead of repeating it on
    /// every match. On by default when stdout is a TTY.
    #[arg(long = "heading")]
    pub heading: bool,

    /// Never use heading mode; repeat the file path on every match line.
    #[arg(long = "no-heading", conflicts_with = "heading")]
    pub no_heading: bool,

    /// Show the byte column of the first match on each match line.
    #[arg(long = "column")]
    pub column: bool,

    /// Never print line numbers.
    #[arg(short = 'N', long = "no-line-number")]
    pub no_line_number: bool,

    /// Print the number of lines before each match.
    #[arg(short = 'B', long = "before-context", default_value_t = 0)]
    pub before_context: usize,

    /// Print the number of lines after each match.
    #[arg(short = 'A', long = "after-context", default_value_t = 0)]
    pub after_context: usize,

    /// Print the number of lines before and after each match.
    #[arg(short = 'C', long = "context", default_value_t = 0)]
    pub context: usize,

    /// Search hidden files and directories.
    #[arg(short = '.', long = "hidden")]
    pub hidden: bool,

    /// Follow symbolic links.
    #[arg(short = 'L', long = "follow")]
    pub follow: bool,

    /// Do not respect .gitignore, .ignore and .rgignore files.
    #[arg(long = "no-ignore")]
    pub no_ignore: bool,

    /// Include only paths that match this glob (repeatable).
    #[arg(short = 'g', long = "glob", value_name = "GLOB")]
    pub globs: Vec<String>,

    /// Include only paths that match this case-insensitive glob (repeatable).
    #[arg(long = "iglob", value_name = "GLOB")]
    pub iglobs: Vec<String>,

    /// Limit the depth of directory traversal.
    #[arg(long = "max-depth", value_name = "NUM")]
    pub max_depth: Option<usize>,

    /// Ignore files larger than this size (accepts K/M/G suffixes).
    #[arg(long = "max-filesize", value_name = "BYTES")]
    pub max_filesize: Option<String>,

    /// The number of threads to use for directory traversal. Defaults to the
    /// number of logical CPUs.
    #[arg(short = 'j', long = "threads", value_name = "NUM")]
    pub threads: Option<usize>,

    /// Search a binary file as if it were text.
    #[arg(short = 'a', long = "text")]
    pub text: bool,

    /// The encoding to use: utf8, utf16le, utf16be or latin1. Default is to
    /// sniff the byte order mark and assume UTF-8 otherwise.
    #[arg(long = "encoding", value_name = "ENC")]
    pub encoding: Option<String>,

    /// When to use color: auto, always or never.
    #[arg(long = "color", value_name = "WHEN", default_value = "auto")]
    pub color: String,

    /// Emit results in a JSON-lines format.
    #[arg(long = "json")]
    pub json: bool,

    /// Print aggregate statistics about the search to stderr.
    #[arg(long = "stats")]
    pub stats: bool,

    /// List every file that would be searched.
    #[arg(long = "files")]
    pub files: bool,

    /// Search file NAMES rather than file contents.
    #[arg(long = "name", value_name = "PATTERN")]
    pub name: Option<String>,

    /// Build or update the search index for the given paths and exit.
    #[arg(long = "index-dir", value_name = "PATH")]
    pub index_dir: Option<PathBuf>,

    /// Force using the on-disk index for faster searches.
    #[arg(long = "index", conflicts_with = "no_index")]
    pub index: bool,

    /// Never use or build the on-disk index.
    #[arg(long = "no-index", conflicts_with = "index")]
    pub no_index: bool,

    /// Suppress all error messages.
    #[arg(long = "no-messages")]
    pub no_messages: bool,

    /// Suppress the Bloodhound index status line printed to stderr.
    #[arg(long = "no-index-messages")]
    pub no_index_messages: bool,
}

impl Cli {
    /// True when no arguments or options were supplied at all. Runs `bh`
    /// with no input, which prints the help menu instead of erroring.
    pub fn is_bare(&self) -> bool {
        self.pattern.is_none()
            && self.paths.is_empty()
            && self.regexp.is_empty()
            && self.pattern_files.is_empty()
            && !self.ignore_case
            && !self.case_sensitive
            && !self.files_with_matches
            && !self.files_without_match
            && !self.count
            && !self.count_matches
            && !self.only_matching
            && !self.invert_match
            && !self.quiet
            && !self.line_number
            && !self.no_line_number
            && !self.heading
            && !self.no_heading
            && !self.column
            && self.before_context == 0
            && self.after_context == 0
            && self.context == 0
            && !self.hidden
            && !self.follow
            && !self.no_ignore
            && self.globs.is_empty()
            && self.iglobs.is_empty()
            && self.max_depth.is_none()
            && self.max_filesize.is_none()
            && self.threads.is_none()
            && !self.text
            && self.encoding.is_none()
            && self.color == "auto"
            && !self.json
            && !self.stats
            && !self.files
            && self.name.is_none()
            && self.index_dir.is_none()
            && !self.index
            && !self.no_index
            && !self.no_messages
            && !self.no_index_messages
    }

    /// `bh help` with no search context: treat the bare word "help" as a
    /// request for the help menu rather than a content pattern.
    pub fn is_help_query(&self) -> bool {
        self.pattern.as_deref() == Some("help")
            && self.paths.is_empty()
            && !self.has_flag_patterns()
            && !self.files
            && self.name.is_none()
            && self.index_dir.is_none()
    }

    /// The full list of content patterns, resolved from -e, -f and the
    /// positional pattern.
    pub fn patterns(&self) -> Vec<String> {
        let mut out = Vec::new();
        for f in &self.pattern_files {
            if let Ok(text) = std::fs::read_to_string(f) {
                for line in text.lines() {
                    out.push(line.to_string());
                }
            }
        }
        out.extend(self.regexp.iter().cloned());
        out
    }

    /// Content patterns for a search: all of -e/-f patterns plus the
    /// positional pattern (unless it was promoted to a path or used with --files).
    pub fn search_patterns(&self) -> Vec<String> {
        let mut out = self.patterns();
        if !self.has_flag_patterns() && !self.files && self.index_dir.is_none() {
            if let Some(p) = &self.pattern {
                let has_globs = !self.globs.is_empty() || !self.iglobs.is_empty();
                if has_globs && self.paths.is_empty() && std::path::Path::new(p).exists() {
                    // Positional arg is an existing path directory
                } else {
                    out.insert(0, p.clone());
                }
            }
        }
        out
    }

    pub fn has_flag_patterns(&self) -> bool {
        !self.regexp.is_empty() || !self.pattern_files.is_empty()
    }

    /// Resolved search paths, accounting for positional pattern vs path disambiguation.
    pub fn resolved_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.paths.clone();
        if self.has_flag_patterns() || self.index_dir.is_some() {
            if let Some(p) = &self.pattern {
                paths.insert(0, PathBuf::from(p));
            }
        } else if self.files || self.name.is_some() || !self.globs.is_empty() || !self.iglobs.is_empty() {
            if paths.is_empty() {
                if let Some(p) = &self.pattern {
                    if self.name.is_some() || self.files || std::path::Path::new(p).exists() {
                        paths.push(PathBuf::from(p));
                    }
                }
            }
        }
        if paths.is_empty() {
            paths.push(PathBuf::from("."));
        }
        paths
    }

    pub fn before_context(&self) -> usize {
        if self.context > 0 {
            self.context
        } else {
            self.before_context
        }
    }

    pub fn after_context(&self) -> usize {
        if self.context > 0 {
            self.context
        } else {
            self.after_context
        }
    }
}