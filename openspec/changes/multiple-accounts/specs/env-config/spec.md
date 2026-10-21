# Spec Delta

## MODIFIED Requirements

### Requirement: Supported environment variables
The CLI SHALL accept the following environment variables, each equivalent to the option it names:

| Variable | Option |
|---|---|
| `YT_UPLOAD_CLI_HOME` | `--config-dir` (all commands) |
| `YT_UPLOAD_CLI_ACCOUNT` | `--account` (`auth` and `upload`) |
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

#### Scenario: Account from the environment
- **WHEN** accounts `personal` and `work` exist, `YT_UPLOAD_CLI_ACCOUNT=work` is set and the user uploads without `--account`
- **THEN** the video is uploaded with the `work` account

#### Scenario: Per-video values are not taken from the environment
- **WHEN** a variable such as `YT_UPLOAD_CLI_TITLE` is set and the user runs `yt-upload upload v.mp4` without `--title`
- **THEN** the command fails with the usual "missing --title" usage error

#### Scenario: Empty variable is ignored
- **WHEN** `YT_UPLOAD_CLI_PRIVACY` is set to an empty string
- **THEN** the built-in default privacy (`private`) is used
