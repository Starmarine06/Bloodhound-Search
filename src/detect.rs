use memchr::memchr;
use memchr::memrchr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Latin1,
}

/// Parse the --encoding flag value.
pub fn parse_encoding(name: &str) -> Option<Encoding> {
    match name.to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => Some(Encoding::Utf8),
        "utf16le" | "utf-16le" => Some(Encoding::Utf16Le),
        "utf16be" | "utf-16be" => Some(Encoding::Utf16Be),
        "latin1" | "latin-1" | "iso-8859-1" => Some(Encoding::Latin1),
        _ => None,
    }
}

/// True if the buffer seems to be binary: a NUL byte appears before the first
/// newline.
pub fn is_binary(buf: &[u8]) -> bool {
    for &b in buf.iter().take(1024) {
        if b == 0 {
            return true;
        }
        if b == b'\n' {
            return false;
        }
    }
    false
}

/// Decide which encoding to use for the buffer and return the decoded bytes.
///
/// When an explicit encoding is given it is used directly. Otherwise the BOM
/// is sniffed; UTF-8 (and latin1) pass through unchanged.
pub fn transcode(buf: &[u8], encoding: Option<Encoding>) -> Vec<u8> {
    let enc = match encoding {
        Some(e) => e,
        None => {
            if buf.starts_with(&[0xFF, 0xFE]) {
                Encoding::Utf16Le
            } else if buf.starts_with(&[0xFE, 0xFF]) {
                Encoding::Utf16Be
            } else {
                Encoding::Utf8
            }
        }
    };
    match enc {
        Encoding::Utf8 | Encoding::Latin1 => buf.to_vec(),
        Encoding::Utf16Le => decode_utf16(buf, true),
        Encoding::Utf16Be => decode_utf16(buf, false),
    }
}

fn decode_utf16(buf: &[u8], little_endian: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len());
    let mut units = Vec::new();
    let chunk = buf.chunks_exact(2);
    for pair in chunk {
        let u = if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        };
        units.push(u);
    }
    for c in char::decode_utf16(units.into_iter()) {
        let c = c.unwrap_or('\u{FFFD}');
        let mut tmp = [0u8; 4];
        out.extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
    }
    out
}

/// Return the index of the first byte of the line containing `pos`, and the
/// index one past the end of that line (with any trailing newline included).
pub fn line_bounds(buf: &[u8], pos: usize) -> (usize, usize) {
    let start = match memrchr(b'\n', &buf[..pos]) {
        Some(i) => i + 1,
        None => 0,
    };
    let end = match memchr(b'\n', &buf[pos..]) {
        Some(i) => {
            let e = pos + i;
            if e + 1 <= buf.len() {
                e + 1
            } else {
                e
            }
        }
        None => buf.len(),
    };
    (start, end)
}