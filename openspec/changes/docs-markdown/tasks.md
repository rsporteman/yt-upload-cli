# Tasks

## 1. Convert the pages

- [x] 1.1 Create branch `docs/markdown-pages` from `main`; convert `docs/index.html`, `docs/privacy.html` and `docs/terms.html` to `index.md`, `privacy.md` and `terms.md` per design decision 3 (no front matter, internal links to `.md`) and delete the HTML files; verify by comparing old and new rendered text side by side (same sections, sentences, links and "Last updated" dates) and by previewing each file in GitHub's Markdown preview (or `gh markdown-preview`, if installed) to check that it renders as formatted text
- [x] 1.2 Add `docs/_config.yml` with `title` and `description` per design decision 2; verify the file is valid YAML (e.g. `python -c "import yaml,sys;yaml.safe_load(open('docs/_config.yml'))"` or any YAML linter)

## 2. Enforcement

- [x] 2.1 Add `tests/docs.rs` with `docs_pages_are_markdown` and `docs_relative_links_exist` per design decision 4, plus unit tests of the link extractor (external links, `#anchors` and `file.md#section` handled); verify `cargo test --test docs` passes on the converted docs, and fails (then revert the probe) after temporarily adding `docs/x.html` and after temporarily adding a link to `missing.md`

## 3. Contributor rules

- [x] 3.1 Update AGENTS.md: in "Project layout", describe `docs/` as GitHub Flavored Markdown pages published by GitHub Pages at stable URLs. In "Security and privacy", replace `docs/privacy.html` with `docs/privacy.md`. In "Documentation", add that docs pages must be `.md` without front matter or inline HTML, and that internal links point to `.md` files. Verify `grep -n "privacy.html" AGENTS.md` returns nothing and the README privacy URL (`.../privacy.html`) is unchanged

## 4. Integration checks

- [x] 4.1 Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and the AGENTS.md coverage command; verify all pass
- [ ] 4.2 Open a PR titled `docs: write the docs pages in github markdown`, noting that the URLs are unchanged and the post-merge check is pending; verify CI is green
- [ ] 4.3 After the maintainer merges it, run `curl -sI` on `https://rsporteman.github.io/yt-upload-cli/`, `.../privacy.html` and `.../terms.html` and open each page; verify all return HTTP 200 with the expected content, and if not, report immediately and propose the revert
