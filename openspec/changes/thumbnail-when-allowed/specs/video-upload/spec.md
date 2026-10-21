# Spec Delta

## Purpose

Uploads a local video file to YouTube through the YouTube Data API v3 with its metadata, and then
applies the optional thumbnail and playlist, reporting results in a way scripts can rely on.

## ADDED Requirements

### Requirement: Video upload through the YouTube Data API
`yt-upload upload <VIDEO> --title <TITLE>` SHALL upload the file to the authorized account through
the YouTube Data API v3 using a resumable upload, sending the title, description, tags, category,
privacy, made-for-kids flag and scheduled publish time given on the command line. On success it
SHALL print `Uploaded: https://youtu.be/<id>` on stdout and report progress on stderr.

#### Scenario: Successful upload
- **WHEN** the user runs `yt-upload upload v.mp4 -t "My video"` with a valid token
- **THEN** the video is uploaded, stdout contains `Uploaded: https://youtu.be/<id>` and the command exits 0

#### Scenario: Missing or empty video file
- **WHEN** the video path does not exist or the file is empty
- **THEN** the command fails before contacting the API, naming the file

#### Scenario: API error during upload
- **WHEN** YouTube rejects the upload
- **THEN** the command exits non-zero with an error that includes Google's error message

### Requirement: Thumbnail validated before the upload
When `--thumbnail` is given, the CLI SHALL check, before any API call, that the file exists and is
readable, has a `.jpg`, `.jpeg` or `.png` extension (case-insensitive), and is at most 2 MB
(2,097,152 bytes). If any check fails, the command SHALL exit non-zero with an error naming the
file and the problem, and no video SHALL be uploaded.

#### Scenario: Thumbnail file missing
- **WHEN** the user passes `--thumbnail missing.jpg` and the file does not exist
- **THEN** the command fails naming `missing.jpg` and no upload request is sent

#### Scenario: Thumbnail too large
- **WHEN** the thumbnail file is larger than 2 MB
- **THEN** the command fails saying the thumbnail exceeds 2 MB and no upload request is sent

#### Scenario: Unsupported thumbnail type
- **WHEN** the user passes `--thumbnail cover.webp`
- **THEN** the command fails saying only JPG and PNG are supported and no upload request is sent

### Requirement: Thumbnail set only when the API allows it
After the video is uploaded, the CLI SHALL set the thumbnail with the matching content type. If
YouTube refuses because the account or channel is not permitted to set custom thumbnails, the CLI
SHALL skip the thumbnail, print a warning on stderr that includes Google's message and explains
that custom thumbnails require a verified channel, continue with the remaining steps, and exit 0.
On success it SHALL print `Thumbnail set.` on stdout.

#### Scenario: Thumbnail set
- **WHEN** the upload succeeds and YouTube accepts the thumbnail
- **THEN** stdout contains `Thumbnail set.`

#### Scenario: Channel not allowed to set custom thumbnails
- **WHEN** the upload succeeds and YouTube answers the thumbnail request with a permission refusal (HTTP 403, reason `forbidden`)
- **THEN** stdout contains the video URL but not `Thumbnail set.`, stderr contains a warning with Google's message and a hint about channel verification, and the command exits 0

#### Scenario: Playlist still applied after a skipped thumbnail
- **WHEN** the thumbnail is refused for permission reasons and `--playlist PL1` was given
- **THEN** the video is added to `PL1`, stdout contains `Added to playlist.` and the command exits 0

#### Scenario: Other thumbnail errors fail the command
- **WHEN** the thumbnail request fails for another reason (for example quota exceeded, HTTP 403 reason `quotaExceeded`, or an HTTP 5xx)
- **THEN** the playlist step (if requested) still runs, and the command then exits non-zero with an error that includes Google's message; the video URL was already printed on stdout

### Requirement: Playlist after upload
When `--playlist <ID>` is given, the CLI SHALL add the uploaded video to that playlist after the
thumbnail step and print `Added to playlist.` on stdout.

#### Scenario: Added to playlist
- **WHEN** the upload succeeds and `--playlist PL1` was given
- **THEN** the video is inserted into playlist `PL1` and stdout contains `Added to playlist.`
