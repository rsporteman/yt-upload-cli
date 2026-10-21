# Design

## Context

- `src/oauth.rs` (excluded from coverage) runs yup-oauth2's `InstalledFlowAuthenticator` with
  `persist_tokens_to_disk(<config dir>/token.json)`. A `BrowserDelegate` receives the consent URL
  and opens it.
- `src/main.rs` resolves the config folder, then calls `oauth::access_token(&dir)` for `auth` and
  `upload`. `Api::channel_names()` lists the titles of `channels?part=snippet&mine=true`.
- For YouTube scopes, Google's consent screen lets the user pick a channel or brand account, and
  the token is bound to that channel. `mine=true` then returns exactly that channel.
- `parse_cli` (from `env-config`) applies flag > process env > OS-persisted env, and clap's `env`
  attribute provides the process-env layer and the help text.
- See proposal.md for motivation and specs/accounts/spec.md for required behavior.

## Goals / Non-Goals

**Goals:**
- All account logic (paths, alias rules, resolution, migration, duplicate check, listing) lives in
  a testable module. `oauth.rs` and `main.rs` stay thin.
- Unattended safety: never guess the target channel when there is a choice.

**Non-Goals:**
- Knowing or displaying the Google login's e-mail. This would need the `openid email` scopes and
  a privacy policy change, and isn't needed to pick a login or to upload.
- A "default account" setting. Explicit `--account` or `YT_UPLOAD_CLI_ACCOUNT` covers it.
- Revoking tokens at Google from the CLI (`accounts remove` only deletes local data and prints the
  revoke URL).
- Separate OAuth clients per account.

## Decisions

### 1. Storage layout: one folder per account
```
<config dir>/
  client_secret.json
  accounts/<alias>/token.json     # yup-oauth2 token cache, unchanged format
  accounts/<alias>/channel.json   # {"id": "UC…", "title": "…", "handle": "@…"}
```
Folders keep the token in yup-oauth2's own format (no re-serialization) and make removal a
`remove_dir_all`. Aliases are lowercase-only, which avoids collisions on case-insensitive file
systems (Windows, macOS).

*Alternative:* one `accounts.json` index plus `token-<alias>.json`. Rejected because two files
must stay consistent and partial writes can corrupt the index.

### 2. `src/accounts.rs` module
- `pub struct Channel { id, title, handle: Option<String> }` (serde_json `Value`-based, no new
  derive dependency).
- `pub struct Store { root: PathBuf }` with `validate_alias`, `token_path(alias)`, `list() ->
  Vec<(alias, Option<Channel>)>` (sorted), `save_channel`, `remove`, `exists`.
- `pub fn migrate_legacy(store) -> Result<bool>`: if `<root>/token.json` exists and `accounts/` is
  empty or missing, create `accounts/default/` and `rename` the token into it. Returns whether a
  migration happened, so `main` prints the note once.
- `pub fn resolve(store, requested: Option<&str>, purpose: Purpose) -> Result<String>`
  (`Purpose::Auth | Upload`) implements the selection table from the spec, including
  "`default` when none exist" for `Auth`. Errors list aliases.
- `pub fn register(store, alias, channels: &[Channel]) -> Result<Channel>`, run after the
  browser flow:
  - zero channels → remove the alias folder and fail;
  - a channel id already saved under another alias → remove the alias folder and fail naming the
    other alias;
  - otherwise save `channel.json`.
  `mine=true` returns at most one channel, so the first one is used.

### 3. CLI shape
- `auth` gains `#[arg(long, env = "YT_UPLOAD_CLI_ACCOUNT")] account: Option<String>`. `upload`
  gets the same, and `parse_cli` gets an `ACCOUNT_VAR` fallback for both, following the existing
  pattern (sub-matches of `auth` or `upload`).
- New `accounts` subcommand with `list` and `remove <ALIAS>` (`#[command(subcommand)]`).
- Output: `accounts list` prints `alias\ttitle\tid\thandle` lines on stdout, and hints go to
  stderr, per the AGENTS.md stdout/stderr rule.

### 4. OAuth changes (thin)
- `oauth::access_token(dir: &Path, token_path: &Path)` replaces the hard-coded `token.json`.
- `BrowserDelegate` passes the URL through `lib::with_account_chooser(url)`, which appends
  `prompt=select_account%20consent` unless a `prompt=` is already present. Testable in lib.
  `consent` also makes Google return a refresh token for a login that authorized the app before.
- The "has a usable token → no browser" behavior comes from yup-oauth2's token cache: if the
  cached token for the scopes is valid or refreshable, `token()` returns without the browser.

### 5. Main wiring (sketch)
```
let store = Store::new(&dir);
if accounts::migrate_legacy(&store)? { eprintln!("Moved the saved token to account 'default'."); }
match cli.command {
  Auth(a)   => { let alias = resolve(&store, a.account, Auth)?; validate; token = oauth(...);
                 let ch = register(&store, &alias, &api.channels().await?)?; println!(...) }
  Upload(a) => { let alias = resolve(&store, a.account, Upload)?;
                 eprintln!("Uploading to {title} (account {alias})"); ... }
  Accounts(List | Remove) => lib functions that format output
}
```
`Api::channels()` replaces `channel_names()` and returns `Vec<Channel>` (`id`,
`snippet.title`, `snippet.customUrl`). `auth_message` adapts to it.

### 6. Testing
- `accounts.rs` unit tests with `tempfile`: alias validation table; list sorting and `-` for
  missing `channel.json`; resolve for 0/1/many accounts × requested/unrequested × Auth/Upload,
  plus unknown aliases; migration (moves the token, no-op when accounts exist or no legacy token);
  register (no channel, duplicate, success writes `channel.json`); remove (existing and unknown).
- `wiremock` test for `Api::channels()` parsing id, title and handle.
- `parse_cli` tests for `--account` and the `YT_UPLOAD_CLI_ACCOUNT` OS fallback on `auth` and
  `upload`. A `tests/cli.rs` case runs `accounts list` against a temp config dir with fake
  account folders, checking stdout format and exit codes, and `upload` with 2 accounts and no
  `--account` failing before any network call.
- Manual: two channels of one Google login and one channel of another login, with uploads using
  `--privacy private`, cleaned up afterwards.

## Risks / Trades-offs

- [User picks the same channel twice] → rejected by `register`, and the new alias is cleaned up.
- [OAuth app in "Testing" status] → each additional Google login must be added as a test user,
  otherwise Google blocks consent. Documented in the README setup.
- [`prompt=consent` makes Google show the consent screen on every new authorization] → only on
  the `auth` browser flow, which is interactive anyway.
- [Migration races if two processes start at once] → `rename` is atomic on the same file system,
  and the loser sees `accounts/default` already present and skips.
- [yup-oauth2 might add its own `prompt` parameter in a future version] → `with_account_chooser`
  leaves an existing `prompt=` untouched. Covered by a test.

## Migration Plan

Automatic on first run (decision 2). Rollback to an older binary: the old version looks for
`<config dir>/token.json`, which no longer exists, so it runs the browser flow again. This is
documented in the PR, and the token can also be copied back manually.
