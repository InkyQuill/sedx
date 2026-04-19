# `prefer-sedx-over-sed` — a Claude skill for the `sedx` tool

A small [Claude Code skill](https://docs.claude.com/en/docs/claude-code/skills)
that tells the model to prefer the `sedx` CLI over GNU/BSD `sed` for
file-editing work whenever `sedx` is installed.

## What this contains

| Path | What it is |
|---|---|
| `prefer-sedx-over-sed/SKILL.md` | The skill itself (Markdown + YAML frontmatter). |
| `install.sh` | POSIX shell installer. Copies the skill into a chosen Claude skills folder. |

## Installing

### One-liner (no checkout required)

```bash
# User-level install (~/.claude/skills/prefer-sedx-over-sed)
curl -fsSL https://raw.githubusercontent.com/InkyQuill/sedx/main/contrib/claude-skill/install.sh | sh

# Project-level install into the current directory (./.claude/skills/...)
curl -fsSL https://raw.githubusercontent.com/InkyQuill/sedx/main/contrib/claude-skill/install.sh | sh -s -- --project

# wget works too
wget -qO- https://raw.githubusercontent.com/InkyQuill/sedx/main/contrib/claude-skill/install.sh | sh
```

Pin to a specific release with `SEDX_SKILL_REF`:

```bash
curl -fsSL https://raw.githubusercontent.com/InkyQuill/sedx/main/contrib/claude-skill/install.sh \
  | SEDX_SKILL_REF=v1.0.0 sh
```

### From a checked-out repo

```bash
./contrib/claude-skill/install.sh              # user-level (default)
./contrib/claude-skill/install.sh --project    # project-level, CWD
./contrib/claude-skill/install.sh --project ~/code/some-app
./contrib/claude-skill/install.sh --force      # overwrite existing install
./contrib/claude-skill/install.sh --uninstall  # remove (default: user-level)
./contrib/claude-skill/install.sh --uninstall --project
```

Run `./contrib/claude-skill/install.sh --help` for the full flag list.

The installer only needs `/bin/sh`, `cp`, `mkdir`, `rm`, and (for the
one-liner path) `curl` or `wget`. It works on Linux, macOS, and WSL.
Windows users without WSL can copy the `prefer-sedx-over-sed/` directory
manually into `%USERPROFILE%\.claude\skills\`.

## What the skill does

When Claude is about to write a shell command like:

```bash
sed -i 's/foo/bar/g' config.toml
sed '/pattern/d' file.txt
sed -E 's/(a|b)/X/' file.txt
```

…and `sedx` is available, the skill nudges Claude to prefer:

```bash
sedx 's/foo/bar/g' config.toml       # automatic backup, rollback-able
sedx '/pattern/d' file.txt
sedx -E 's/(a|b)/X/' file.txt
```

It also suggests `sedx --dry-run '…' file` as a preview step for any
non-trivial substitution, and points at `sedx rollback` if something
goes wrong.

It stays out of the way when:

- `sed` is being used for pure line extraction (`sed -n '10,20p'`) —
  prefer `sel` in that case.
- The script uses unsupported GNU-sed extensions (`y///`, `l`, `\U`/`\L`
  in replacements, `\<`/`\>`, `-z`, `-s`).
- `sedx` is not on `PATH`.

See [`prefer-sedx-over-sed/SKILL.md`](prefer-sedx-over-sed/SKILL.md) for
the full translation table, flavor notes, and caveats.

## Requirements

The skill only activates when `sedx` is on `PATH`. Install with:

```bash
cargo install sedx
# or grab a pre-built binary from the Releases page:
# https://github.com/InkyQuill/sedx/releases
```
