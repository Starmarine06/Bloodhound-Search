use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use ignore::overrides::Override;
use regex::Regex;

use crate::detect::{self, Encoding};
use crate::engine::Engine;
use crate::index::{Index, Serialized};
use crate::printer::Printer;
use crate::scan::{scan_context, scan_fast, EventKind};
use crate::stats::Stats;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    SearchLines,
    Count,
    ListWithMatches,
    ListWithoutMatches,
    ListFiles,
}

pub struct Walker<'a> {
    pub engine: Option<&'a Engine>,
    pub pr: &'a Printer,
    pub stats: Stats,
    pub action: Action,
    /// Search roots exactly as the user typed them.
    pub roots: Vec<PathBuf>,
    /// (given, canonical) pairs used to compute display paths.
    pub root_infos: Vec<(String, String)>,
    pub hidden: bool,
    pub follow: bool,
    pub no_ignore: bool,
    pub max_depth: Option<usize>,
    pub max_filesize: Option<u64>,
    pub threads: Option<usize>,
    pub name_re: Option<Regex>,
    pub globs: Vec<String>,
    pub iglobs: Vec<String>,
    pub invert: bool,
    pub before: usize,
    pub after: usize,
    pub encoding: Option<Encoding>,
    pub force_text: bool,
    pub no_messages: bool,
    pub quiet: bool,
    pub found: AtomicBool,
    /// --count-matches: accumulate one total across all files.
    pub count_total: bool,
    pub index: Index,
    pub store: Option<Serialized>,
    pub index_query: bool,
    pub update_index: bool,
    pub force_index: bool,
    pub no_index_messages: bool,
}

