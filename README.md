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
yt-upload upload <VIDEO> --title <TITLE> [OPTIONS]

  -d, --description <TEXT>
      --tags <a,b,c>
  -p, --privacy <private|unlisted|public>   [default: private]
      --category <ID>                       [default: 22 = People & Blogs]
      --thumbnail <IMAGE>                   JPG or PNG, up to 2 MB
      --playlist <PLAYLIST_ID>
      --publish-at <RFC3339>                e.g. 2026-10-25T15:00:00Z
      --made-for-kids
```

## Limits to know

- **Unaudited projects upload as private.** Until your Google Cloud project passes YouTube's
  [API audit](https://support.google.com/youtube/contact/yt_api_form), every video uploaded through
  the API is locked to private. Change it in YouTube Studio, or request the audit.
- **Quota:** the default is 10,000 units/day and an upload costs about 1,600, so roughly 6 uploads a day.
- Custom thumbnails need a channel with phone verification.

## Privacy

yt-upload-cli talks only to Google's APIs. See the [privacy policy](https://rsporteman.github.io/yt-upload-cli/privacy.html).

## License

MIT
