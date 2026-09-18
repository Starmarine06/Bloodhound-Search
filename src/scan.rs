use grep_matcher::Match;
use memchr::{memchr, memchr_iter};
use grep_matcher::Matcher;

use crate::detect::line_bounds;
use crate::engine::{Engine, NeverError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Match,
    Context,
    Separator,
}

pub struct LineEvent<'a> {
    pub line_no: u64,
    /// The full line slice, including its trailing newline.
    pub line: &'a [u8],
    /// Matches within the line (non-overlapping), relative to `line`.
    pub matches: Vec<Match>,
    pub kind: EventKind,
}

/// The default fast path: find the next match in the whole buffer, widen to
/// its line, and jump past that line. No per-line regex work when nothing
/// matches.
pub fn scan_fast<F>(hay: &[u8], eng: &Engine, mut emit: F) -> Result<(), NeverError>
where
    F: FnMut(LineEvent<'_>) -> Option<()>,
{
    let mut pos = 0usize;
    let mut line_no = 1u64;
    let mut prev_le = 0usize;
    loop {
        let some = eng.find_at(hay, pos)?;
        let Some(m) = some else { break };
        if m.start() == hay.len() {
            break;
        }
        let (ls, le) = line_bounds(hay, m.start());
        line_no += memchr_iter(b'\n', &hay[prev_le..ls]).count() as u64;
        let matches = collect_matches(hay, eng, m.start(), le, ls)?;
        if emit(LineEvent {
            line_no,
            line: &hay[ls..le],
            matches,
            kind: EventKind::Match,
        })
        .is_none()
        {
            break;
        }
        if le <= pos {
            break;
        }
        pos = le;
        prev_le = le;
        line_no += 1;
    }
    Ok(())
}

/// Line-based scan that supports --invert-match and before/after context.
/// Context groups are merged when they overlap and separated with a `--` line
/// when there is a real gap.
pub fn scan_context<F>(
    hay: &[u8],
    eng: &Engine,
    invert: bool,
    before: usize,
    after: usize,
    mut emit: F,
) -> Result<(), NeverError>
where
    F: FnMut(LineEvent<'_>) -> Option<()>,
{
    let context_wanted = before > 0 || after > 0;
    let mut ring: Vec<(u64, &[u8])> = Vec::new();
    let mut after_left = 0usize;
    let mut emitted_any = false;
    let mut last_emitted_no = 0u64;
    let mut line_no = 1u64;
    let mut start = 0usize;

    while start < hay.len() {
        let (nl, line_end) = match memchr(b'\n', &hay[start..]) {
            Some(i) => (start + i, start + i + 1),
            None => (hay.len(), hay.len()),
        };
        let end_excl = if nl > start && hay[nl - 1] == b'\r' {
            nl - 1
        } else {
            nl
        };
        let content = &hay[start..end_excl];

        let matched = eng.find_at(content, 0)?.is_some();
        let hit = if invert { !matched } else { matched };
        let line_slice = &hay[start..line_end];

        if hit {
            let has_gap = context_wanted
                && emitted_any
                && last_emitted_no + 1 < line_no
                && ring.len() < before;
            if has_gap {
                if emit(LineEvent {
                    line_no,
                    line: b"",
                    matches: Vec::new(),
                    kind: EventKind::Separator,
                })
                .is_none()
                {
                    return Ok(());
                }
            }
            for &(no, ln) in ring.iter() {
                if emit(LineEvent {
                    line_no: no,
                    line: ln,
                    matches: Vec::new(),
                    kind: EventKind::Context,
                })
                .is_none()
                {
                    return Ok(());
                }
            }
            ring.clear();
            let matches = if invert {
                Vec::new()
            } else {
                collect_matches(content, eng, 0, content.len(), 0)?
            };
            if emit(LineEvent {
                line_no,
                line: line_slice,
                matches,
                kind: EventKind::Match,
            })
            .is_none()
            {
                return Ok(());
            }
            emitted_any = true;
            last_emitted_no = line_no;
            after_left = after;
        } else if after_left > 0 {
            if emit(LineEvent {
                line_no,
                line: line_slice,
                matches: Vec::new(),
                kind: EventKind::Context,
            })
            .is_none()
            {
                return Ok(());
            }
            emitted_any = true;
            last_emitted_no = line_no;
            after_left -= 1;
        } else {
            ring.push((line_no, line_slice));
            if ring.len() > before {
                ring.remove(0);
            }
        }

        line_no += 1;
        start = line_end;
    }
    Ok(())
}

/// Collect every non-overlapping match within [abs_start, abs_end), with
/// offsets rebased so that matches are relative to the emitted line.
fn collect_matches(
    hay: &[u8],
    eng: &Engine,
    abs_start: usize,
    abs_end: usize,
    rel_base: usize,
) -> Result<Vec<Match>, NeverError> {
    let mut out = Vec::new();
    let mut cur = abs_start;
    while cur < abs_end {
        let some = eng.find_at(hay, cur)?;
        let Some(m) = some else { break };
        if m.start() >= abs_end {
            break;
        }
        if m.end() <= m.start() {
            cur += 1;
            continue;
        }
        out.push(Match::new(m.start() - rel_base, m.end() - rel_base));
        cur = m.end();
    }
    Ok(out)
}