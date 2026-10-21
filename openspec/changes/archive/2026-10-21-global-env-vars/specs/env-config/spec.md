# Spec Delta

## Purpose

Lets users and unattended scripts configure yt-upload-cli through environment variables, including
variables defined globally at the operating-system level on Windows, macOS and Linux, so common
defaults do not have to be repeated on every command.

## ADDED Requirements

### Requirement: Supported environment variables
The CLI SHALL accept the following environment variables, each equivalent to the option it names:

| Variable | Option |
|---|---|
| `YT_UPLOAD_CLI_HOME` | `--config-dir` (all commands) |
| `YT_UPLOAD_CLI_PRIVACY` | `upload --privacy` |
| `YT_UPLOAD_CLI_CATEGORY` | `upload --category` |
| `YT_UPLOAD_CLI_TAGS` | `upload --tags` |
| `YT_UPLOAD_CLI_PLAYLIST` | `upload --playlist` |
| `YT_UPLOAD_CLI_MADE_FOR_KIDS` | `upload --made-for-kids` |

The video path, `--title`, `--description`, `--thumbnail` and `--publish-at` MUST NOT be read
from the environment. An environment variable whose value is empty SHALL be treated as unset.
`YT_UPLOAD_CLI_MADE_FOR_KIDS` SHALL accept `true`/`false`, `1`/`0`, `yes`/`no` and `on`/`off`,
case-insensitively.

#### Scenario: Default privacy from the environment
- **WHEN** `YT_UPLOAD_CLI_PRIVACY=unlisted` is set and the user runs `yt-upload upload v.mp4 -t T` without `--privacy`
- **THEN** the video is created with privacy status `unlisted`

#### Scenario: Playlist from the environment
- **WHEN** `YT_UPLOAD_CLI_PLAYLIST=PL1` is set and the user uploads without `--playlist`
- **THEN** the uploaded video is added to playlist `PL1`

#### Scenario: Made for kids from the environment
- **WHEN** `YT_UPLOAD_CLI_MADE_FOR_KIDS=yes` is set and the user uploads without `--made-for-kids`
- **THEN** the video is declared as made for kids

#### Scenario: Per-video values are not taken from the environment
- **WHEN** a variable such as `YT_UPLOAD_CLI_TITLE` is set and the user runs `yt-upload upload v.mp4` without `--title`
- **THEN** the command fails with the usual "missing --title" usage error

#### Scenario: Empty variable is ignored
- **WHEN** `YT_UPLOAD_CLI_PRIVACY` is set to an empty string
- **THEN** the built-in default privacy (`private`) is used

### Requirement: Precedence
For every supported option the CLI SHALL use the first value found in this order: command-line
flag, process environment, persisted OS environment, built-in default.

#### Scenario: Flag beats environment
- **WHEN** `YT_UPLOAD_CLI_PRIVACY=public` is set in the process environment and the user passes `--privacy private`
- **THEN** the video is created as `private`

#### Scenario: Process environment beats persisted environment
- **WHEN** the persisted OS environment defines `YT_UPLOAD_CLI_PRIVACY=public` and the process environment defines `YT_UPLOAD_CLI_PRIVACY=unlisted`
- **THEN** the video is created as `unlisted`

#### Scenario: Built-in default when nothing is set
- **WHEN** no flag and no process or persisted variable is set for privacy
- **THEN** the video is created as `private`

### Requirement: Fallback to the persisted OS environment
When a supported variable is absent (or empty) in the process environment, the CLI SHALL look up
its persisted, OS-level definition:

- **Windows**: the current user's environment, then the system environment, as stored by
  `setx`, System Properties or the Settings app. References such as `%USERPROFILE%` inside the
  value SHALL be expanded.
- **macOS**: the launchd user environment (values set with `launchctl setenv`).
- **Linux and other Unix systems**: the user's `~/.config/environment.d/*.conf` files (later
  files override earlier ones, in lexical order), then `/etc/environment`. Lines are `KEY=VALUE`;
  blank lines and lines starting with `#` are ignored, and one pair of surrounding single or
  double quotes around the value is removed.

A user-level definition SHALL take precedence over a system-level one. Shell startup files
(`.bashrc`, `.zshrc`, `.profile`, PowerShell profiles) MUST NOT be read or executed.

#### Scenario: Windows variable set with setx in another terminal
- **WHEN** on Windows the user ran `setx YT_UPLOAD_CLI_HOME D:\yt` and then runs `yt-upload config-dir` in a terminal opened before that
- **THEN** the command prints `D:\yt`

#### Scenario: Windows user value overrides system value
- **WHEN** on Windows `YT_UPLOAD_CLI_PRIVACY` is `public` in the system environment and `unlisted` in the user environment, and absent from the process
- **THEN** uploads use `unlisted`

#### Scenario: Windows value with references
- **WHEN** on Windows the persisted `YT_UPLOAD_CLI_HOME` is `%USERPROFILE%\yt`
- **THEN** the config folder is the user's profile folder followed by `\yt`

#### Scenario: macOS launchd variable
- **WHEN** on macOS the user ran `launchctl setenv YT_UPLOAD_CLI_PRIVACY unlisted` and the variable is absent from the process
- **THEN** uploads use `unlisted`

#### Scenario: Linux environment.d variable
- **WHEN** on Linux `~/.config/environment.d/yt.conf` contains `YT_UPLOAD_CLI_CATEGORY="10"` and the variable is absent from the process
- **THEN** uploads use category `10`

#### Scenario: Linux user file overrides /etc/environment
- **WHEN** on Linux `/etc/environment` sets `YT_UPLOAD_CLI_CATEGORY=22` and `~/.config/environment.d/yt.conf` sets it to `10`
- **THEN** uploads use category `10`

#### Scenario: Persisted source unavailable
- **WHEN** a persisted source cannot be read (missing file, no permission, `launchctl` not available, missing registry key)
- **THEN** that source is treated as defining no variables and the command continues without error or extra output

### Requirement: Invalid values name their source
An invalid value coming from the environment SHALL make the command fail with a non-zero exit
code and an error on stderr that lists the allowed values. When the value came from the persisted
OS environment, the error SHALL also name the variable and say it came from the OS environment,
since the user cannot see it with `echo` in the current shell.

#### Scenario: Invalid privacy in the persisted environment
- **WHEN** the persisted OS environment sets `YT_UPLOAD_CLI_PRIVACY=secret` and no flag or process variable overrides it
- **THEN** `yt-upload upload` exits non-zero and the error mentions `YT_UPLOAD_CLI_PRIVACY`, the OS environment, and the values `private, unlisted, public`

#### Scenario: Invalid value overridden by a flag
- **WHEN** the persisted OS environment sets `YT_UPLOAD_CLI_PRIVACY=secret` and the user passes `--privacy public`
- **THEN** the command succeeds using `public`

### Requirement: Discoverability
`yt-upload upload --help` (and the top-level help for `YT_UPLOAD_CLI_HOME`) SHALL list the
environment variable for each supported option.

#### Scenario: Help shows the variables
- **WHEN** the user runs `yt-upload upload --help`
- **THEN** the help for `--privacy`, `--category`, `--tags`, `--playlist` and `--made-for-kids` shows the matching `YT_UPLOAD_CLI_*` variable

### Requirement: Values stay local
Environment values SHALL be read only from the local machine, MUST NOT be sent anywhere other than
the YouTube API fields they configure, and MUST NOT be printed or logged by the lookup itself.

#### Scenario: No extra output from lookup
- **WHEN** persisted variables are found and used during a successful upload
- **THEN** stdout contains only the normal upload results and stderr only the normal progress
