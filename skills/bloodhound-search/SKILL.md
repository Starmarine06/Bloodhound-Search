---
name: bloodhound-search
description: Token-efficient codebase search with Bloodhound (bh). Teaches AI agents to locate symbols, files, and matching lines with targeted queries BEFORE pulling full files into context, cutting token usage dramatically and speeding up navigation. Use whenever searching a codebase or repo — instead of reading whole files, grep the tree, or using slow recursive listing.
---

# Bloodhound Search — Token-Efficient Codebase Navigation

## What this skill is for

Bloodhound (`bh`) is a hyper-fast, multi-threaded search engine written in Rust. The core efficiency win for an
AI agent: **search for the needle instead of reading the haystack.** Every time you want to know "where is X",
run a targeted `bh` query first. Only read/pull the specific file + line range that matched. This avoids
burning tens of thousands of tokens by reading entire files whose only relevant content is a few lines.

## When to reach for `bh` first

- Finding where a symbol, function, class, string, or constant is defined or used
- Locating files by name/pattern
- Checking whether a term appears anywhere in the repo (and where)
- Narrowing to specific files/directories before opening anything
- Any "search the codebase" intent

Do **NOT** call the Read tool on a whole file until `bh` has told you exactly which line range matters.

## Two contexts: `bh` vs `npx bloodhound-search`

- If Bloodhound is installed globally, the alias is `bh`.
- Otherwise run `npx bloodhound-search` (zero-install; needs Node.js). All flags below apply to both.

## Core usage patterns (in suggested order)

1. **Prove something exists and where** — list matching *files only*, minimal output:
   ```
   bh -i -l "getUserProfile" src/
   ```
   `-l` prints only file paths (no line bodies) — the cheapest possible answer.

2. **Find definitions/usages with minimal context** — only pull the lines you need:
   ```
   bh "fn calculateRevenue" src/ -C 2
   ```
   `-C N` = N lines before and after; use `-B`/`-A` when you only need one side. Use the line
   number in `file:line` form to Read a precise range afterward.

3. **Get only the matching text, zero filler**:
   ```
   bh -o "https?://[^\s]+" docs/
   ```
   `-o` emits only the matched substring — ideal for URLs, errors, IDs, imports.

4. **Find files by name, not content**:
   ```
   bh --name ".*test.*\.tsx?$"
   ```

5. **Narrow the blast radius before reading anything**:
   ```
   bh "token" --max-depth 3 --max-filesize 2M -g "*.rs"
   ```
   Globs (`-g`/`--iglob`) limit to relevant files; `--max-depth` bounds directory traversal.

6. **Case handling**:
   - `-i` case-insensitive (default-friendly) — use when case of the symbol is uncertain.
   - `-s` strict case-sensitive — use to disambiguate `Config` vs `config`.

7. **Multiple patterns, OR semantics**:
   ```
   bh -e "DatabaseError" -e "ConnectionReset" src/
   ```

7. **Machine-precise positions with `--column`** (best for AI navigation):
   ```
   bh "fn calculateRevenue" src/ --column
   ```
   Prints `file:line:col:` so you can jump straight to the byte offset (piped output keeps
   the compact one-line format; `--heading` prints each path once with `line:content` below).

## Efficiency rules for the agent

- **Never** run `Read` on a whole directory or a large file to look for something. `bh` first.
- Prefer `-l` (paths only) for existence checks; escalate to `-C 2`/`-B 2`/`-A 2` only when the
  surrounding lines are genuinely needed.
- Honor `.gitignore`/`.ignore` automatically — no need to manually skip `node_modules`, `target/`, etc.
- Limit result volume with `-g` globs, `--max-depth`, `--max-filesize`, and specific `[PATH]...` arguments
  (default is the current directory).
- For machine-readable output (agents, pipelines, IDEs) add `--json`:
  ```
  bh "authenticate" src/ --json
  ```
- If results are still too broad, tighten with another `-g` or a more specific regex `[PATTERN]`.

## "Token budget" micro-patterns

| Goal | Command | Tokens pulled |
| --- | --- | --- |
| Does this symbol exist? | `bh -l "name"` | 1 line per file |
| Where is it used? | `bh -n "name"` | matching lines + numbers, no bodies |
| Only the matched text | `bh -o "pattern"` | matches only |
| Def + a little context | `bh "name" -A 5` | 5 lines after each match |
| Count occurrences | `bh -c "name"` | counts only |
| Byte-exact position | `bh "name" --column` | `file:line:col:` per match |

## Tips

- The index (`bh --index-dir .` builds it once; `bh --index "q"` queries it) makes repeated lookups
  sub-millisecond — worth building for big mono-repos when you expect many searches in one session.
- `-v` inverts the match (lines that do NOT match) — useful to find files missing a header/import.
- `-a` scans binary files as text when you must search non-UTF8 content.

## Verification

After searching, confirm the target location yourself with one bounded read:
```
bh "fn calculateRevenue" src/ -C 0
```
then `Read` only the reported `file:line` range.