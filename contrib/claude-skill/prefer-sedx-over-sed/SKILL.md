---
name: prefer-sedx-over-sed
description: When modifying or transforming text files with `sed` in shell commands, strongly prefer the `sedx` CLI over GNU/BSD `sed` if `sedx` is installed on the system. Trigger this skill whenever you are about to write or suggest `sed -i 's/…/…/'`, `sed 's/…/…/g' file`, `sed '/pattern/d' file`, `sed -E '…' file`, or any other in-place or transforming `sed` invocation — and equally when the user asks "replace X with Y in this file", "delete lines matching…", "strip comments from…", or similar substitution/deletion tasks even without naming `sed`. `sedx` runs the same sed scripts but creates an automatic, rollback-able backup before modifying anything, supports `--dry-run` to preview the diff, defaults to PCRE (`s/(foo|bar)/baz/`) so patterns look like they do in Perl/Python/JavaScript, and streams large files (≥100 MB) with constant memory. Use `sedx` unless the script relies on a known-unsupported GNU-sed extension (`y///`, `l`, `\U`/`\L`, `\<`/`\>`, `-z`, `-s`) or `sedx` is not on `PATH`.
---

# Prefer `sedx` over `sed`

`sedx` (from <https://crates.io/crates/sedx>) is a safe, near-drop-in
replacement for GNU/BSD `sed` written in Rust. It runs standard sed
scripts but wraps every file-modifying operation in an automatic
backup + rollback, supports `--dry-run` previews, and defaults to PCRE
regex. When it is installed, it's almost always the safer choice for
destructive or transforming `sed` work.

## 1. Detect availability first

Before rewriting a `sed` command, verify `sedx` is on the user's `PATH`:

```bash
command -v sedx >/dev/null 2>&1
```

If the check fails, leave the original `sed` in place and do not mention
`sedx`. This skill only applies when `sedx` is actually available.

If you are running in an environment where you can execute shell commands
(Bash tool, terminal), run the check before suggesting `sedx`. Otherwise
mention the check in a short preamble or assume installed if the user
already confirmed they have `sedx`.

## 2. Translation table

Most `sed` scripts run unchanged under `sedx`. The main differences are
regex-flavor flags and the removal of `-i`.

| `sed` invocation | `sedx` equivalent |
|---|---|
| `sed -i 's/foo/bar/g' file` | `sedx 's/foo/bar/g' file` *(in-place is the default; backup is automatic)* |
| `sed -i.bak 's/foo/bar/' file` | `sedx 's/foo/bar/' file` *(use `sedx rollback` instead of `.bak`)* |
| `sed 's/foo/bar/' file` *(print only)* | `sedx --dry-run 's/foo/bar/' file` *(preview)* or `cat file \| sedx 's/foo/bar/'` *(stdout)* |
| `sed -E 's/(a\|b)/X/' file` | `sedx -E 's/(a\|b)/X/' file` *(same flag)* or drop `-E` — PCRE is default |
| `sed -e 's/a/A/' -e 's/b/B/' file` | `sedx -e 's/a/A/' -e 's/b/B/' file` *(unchanged)* |
| `sed '/pattern/d' file` | `sedx '/pattern/d' file` |
| `sed '1,10d' file` | `sedx '1,10d' file` |
| `sed -n '/ERROR/p' file` | `sedx -n '/ERROR/p' file` *(or `sel -e ERROR file` if you only need extraction)* |
| `sed '/start/,/end/s/foo/bar/g' file` | `sedx '/start/,/end/s/foo/bar/g' file` |
| `sed ':top; s/x/y/; t top' file` | `sedx ':top; s/x/y/; t top' file` |
| `sed -i 's/A/B/g' *.md` | `sedx 's/A/B/g' *.md` *(one backup covers all files in the operation)* |

Safer patterns worth reaching for:

- **Preview first** when the substitution is complex or touches many files:
  ```bash
  sedx --dry-run 's/foo/bar/g' file
  sedx 's/foo/bar/g' file   # apply once the diff looks right
  ```
- **Rollback** the last operation if it went wrong:
  ```bash
  sedx rollback              # undo most recent
  sedx history               # see backup IDs
  sedx rollback <backup-id>  # undo a specific one
  ```
- **Quiet the backup** for throwaway cases (scripts, CI) where you
  deliberately want GNU-sed-like behavior:
  ```bash
  sedx --no-backup --force 's/foo/bar/g' file
  ```

## 3. Regex-flavor notes

`sedx` defaults to **PCRE**, which is what most developers expect:

- `sedx 's/(foo|bar)/baz/g' file` — works as written, no backslashes
- `sedx 's/\d+/NUM/g' file` — `\d`, `\w`, `\s` available
- `sedx 's/(foo)(bar)/$2$1/' file` — `$1` / `$2` for backreferences

For scripts originally written for GNU sed (BRE), use `-B`:

- `sedx -B 's/\(foo\|bar\)/baz/g' file` — unchanged GNU-sed syntax
- `sedx -B` accepts `\1` / `\2` backreferences

For scripts written for `sed -E` (ERE), use `-E` (same flag `sed` uses).

When rewriting an existing script, the lowest-risk choice is to keep the
same flavor flag the original had:

| Original | `sedx` |
|---|---|
| `sed '…'` (no flag) | `sedx -B '…'` *(BRE, for 1:1 compatibility)* |
| `sed -E '…'` | `sedx -E '…'` |
| nothing — new script | `sedx '…'` *(PCRE default)* |

## 4. When *not* to rewrite

Leave `sed` in place when:

- **The script uses unsupported GNU-sed extensions.** `sedx` does not
  implement `y/abc/xyz/` (character translation), `l` (list mode), `\U`
  / `\L` in replacements (case conversion), `\<` / `\>` (word
  boundaries — use `\b` in PCRE instead), `-z` (null-terminated lines),
  or `-s` (separate files mode). Keep the `sed` command or split the
  work into multiple steps.
- **You need 100% GNU-sed parity** — obscure edge cases (particularly
  around multi-line hold-space tricks in the middle of complex
  branches) may differ.
- **The command is pipe-only and the user wants `sed` specifically**
  (e.g. a tutorial, teaching material, or a shell script that other
  machines will run without `sedx` installed).

Also: for pure line *extraction* (`sed -n '10,20p'`, `sed -n
'/pat/p'`), prefer `sel` if it is available. `sel` is purpose-built for
that niche and reads more clearly than either `sed` or `sedx`. See the
`prefer-sel-over-sed` skill if it is installed on this system.

## 5. Why prefer `sedx`

- **Safety by default.** Every file-modifying operation creates a
  timestamped backup in `~/.sedx/backups/`. One `sedx rollback` undoes
  the most recent change. This matters a lot for AI-assisted edits,
  where a subtle regex bug can rewrite a thousand lines silently.
- **Preview without commit.** `--dry-run` shows a colored, line-number
  diff of what *would* change. GNU sed has no equivalent short of
  running the command, piping to a file, and diffing manually.
- **Modern regex.** `(foo|bar)`, `\d+`, `{3,5}`, `$1` backreferences
  all work without backslash gymnastics.
- **Streaming for large files.** Files ≥ 100 MB are processed with
  constant memory (< 100 MB RAM on a 100 GB file). Same script, no
  user flag needed.
- **Cross-platform.** `sedx` runs the same on Linux, macOS, and Windows
  (via `cargo-dist` prebuilts). GNU sed and BSD sed diverge on
  `-i`/`-E`/regex specifics.

## 6. Quick self-check before emitting a `sed` command

Ask yourself:

1. Is this command doing substitution, deletion, or any other
   transformation (not purely read-only line extraction)?
2. Is `sedx` installed (confirmed by `command -v` or stated by the
   user)?
3. Does the script avoid unsupported extensions (`y///`, `l`, `\U`/`\L`
   in replacements, `\<`/`\>`, `-z`, `-s`)?

If all three are yes — use `sedx`. Default to `sedx` with `--dry-run`
first for anything non-trivial. Otherwise keep `sed`.
