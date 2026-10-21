# Spec Delta

## Purpose

Lets one installation of yt-upload hold several authorized YouTube channels (accounts), whether
they belong to the same Google login or to different ones, and makes every upload target an
explicitly known channel.

## ADDED Requirements

### Requirement: Account model
An account SHALL be a saved OAuth authorization bound to exactly one YouTube channel, identified
by a user-chosen alias matching `[a-z0-9][a-z0-9_-]{0,31}`. For each account the CLI SHALL store
the token and the channel's id, title and handle locally in the config folder. Several accounts
MAY point to channels of the same Google login or of different Google logins. Two accounts MUST
NOT point to the same channel. The OAuth client (`client_secret.json`) SHALL be shared by all
accounts. The CLI MUST NOT request or store the Google login's e-mail address.

#### Scenario: Invalid alias
- **WHEN** the user runs `yt-upload auth --account "My Channel"`
- **THEN** the command fails before opening the browser, explaining the allowed alias format

#### Scenario: Two channels of the same Google login
- **WHEN** the user authorizes `--account gaming` and picks brand channel A, then authorizes `--account music` with the same Google login and picks brand channel B
- **THEN** both accounts are saved and each one uploads to its own channel

#### Scenario: Channels of different Google logins
- **WHEN** the user authorizes `--account personal` with one Google login and `--account work` with another
- **THEN** both accounts are saved and listed with their channels

### Requirement: Authorizing an account
`yt-upload auth --account <alias>` SHALL run the browser consent flow when the alias has no
usable token. The consent page SHALL always show Google's account chooser, so a different
Google login can be selected. After authorization the CLI SHALL query which channel the token
can access, save it with the account, and print `Authorized. Channel: <title>` on stdout. When
the alias already has a usable token, it SHALL refresh the stored channel details without opening
the browser. Without `--account` (and without `YT_UPLOAD_CLI_ACCOUNT`), `auth` SHALL use the
same account resolution as `upload`, except that with no saved accounts it SHALL create the
account `default`.

#### Scenario: First account
- **WHEN** no account exists and the user runs `yt-upload auth`
- **THEN** the browser flow runs and the account `default` is saved with its channel

#### Scenario: Re-running auth on an existing account
- **WHEN** account `work` has a valid token and the user runs `yt-upload auth --account work`
- **THEN** no browser opens, the channel details are refreshed and printed

#### Scenario: Same channel under a second alias
- **WHEN** channel C is saved as `work` and the user authorizes `--account work2` and picks channel C again
- **THEN** the command fails saying channel C is already saved as `work`, and `work2` is not created

#### Scenario: Google login without a YouTube channel
- **WHEN** the authorized Google login has no YouTube channel
- **THEN** the command fails saying the account has no YouTube channel, and no account is saved

### Requirement: Selecting the account for an upload
`upload` SHALL target the account given by `--account <alias>` (or `YT_UPLOAD_CLI_ACCOUNT`). When
none is given and exactly one account exists, it SHALL use that account. When none is given and
several accounts exist, it SHALL fail without uploading and list the available aliases. When no
account exists, it SHALL fail telling the user to run `yt-upload auth`. An unknown alias SHALL
fail listing the available aliases. Before uploading, the CLI SHALL print the target channel and
alias on stderr.

#### Scenario: Single account
- **WHEN** only account `default` exists and the user runs `yt-upload upload v.mp4 -t T`
- **THEN** the video is uploaded to `default`'s channel and stderr names that channel

#### Scenario: Several accounts without a choice
- **WHEN** accounts `personal` and `work` exist and the user runs `yt-upload upload v.mp4 -t T`
- **THEN** the command exits non-zero without contacting the upload API, and the error lists `personal` and `work` and mentions `--account`

#### Scenario: Explicit account
- **WHEN** accounts `personal` and `work` exist and the user runs `yt-upload upload v.mp4 -t T --account work`
- **THEN** the video is uploaded with `work`'s token

#### Scenario: Unknown alias
- **WHEN** the user passes `--account nope` and no such account exists
- **THEN** the command fails listing the existing aliases

#### Scenario: No accounts
- **WHEN** no account exists and the user runs `upload`
- **THEN** the command fails telling the user to run `yt-upload auth`

### Requirement: Listing and removing accounts
`yt-upload accounts list` SHALL print one line per account on stdout, sorted by alias, with
tab-separated fields `alias`, `channel title`, `channel id` and `handle`, using `-` for unknown
values. With no accounts it SHALL print nothing on stdout, print a hint on stderr and exit 0.
`yt-upload accounts remove <alias>` SHALL delete that account's local data, print on stderr how
to revoke the app's access at Google, and fail if the alias does not exist.

#### Scenario: List accounts
- **WHEN** accounts `work` and `personal` exist
- **THEN** stdout has two lines, `personal` first, each with alias, title, id and handle separated by tabs

#### Scenario: Remove an account
- **WHEN** the user runs `yt-upload accounts remove work`
- **THEN** `work` no longer appears in `accounts list` and its token file is gone

#### Scenario: Remove unknown account
- **WHEN** the user runs `yt-upload accounts remove nope`
- **THEN** the command exits non-zero naming `nope`

### Requirement: Migration of the single-token setup
When the config folder contains a legacy `token.json` and no accounts, the CLI SHALL move that
token into a new account named `default` before running any command that uses accounts, and
SHALL note the migration once on stderr. The channel details of a migrated account MAY be
unknown until the next `auth`. Uploads with the migrated account SHALL work without re-authorizing.

#### Scenario: Upgrade from a previous version
- **WHEN** a user who authorized with a previous version runs `yt-upload upload v.mp4 -t T`
- **THEN** the token is migrated to account `default`, a note is printed on stderr, and the upload proceeds without opening the browser

#### Scenario: Migrated account details
- **WHEN** the user runs `yt-upload accounts list` right after migration
- **THEN** `default` is listed with `-` for unknown channel fields, and running `yt-upload auth --account default` fills them in
