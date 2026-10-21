# Proposal

## Why

The `docs/` folder (home, privacy policy, terms of service) is written as hand-styled HTML. On
github.com those files show as raw source, so anyone browsing the repository, reviewing a PR or
checking the privacy policy next to the code reads markup instead of a document. Every other
document in the repo (README, AGENTS, CHANGELOG) is already Markdown, and HTML pages are harder to
edit in PRs that must update the privacy policy (a rule in AGENTS.md).

## What Changes

- Convert `docs/index.html`, `docs/privacy.html` and `docs/terms.html` to GitHub Flavored
  Markdown (`index.md`, `privacy.md`, `terms.md`) with the same content and the same
  "Last updated" dates. Content is moved, not reworded.
- Keep the published URLs unchanged: GitHub Pages (already building `main:/docs` with Jekyll)
  turns `privacy.md` into `privacy.html`. This matters because the Google OAuth consent screen
  and README link to `https://rsporteman.github.io/yt-upload-cli/privacy.html` and `terms.html`.
- Links between docs pages point to the `.md` files, so they work on github.com. On the site,
  GitHub Pages rewrites them to `.html`.
- Add a minimal `docs/_config.yml` (site title and description) so the site keeps a proper title
  and look now that pages are rendered by the default theme instead of inline CSS.
- Enforce the format: `cargo test` fails if `docs/` contains `.html` files or if a relative link
  in a docs Markdown file points to a file that does not exist.
- AGENTS.md: docs pages are GitHub Flavored Markdown, and the privacy-policy rule now names
  `docs/privacy.md`.
- Assumption: "docs" means the `docs/` site. README, AGENTS.md and CHANGELOG are already Markdown
  and stay as they are, apart from the AGENTS.md references above.

## Capabilities

### New Capabilities

- `project-docs`: format, location and public URLs of the project's documentation pages (home,
  privacy policy, terms of service), readable both on github.com and on the GitHub Pages site.

### Modified Capabilities

None (no specs exist yet).

## Impact

- Files: `docs/*.html` removed; `docs/index.md`, `docs/privacy.md`, `docs/terms.md` and
  `docs/_config.yml` added; `AGENTS.md` updated; new `tests/docs.rs`.
- External: GitHub Pages output changes from hand-styled pages to the default theme at the same
  URLs. Google's OAuth consent screen keeps working because the URLs are unchanged.
- No code, dependency or OAuth-scope change. Privacy policy content is unchanged.
- Release: `docs:` PR. It is independent of `readme-from-help`; both touch AGENTS.md, so whichever
  merges second rebases.
