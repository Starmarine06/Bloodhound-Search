use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const CURRENT_VERSION: u32 = 1;
const MAX_WORDS_PER_FILE: usize = 200_000;

#[derive(Serialize, Deserialize)]
pub struct FileEntry {
    pub rel: String,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Serialized {
    pub version: u32,
    pub root: String,
    pub files: Vec<FileEntry>,
    pub postings: HashMap<String, Vec<u32>>,
}

pub struct Index {
    path: PathBuf,
}

pub struct UpdateResult {
    /// Total number of files in the tree.
    pub total_files: usize,
    /// Files whose content was actually tokenized during this update.
    pub files_tokenized: usize,
    /// Files skipped because their size+mtime were unchanged.
    pub files_reused: usize,
}

impl Index {
    /// The on-disk location for the index that covers `root`.
    pub fn open(root: &Path) -> Index {
        let root_abs = absolutize(root);
        let hash = fnv1a(&root_abs);
        let base = match std::env::var_os("BLOODHOUND_INDEX_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => {
                if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                    PathBuf::from(local).join("bloodhound").join("index")
                } else if let Some(cache) = std::env::var_os("XDG_CACHE_HOME") {
                    PathBuf::from(cache).join("bloodhound")
                } else {
                    std::env::var_os("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join(".cache")
                        .join("bloodhound")
                }
            }
        };
        Index {
            path: base.join(format!("{hash:016x}.bhx")),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[allow(dead_code)]
    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    pub fn load(&self) -> Option<Serialized> {
        let bytes = std::fs::read(&self.path).ok()?;
        let raw = lz4_flex::decompress_size_prepended(&bytes).ok()?;
        let (store, _) =
            bincode::serde::decode_from_slice::<Serialized, _>(&raw, bincode::config::standard())
                .ok()?;
        if store.version != CURRENT_VERSION {
            return None;
        }
        Some(store)
    }

    /// Build (or incrementally update) the index from the files discovered in
    /// `root_files`, each an absolute path. Entries whose size and mtime are
    /// unchanged from a previous index are reused without re-reading contents.
    pub fn update(&self, root: &Path, root_files: &[PathBuf]) -> anyhow::Result<UpdateResult> {
        let root_abs = absolutize(root);
        let old = self.load();

        let mut cur: Vec<(String, u64, i64)> = Vec::with_capacity(root_files.len());
        for abs in root_files {
            let Ok(meta) = std::fs::metadata(abs) else { continue };
            if !meta.is_file() {
                continue;
            }
            let rel = abs
                .strip_prefix(&root_abs)
                .unwrap_or(abs)
                .to_string_lossy()
                .replace('\\', "/");
            let mtime = meta.modified().ok().map(ts).unwrap_or(0);
            cur.push((rel, meta.len(), mtime));
        }

        // old rel -> (size, mtime, old_id)
        let mut old_by_rel: HashMap<String, (u64, i64, usize)> = HashMap::new();
        if let Some(old) = &old {
            for (i, e) in old.files.iter().enumerate() {
                old_by_rel.insert(e.rel.clone(), (e.size, e.mtime, i));
            }
        }

        let mut files: Vec<FileEntry> = Vec::with_capacity(cur.len());
        let mut id_map: HashMap<usize, u32> = HashMap::new();
        let mut tokenize_list: Vec<(u32, PathBuf)> = Vec::new();
        let mut files_reused = 0usize;
        for (rel, size, mtime) in &cur {
            let new_id = files.len() as u32;
            match old_by_rel.get(rel) {
                Some((osz, omt, old_id)) if *osz == *size && *omt == *mtime => {
                    id_map.insert(*old_id, new_id);
                    files_reused += 1;
                }
                _ => {
                    tokenize_list.push((new_id, build_abs(&root_abs, rel)));
                }
            }
            files.push(FileEntry {
                rel: rel.clone(),
                size: *size,
                mtime: *mtime,
            });
        }

        // Reincarnate postings for kept files, remapping old ids to new ids.
        let mut postings: HashMap<String, Vec<u32>> = HashMap::new();
        if let Some(old) = &old {
            for (word, ids) in &old.postings {
                let new_ids: Vec<u32> = ids
                    .iter()
                    .filter_map(|id| id_map.get(&(*id as usize)).copied())
                    .collect();
                if !new_ids.is_empty() {
                    postings.insert(word.clone(), new_ids);
                }
            }
        }

        // Tokenize new/changed files and add postings.
        let mut files_tokenized = 0usize;
        for (id, abs) in tokenize_list {
            let Ok(buf) = std::fs::read(&abs) else { continue };
            files_tokenized += 1;
            for word in tokenize(&buf) {
                rename_push(&mut postings, &word, id);
            }
        }

        let store = Serialized {
            version: CURRENT_VERSION,
            root: root_abs.clone(),
            files,
            postings,
        };
        let encoded = bincode::serde::encode_to_vec(&store, bincode::config::standard())
            .map_err(|e| anyhow::anyhow!("encode: {e}"))?;
        let compressed = lz4_flex::compress_prepend_size(&encoded);
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, &compressed)?;
        std::fs::rename(&tmp, &self.path)?;

        Ok(UpdateResult {
            total_files: cur.len(),
            files_tokenized,
            files_reused,
        })
    }

