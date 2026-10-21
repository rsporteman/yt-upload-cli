# Proposal

## Why

`yt-upload upload` already sends the video through the YouTube Data API v3 (resumable, chunked)
and sets a custom thumbnail with `--thumbnail`. But YouTube only allows custom thumbnails on
channels with phone verification (and other account standing requirements). Today, when the API
refuses the thumbnail, the command exits with an error **after** the video was uploaded and
before the playlist step runs, so scripts and agents see a failure, may retry, and create
duplicate videos. A bad thumbnail file (missing, too large, wrong type) is also only detected
after the whole video was uploaded and quota spent.

## What Changes

- Validate `--thumbnail` before uploading the video: the file must exist and be readable, be a
  `.jpg`, `.jpeg` or `.png` file, and be at most 2 MB. Invalid → fail before any API call.
- After the upload, set the thumbnail **only when the API allows it**: if YouTube refuses because
  the account/channel is not allowed to set custom thumbnails, print a warning with Google's
  message and how to enable it on stderr, skip the thumbnail, continue with the playlist step and
  exit with status 0.
- Other thumbnail errors (quota exceeded, invalid image, server errors) still fail the command,
  as today, after the video URL has been printed; the playlist step still runs before the
  failure is reported so the video is not left half-configured.
- Thumbnails with another extension (e.g. `.webp`), which were previously sent as JPEG and
  rejected by YouTube after the upload, are now rejected up front. Not marked breaking: the
  previous behavior always failed for them anyway.

## Capabilities

### New Capabilities

- `video-upload`: uploading a video through the YouTube Data API with its metadata, and setting
  the thumbnail and playlist after the upload, including thumbnail validation and behavior when
  the API does not allow custom thumbnails.

### Modified Capabilities

None (no specs exist yet).

## Impact

- Code: `src/lib.rs` (`Api::upload`, `set_thumbnail`, `thumbnail_mime`, new validation and error
  classification), tests in the same file using `wiremock`.
- No new dependencies, OAuth scopes or network endpoints; `docs/privacy.html` unchanged.
- Docs: README "Usage" and "Limits to know" sections.
- Release: `feat:` PR (minor bump). Independent of the in-flight `global-env-vars` change, though
  both edit `UploadArgs`/`src/lib.rs`; whichever merges second rebases.
