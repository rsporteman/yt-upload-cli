# Spec Delta

## Purpose

Defines how the project's documentation pages (home, privacy policy, terms of service) are
written and published, so they read correctly on github.com and keep the stable public URLs that
Google's OAuth consent screen depends on.

## ADDED Requirements

### Requirement: Docs pages are GitHub Flavored Markdown
Every page in `docs/` SHALL be a GitHub Flavored Markdown file (`.md`) that renders as a readable
document when opened on github.com: no raw HTML pages, no inline styles, and no front matter that
github.com would show as a table. The `docs/` folder MUST NOT contain `.html` files.

#### Scenario: Privacy policy on github.com
- **WHEN** a user opens `docs/privacy.md` in the GitHub repository
- **THEN** GitHub renders it as formatted text with headings and links, not as markup

#### Scenario: HTML page added by mistake
- **WHEN** a contributor adds `docs/faq.html`
- **THEN** `cargo test` fails, naming the file and saying that docs pages must be Markdown

### Requirement: Stable public URLs
The GitHub Pages site SHALL keep serving the home page at
`https://rsporteman.github.io/yt-upload-cli/`, the privacy policy at `.../privacy.html` and the
terms at `.../terms.html`.

#### Scenario: OAuth consent screen link
- **WHEN** someone opens `https://rsporteman.github.io/yt-upload-cli/privacy.html` after the change is deployed
- **THEN** the site answers HTTP 200 with the privacy policy

#### Scenario: Terms link
- **WHEN** someone opens `https://rsporteman.github.io/yt-upload-cli/terms.html`
- **THEN** the site answers HTTP 200 with the terms of service

### Requirement: Content preserved
The converted pages SHALL keep the existing wording, section headings, external links and "Last
updated" dates of the HTML pages they replace.

#### Scenario: Privacy policy text
- **WHEN** the old `privacy.html` and the new `privacy.md` are compared as rendered text
- **THEN** they contain the same sections, sentences, links and date

### Requirement: Links work on both github.com and the site
Links between docs pages SHALL be relative links to the `.md` files, and every relative link in a
docs page SHALL point to a file that exists in the repository.

#### Scenario: Back link on github.com
- **WHEN** a user clicks "Back" in `docs/privacy.md` on github.com
- **THEN** GitHub opens `docs/index.md`

#### Scenario: Link on the site
- **WHEN** a user clicks "Privacy Policy" on the published home page
- **THEN** the browser opens `.../privacy.html`

#### Scenario: Broken relative link
- **WHEN** a docs page links to `faq.md` and that file does not exist
- **THEN** `cargo test` fails, naming the page and the link

### Requirement: Contributor rules
AGENTS.md SHALL state that docs pages are GitHub Flavored Markdown, and SHALL name
`docs/privacy.md` as the privacy policy to update whenever data handling or OAuth scopes change.

#### Scenario: Agent changes OAuth scopes
- **WHEN** an agent reads AGENTS.md before adding a scope
- **THEN** it is told to update `docs/privacy.md` in Markdown in the same PR
