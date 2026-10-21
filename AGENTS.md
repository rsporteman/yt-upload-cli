# AGENTS.md

Guidelines for AI coding agents (and humans) working on **yt-upload-cli**, an open source Rust CLI
that uploads videos to YouTube through the YouTube Data API v3.

## Language

- **Everything in the repository is in English**: file and folder names, branch names, code,
  identifiers, comments, CLI help and error messages, docs, commit messages, PR
  titles/descriptions, issues, labels and release notes.
- Branch names: `<type>/<short-kebab-description>`, using the Conventional Commit types
  (e.g. `feat/language-flag`, `fix/chunk-retry`).
- Talking with the maintainer in another language is fine; what lands in the repo is English.

## Project layout

- `src/lib.rs`: CLI definition (clap) and the YouTube API client (`reqwest`), with unit tests.
  Split into modules when it gets hard to follow (`upload.rs`, ...).
- `src/main.rs`: thin binary that wires the CLI to the API client.
- `src/oauth.rs`: interactive OAuth browser flow (`yup-oauth2`).
- `docs/`: GitHub Pages site (home, privacy policy, terms). The Google OAuth consent screen links
  to these pages, so keep the URLs stable.
- `.github/workflows/`: `ci.yml` (fmt, clippy, tests on Linux/Windows/macOS, coverage gate), `pr-title.yml`
  (Conventional Commits check) and `release.yml` (release-please + binaries).
- `openspec/`: change proposals (`changes/`) and the accepted specs (`specs/`); see
  "OpenSpec workflow".
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

## OpenSpec workflow

Changes are planned with [OpenSpec](https://github.com/Fission-AI/OpenSpec) in `openspec/`. Each
change lives on its own branch and PR, so planning never piles up untracked or on another
change's branch.

- **Before every propose**, start from an up-to-date `main`:

  ```
  git switch main
  git pull --ff-only
  git switch -c <type>/<change-name>
  ```

  Then run the propose, commit the change's artifacts (`openspec/changes/<change-name>/`), push,
  and open the PR from that branch. If `git pull --ff-only` fails or the working tree has
  uncommitted changes, stop and ask the maintainer instead of stashing, resetting or merging.
- **Apply** works on that same branch and PR: implementation commits go on top of the artifacts.
- **Archive** after the PR is merged, again from an up-to-date `main` on a new branch
  (e.g. `chore/archive-<change-name>`), in its own PR.

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

## Testing and coverage

- **Minimum 95% line coverage**, enforced by the `coverage` CI job (`cargo llvm-cov
  --fail-under-lines 95`). A PR that drops coverage below 95% cannot be merged. Every new
  feature or fix comes with tests.
- Only two files are excluded from coverage: `src/main.rs` (thin wiring) and `src/oauth.rs`
  (interactive browser flow). Keep logic out of them: put it in `src/lib.rs` (or modules under
  it) where it can be tested. Don't add new exclusions without the maintainer's approval.
- Test HTTP calls against a local mock server (`wiremock`) using `Api::with_endpoints`.
  Never hit the real YouTube API in automated tests. Manual tests that upload a video must use
  `--privacy private` and be cleaned up afterwards.
- **Manual tests never block the flow.** Automated tests and CI are the merge gate; manual tests
  (real Google accounts, a browser, the live Pages site after a merge) are extra confidence. When
  an agent cannot run one, it does not pause or wait. Instead it:
  - leaves the task unchecked with `(manual, not run: <reason>)`;
  - lists the test as pending in the PR description, with the exact steps for the maintainer;
  - opens the PR ready for review (not as a draft) once the automated checks pass, and moves on
    to the next task or change;
  - may archive a change whose only open tasks are manual tests, naming them in the summary.

  Whether to merge without the manual test is the maintainer's call.
- Check coverage locally with:

  ```
  cargo llvm-cov --ignore-filename-regex 'src[\\/](main|oauth)\.rs$' --fail-under-lines 95 --show-missing-lines
  ```

## Documentation

- Keep `README.md` in sync with `yt-upload --help` when flags change.
- `CHANGELOG.md` is generated from PR titles, so write titles that make sense to users.
