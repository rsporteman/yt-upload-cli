//! The `docs/` pages must read well on github.com and on GitHub Pages: Markdown only, and every
//! relative link must point to a file that exists.

use std::path::{Path, PathBuf};

fn docs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("docs")
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files(&path));
        } else {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Targets of inline `[text](target)` links that point inside the repository, without any
/// `#fragment`.
fn relative_links(markdown: &str) -> Vec<&str> {
    markdown
        .split("](")
        .skip(1)
        .filter_map(|rest| rest.split_once(')').map(|(target, _)| target.trim()))
        .filter(|t| !t.starts_with('#') && !t.contains(':'))
        .map(|t| t.split('#').next().unwrap_or(t))
        .collect()
}

#[test]
fn docs_pages_are_markdown() {
    let html: Vec<_> = files(&docs_dir())
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "html" || e == "htm"))
        .collect();
    assert!(
        html.is_empty(),
        "docs pages must be GitHub Flavored Markdown (.md), found: {html:?}"
    );
}

#[test]
fn docs_relative_links_exist() {
    let mut broken = Vec::new();
    for page in files(&docs_dir()) {
        if page.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let text = std::fs::read_to_string(&page).unwrap();
        for link in relative_links(&text) {
            if !page.parent().unwrap().join(link).exists() {
                broken.push(format!("{}: {link}", page.display()));
            }
        }
    }
    assert!(broken.is_empty(), "broken links in docs: {broken:#?}");
}

#[test]
fn extracts_only_relative_links() {
    let md = "[a](privacy.md) [b](https://x.y/z) [c](#top) [d](terms.md#contact) \
              [e](mailto:a@b.c) [f]( index.md )";
    assert_eq!(relative_links(md), ["privacy.md", "terms.md", "index.md"]);
}
