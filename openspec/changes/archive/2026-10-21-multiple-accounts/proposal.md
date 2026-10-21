# Proposal

## Why

yt-upload stores a single OAuth token (`<config dir>/token.json`), so it can upload to only one
YouTube channel. Creators commonly run several channels: several channels (brand accounts)
under one Google login, and channels under different Google logins (e.g. personal and work).
Today switching means deleting the token or juggling `--config-dir` folders by hand, which is
error-prone in unattended scripts and makes it easy to upload to the wrong channel.

## What Changes

- New **account** concept: a saved authorization for exactly one YouTube channel, identified by a
  user-chosen alias (`[a-z0-9][a-z0-9_-]{0,31}`). Each account stores its own token plus the
  channel it is bound to (id, title, handle). Several channels of the same Google login and
  channels of different Google logins are modeled the same way: each is one account.
- `yt-upload auth --account <alias>` authorizes a new account or refreshes an existing one. The
  browser always shows Google's account chooser, so a different Google login can be picked, and
  Google's channel/brand-account picker selects the channel. After authorization the CLI asks the
  API which channel was authorized and records it.
- New `yt-upload accounts list` (aliases with their channels, tab-separated on stdout) and
  `yt-upload accounts remove <alias>` (deletes the local token).
- `upload` takes `--account <alias>` (also `YT_UPLOAD_CLI_ACCOUNT`). With exactly one saved
  account it is used automatically. **With more than one, `--account` is required**: the
  command fails, listing the aliases, instead of guessing. The target channel is printed on
  stderr before uploading.
- Safety: authorizing a channel that is already saved under another alias is rejected.
  Authorizing a Google login that has no YouTube channel does not create an account.
- Migration: an existing single `token.json` is moved automatically to an account named
  `default`. Users with one account notice no change.
- No e-mail is stored or requested: the Google login only matters at consent time, and channels
  are what uploads target. OAuth scopes are unchanged.

## Capabilities

### New Capabilities

- `accounts`: saving, listing, removing and selecting multiple authorized YouTube channels
  (accounts), across one or several Google logins, including migration of the legacy single
  token.

### Modified Capabilities

- `env-config`: the "Supported environment variables" requirement gains
  `YT_UPLOAD_CLI_ACCOUNT` for `--account`.

## Impact

- Code: new `src/accounts.rs` (store, alias rules, resolution, migration, listing), `src/lib.rs`
  (CLI: `--account`, `accounts` subcommands; `Api::channel()` returning id/title/handle),
  `src/oauth.rs` (per-account token path, account chooser in the consent URL), `src/main.rs`
  (wiring).
- Config folder layout: `accounts/<alias>/token.json` and `accounts/<alias>/channel.json`;
  `client_secret.json` stays shared at the root.
- No new dependencies and no new OAuth scopes. `docs/privacy.html` gets a wording update (one
  token per authorized account, removal via `accounts remove`) without new data uses.
- Docs: README (setup for several channels/logins, test users when the OAuth app is in testing,
  usage) and AGENTS.md layout.
- Compatibility: additive. Existing single-account setups keep working through the automatic
  migration. Not breaking.
- Release: `feat:` PR (minor bump).
