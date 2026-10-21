# Design

## Context

- The CLI is a clap derive struct in `src/lib.rs`; `src/main.rs` calls `Cli::parse()` and is
  excluded from coverage. Only `--config-dir` has `env = "YT_UPLOAD_CLI_HOME"`, and clap reads it
  from the process environment only.
- The crate uses edition 2024, where `std::env::set_var` is `unsafe`, and `main` runs inside
  `#[tokio::main]`, so worker threads already exist when parsing starts.
- CI runs tests on Linux, Windows and macOS, but the 95% coverage gate runs on Linux only.
- See proposal.md for motivation and specs/env-config/spec.md for the behavior contract.

## Goals / Non-Goals

**Goals:**
- One parse entry point in `lib.rs` that applies the full precedence chain and is unit-testable
  with an injected lookup, so no test touches the real environment or registry of the machine.
- Keep OS-specific code thin; put the parsing logic (env files, `%VAR%` expansion) in pure
  functions tested on every platform.

**Non-Goals:**
- A config file format for yt-upload-cli.
- `${VAR}` expansion inside `environment.d` values, `export`-style shell syntax beyond stripping a
  leading `export `, or reading per-session PAM variables.

## Decisions

### 1. Process environment through clap's `env` attribute
Add `env = "YT_UPLOAD_CLI_..."` to `privacy`, `category`, `tags`, `playlist` and
`made_for_kids`. Clap then gives flag > env > default precedence, shows `[env: ...]` in `--help`,
and treats empty values as absent. For the `made_for_kids` flag, clap's env handling for
`SetTrue` flags uses falsey parsing; use `BoolishValueParser` explicitly so the accepted values
match the spec (`true/false`, `1/0`, `yes/no`, `on/off`).

*Alternative:* resolve the process environment ourselves too. Rejected: duplicates what clap does
and loses the help text.

### 2. Persisted fallback applied after parsing, using `ArgMatches::value_source`
New `pub fn parse_cli<I, T>(args: I, lookup: impl Fn(&str) -> Option<Persisted>) -> Result<Cli>`
in `lib.rs`:

1. `Cli::command().try_get_matches_from(args)` → `ArgMatches`, then `Cli::from_arg_matches`.
2. For each supported arg (global `config_dir`; `upload`'s sub-matches for the others), if
   `value_source(id)` is `None` or `DefaultValue`, call `lookup(VAR)`.
3. Parse a found value with the same rules as the flag (`Privacy::from_str(v, true)`,
   `BoolishValueParser`, plain strings otherwise) and overwrite the field. On error, return an
   `anyhow` error like `invalid value 'secret' for YT_UPLOAD_CLI_PRIVACY (from the Windows user
   environment); possible values: private, unlisted, public`.

`main.rs` becomes `let cli = parse_cli(std::env::args_os(), env::os_lookup)` and keeps clap's
own exit behavior for `--help`/usage errors by calling `err.exit()` on `clap::Error`.

*Alternatives:* `Command::mut_arg(..).default_value(v)` — rejected: clap debug-asserts on invalid
default values and the error would not name the OS source. `unsafe { set_var }` before parsing —
rejected: unsound with the tokio runtime already running. Rewriting argv — rejected: placement
of global vs. subcommand flags makes it fragile.

### 3. New module `src/env.rs`
- `pub struct Persisted { pub value: String, pub source: &'static str }` (`source` like
  `"Windows user environment"`, `"launchd"`, `"~/.config/environment.d"`, `"/etc/environment"`).
- `pub fn os_lookup(name: &str) -> Option<Persisted>`: `#[cfg]`-dispatched; every I/O failure
  maps to `None` (spec: unavailable source is silent).
- Pure, always-compiled helpers: `parse_env_file(&str) -> Vec<(String, String)>` (skip blanks and
  `#`, strip optional `export `, trim one pair of matching quotes), `lookup_env_dirs(name,
  user_dir, system_file)` (sorted `*.conf` in the user dir, last wins; then system file), and
  `expand_percent(value, get: impl Fn(&str) -> Option<String>)` (unknown `%X%` left as-is, like
  Windows does).
- **Windows**: `winreg` reads `HKCU\Environment` then
  `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`, reading the raw value for
  both `REG_SZ` and `REG_EXPAND_SZ` and expanding with `expand_percent` against the process env
  (falling back to persisted lookup is not needed for `%USERPROFILE%`-style variables). Registry
  reading goes through `read_registry(root, path, name)` so tests can target a throwaway
  `HKCU\Software\yt-upload-cli-test-<pid>` key instead of the real environment.
- **macOS**: `launchctl getenv NAME`; trim the trailing newline; empty output or non-zero exit →
  `None`.
- **Other Unix**: `lookup_env_dirs(name, dirs::config_dir()/environment.d, "/etc/environment")`.

*Alternative:* `windows-sys` + `RegGetValueW`/`ExpandEnvironmentStringsW`. Rejected: more unsafe
FFI for the same result; `winreg` is small and Windows-only (`[target.'cfg(windows)'.dependencies]`).

### 4. Testing
- `parse_cli` unit tests with a closure-based lookup cover: fallback used, flag beats persisted,
  invalid persisted value error text, invalid persisted value ignored when a flag is given,
  `config_dir` fallback, empty persisted value ignored.
- Process-environment behavior (clap `env`) is tested in `tests/cli.rs` by spawning
  `env!("CARGO_BIN_EXE_yt-upload")` with `Command::env` / `env_remove` (no new dependency):
  `config-dir` with `YT_UPLOAD_CLI_HOME`, and `upload --help` listing the variables. This avoids
  `set_var` in tests.
- Pure helpers get table tests on every OS; the Linux `os_lookup` path is covered by the
  coverage job; the Windows registry reader is tested on the Windows CI runner against a
  temporary key; macOS `launchctl` gets a smoke test for an unset name returning `None`.

## Risks / Trade-offs

- [`launchctl getenv` spawns a process per missing variable] → only called for supported
  variables not set by flag or process env (at most 6 per run, a few ms each).
- [Persisted value differs from what the user sees in the current shell] → precedence keeps the
  process env first, and errors name the OS source.
- [Shell-profile-only variables still invisible to processes not started from that shell] →
  documented in README; reading/executing profiles is out of scope by design.
- [Registry tests leaving keys behind on failure] → use a per-process key name and delete it in a
  drop guard.
- [Coverage of `cfg(windows)`/`cfg(target_os = "macos")` code is not measured] → keep those
  functions to a few lines; logic lives in shared helpers covered on Linux.

## Migration Plan

Additive: no existing flag or variable changes meaning. Rollback is reverting the PR.
