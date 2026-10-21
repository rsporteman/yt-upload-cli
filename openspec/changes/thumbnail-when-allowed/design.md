# Design

## Context

- `Api::upload` in `src/lib.rs` uploads the video, prints the URL, then calls `set_thumbnail` and
  `add_to_playlist` with `?`, so any thumbnail error aborts before the playlist step.
- `check()` turns non-2xx responses into `anyhow` errors carrying only status and
  `error.message`; it drops `error.errors[0].reason`, which is what separates "not allowed"
  (`forbidden`) from `quotaExceeded`, both HTTP 403.
- `thumbnail_mime` maps unknown extensions to `image/jpeg`.
- See proposal.md for motivation and specs/video-upload/spec.md for required behavior.

## Goals / Non-Goals

**Goals:**
- Fail fast on a bad thumbnail file before spending upload quota.
- Classify the thumbnail refusal precisely, without string-matching Google's human message.

**Non-Goals:**
- Checking channel verification status in advance (no API exposes it reliably; trying and
  handling the refusal costs the same 50 units).
- Image dimension/aspect checks or format conversion.
- Retrying thumbnail requests.

## Decisions

### 1. Typed API error instead of a bare `anyhow!`
Introduce `ApiError { status: StatusCode, reason: Option<String>, message: String }`
(implements `std::error::Error` + `Display` keeping today's text `YouTube API error ({status}):
{message}`). `check()` returns it wrapped in `anyhow`, so other callers are unchanged, and the
thumbnail path uses `err.downcast_ref::<ApiError>()` to test
`status == 403 && reason == Some("forbidden")`.

*Alternative:* match `"permissions"` in the message. Rejected: Google's text is localized and
not a stable contract.

### 2. `validate_thumbnail(path) -> Result<&'static str>` before the upload
Called at the start of `Api::upload` (before opening the session) when `args.thumbnail` is set.
Checks metadata (exists/readable, `len() <= 2 * 1024 * 1024`) and extension, and returns the MIME
type, replacing `thumbnail_mime`'s silent JPEG fallback. Errors use `.context` with the path.

*Alternative:* validate in `main.rs`. Rejected: `main.rs` is excluded from coverage and must stay
thin.

### 3. Thumbnail outcome drives the remaining steps
`set_thumbnail` becomes the source of an outcome:

- `Ok` → print `Thumbnail set.` (stdout).
- permission refusal → `eprintln!` warning with Google's message and the hint "custom thumbnails
  need a verified channel: https://www.youtube.com/verify"; treated as success.
- other error → kept in a local variable; the playlist step runs, then the stored error is
  returned (playlist error wins only if the thumbnail succeeded or was skipped; if both fail,
  return the thumbnail error with the playlist error attached as context).

`Api::upload` still returns the video id on success, so `main.rs` needs no change.

## Risks / Trade-offs

- [Google changes the reason code for this refusal] → covered by a single matcher function with
  a unit test; failure mode is the current behavior (command fails), not data loss.
- [Rejecting `.webp`/`.gif`/`.bmp` that YouTube might accept as `application/octet-stream`] →
  README already documents JPG/PNG only; widening later is additive.
- [Exit 0 hides a skipped thumbnail from scripts that ignore stderr] → stdout lacks
  `Thumbnail set.`, which scripts can check; documented in README.

## Migration Plan

Additive. Rollback is reverting the PR.