    /// Candidate file ids whose content contains every query word, or None if
    /// the index cannot answer (a word is missing from the postings).
    pub fn candidates(&self, store: Option<&Serialized>, words: &[String]) -> Option<Vec<u32>> {
        let store = store?;
        let mut acc: Option<HashSet<u32>> = None;
        for word in words {
            let ids = store.postings.get(word)?;
            let cur: HashSet<u32> = ids.iter().copied().collect();
            acc = Some(match acc {
                None => cur,
                Some(a) => a.intersection(&cur).copied().collect(),
            });
        }
        acc.map(|s| {
            let mut v: Vec<u32> = s.into_iter().collect();
            v.sort_unstable();
            v
        })
    }

    #[allow(dead_code)]
    pub fn entry_count(&self, store: &Serialized) -> usize {
        store.files.len()
    }

    pub fn entry_path(&self, store: &Serialized, id: u32) -> PathBuf {
        let rel = store.files.get(id as usize).map(|e| e.rel.as_str()).unwrap_or("");
        PathBuf::from(&store.root).join(rel)
    }
}

fn rename_push(map: &mut HashMap<String, Vec<u32>>, word: &str, id: u32) {
    match map.entry(word.to_string()) {
        Entry::Occupied(mut o) => {
            let v = o.get_mut();
            if !v.contains(&id) {
                v.push(id);
            }
        }
        Entry::Vacant(v) => {
            v.insert(vec![id]);
        }
    }
}

fn tokenize(buf: &[u8]) -> Vec<String> {
    let mut words: HashSet<String> = HashSet::new();
    let mut cur = Vec::with_capacity(32);
    for &b in buf {
        if b.is_ascii_alphanumeric() || b == b'_' {
            cur.push(if b.is_ascii_uppercase() { b + 32 } else { b });
        } else if !cur.is_empty() {
            if words.len() < MAX_WORDS_PER_FILE {
                // SAFETY: cur only ever holds ASCII bytes.
                words.insert(unsafe { String::from_utf8_unchecked(std::mem::take(&mut cur)) });
            }
            cur.clear();
        }
    }
    if !cur.is_empty() {
        words.insert(unsafe { String::from_utf8_unchecked(cur) });
    }
    words.into_iter().collect()
}

fn ts(t: std::time::SystemTime) -> i64 {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(_) => 0,
    }
}

fn absolutize(p: &Path) -> String {
    let abs = std::fs::canonicalize(p).unwrap_or_else(|_| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(p)
        }
    });
    abs.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_string()
}

fn build_abs(root: &str, rel: &str) -> PathBuf {
    PathBuf::from(root).join(rel.replace('/', std::path::MAIN_SEPARATOR_STR))
}

pub fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in s.as_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}