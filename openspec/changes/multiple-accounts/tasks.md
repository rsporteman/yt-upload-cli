# Tasks

## 1. Account store

- [x] 1.1 Create `src/accounts.rs` (declared in `lib.rs`) with `Channel`, `Store`, `validate_alias`, `token_path`, `list`, `save_channel`, `remove` per design decisions 1–2; verify unit tests with `tempfile` for the alias table (valid, uppercase, spaces, leading `-`, 33 chars), sorted listing with `-` for a missing `channel.json`, and remove for existing and unknown aliases
- [x] 1.2 Add `migrate_legacy`; verify tests: legacy `token.json` moved to `accounts/default/token.json` and returns `true`; no-op returning `false` when there is no legacy token or when accounts already exist
- [x] 1.3 Add `resolve(store, requested, Purpose)`; verify table tests: 0 accounts (Auth → `default`, Upload → "run yt-upload auth"), 1 account (used automatically), 2 accounts without a request (error listing both aliases and mentioning `--account`), unknown alias (Upload errors listing aliases; Auth accepts it as a new account after validation)
- [x] 1.4 Add `register(store, alias, channels)`; verify tests: no channels → error and alias folder removed; channel id saved under another alias → error naming it and folder removed; success writes `channel.json` and returns the channel

## 2. API and OAuth

- [x] 2.1 Replace `Api::channel_names` with `Api::channels() -> Vec<Channel>` (id, `snippet.title`, `snippet.customUrl`) and adapt `auth_message`; verify an updated `wiremock` test and the `auth_messages` test
- [x] 2.2 Add `with_account_chooser(url)` in `lib.rs` (appends `prompt=select_account%20consent`, keeps an existing `prompt=`, handles URLs with and without `?`); verify unit tests for the three cases
- [x] 2.3 Change `oauth::access_token` to take the account's token path and pass the consent URL through `with_account_chooser`; verify `cargo build` and clippy pass (file excluded from coverage)

## 3. CLI and wiring

- [x] 3.1 Add `--account` (env `YT_UPLOAD_CLI_ACCOUNT`) to `auth` and `upload`, the `ACCOUNT_VAR` OS fallback in `parse_cli`, and the `accounts list` / `accounts remove <ALIAS>` subcommands; verify `parse_cli` unit tests for flag, OS fallback on both commands, and the new subcommands
- [x] 3.2 Wire `main.rs` per design decision 5 (migration note, resolve, register after auth, stderr target line before upload, list/remove output via lib formatting functions); verify `tests/cli.rs` cases against a temp `--config-dir`: `accounts list` stdout format and order, empty list hint on stderr with exit 0, `accounts remove` success and unknown alias, `upload` with two accounts and no `--account` exiting non-zero and listing aliases before any network call, legacy `token.json` migrated by `accounts list`
- [x] 3.3 Update `README.md` (Setup: several channels or Google logins, test users for apps in testing, `accounts` commands, `--account`/`YT_UPLOAD_CLI_ACCOUNT` in usage and the environment variables table), `docs/privacy.html` ("one token per authorized account", removal via `yt-upload accounts remove` or by deleting the account folder; no new data) and AGENTS.md (layout of the config folder, `accounts.rs`); verify README options match `yt-upload --help`, `yt-upload auth --help` and `yt-upload upload --help`

## 4. Integration checks

- [x] 4.1 Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and the AGENTS.md coverage command; verify all pass with ≥95% lines
- [ ] 4.2 Manual test (maintainer machine with `client_secret.json`): `auth --account a` and `--account b` with two channels of one Google login, `--account c` with another login, re-run `auth --account a` (no browser), try the same channel under a new alias (rejected), `accounts list`, private uploads with `--account b` and `--account c` landing on the right channels, `upload` without `--account` (refused), legacy `token.json` migration; then delete the test videos and `accounts remove` the extra aliases
- [ ] 4.3 Commit the artifacts and code on `feat/multiple-accounts` and open a PR titled `feat: support multiple youtube accounts` describing the model, the migration and rollback note, tests and the manual check; verify CI is green on Linux, Windows and macOS with the coverage and PR-title checks
