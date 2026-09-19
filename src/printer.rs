use std::io::{IsTerminal, Write};
use std::sync::Mutex;

use serde_json::json;

use crate::scan::{EventKind, LineEvent};

const RESET: &str = "\x1b[0m";
const MATCH_OPEN: &str = "\x1b[0m\x1b[1;31m";
const PATH_OPEN: &str = "\x1b[0m\x1b[35m";
const LINE_NO_OPEN: &str = "\x1b[0m\x1b[32m";

pub struct PrinterConfig {
    pub color: bool,
    pub json: bool,
    pub line_numbers: bool,
    pub only_matching: bool,
    pub context_requested: bool,
    pub heading: bool,
    pub column: bool,
    pub out: Box<dyn Write + Send>,
}

pub struct Printer {
    inner: Mutex<State>,
}

struct State {
    out: Box<dyn Write + Send>,
    color: bool,
    json: bool,
    line_numbers: bool,
    only_matching: bool,
    heading: bool,
    column: bool,
    file_separator: bool,
    current_path: Option<String>,
    path_started: bool,
    buf: Vec<u8>,
    matched_any: bool,
    failed: bool,
}

impl Printer {
    pub fn new(cfg: PrinterConfig) -> Printer {
        let file_separator = cfg.context_requested || cfg.only_matching;
        Printer {
            inner: Mutex::new(State {
                out: cfg.out,
                color: cfg.color,
                json: cfg.json,
                line_numbers: cfg.line_numbers,
                only_matching: cfg.only_matching,
                heading: cfg.heading,
                column: cfg.column,
                file_separator,
                current_path: None,
                path_started: false,
                buf: Vec::with_capacity(1024),
                matched_any: false,
                failed: false,
            }),
        }
    }

    pub fn color_from(when: &str) -> bool {
        match when {
            "always" => true,
            "never" => false,
            _ => std::io::stdout().is_terminal(),
        }
    }

    pub fn push(&self, path: &str, ev: &LineEvent) {
        let mut s = match self.inner.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        if s.failed {
            return;
        }
        match ev.kind {
            EventKind::Separator => s.separator(),
            EventKind::Match => s.major(path, ev, true),
            EventKind::Context => s.major(path, ev, false),
        }
    }

    /// Per-file count output (also used for --files-without-match via the
    /// `nonzero_only` flag).
    pub fn count_file(&self, path: &str, count: u64, nonzero_only: bool) {
        let mut s = match self.inner.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        if s.failed {
            return;
        }
        if nonzero_only && count == 0 {
            return;
        }
        s.buf.clear();
        if s.json {
            let event = json!({"type":"summary","data":{"path":{"text":path},"lines_with_matches":count}});
            s.buf.extend_from_slice(serde_json::to_string(&event).unwrap_or_default().as_bytes());
            s.buf.push(b'\n');
        } else {
            s.buf.extend_from_slice(path.as_bytes());
            s.buf.push(b':');
            s.buf.extend_from_slice(count.to_string().as_bytes());
            s.buf.push(b'\n');
        }
        s.write_buf();
        if count > 0 {
            s.matched_any = true;
        }
    }

    pub fn files_with(&self, path: &str, has_match: bool, invert: bool) {
        let mut s = match self.inner.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        if s.failed {
            return;
        }
        if invert {
            if has_match {
                return;
            }
        } else if !has_match {
            return;
        }
        s.buf.clear();
        if s.json {
            let event = json!({"type":"begin","data":{"path":{"text":path}}});
            s.buf.extend_from_slice(serde_json::to_string(&event).unwrap_or_default().as_bytes());
            s.buf.push(b'\n');
        } else {
            s.buf.extend_from_slice(path.as_bytes());
            s.buf.push(b'\n');
        }
        s.write_buf();
        s.matched_any = true;
    }

