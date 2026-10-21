# Proposal

## Why

The README's "Usage" block is hand-copied from `yt-upload --help` and already drifts from it
(no environment variables, options without descriptions, no top-level commands). Two in-flight
changes (`global-env-vars`, PR #3, and `thumbnail-when-allowed`) add new ways to use the CLI, and
each one relies on someone remembering to edit the README. AGENTS.md asks to "keep README.md in
sync with `yt-upload --help`", but nothing enforces it, so new usage goes undocumented and removed
usage lingers.

## What Changes

- The README "Usage" section becomes **generated** from the real CLI help (top-level help and
  `upload --help`), between `<!-- usage:start -->` / `<!-- usage:end -->` markers. Its output is
  deterministic: fixed program name `yt-upload`, fixed width, and no environment *values*, only
  the variable names.
- A versioned git **pre-commit hook** (`.githooks/pre-commit`) regenerates that section and stages
  `README.md`, so every commit carries up-to-date usage. Enabled once per clone with
  `git config core.hooksPath .githooks`.
- A test in `cargo test` (and therefore CI) **fails when the README is out of date**, and also
  when any `yt-upload ...` command shown in the README no longer parses with the current CLI. This
  catches legacy flags and commands that were removed.
- The existing README text is reviewed and **rewritten**: new usage (environment variables,
  thumbnail behavior, `auth`/`config-dir`) documented, outdated statements removed, and the
  hand-written option list replaced by the generated block.
- CLI help gets descriptions for options that currently show none (`--title`, `--description`,
  `--privacy`) and an explicit program name, since the help is now user-facing documentation.
- AGENTS.md: "Before every commit" and "Documentation" are updated to explain the generated
  section, the hook, and the remaining manual duty, which is reviewing the prose and examples
  whenever usage changes.

## Capabilities

### New Capabilities

- `readme-sync`: keeping the README's usage documentation identical to the CLI's real help and
  free of commands the CLI no longer accepts, enforced at commit time and in CI.

### Modified Capabilities

None (no specs exist yet).

## Impact

- New files: `.githooks/pre-commit`, `tests/readme.rs`.
- Changed: `README.md` (rewrite plus markers), `AGENTS.md`, `src/lib.rs` (help text and command
  name only; no behavior change).
- No new dependencies. CI unchanged: `cargo test` already runs on all three OSes.
- Sequencing: implement after PR #3 (`global-env-vars`) and the `thumbnail-when-allowed` branch
  are merged, so the generated usage and the rewritten text cover them. Otherwise the work is
  repeated.
- Release: `docs:` PR (no version bump, appears in the changelog).
