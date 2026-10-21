# Design

## Context

- `Cli` is a public clap derive struct in `src/lib.rs`, so an integration test can build its
  `clap::Command` with `CommandFactory` and render help in-process. No binary spawn is needed.
- Runtime help is not deterministic. The program name comes from `argv[0]` (`yt-upload.exe` on
  Windows), and clap prints current env values (`[env: YT_UPLOAD_CLI_HOME=...]`, seen while
  implementing `global-env-vars`). Without `#[command(name)]`, `Cli::command()` is named after
  the package, `yt-upload-cli`.
- `cargo test` already runs in CI on Linux, Windows and macOS, and AGENTS.md requires it before
  every commit.
- See proposal.md for motivation and specs/readme-sync/spec.md for required behavior.

## Goals / Non-Goals

**Goals:**
- One source of truth for usage (the clap definitions); README follows automatically.
- Zero new dependencies and no new tooling to install (no Python `pre-commit`, no Node husky).

**Non-Goals:**
- Generating the prose sections (Setup, Limits, Environment variables). Those stay hand-written,
  guarded by the example-parsing check and the AGENTS.md rule.
- Man pages or a docs website for the CLI.

## Decisions

### 1. Generator lives in `tests/readme.rs`, with an update mode
- `usage_markdown() -> String` builds `Cli::command()`, applies `.term_width(100)`, sets
  `.bin_name("yt-upload")`, sets `hide_env_values(true)` on every arg (recursively for
  subcommands), and renders `render_long_help()` for the root and for `upload`. Both are wrapped
  in fenced blocks under small headings.
- The test `readme_usage_is_current` reads README.md, replaces the text between the markers, and
  then either:
  - when `UPDATE_README=1`: writes the file if it changed, or
  - otherwise: asserts equality, with the message
    `README usage is out of date; run: UPDATE_README=1 cargo test --test readme`.
- Line endings are normalized to `\n` before comparing, so a Windows checkout with
  `core.autocrlf=true` does not fail.

*Alternatives:* a `cargo xtask`, or an `examples/` generator. Rejected because they add
structure for one function, and the test must exist anyway to enforce sync in CI. A `build.rs`
writing README was rejected because builds must not modify the source tree.

### 2. Example check in the same test file
`readme_examples_parse` scans fenced blocks outside the markers. For each line starting with
`yt-upload `, it skips lines containing `<` or `[`, splits the rest with a small quote-aware
splitter (double quotes only, enough for README examples), and runs
`Cli::try_parse_from(words)`. The `--help`/`--version` "errors" count as success. All failures
are collected and reported together.

*Alternative:* `shell-words` crate. Rejected because a 15-line splitter avoids a dependency.

### 3. Hook in `.githooks/pre-commit` (POSIX sh)
```
#!/bin/sh
set -e
if ! git diff --quiet -- README.md; then
  echo "pre-commit: README.md has unstaged changes; stage or stash them first." >&2
  exit 1
fi
UPDATE_README=1 cargo test --quiet --test readme
git add README.md
```
- Committed with the executable bit (`git update-index --chmod=+x`), which Git for Windows
  honors through its bundled sh.
- `git add README.md` is safe because the first check guarantees there were no unstaged edits.
- Opt-in through `core.hooksPath`, since git cannot enable hooks automatically and the project
  must not modify user config behind their back. CI is the backstop for anyone without the hook.

### 4. Help text is now documentation
Add `#[command(name = "yt-upload")]`, plus doc comments for `--title`, `--description` and
`--privacy`, so the generated block is self-explanatory. Help-only change; parsing is unaffected.

### 5. README restructure
The intro example and the Setup steps stay as prose. "Usage" becomes a short sentence plus the
markers, and the hand-written option list is deleted. The "Environment variables" table (from
PR #3) and the thumbnail notes are kept and checked against the final behavior. The
`config-dir` and `auth` commands are covered by the generated top-level help.

## Risks / Trade-offs

- [clap upgrade changes help formatting] → the test fails with a clear regeneration command, and
  a regenerated README in a dependency-bump PR is expected.
- [Hook adds a `cargo test --test readme` compile to each commit] → only the small test target
  is built; incremental builds keep it to a few seconds.
- [Contributors never enable the hook] → CI fails the PR with the regeneration command, so the
  README cannot silently drift.
- [Example check misses commands in inline code or prose] → limited to fenced blocks by design.
  Documented in AGENTS.md.

## Migration Plan

Merge after PR #3 and `thumbnail-when-allowed`. After merging, each clone runs
`git config core.hooksPath .githooks` once (documented in AGENTS.md). Rollback: revert the PR
and unset `core.hooksPath`.
