# Contributing

Thanks for helping with yt-upload-cli! Everything in this repository is written in English.

## Workflow

1. Fork the repo and create a branch from `main`.
2. Make your change. Before pushing, run:

   ```
   cargo fmt --all
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```

   New code needs tests: CI requires at least **95% line coverage** (see [AGENTS.md](AGENTS.md#testing-and-coverage)).
   Name branches in English as `<type>/<short-description>`, e.g. `feat/language-flag`.

3. Open a pull request whose **title follows [Conventional Commits](https://www.conventionalcommits.org/)**,
   for example `feat: add --language flag` or `fix(upload): retry failed chunks`.
   PRs are squash-merged, so the title becomes the commit message and the changelog entry.
4. CI must pass and a maintainer must approve before merging.

You don't need to bump versions or edit `CHANGELOG.md`: releases are automated by
[release-please](https://github.com/googleapis/release-please) following
[Semantic Versioning](https://semver.org/).

See [AGENTS.md](AGENTS.md) for the full guidelines (code style, security rules, commit types).

## Security

Never include credentials (`client_secret*.json`, `token.json`, tokens) in issues, PRs or logs.
To report a vulnerability privately, use
[GitHub's private vulnerability reporting](https://github.com/rsporteman/yt-upload-cli/security/advisories/new).
