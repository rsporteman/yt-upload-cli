# Tasks

## 1. Guidelines

- [x] 1.1 In AGENTS.md "Testing and coverage", after the bullet on manual tests (`--privacy private`, cleanup), add the non-blocking rule: when an agent cannot run a manual test it does not pause; it leaves the task unchecked with `(manual, not run: <reason>)`, lists the test with exact steps as pending in the PR description, opens the PR ready for review once automated checks pass, and continues; archiving with only manual tasks pending is allowed and the archive summary names them. Verify by rereading that the rule names all four behaviors and keeps the existing safety rule unchanged
- [x] 1.2 Add to `openspec/config.yaml` a `rules.tasks` entry saying manual test tasks are labeled `(manual, non-blocking)` and never gate other tasks; verify `openspec instructions tasks --change manual-tests-non-blocking --json` returns the rule under `rules` and the YAML parses

## 2. Integration checks

- [x] 2.1 Run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (expected unaffected by a docs-only change); verify all pass
- [ ] 2.2 Commit on `docs/manual-tests-non-blocking` and open a PR titled `docs: make manual tests non-blocking for agents`; verify CI and the PR-title check are green
