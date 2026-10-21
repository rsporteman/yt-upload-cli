# yt-upload-cli

Upload videos to YouTube from the command line, using the official YouTube Data API v3.
A single binary written in Rust, made to be scriptable and easy for AI agents (like Claude Code) to drive.

```
yt-upload upload video.mp4 --title "My video" --privacy unlisted --thumbnail cover.jpg
```

- Resumable, chunked uploads with progress
- Title, description, tags, category, privacy, made-for-kids flag
- Scheduled publishing (`--publish-at`)
- Thumbnail and playlist in the same command
- Defaults from environment variables, including ones set system-wide
- Your credentials stay on your machine: no server in the middle

## Install

Download the binary for your system from the [latest release](https://github.com/rsporteman/yt-upload-cli/releases/latest)
(Linux x86_64/arm64, Windows x86_64, macOS Apple Silicon/Intel), extract it and put `yt-upload`
somewhere on your `PATH`.

Or build from source with Rust installed:

```
cargo install --git https://github.com/rsporteman/yt-upload-cli
```

## Setup (once)

You use your own Google Cloud OAuth client, so the quota and the access are yours.

1. Go to [console.cloud.google.com](https://console.cloud.google.com) and create a project.
2. **APIs & Services → Library**: enable **YouTube Data API v3**.
3. **Google Auth Platform → Branding**: set an app name, support email and developer contact email.
   Don't upload a logo (that requires Google verification).
4. **Google Auth Platform → Audience**: choose **External**, then either
   - click **Publish app** (recommended; tokens don't expire), or
   - add your Google account under **Test users** (tokens expire every 7 days).
5. **Google Auth Platform → Clients → Create client**: type **Desktop app**. Download the JSON.
6. Save it as `client_secret.json` in the config folder. Find the folder with:

   ```
   yt-upload config-dir
   ```

   (Windows: `%APPDATA%\yt-upload-cli`, macOS: `~/Library/Application Support/yt-upload-cli`,
   Linux: `~/.config/yt-upload-cli`. Override with `--config-dir` or `YT_UPLOAD_CLI_HOME`.)
7. Authorize your channel:

   ```
   yt-upload auth
   ```

   Your browser opens; pick the account/channel. With a published but unverified app, Google shows
   "Google hasn't verified this app": click **Advanced → Go to (app name)**.
   The token is saved next to `client_secret.json`.

## Usage

```
yt-upload upload talk.mp4 --title "Conference talk" --description "Recorded live" --tags "rust,cli"
yt-upload upload trailer.mp4 --title "Trailer" --publish-at 2026-10-25T15:00:00Z --playlist PLxxxxxxxx
yt-upload upload short.mp4 --title "Short" --privacy public --category 10 --made-for-kids
```

Results go to stdout, one line each, so scripts can parse them: `Uploaded: https://youtu.be/<id>`,
then `Thumbnail set.` and `Added to playlist.` when requested. Progress, warnings and errors go
to stderr. A non-zero exit code means the command failed.

All commands and options (generated from `yt-upload --help`):

<!-- usage:start -->
<!-- Generated from the CLI help; refresh with: UPDATE_README=1 cargo test --test readme -->

`yt-upload --help`

```
Upload videos to YouTube from the command line

Usage: yt-upload [OPTIONS] <COMMAND>

Commands:
  auth        Authorize a YouTube account and show the connected channel
  upload      Upload a video
  config-dir  Print the config folder path
  help        Print this message or the help of the given subcommand(s)

Options:
      --config-dir <CONFIG_DIR>  Folder holding client_secret.json and the saved token [default: <config dir>/yt-upload-cli] [env: YT_UPLOAD_CLI_HOME]
  -h, --help                     Print help
  -V, --version                  Print version
```

`yt-upload upload --help`

```
Upload a video

Usage: yt-upload upload [OPTIONS] --title <TITLE> <VIDEO>

Arguments:
  <VIDEO>  Video file to upload

Options:
      --config-dir <CONFIG_DIR>    Folder holding client_secret.json and the saved token [default: <config dir>/yt-upload-cli] [env: YT_UPLOAD_CLI_HOME]
  -t, --title <TITLE>              Video title
  -d, --description <DESCRIPTION>  Video description [default: ""]
      --tags <TAGS>                Comma-separated tags [env: YT_UPLOAD_CLI_TAGS] [default: ""]
  -p, --privacy <PRIVACY>          Who can watch the video [env: YT_UPLOAD_CLI_PRIVACY] [default: private] [possible values: private, unlisted, public]
      --category <CATEGORY>        Category id (22 = People & Blogs) [env: YT_UPLOAD_CLI_CATEGORY] [default: 22]
      --thumbnail <THUMBNAIL>      Thumbnail image (JPG or PNG, up to 2 MB)
      --playlist <PLAYLIST>        Playlist id to add the video to [env: YT_UPLOAD_CLI_PLAYLIST]
      --publish-at <PUBLISH_AT>    Schedule publishing (RFC 3339, e.g. 2026-10-25T15:00:00Z); forces private until then
      --made-for-kids              Mark the video as made for kids [env: YT_UPLOAD_CLI_MADE_FOR_KIDS]
  -h, --help                       Print help
```
<!-- usage:end -->

## Environment variables

Options you repeat on every upload can come from the environment instead:

| Variable | Option |
|---|---|
| `YT_UPLOAD_CLI_HOME` | `--config-dir` |
| `YT_UPLOAD_CLI_PRIVACY` | `--privacy` |
| `YT_UPLOAD_CLI_CATEGORY` | `--category` |
| `YT_UPLOAD_CLI_TAGS` | `--tags` |
| `YT_UPLOAD_CLI_PLAYLIST` | `--playlist` |
| `YT_UPLOAD_CLI_MADE_FOR_KIDS` | `--made-for-kids` (`true`/`false`, `1`/`0`, `yes`/`no`, `on`/`off`) |

The video, title, description, thumbnail and publish time are always given on the command line.
An empty variable counts as unset.

Each option uses the first value found: command-line flag, the current environment, the
environment saved by your operating system, then the built-in default. So a variable you set
globally works even in a terminal, scheduler or agent that was started before you set it:

- **Windows**: `setx YT_UPLOAD_CLI_PRIVACY unlisted` (or *Settings → System → About → Advanced
  system settings → Environment Variables*). User variables win over system ones; `%VAR%`
  references are expanded.
- **macOS**: `launchctl setenv YT_UPLOAD_CLI_PRIVACY unlisted`.
- **Linux**: add `YT_UPLOAD_CLI_PRIVACY=unlisted` to a file in `~/.config/environment.d/`
  (e.g. `yt-upload.conf`), or to `/etc/environment` for all users.

Variables exported only in shell startup files (`.bashrc`, `.zshrc`, PowerShell profile) are seen
only by programs started from that shell; yt-upload never reads or runs those files.

## Limits to know

- **Unaudited projects upload as private.** Until your Google Cloud project passes YouTube's
  [API audit](https://support.google.com/youtube/contact/yt_api_form), every video uploaded through
  the API is locked to private. Change it in YouTube Studio, or request the audit.
- **Quota:** the default is 10,000 units/day and an upload costs about 1,600, so roughly 6 uploads a day.
- Custom thumbnails need a channel with phone verification. The thumbnail file is checked
  before the upload starts (JPG or PNG, up to 2 MB). If YouTube does not allow custom thumbnails
  on your channel, the video is still uploaded (and added to the playlist), a warning is printed
  on stderr and the command exits 0; scripts can check stdout for `Thumbnail set.`.

## Privacy

yt-upload-cli talks only to Google's APIs. See the [privacy policy](https://rsporteman.github.io/yt-upload-cli/privacy.html).

## License

MIT
