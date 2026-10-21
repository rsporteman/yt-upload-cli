# Tasks

## 1. Setup

- [x] 1.1 Create branch `feat/global-env-vars`, add `winreg` under `[target.'cfg(windows)'.dependencies]` in `Cargo.toml`, and verify `cargo build` and `cargo clippy --all-targets -- -D warnings` pass on Windows

## 2. Process environment variables (clap `env`)

- [x] 2.1 Add `env = "YT_UPLOAD_CLI_PRIVACY|CATEGORY|TAGS|PLAYLIST|MADE_FOR_KIDS"` to the matching `UploadArgs` fields in `src/lib.rs`, with `BoolishValueParser` for `made_for_kids`; verify `cargo run -- upload --help` shows `[env: YT_UPLOAD_CLI_*]` for each of the five options and not for title/description/thumbnail/publish-at
- [x] 2.2 Add `tests/cli.rs` that spawns `env!("CARGO_BIN_EXE_yt-upload")` with `Command::env`/`env_remove`: `config-dir` prints `YT_UPLOAD_CLI_HOME`, `upload --help` lists all five variables, an empty `YT_UPLOAD_CLI_HOME` falls back to the default folder, and an invalid `YT_UPLOAD_CLI_PRIVACY` exits non-zero listing `private, unlisted, public`; verify `cargo test --test cli` passes

## 3. Pure helpers in `src/env.rs`

- [x] 3.1 Create `src/env.rs` (declared in `lib.rs`) with `Persisted { value, source }` and `parse_env_file` (blank/`#` lines skipped, optional `export ` stripped, one pair of matching quotes removed); verify table tests for each rule pass
- [x] 3.2 Add `lookup_env_dirs(name, user_dir, system_file)` (sorted `*.conf`, last wins, user before system, missing dir/file â†’ `None`, empty value â†’ `None`); verify tests with `tempfile` dirs covering override order, missing paths and empty values
- [x] 3.3 Add `expand_percent(value, get)` (known `%X%` expanded, unknown left as-is, lone `%` kept); verify unit tests pass on all OSes

## 4. OS lookups

- [x] 4.1 Implement `os_lookup` for non-Windows, non-macOS Unix using `dirs::config_dir()/environment.d` and `/etc/environment`; verify a test that an unlikely variable name returns `None` and that coverage of `src/env.rs` stays â‰¥95% with the coverage command from AGENTS.md
- [x] 4.2 Implement the Windows path: `read_registry(root, path, name)` via `winreg` for HKCU `Environment` then HKLM `Session Manager\Environment`, expanding with `expand_percent`; verify a `#[cfg(windows)]` test that writes REG_SZ and REG_EXPAND_SZ values under a temporary `HKCU\Software\yt-upload-cli-test-<pid>` key (removed by a drop guard) reads and expands them, and manually verify `setx YT_UPLOAD_CLI_HOME D:\yt` then `yt-upload config-dir` in an already-open terminal prints `D:\yt` (then `reg delete HKCU\Environment /v YT_UPLOAD_CLI_HOME /f`)
- [x] 4.3 Implement the macOS path with `launchctl getenv` (trim newline, empty/failed â†’ `None`); verify a `#[cfg(target_os = "macos")]` test that an unset name returns `None` passes on the macOS CI runner

## 5. Precedence and wiring

- [x] 5.1 Add `parse_cli(args, lookup)` in `src/lib.rs` using `ArgMatches::value_source` to apply persisted values only when the source is absent or `DefaultValue`, returning errors that name the variable, the `Persisted.source` and the allowed values; verify unit tests with a closure lookup for: privacy/category/tags/playlist/made-for-kids/config-dir fallback, flag beats persisted, invalid persisted value error text, invalid persisted value ignored when the flag is given, empty persisted value ignored, title never looked up
- [x] 5.2 Switch `src/main.rs` to `parse_cli(std::env::args_os(), env::os_lookup)`, exiting via `clap::Error::exit()` for help/usage errors; verify `tests/cli.rs` still passes and `cargo run -- --help` / `cargo run -- upload` (missing args) behave as before
- [x] 5.3 Update `README.md`: an "Environment variables" section with the table of variables, precedence, how to set them globally on Windows (`setx`/Settings), macOS (`launchctl setenv`) and Linux (`~/.config/environment.d/*.conf`), and the note that shell-profile-only variables are not read; verify the README matches `yt-upload upload --help`

## 6. Integration checks

- [x] 6.1 Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `cargo llvm-cov --ignore-filename-regex 'src[\\/](main|oauth)\.rs$' --fail-under-lines 95`; verify all pass
- [ ] 6.2 Open a PR titled `feat: read config from global environment variables` describing the change, the new `winreg` dependency and how it was tested; verify the CI matrix (Linux/Windows/macOS), coverage and PR-title checks are green