    /// Signal the end of a file so the printer can close its JSON events.
    pub fn finish_file(&self) {
        let mut s = match self.inner.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        if s.failed {
            return;
        }
        s.end_current_path();
    }

    pub fn finish(&self) {
        let mut s = match self.inner.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        s.end_current_path();
        let _ = s.out.flush();
    }

    pub fn matched_any(&self) -> bool {
        self.inner
            .lock()
            .map(|s| s.matched_any)
            .unwrap_or(false)
    }

    /// Emit a bare line (used by --count-matches totals).
    pub fn write_line(&self, line: &str) {
        let mut s = match self.inner.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        if s.failed {
            return;
        }
        s.buf.clear();
        s.buf.extend_from_slice(line.as_bytes());
        s.buf.push(b'\n');
        s.write_buf();
    }
}

impl State {
    fn major(&mut self, path: &str, ev: &LineEvent, is_match: bool) {
        let is_new_file = self.current_path.as_deref() != Some(path);
        if self.json {
            if is_new_file {
                let event = json!({"type":"begin","data":{"path":{"text":path}},"encoding":{"charset":"utf-8"}});
                self.write_str(&serde_json::to_string(&event).unwrap_or_default());
                self.write_byte(b'\n');
            }
        } else if self.heading {
            // Heading mode: print the path once as its own line instead of
            // repeating it on every match.
            if is_new_file {
                self.heading_line(path);
            }
        } else if self.file_separator && is_new_file && self.path_started {
            self.separator();
        }
        self.current_path = Some(path.to_string());
        self.path_started = true;

        if self.only_matching && is_match {
            for m in &ev.matches {
                let start = m.start().min(ev.line.len());
                let end = m.end().min(ev.line.len());
                self.buf.clear();
                if !self.json {
                    self.push_prefix(path, Some(ev.line_no), Some(start as u64));
                }
                if self.color && !self.json {
                    self.buf.extend_from_slice(MATCH_OPEN.as_bytes());
                }
                self.buf.extend_from_slice(&ev.line[start..end]);
                if self.color && !self.json {
                    self.buf.extend_from_slice(RESET.as_bytes());
                }
                self.buf.push(b'\n');
                if self.json {
                    let txt = String::from_utf8_lossy(&ev.line[start..end]);
                    let event = json!({"type":"match","data":{"path":{"text":path},"lines":{"text":format!("{txt}\n")},"submatches":[{"match":{"text":txt},"start":start,"end":end}]}});
                    self.write_str(&serde_json::to_string(&event).unwrap_or_default());
                    self.write_byte(b'\n');
                } else {
                    self.write_buf();
                }
                self.matched_any = true;
            }
            return;
        }

        self.buf.clear();
        if self.json {
            let line_txt = String::from_utf8_lossy(ev.line);
            let subs: Vec<_> = ev
                .matches
                .iter()
                .map(|m| {
                    let start = m.start().min(ev.line.len());
                    let end = m.end().min(ev.line.len());
                    let txt = String::from_utf8_lossy(&ev.line[start..end]);
                    json!({"match":{"text":txt},"start":start,"end":end})
                })
                .collect();
            let mut event = json!({
                "type": if is_match { "match" } else { "context" },
                "data": {
                    "path": {"text": path},
                    "lines": {"text": line_txt},
                }
            });
            if let Some(d) = event["data"].as_object_mut() {
                if is_match {
                    d.insert("submatches".into(), json!(subs));
                    d.insert("line_number".into(), json!(ev.line_no));
                }
            }
            self.write_str(&serde_json::to_string(&event).unwrap_or_default());
            self.write_byte(b'\n');
            if is_match {
                self.matched_any = true;
            }
            return;
        }

        // Plain text output with segmented highlighting.
        let column = self.match_column(ev, is_match);
        self.push_prefix(path, Some(ev.line_no), column);
        if self.color {
            let mut cursor = 0usize;
            for m in &ev.matches {
                let start = m.start().min(ev.line.len());
                let end = m.end().min(ev.line.len());
                if cursor < start {
                    self.buf.extend_from_slice(&ev.line[cursor..start]);
                }
                if start < end {
                    self.buf.extend_from_slice(MATCH_OPEN.as_bytes());
                    self.buf.extend_from_slice(&ev.line[start..end]);
                    self.buf.extend_from_slice(RESET.as_bytes());
                }
                cursor = end;
            }
            self.buf.extend_from_slice(&ev.line[cursor..]);
        } else {
            self.buf.extend_from_slice(ev.line);
        }
        if !self.buf.last().is_some_and(|&b| b == b'\n') {
            self.buf.push(b'\n');
        }
        self.write_buf();
        if is_match {
            self.matched_any = true;
        }
    }

