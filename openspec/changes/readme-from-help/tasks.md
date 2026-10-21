# Tasks

## 1. Prepare

- [x] 1.1 After PR #3 (`global-env-vars`) and `feat/thumbnail-when-allowed` are merged, rebase `docs/usage-in-readme` on `main`, then discard the scaffolded untracked `openspec/config.yaml` in favor of the versioned one; verify `git log` shows both features on the branch and `openspec status --change readme-from-help` still reports 4/4 artifacts

## 2. Help text

- [ ] 2.1 Add `#[command(name = "yt-upload")]` and doc comments for `--title`, `--description` and `--privacy` in `src/lib.rs`; verify `cargo run -- upload --help` shows a description for every option and the existing unit tests pass

## 3. Generated usage and its check

- [ ] 3.1 Create `tests/readme.rs` with `usage_markdown()` (bin name `yt-upload`, width 100, env values hidden recursively, root and `upload` long help in fenced blocks) and `readme_usage_is_current` (marker replacement, `UPDATE_README=1` write mode, CRLF normalization, clear failure message, error on missing or duplicated markers); verify with unit tests in the same file for the marker logic (replace, missing, duplicate) and that the output contains `[env: YT_UPLOAD_CLI_PRIVACY]` without `=` when generated with that variable set in a spawned `cargo test` (or by asserting no `[env: ...=` substring)
- [ ] 3.2 Add `readme_examples_parse` (fenced blocks outside markers, lines starting with `yt-upload `, placeholders skipped, quote-aware split, `--help`/`--version` treated as success, all failures listed); verify with unit tests of the splitter and of a sample README containing one legacy flag that is reported by line

## 4. README rewrite

- [ ] 4.1 Restructure README.md per design decision 5: insert the markers in "Usage", delete the hand-written option list, review every remaining section against current behavior (environment variables, thumbnail validation and skip, `config-dir`/`auth`, limits), remove outdated statements, then run `UPDATE_README=1 cargo test --test readme`; verify `cargo test --test readme` passes without the variable and a manual read finds no flag or command that the CLI lacks

## 5. Pre-commit hook and contributor rules

- [ ] 5.1 Add `.githooks/pre-commit` per design decision 3, committed with `git update-index --chmod=+x`; verify on Windows (Git Bash) that with `core.hooksPath=.githooks`: (a) a help-text change is committed with the regenerated README, (b) unstaged README edits abort the commit, (c) an unrelated commit leaves README untouched
- [ ] 5.2 Update AGENTS.md: in "Before every commit", add `git config core.hooksPath .githooks` (once per clone) and `UPDATE_README=1 cargo test --test readme`. In "Documentation", replace "Keep README.md in sync with `yt-upload --help`" with the generated-section explanation plus the rule to document new usage, remove usage that no longer exists and fix changed statements in the README prose in the same PR. In "Project layout", list `.githooks/` and `tests/`. Verify by rereading that the instructions match the hook and test names

## 6. Integration checks

- [ ] 6.1 Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and the AGENTS.md coverage command; verify all pass and coverage stays at or above 95%
- [ ] 6.2 Open a PR titled `docs: generate the readme usage section from the cli help` explaining the hook opt-in and the new tests; verify CI is green on Linux, Windows and macOS (proving the generated text is identical across OSes) and the PR-title check passes
