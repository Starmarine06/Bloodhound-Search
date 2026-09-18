use std::fmt;

use aho_corasick::{AhoCorasick, MatchKind};
use grep_matcher::{Match, Matcher, NoCaptures};
use grep_regex::RegexMatcher;
use memchr::memchr2;

/// Matcher errors can never happen for the engines we build (memmem and
/// aho-corasick are infallible; the regex matcher's errors are reported at
/// construction time).
#[derive(Debug)]
pub struct NeverError;

impl fmt::Display for NeverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "internal matcher error")
    }
}

impl std::error::Error for NeverError {}

/// Bytes that make a pattern a real regex (not a pure literal).
const REGEX_META: &[u8] = b".()[]{}*+?^$|\\";

fn is_plain_literal(pat: &str) -> bool {
    if pat.is_empty() {
        return false;
    }
    pat.is_ascii() && !pat.contains('\n') && !pat.bytes().any(|b| REGEX_META.contains(&b))
}

/// The compiled search engine. `Literal` is the single-pattern fast path
/// (optionally case-folded for ASCII), `Set` is the aho-corasick multi-literal
/// path, and `Regex` covers everything else.
pub enum Engine {
    Literal(Literal),
    Set(Set),
    Regex(RegexMatcher),
}

impl Engine {
    /// Build an engine from one or more patterns.
    ///
    /// `case_insensitive` must already encode smart-case / -i / -s decisions.
    pub fn build(patterns: &[String], case_insensitive: bool) -> anyhow::Result<Engine> {
        anyhow::ensure!(!patterns.is_empty(), "no patterns given");
        if patterns.iter().any(|p| p.is_empty()) {
            return Self::regex(patterns, case_insensitive);
        }
        let all_literal = patterns.iter().all(|p| is_plain_literal(p));
        if all_literal {
            if patterns.len() == 1 {
                let needle = if case_insensitive {
                    patterns[0].bytes().map(fold_byte).collect()
                } else {
                    patterns[0].bytes().collect()
                };
                return Ok(Engine::Literal(Literal {
                    needle,
                    ci: case_insensitive,
                }));
            }
            // Multiple pure literals: aho-corasick. Case-folded multi-literal
            // is deferred to the regex engine for now.
            if !case_insensitive {
                let ac = AhoCorasick::builder()
                    .match_kind(MatchKind::LeftmostFirst)
                    .build(patterns)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                return Ok(Engine::Set(Set { ac }));
            }
            return Self::regex(patterns, case_insensitive);
        }
        Self::regex(patterns, case_insensitive)
    }

    fn regex(patterns: &[String], case_insensitive: bool) -> anyhow::Result<Engine> {
        let joined = patterns.join("|");
        let name = if case_insensitive {
            format!("(?i:{joined})")
        } else {
            joined
        };
        let matcher = RegexMatcher::new(&name).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(Engine::Regex(matcher))
    }

    /// True if this engine can be served from the word index (every pattern is
    /// one ASCII word).
    pub fn is_index_queryable(&self) -> bool {
        match self {
            Engine::Literal(l) => is_one_word(&l.needle),
            _ => false,
        }
    }

    /// The word(s) that must all appear for an indexed candidate, when
    /// index-queryable.
    pub fn query_words(&self) -> Vec<String> {
        match self {
            Engine::Literal(l) => split_words(&l.needle),
            _ => Vec::new(),
        }
    }
}

fn fold_byte(b: u8) -> u8 {
    if b.is_ascii_uppercase() {
        b + 32
    } else {
        b
    }
}

fn is_one_word(bytes: &[u8]) -> bool {
    !bytes.is_empty() && split_words(bytes).len() == 1
}

fn split_words(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    for byte in bytes.iter().copied() {
        let fb = fold_byte(byte);
        if fb.is_ascii_alphanumeric() || fb == b'_' {
            cur.push(fb);
        } else if !cur.is_empty() {
            out.push(String::from_utf8(std::mem::take(&mut cur)).unwrap());
        }
    }
    if !cur.is_empty() {
        out.push(String::from_utf8(cur).unwrap());
    }
    out
}

pub struct Literal {
    /// The needle bytes, lower-cased when case-insensitive.
    needle: Vec<u8>,
    ci: bool,
}

impl fmt::Debug for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Literal")
            .field("needle", &self.needle)
            .field("ci", &self.ci)
            .finish()
    }
}

impl Literal {
    fn find_pos(&self, hay: &[u8], start: usize) -> Option<usize> {
        if self.ci {
            find_ci(hay, start, &self.needle)
        } else {
            memchr::memmem::Finder::new(&self.needle).find(&hay[start..]).map(|o| start + o)
        }
    }
}

impl Matcher for Literal {
    type Captures = NoCaptures;
    type Error = NeverError;

    fn find_at(&self, haystack: &[u8], start: usize) -> Result<Option<Match>, NeverError> {
        Ok(self
            .find_pos(haystack, start)
            .map(|s| Match::new(s, s + self.needle.len())))
    }

    fn new_captures(&self) -> Result<Self::Captures, NeverError> {
        Ok(NoCaptures::new())
    }
}

pub struct Set {
    ac: AhoCorasick,
}

impl fmt::Debug for Set {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Set").finish()
    }
}

impl Matcher for Set {
    type Captures = NoCaptures;
    type Error = NeverError;

    fn find_at(&self, haystack: &[u8], start: usize) -> Result<Option<Match>, NeverError> {
        Ok(self
            .ac
            .find(&haystack[start..])
            .map(|m| Match::new(start + m.start(), start + m.end())))
    }

    fn new_captures(&self) -> Result<Self::Captures, NeverError> {
        Ok(NoCaptures::new())
    }
}

impl Matcher for Engine {
    type Captures = NoCaptures;
    type Error = NeverError;

    fn find_at(&self, haystack: &[u8], start: usize) -> Result<Option<Match>, NeverError> {
        match self {
            Engine::Literal(l) => l.find_at(haystack, start),
            Engine::Set(s) => s.find_at(haystack, start),
            Engine::Regex(r) => {
                let m = r.find_at(haystack, start)?;
                Ok(m)
            }
        }
    }

    fn new_captures(&self) -> Result<Self::Captures, NeverError> {
        Ok(NoCaptures::new())
    }
}

/// Case-insensitive ASCII search: look for the first needle byte (and its
/// uppercase twin) with memchr2, then verify the whole needle with folded
/// comparison. Small, sparse candidate sets make this dramatically faster
/// than running a regex per line.
fn find_ci(hay: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(start);
    }
    let first = needle[0];
    let (lo, hi) = if first.is_ascii_lowercase() {
        (first, first - 32)
    } else if first.is_ascii_uppercase() {
        (first + 32, first)
    } else {
        (first, first)
    };
    let nlen = needle.len();
    let mut pos = start;
    while pos < hay.len() {
        let rel = if lo == hi {
            memchr::memchr(lo, &hay[pos..])?
        } else {
            memchr2(lo, hi, &hay[pos..])?
        };
        let i = pos + rel;
        if i + nlen <= hay.len() && folded_eq(&hay[i..i + nlen], needle) {
            return Some(i);
        }
        pos = i + 1;
    }
    None
}

fn folded_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|(x, y)| fold_byte(*x) == *y)
}

impl From<grep_matcher::NoError> for NeverError {
    fn from(_: grep_matcher::NoError) -> NeverError {
        NeverError
    }
}