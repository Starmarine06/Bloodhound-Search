# Bloodhound (`bh`) ⚡

[![npm version](https://img.shields.io/npm/v/bloodhound-search.svg?style=flat-square&color=cb3837)](https://www.npmjs.com/package/bloodhound-search)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![Platform Support](https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-lightgrey.svg?style=flat-square)](#platform-support)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg?style=flat-square)](Cargo.toml)

> **Hyper-fast, multi-threaded code & file search engine built in Rust.**  
> Instant zero-install access via `npx`, raw SIMD-accelerated throughput, and persistent on-disk trigram indexing for sub-millisecond repeated queries.

---

## 🚀 Quick Start

### 1. Zero-Install via `npx` (Instant everywhere)
Run immediately on any machine with Node.js installed without installing Rust toolchains, compiler binaries, or package managers:

```bash
# Search file contents for 'TODO'
npx bloodhound-search "TODO"

# Fast file name search (regex)
npx bloodhound-search --name ".*\.tsx?$" --files

# Pass arguments directly with the alias
npx bloodhound-search -i "error" src/ -C 2
```

### 2. Global Install via `npm` (Fast `bh` command)
Install globally to get the instant `bh` alias anywhere in your terminal:

```bash
npm install -g bloodhound-search

# Search immediately with 'bh'
bh "database_connection"
```

### 3. Build & Install from Source (Rust Toolchain)
```bash
cargo install --path .
# Or build optimized release binary
cargo build --release
```

---

## ⚡ Why Bloodhound is Better

| Feature / Capability | **Bloodhound (`bh`)** | **ripgrep (`rg`)** | **GNU `grep`** | **`find`** |
| :--- | :---: | :---: | :---: | :---: |
| **Instant `npx` Zero-Install** | ✅ **Yes** (`npx bloodhound-search`) | ❌ No (requires brew/apt/choco) | ❌ No | ❌ No |
| **Persistent Trigram Indexing** | ✅ **Yes** (sub-ms queries) | ❌ No (cold scan only) | ❌ No | ❌ No |
| **Unified Content & File Search**| ✅ **Yes** (`bh` & `bh --name`) | ⚠️ Partial (`rg --files`) | ❌ No | ⚠️ Names only |
| **SIMD Literal Acceleration** | ✅ **Yes** (`memchr` + `aho-corasick`) | ✅ Yes | ❌ No | ❌ No |
| **Zero-Copy Memory Mapping** | ✅ **Yes** (`memmap2`) | ✅ Yes | ❌ No | ❌ No |
| **Gitignore & Filter Hierarchy** | ✅ **Yes** (`.gitignore`, `.ignore`) | ✅ Yes | ❌ Manual | ❌ Manual |
| **JSON-Lines Structured Output** | ✅ **Yes** (`--json` for AI / IDEs) | ✅ Yes | ❌ No | ❌ No |
| **Cross-Platform Native Binaries**| ✅ Linux, macOS & Windows | ✅ Yes | ⚠️ Unix first | ⚠️ Unix first |

### Key Advantages:

1. **Sub-Millisecond Repeated Lookups (`--index`)**  
   Traditional tools re-walk every directory and re-read every byte from disk on every search. For massive codebases, mono-repos, or frequent searches, Bloodhound builds a lightweight, LZ4-compressed inverted trigram index (`--index-dir`). Subsequent queries locate matching candidates in microseconds.
2. **Zero-Friction Node.js & CI/CD Integration**  
   Precompiled native Rust binaries are packaged for Linux (x64, arm64), macOS (Apple Silicon & Intel), and Windows (x64, arm64). In CI scripts, containerized builds, or developer terminals, just run `npx bloodhound-search` without configuring external package managers or compilers.
3. **Hardware-Vectorized Search Pipeline**  
   Combines multi-threaded parallel directory traversal (`ignore`), kernel zero-copy memory mapping (`memmap2`), multi-pattern matching (`aho-corasick`), and SIMD vectorized byte scanning (`memchr`) to saturate modern NVMe and CPU bandwidth.
4. **Agent & IDE Ready (`--json`)**  
   Designed for modern AI coding agents, plugins, and editor tooling. Stream clean, machine-parseable JSON-lines events for matches, file boundaries, contexts, and statistics.

---

## 📖 Practical Usage Guide

### 1. Content Search
```bash
# Basic case-sensitive search
bh "getUserProfile"

# Case-insensitive search (-i)
bh -i "apigateway" src/

# Strict case-sensitive match (-s)
bh -s "Config"

# Search using Regular Expressions
bh "pub (async )?fn [a-z_]+" src/

# Search multiple patterns (logical OR)
bh -e "DatabaseError" -e "ConnectionReset" src/

# Invert match (lines that do NOT match)
bh -v "DEBUG" server.log
```

### 2. Context & Display Control
```bash
# Show 2 lines before and after match (-C)
bh -C 2 "panic!" src/

# Show 3 lines before (-B) or 4 lines after (-A)
bh -B 3 -A 4 "throw new Error" src/

# Show line numbers (-n) or omit them (-N)
bh -n "handleClick" src/

# Only show matching substring (-o)
bh -o "https?://[^\s]+" docs/

# Color control (auto, always, never)
bh --color always "TODO" | less -R
```

### 2b. Heading & Column Modes
On a terminal, `bh` prints each file's path once as a heading and shows only `line:content`
per match (`--heading`). Piped output keeps the compact `path:line:content` format — switch
with `--no-heading` / `--heading`. Add byte columns for precise jump-to positions:

```bash
# Terminal (TTY): path heading once, then line:content
bh "batch" src/

# Force the one-line path:line:content format even in a terminal
bh --no-heading "batch" src/

# Show byte column of each match: path:line:col:content
bh --column "batch" src/

# Editable selector-friendly output: line:col per match after a heading
bh -o "import\s+\w+" src/ --heading --column
```

### 3. File Filtering & Traversal
```bash
# Only list files that contain matches (-l)
bh -l "import React"

# List files that do NOT contain matches
bh --files-without-match "MIT License"

# Filter by glob pattern (-g / --iglob)
bh "parse" -g "*.rs" -g "*.toml"
bh "styles" --iglob "*.SCSS"

# Limit search depth & file size
bh "token" --max-depth 3 --max-filesize 2M

# Search hidden files (-.) and follow symlinks (-L)
bh -. -L "secret_key"
```

### 4. File Name Search
Search file paths and names directly instead of file contents:
```bash
# Search for files matching regex pattern
bh --name ".*test.*\.tsx?$"

# List all files that would be traversed
bh --files
```

### 5. Persistent Trigram Indexing Mode
```bash
# Build or update index cache for current project
bh --index-dir .

# Force fast search against index
bh --index "calculateRevenue"

# Bypass index even if present
bh --no-index "raw_bytes"
```

### 6. JSON Streaming for Automation & AI Agents
```bash
bh "authenticate" src/ --json
```
Produces structured JSON events:
```json
{"type":"match","path":"src/auth.rs","line_number":42,"line":"pub fn authenticate(token: &str) -> bool {","submatches":[{"match":"authenticate","start":7,"end":19}]}
```

### 7. Performance Profiling
```bash
bh "State" src/ --stats
```
Outputs high-precision execution timings, total scanned bytes, files visited, and match tallies.

---

## ⚙️ CLI Options Reference

| Flag / Option | Description |
| :--- | :--- |
| `[PATTERN]` | Pattern to search for (or file name pattern with `--name`) |
| `[PATH]...` | Search paths (defaults to current directory `.`) |
| `-i, --ignore-case` | Case-insensitive matching |
| `-s, --case-sensitive` | Strict case-sensitive matching |
| `-e, --regexp <PAT>` | Additional patterns (logical OR) |
| `-f, --file <FILE>` | Read patterns from file (one per line) |
| `-C, --context <NUM>` | Show `NUM` lines before and after match |
| `-B, --before-context <N>` | Show `N` lines before match |
| `-A, --after-context <N>` | Show `N` lines after match |
| `-n, --line-number` | Print 1-indexed line numbers (default in TTY) |
| `-N, --no-line-number` | Suppress line numbers |
| `--heading` | Print each file path once as its own heading line (default in TTY) |
| `--no-heading` | Never use heading mode; repeat the path on every match line |
| `--column` | Print the byte column of the first match on each match line |
| `-l, --files-with-matches` | Print only paths containing matches |
| `--files-without-match` | Print only paths without matches |
| `-c, --count` | Print match count per file |
| `--count-matches` | Print total match count across all files |
| `-o, --only-matching` | Print only matched text |
| `-v, --invert-match` | Print non-matching lines |
| `-g, --glob <GLOB>` | Include paths matching glob (case-sensitive) |
| `--iglob <GLOB>` | Include paths matching glob (case-insensitive) |
| `--max-depth <NUM>` | Limit directory traversal depth |
| `--max-filesize <SZ>` | Skip files larger than size (e.g. `5M`, `500K`) |
| `-j, --threads <NUM>` | Thread count (defaults to CPU core count) |
| `-a, --text` | Scan binary files as text |
| `--encoding <ENC>` | File encoding (`utf8`, `utf16le`, `utf16be`, `latin1`) |
| `--name <PATTERN>` | Search file names instead of contents |
| `--files` | Print traversed file list without searching content |
| `--index-dir <PATH>` | Build / update on-disk persistent trigram index |
| `--index` | Use persistent index for instant lookup |
| `--no-index` | Bypass index and scan directly |
| `--json` | Stream results as JSON-lines |
| `--stats` | Print traversal metrics and timings to stderr |
| `-q, --quiet` | Suppress output; exit code indicates match found |

---

## 🏗 Platform Support

Bloodhound publishes pre-compiled native binaries for the following architectures:

- **Windows**: `x64` (`x86_64-pc-windows-msvc`), `arm64` (`aarch64-pc-windows-msvc`)
- **macOS**: Apple Silicon (`aarch64-apple-darwin`), Intel (`x86_64-apple-darwin`)
- **Linux**: `x64` (`x86_64-unknown-linux-musl`), `arm64` (`aarch64-unknown-linux-musl`)

When installed via `npm` or executed via `npx`, the appropriate native binary is automatically selected via npm's `optionalDependencies` without requiring local compilation or runtime overhead.

---

## 📦 Publishing & Maintenance

### Local Single-Platform Publishing
```bash
# 1. Build release binary
npm run build

# 2. Package for platform
npm run prepare-packages

# 3. Publish to npm
npm run publish-all
```

### Multi-Platform CI/CD Automated Publishing
A GitHub Actions workflow is included at `.github/workflows/release.yml`. When you push a git version tag:
```bash
git tag v0.1.0
git push origin v0.1.0
```
GitHub Actions will automatically matrix-build native binaries across Windows, macOS, and Linux runners, assemble the `@bloodhound-search/*` platform packages, and publish them with npm provenance.

---

## 📄 License

MIT © [Dev Nambiar](https://github.com/devnambiar)
