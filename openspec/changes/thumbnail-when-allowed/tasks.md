# Tasks

## 1. Typed API errors

- [x] 1.1 Create branch `feat/thumbnail-when-allowed`; add `ApiError { status, reason, message }` in `src/lib.rs` and make `check()` return it (reason from `error.errors[0].reason`), keeping the `YouTube API error ({status}): {message}` text; verify existing `api_errors_show_googles_message` / `api_errors_fall_back_to_raw_body` tests still pass and a new test asserts `reason == Some("quotaExceeded")` is parsed from a 403 body
- [x] 1.2 Add `is_thumbnail_not_allowed(&anyhow::Error) -> bool` (403 + reason `forbidden`); verify unit tests: true for 403/`forbidden`, false for 403/`quotaExceeded`, 500, and non-`ApiError` errors

## 2. Thumbnail validation before upload

- [x] 2.1 Replace `thumbnail_mime` with `validate_thumbnail(path) -> Result<&'static str>` (exists, ≤ 2,097,152 bytes, `.jpg`/`.jpeg`/`.png` case-insensitive); verify unit tests for each error message (missing, too large, `.webp`) and for MIME of `a.PNG`/`a.jpeg`
- [x] 2.2 Call it at the start of `Api::upload` when `--thumbnail` is set; verify a `wiremock` test with `.expect(0)` on `POST /videos` shows no upload request is sent for an invalid thumbnail (update `upload_reports_missing_thumbnail_file` accordingly)

## 3. Thumbnail only when allowed

- [x] 3.1 Rework the post-upload steps in `Api::upload` per design decision 3 (stdout `Thumbnail set.`, stderr warning + hint on permission refusal, playlist always attempted, deferred non-permission thumbnail error); verify `wiremock` tests: (a) 403/`forbidden` thumbnail + playlist → `Ok(id)` and playlist mock hit once, (b) 403/`quotaExceeded` thumbnail + playlist → `Err` containing Google's message and playlist mock hit once, (c) happy path still passes
- [x] 3.2 Update `README.md` ("Usage" and "Limits to know"): thumbnail validated up front (JPG/PNG, ≤ 2 MB), skipped with a warning and exit 0 when the channel cannot set custom thumbnails, and scripts can check for `Thumbnail set.` on stdout; verify README options match `yt-upload upload --help`

## 4. Integration checks

- [x] 4.1 Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and the AGENTS.md `cargo llvm-cov ... --fail-under-lines 95` command; verify all pass
- [ ] 4.2 (Skipped by maintainer decision on 2026-10-21; only mock-server tests) Manual test on a real channel with `--privacy private`: upload with a PNG thumbnail and confirm either `Thumbnail set.` or the warning with exit 0 (on an unverified channel), then delete the test video in YouTube Studio
- [x] 4.3 Open a PR titled `feat: set the thumbnail only when the channel allows it` describing behavior, tests and the manual check; verify CI (matrix, coverage, PR title) is green