    /// The byte column of the first match on a match line, when --column is on.
    fn match_column(&self, ev: &LineEvent, is_match: bool) -> Option<u64> {
        if self.column && is_match {
            ev.matches
                .first()
                .map(|m| m.start().min(ev.line.len()) as u64)
        } else {
            None
        }
    }

    fn push_prefix(&mut self, path: &str, line_no: Option<u64>, column: Option<u64>) {
        let show_ln = self.line_numbers || column.is_some();
        if !self.heading {
            if self.color {
                self.buf.extend_from_slice(PATH_OPEN.as_bytes());
                self.buf.extend_from_slice(path.as_bytes());
                self.buf.extend_from_slice(RESET.as_bytes());
                self.buf.push(b':');
            } else {
                self.buf.extend_from_slice(path.as_bytes());
                self.buf.push(b':');
            }
        }
        if let Some(n) = line_no {
            if show_ln {
                if self.color {
                    self.buf.extend_from_slice(LINE_NO_OPEN.as_bytes());
                }
                self.buf.extend_from_slice(n.to_string().as_bytes());
                if self.color {
                    self.buf.extend_from_slice(RESET.as_bytes());
                }
                self.buf.push(b':');
                if let Some(c) = column {
                    if self.color {
                        self.buf.extend_from_slice(LINE_NO_OPEN.as_bytes());
                    }
                    self.buf.extend_from_slice(c.to_string().as_bytes());
                    if self.color {
                        self.buf.extend_from_slice(RESET.as_bytes());
                    }
                    self.buf.push(b':');
                }
            }
        }
    }

    /// The file path on its own line, used by heading mode.
    fn heading_line(&mut self, path: &str) {
        self.buf.clear();
        if self.color {
            self.buf.extend_from_slice(PATH_OPEN.as_bytes());
            self.buf.extend_from_slice(path.as_bytes());
            self.buf.extend_from_slice(RESET.as_bytes());
        } else {
            self.buf.extend_from_slice(path.as_bytes());
        }
        self.buf.push(b'\n');
        self.write_buf();
    }

    fn separator(&mut self) {
        if self.json {
            return;
        }
        self.buf.clear();
        if self.color {
            self.buf.extend_from_slice(b"\x1b[0m\x1b[1;31m--\x1b[0m\n");
        } else {
            self.buf.extend_from_slice(b"--\n");
        }
        self.write_buf();
    }

    fn end_current_path(&mut self) {
        if self.json {
            if let Some(p) = self.current_path.take() {
                let event = json!({"type":"end","data":{"path":{"text":p}}});
                self.write_str(&serde_json::to_string(&event).unwrap_or_default());
                self.write_byte(b'\n');
            }
        }
        self.current_path = None;
        self.path_started = false;
    }

    fn write_str(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
        self.write_buf();
    }

    fn write_byte(&mut self, b: u8) {
        self.buf.clear();
        self.buf.push(b);
        self.write_buf();
    }

    fn write_buf(&mut self) {
        if self.out.write_all(&self.buf).is_err() {
            self.failed = true;
        }
        self.buf.clear();
    }
}