use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[derive(Default)]
pub struct Stats {
    pub files_visited: AtomicU64,
    pub files_searched: AtomicU64,
    pub matched_files: AtomicU64,
    pub matched_lines: AtomicU64,
    pub bytes_searched: AtomicU64,
    pub binary_skipped: AtomicU64,
    pub errors: AtomicU64,
}

impl Stats {
    pub fn add(&self, field: &'static str, n: u64) {
        match field {
            "files_visited" => self.files_visited.fetch_add(n, Ordering::Relaxed),
            "files_searched" => self.files_searched.fetch_add(n, Ordering::Relaxed),
            "matched_files" => self.matched_files.fetch_add(n, Ordering::Relaxed),
            "matched_lines" => self.matched_lines.fetch_add(n, Ordering::Relaxed),
            "bytes_searched" => self.bytes_searched.fetch_add(n, Ordering::Relaxed),
            "binary_skipped" => self.binary_skipped.fetch_add(n, Ordering::Relaxed),
            "errors" => self.errors.fetch_add(n, Ordering::Relaxed),
            _ => 0,
        };
    }
}

/// Tracks timing for --stats.
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn start() -> Timer {
        Timer { start: Instant::now() }
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

pub fn render(stats: &Stats, elapsed: f64, mode: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("stats: {mode} search\n"));
    let files_v = stats.files_visited.load(Ordering::Relaxed);
    let files_s = stats.files_searched.load(Ordering::Relaxed);
    let matched_f = stats.matched_files.load(Ordering::Relaxed);
    let matched_l = stats.matched_lines.load(Ordering::Relaxed);
    let bytes = stats.bytes_searched.load(Ordering::Relaxed);
    let bin = stats.binary_skipped.load(Ordering::Relaxed);
    let err = stats.errors.load(Ordering::Relaxed);
    out.push_str(&format!("  files visited:        {files_v}\n"));
    out.push_str(&format!("  files searched:       {files_s}\n"));
    out.push_str(&format!("  files with matches:   {matched_f}\n"));
    out.push_str(&format!("  lines matched:        {matched_l}\n"));
    out.push_str(&format!("  bytes searched:       {}\n", human_bytes(bytes)));
    out.push_str(&format!("  binary files skipped: {bin}\n"));
    out.push_str(&format!("  errors:               {err}\n"));
    out.push_str(&format!("  elapsed:              {elapsed:.3}s\n"));
    out
}