impl<'a> Walker<'a> {
    pub fn new(
        engine: Option<&'a Engine>,
        pr: &'a Printer,
        roots: Vec<PathBuf>,
        name_re: Option<Regex>,
    ) -> Walker<'a> {
        let mut root_infos = Vec::with_capacity(roots.len());
        for r in &roots {
            let canon = canonical_str(r);
            root_infos.push((r.to_string_lossy().into_owned(), canon));
        }
        let index = Index::open(if roots.is_empty() { Path::new(".") } else { &roots[0] });
        Walker {
            engine,
            pr,
            stats: Stats::default(),
            action: Action::SearchLines,
            roots,
            root_infos,
            hidden: false,
            follow: false,
            no_ignore: false,
            max_depth: None,
            max_filesize: None,
            threads: None,
            name_re,
            globs: Vec::new(),
            iglobs: Vec::new(),
            invert: false,
            before: 0,
            after: 0,
            encoding: None,
            force_text: false,
            no_messages: false,
            quiet: false,
            found: AtomicBool::new(false),
            count_total: false,
            index,
            store: None,
            index_query: false,
            update_index: false,
            force_index: false,
            no_index_messages: false,
        }
    }

    fn override_for<'b>(&self, root: &Path) -> anyhow::Result<Option<Override>> {
        if self.globs.is_empty() && self.iglobs.is_empty() {
            return Ok(None);
        }
        let mut ob = ignore::overrides::OverrideBuilder::new(root);
        if !self.iglobs.is_empty() {
            ob.case_insensitive(true).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        for g in self.globs.iter().chain(self.iglobs.iter()) {
            ob.add(g).map_err(|e| anyhow::anyhow!("bad glob {g:?}: {e}"))?;
        }
        ob.build().map(Some).map_err(|e| anyhow::anyhow!("bad glob: {e}"))
    }

    fn key(&self, p: &Path) -> PathBuf {
        if self.index_query && self.store.is_some() {
            std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
        } else {
            p.to_path_buf()
        }
    }

    /// Pure display path for an absolute path (keeps the user's root text).
    fn display_for(&self, abs: &Path) -> String {
        let abs_str = abs.to_string_lossy().replace('\\', "/");
        let abs_clean = abs_str.strip_prefix("//?/").unwrap_or(&abs_str);
        for (given, canon) in &self.root_infos {
            let canon_clean = canon.strip_prefix("//?/").unwrap_or(canon);
            if let Some(rel) = abs_clean.strip_prefix(canon_clean) {
                let rel_str = rel.trim_start_matches('/');
                if rel_str.is_empty() {
                    return given.clone();
                }
                if given.ends_with('/') || given.ends_with('\\') {
                    return format!("{given}{rel_str}");
                }
                return format!("{given}/{rel_str}");
            }
        }
        abs_clean.to_string()
    }

    fn rel_to_root(&self, abs: &Path, root_canon: &str) -> Option<String> {
        abs.strip_prefix(Path::new(root_canon))
            .ok()
            .map(|r| r.to_string_lossy().replace('\\', "/").to_string())
    }

    /// Walk every matching file, dispatching each to `process`. Also collects
    /// files for the index update and decides index freshness.
    pub fn walk(&self, process: &(dyn Fn(&Path, &str) + Send + Sync)) -> anyhow::Result<()> {
        let root0 = &self.roots[0];
        let mut wb = ignore::WalkBuilder::new(root0);
        for r in &self.roots[1..] {
            wb.add(r);
        }
        wb.hidden(self.hidden)
            .follow_links(self.follow)
            .sort_by_file_path(|a, b| a.cmp(b));
        if self.no_ignore {
            wb.ignore(false)
                .git_ignore(false)
                .git_global(false)
                .git_exclude(false);
        }
        if let Some(d) = self.max_depth {
            wb.max_depth(Some(d));
        }
        if let Some(t) = self.threads {
            wb.threads(t);
        }
        if let Some(ov) = self.override_for(root0)? {
            wb.overrides(ov);
        }

        let store = self.store.as_ref();
        let mut stale = false;
        let mut file_map: HashMap<String, (u64, i64)> = HashMap::new();
        if let Some(s) = store {
            for e in &s.files {
                file_map.insert(e.rel.clone(), (e.size, e.mtime));
            }
        }

        let mut candidates: Option<HashSet<PathBuf>> = None;
        if self.index_query {
            if let Some(s) = store {
                let words = self.engine.map(|e| e.query_words()).unwrap_or_default();
                if let Some(ids) = self.index.candidates(Some(s), &words) {
                    let mut set = HashSet::with_capacity(ids.len());
                    for id in ids {
                        set.insert(self.index.entry_path(s, id));
                    }
                    candidates = Some(set);
                }
            }
        }

        if !self.update_index && candidates.is_none() {
            let walker = self;
            wb.build_parallel().run(|| {
                Box::new(move |entry| {
                    let entry = match entry {
                        Ok(e) => e,
                        Err(err) => {
                            walker.stats.add("errors", 1);
                            if !walker.no_messages {
                                eprintln!("{err}");
                            }
                            return ignore::WalkState::Continue;
                        }
                    };
                    let Some(ft) = entry.file_type() else { return ignore::WalkState::Continue };
                    if !ft.is_file() {
                        return ignore::WalkState::Continue;
                    }
                    let path = entry.path();
                    if let Some(re) = &walker.name_re {
                        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if !re.is_match(fname) {
                            return ignore::WalkState::Continue;
                        }
                    }
                    if let Some(max) = walker.max_filesize {
                        if let Ok(meta) = entry.metadata() {
                            if meta.len() > max {
                                return ignore::WalkState::Continue;
                            }
                        }
                    }
                    let abs = walker.key(path);
                    let display = walker.display_for(&abs);
                    walker.stats.add("files_visited", 1);

                    process(&abs, &display);
                    if walker.quiet && walker.found.load(Ordering::Relaxed) {
                        return ignore::WalkState::Quit;
                    }
                    ignore::WalkState::Continue
                })
            });
            return Ok(());
        }

        let mut root_files: Vec<PathBuf> = Vec::new();
        let mut entries_processed = 0u64;
        let index_root = self.root_infos[0].1.clone();

        for entry in wb.build() {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    self.stats.add("errors", 1);
                    if !self.no_messages {
                        eprintln!("{err}");
                    }
                    continue;
                }
            };
            let Some(ft) = entry.file_type() else { continue };
            if !ft.is_file() {
                continue;
            }
            let path = entry.path();
            if let Some(re) = &self.name_re {
                let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !re.is_match(fname) {
                    continue;
                }
            }
            let abs = self.key(path);
            let display = self.display_for(&abs);
            self.stats.add("files_visited", 1);

            if self.quiet && self.found.load(Ordering::Relaxed) {
                break;
            }

            if let Ok(meta) = entry.metadata() {
                if let Some(max) = self.max_filesize {
                    if meta.len() > max {
                        continue;
                    }
                }
                if self.update_index {
                    if let Some(rel) = self.rel_to_root(&abs, &index_root) {
                        root_files.push(abs.clone());
                        entries_processed += 1;
                        if let Some(_s) = store {
                            let ts = meta
                                .modified()
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs() as i64)
                                .unwrap_or(0);
                            match file_map.get(&rel) {
                                Some((osz, omt)) if *osz == meta.len() && *omt == ts => {}
                                _ => stale = true,
                            }
                        }
                    }
                }
            }

            if let Some(set) = &candidates {
                if !set.contains(&abs) {
                    continue;
                }
            }

            process(&abs, &display);
            if self.quiet && self.found.load(Ordering::Relaxed) {
                break;
            }
        }

        // Index update: forced, or the tree drifted from the stored index.
        if self.update_index && (self.force_index || store.is_none() || stale) {
            if store.is_some() && self.no_index_messages {
                // silent rebuild
            }
            let res = self.index.update(&self.roots[0], &root_files);
            match res {
                Ok(r) => {
                    if !self.no_index_messages {
                        eprintln!(
                            "index: {} files indexed ({} tokenized, {} reused) -> {}",
                            r.total_files,
                            r.files_tokenized,
                            r.files_reused,
                            self.index.path().display()
                        );
                    }
                }
                Err(e) => {
                    self.stats.add("errors", 1);
                    if !self.no_messages {
                        eprintln!("index update failed: {e}");
                    }
                }
            }
        } else if self.update_index && store.is_some() && !stale && !self.no_index_messages {
            eprintln!("index: up to date ({} files)", entries_processed);
        }

        Ok(())
    }

    /// Process a single file according to the configured action.
    pub fn process_file(&self, abs: &Path, display: &str) {
        if self.action == Action::ListFiles {
            self.pr.files_with(display, true, false);
            return;
        }
        let Some(engine) = self.engine else { return };
        self.found.store(
            self.found.load(Ordering::Relaxed) || self.pr.matched_any(),
            Ordering::Relaxed,
        );
        let buf = match std::fs::read(abs) {
            Ok(b) => b,
            Err(e) => {
                self.stats.add("errors", 1);
                if !self.no_messages {
                    eprintln!("failed to read {}: {e}", abs.display());
                }
                return;
            }
        };
        if !self.force_text && self.encoding.is_none() && detect::is_binary(&buf) {
            self.stats.add("binary_skipped", 1);
            return;
        }
        self.stats.add("bytes_searched", buf.len() as u64);
        self.stats.add("files_searched", 1);
        let text = detect::transcode(&buf, self.encoding);

        let mut matched_lines: u64 = 0;
        let mut file_found = false;
        let has_context = self.invert || self.before > 0 || self.after > 0;

        let emit = |meth_line: &mut u64,
                    found: &mut bool,
                    ev: crate::scan::LineEvent<'_>|
         -> Option<()> {
            let is_match = ev.kind == EventKind::Match;
            match self.action {
                Action::SearchLines => {
                    if is_match {
                        *meth_line += 1;
                        *found = true;
                    }
                    self.pr.push(display, &ev);
                }
                Action::Count | Action::ListWithoutMatches => {
                    if is_match {
                        *meth_line += 1;
                        *found = true;
                    }
                }
                Action::ListWithMatches => {
                    if is_match {
                        *found = true;
                        return None;
                    }
                }
                Action::ListFiles => {}
            }
            Some(())
        };

        let _ = if has_context {
            scan_context(&text, engine, self.invert, self.before, self.after, |ev| {
                emit(&mut matched_lines, &mut file_found, ev)
            })
        } else {
            scan_fast(&text, engine, |ev| emit(&mut matched_lines, &mut file_found, ev))
        };

        if file_found {
            self.stats.add("matched_files", 1);
            self.stats.add("matched_lines", matched_lines);
            if self.quiet {
                self.found.store(true, Ordering::Relaxed);
            }
        }
        match self.action {
            Action::SearchLines => self.pr.finish_file(),
            Action::Count => {
                if !self.count_total {
                    self.pr.count_file(display, matched_lines, false);
                }
            }
            Action::ListWithoutMatches => {
                self.pr.files_with(display, file_found, true);
            }
            Action::ListWithMatches => {
                self.pr.files_with(display, file_found, false);
            }
            Action::ListFiles => {}
        }
    }

    /// Collect every file path under the roots (no filters, no scanning).
    pub fn collect_files(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut wb = ignore::WalkBuilder::new(&self.roots[0]);
        for r in &self.roots[1..] {
            wb.add(r);
        }
        wb.hidden(false).follow_links(self.follow).sort_by_file_path(|a, b| a.cmp(b));
        if let Some(t) = self.threads {
            wb.threads(t);
        }
        for entry in wb.build() {
            let Ok(entry) = entry else { continue };
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                out.push(entry.path().to_path_buf());
            }
        }
        out
    }
}

pub fn canonical_str(p: &Path) -> String {
    let s = std::fs::canonicalize(p)
        .unwrap_or_else(|_| {
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(p)
            }
        })
        .to_string_lossy()
        .replace('\\', "/");
    let trimmed = s.strip_prefix("//?/").unwrap_or(&s);
    trimmed.trim_end_matches('/').to_string()
}