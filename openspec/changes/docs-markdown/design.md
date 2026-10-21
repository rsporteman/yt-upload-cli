# Design

## Context

- GitHub Pages for this repo is a **legacy (Jekyll) build** from `main:/docs`. There is no
  `.nojekyll` and no `_config.yml`, and the three pages are static HTML with inline CSS.
- GitHub Pages enables these Jekyll plugins by default: `jekyll-optional-front-matter` (renders
  `.md` without front matter), `jekyll-titles-from-headings` (page title from the first `#`),
  `jekyll-relative-links` (rewrites `privacy.md` links to `privacy.html`), and
  `jekyll-default-layout`. The default theme is `jekyll-theme-primer`.
- External links to the site: the Google OAuth consent screen (privacy and terms URLs, plus the
  home page) and README's privacy link.
- See proposal.md for motivation and specs/project-docs/spec.md for required behavior.

## Goals / Non-Goals

**Goals:**
- Same files serve both audiences: github.com readers and the Pages site.
- No change to any published URL.

**Non-Goals:**
- Visual redesign, a custom theme or layouts.
- Moving the site to a GitHub Actions Pages workflow.
- Changing the policy wording. Legal text is moved, not edited.

## Decisions

### 1. Rely on GitHub Pages' default Jekyll plugins, without front matter
`privacy.md` is published as `privacy.html` by Jekyll's normal permalink. The title comes from the
first heading, and the layout from `jekyll-default-layout`. Omitting front matter keeps github.com
from showing a YAML table at the top of each page.

*Alternatives:* front matter with `permalink: /privacy.html`. Rejected because it is redundant
with the default and renders as a table on github.com. Switching to an Actions-built site with
a static generator was rejected as more moving parts for 3 pages.

### 2. Minimal `docs/_config.yml`
```yaml
title: yt-upload-cli
description: Upload videos to YouTube from the command line
```
This gives the primer theme a site title and `<meta description>`. No theme override.
`_config.yml` is a config file, not a page, so the "no HTML" rule does not apply to it and
github.com shows it as YAML (expected).

### 3. Conversion rules
- `<h1>`/`<h2>` become `#`/`##`; `<code>` becomes backticks; `<a href>` becomes `[text](url)`;
  `<ul><li>` becomes a `-` list.
- The home page's command example becomes a fenced `` ``` `` block.
- Internal links change from `privacy.html` to `privacy.md` (and so on), and "Back" points to
  `index.md`.
- Inline `<style>` is dropped; the theme provides the styling.

### 4. Enforcement in `tests/docs.rs`
- `docs_pages_are_markdown`: walks `docs/` recursively. Any file ending in `.html`/`.htm` fails
  the test, and the error lists the files with the hint "docs pages must be GitHub Flavored
  Markdown (.md)".
- `docs_relative_links_exist`: for each `.md` in `docs/`, it extracts `](target)` links whose
  target does not start with a scheme (`http:`, `https:`, `mailto:`) or `#`. It strips any
  `#fragment` and checks that the file exists relative to the page. All failures are reported
  together.
- Uses only `std` (a simple scan for `](`…`)`). Images and reference-style links are not used in
  these pages, and the README link check is out of scope.

*Alternative:* a markdown link checker action (e.g. lychee) in CI. Rejected because it adds a CI
dependency and network calls, while `cargo test` already runs locally and on all OSes.

### 5. Post-deploy verification
After merge, the Pages build runs on `main`. Verify with
`curl -sI https://rsporteman.github.io/yt-upload-cli/{,privacy.html,terms.html}` that all three
return 200 and the content matches. If not, revert the PR immediately, since the OAuth consent
screen depends on these URLs.

## Risks / Trade-offs

- [Pages output can't be previewed before merge (source is `main`)] → content is tiny; the
  post-merge curl check and an immediate revert path limit exposure to minutes.
- [Google's OAuth verification flags a changed home page] → same text, links and app name;
  only styling changes.
- [Default theme adds a "View on GitHub" header/branding] → acceptable. No YouTube or Google
  logos are introduced, so the AGENTS.md branding rule is respected.

## Migration Plan

Single `docs:` PR. Deploy happens automatically on merge, followed by the curl check in decision 5.
Rollback means reverting the PR, which restores the HTML files.
