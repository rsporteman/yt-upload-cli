# AGENTS.md

Guidelines for AI coding agents (and humans) working on **yt-upload-cli**, an open source Rust CLI
that uploads videos to YouTube through the YouTube Data API v3.

## Language

- **Everything in the repository is in English**: code, identifiers, comments, CLI help and error
  messages, docs, commit messages, PR titles/descriptions, issues and release notes.
- Talking with the maintainer in another language is fine; what lands in the repo is English.

## Project layout

- `src/main.rs`: the whole CLI (clap commands, OAuth via `yup-oauth2`, REST calls via `reqwest`).
  Split into modules only when a file gets hard to follow (`auth.rs`, `upload.rs`, ...).
- `docs/`: GitHub Pages site (home, privacy policy, terms). The Google OAuth consent screen links
  to these pages, so keep the URLs stable.
- `.github/workflows/`: `ci.yml` (fmt, clippy, tests on Linux/Windows/macOS), `pr-title.yml`
  (Conventional Commits check) and `release.yml` (release-please + binaries).
- `release-please-config.json`, `.release-please-manifest.json`: release automation; the manifest
  holds the last released version and is updated only by release-please.

## Before every commit

Run and fix everything these report:

```
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

## Code style

- Prefer small, readable functions and the standard library; add a dependency only when it removes
  real complexity. Explain new dependencies in the PR.
- Errors: use `anyhow` with `.context(...)` so failures say what was being done. Surface the
  message Google's API returns instead of a bare status code.
- User-facing output: results on **stdout** (so scripts and agents can parse them, e.g. the video
  URL), progress and hints on **stderr**.
- Keep the CLI stable and scriptable: don't rename commands or flags without a deprecation path.
  Never add interactive prompts to `upload`; it must run unattended.
- Comments explain *why*, not *what*.

## Security and privacy (non-negotiable)

- Never commit, print or log credentials: `client_secret*.json`, `token.json`, access/refresh
  tokens, API keys. They live only in the user's config folder (`yt-upload config-dir`).
- No telemetry and no servers of our own. The tool talks only to Google APIs. If this ever
  changes, `docs/privacy.html` must be updated in the same PR.
- Request the minimum OAuth scopes needed. Adding a scope requires updating the privacy policy.
- Don't use the YouTube or Google logos/branding in the project.

## Commits and pull requests

- Every change reaches `main` through a pull request, and PRs are **squash-merged**: the PR title
  becomes the single commit on `main`. Direct pushes to `main` are blocked.
- PR titles **must** follow [Conventional Commits](https://www.conventionalcommits.org/)
  (checked by `pr-title.yml`): `type(optional scope): subject`, lowercase subject, imperative mood,
  under 72 characters. Examples: `feat: add --language flag`, `fix(upload): retry failed chunks`.
- Types and their effect on the next release:

  | Type | Use for | Version bump | In changelog |
  |---|---|---|---|
  | `feat` | new user-facing capability | minor | yes |
  | `fix` | bug fix | patch | yes |
  | `perf` | performance improvement | patch | yes |
  | `revert` | reverting a previous change | patch | yes |
  | `docs` | documentation only | none | yes |
  | `refactor`, `test`, `build`, `ci`, `chore` | everything else | none | no |

- Breaking changes: add `!` after the type (`feat!: rename --privacy to --visibility`) and explain
  the migration in a `BREAKING CHANGE:` footer in the PR description.
- Commits inside a PR branch can be messy; only the PR title matters. Keep one logical change per
  PR and describe what changed, why, and how it was tested.
- Agents: don't merge PRs, publish releases, push tags or change repository settings unless the
  maintainer asks explicitly.

## Versioning and releases

- [Semantic Versioning](https://semver.org/), fully automated by
  [release-please](https://github.com/googleapis/release-please). Before 1.0, breaking changes
  bump the minor version.
- **Never edit the `version` in `Cargo.toml`, `CHANGELOG.md` or tags by hand.** After each merge
  to `main`, release-please opens or updates a `chore(main): release X.Y.Z` PR with the version
  bump and the generated changelog.
- Merging that release PR creates the `vX.Y.Z` tag and GitHub Release; `release.yml` then builds
  binaries for Linux (x86_64, arm64), Windows (x86_64) and macOS (Apple Silicon, Intel) and
  attaches them with `SHA256SUMS.txt`.
- To force a specific version, add a `Release-As: X.Y.Z` footer to a commit (PR description).

## Testing

- Unit-test pure logic (argument handling, request bodies, chunk ranges) without network access.
- Never hit the real YouTube API in automated tests. Manual tests that upload a video must use
  `--privacy private` and be cleaned up afterwards.

## Documentation

- Keep `README.md` in sync with `yt-upload --help` when flags change.
- `CHANGELOG.md` is generated from PR titles, so write titles that make sense to users.
