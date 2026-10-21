//! README.md documents the CLI: its "Usage" section is generated from the real help, and every
//! example command in it must still parse. Regenerate with:
//!
//!     UPDATE_README=1 cargo test --test readme

use std::path::PathBuf;

use clap::{CommandFactory, Parser};
use clap::error::ErrorKind;
use yt_upload_cli::Cli;

const START: &str = "<!-- usage:start -->";
const END: &str = "<!-- usage:end -->";
const REGENERATE: &str = "UPDATE_README=1 cargo test --test readme";

fn readme_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("README.md")
}

fn read_readme() -> String {
    std::fs::read_to_string(readme_path())
        .unwrap()
        .replace("\r\n", "\n")
}

/// The generated section, identical on every OS and independent of the environment: the help
/// would otherwise show `yt-upload.exe` on Windows and the current values of env variables.
fn usage_markdown() -> String {
    let hide_values = |a: clap::Arg| a.hide_env_values(true);
    let mut cmd = Cli::command()
        .bin_name("yt-upload")
        .mut_args(hide_values)
        .mut_subcommands(|s| s.mut_args(hide_values));
    cmd.build();
    let root = cmd.render_help().to_string();
    let upload = cmd
        .find_subcommand_mut("upload")
        .unwrap()
        .render_help()
        .to_string();
    let block = |help: &str| {
        let lines: Vec<_> = help.trim_end().lines().map(str::trim_end).collect();
        format!("```\n{}\n```\n", lines.join("\n"))
    };
    format!(
        "{START}\n<!-- Generated from the CLI help; refresh with: {REGENERATE} -->\n\n\
         `yt-upload --help`\n\n{}\n`yt-upload upload --help`\n\n{}{END}",
        block(&root),
        block(&upload)
    )
}

/// `readme` with the text between the markers (markers included) replaced by `section`.
fn replace_usage(readme: &str, section: &str) -> Result<String, String> {
    let count = |m: &str| readme.matches(m).count();
    if count(START) != 1 || count(END) != 1 {
        return Err(format!(
            "README.md must contain {START} and {END} exactly once each"
        ));
    }
    let start = readme.find(START).unwrap();
    let end = readme.find(END).unwrap();
    if end < start {
        return Err(format!("{END} comes before {START} in README.md"));
    }
    Ok(format!(
        "{}{section}{}",
        &readme[..start],
        &readme[end + END.len()..]
    ))
}

/// Splits a command line on spaces, keeping "double quoted" words together.
fn split_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut in_word = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                in_word = true;
            }
            ' ' if !quoted => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            _ => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(word);
    }
    words
}

/// `yt-upload ...` lines in fenced code blocks outside the generated section, with their line
/// numbers. Lines with `<placeholders>` or `[OPTIONS]` are syntax summaries, not examples.
fn examples(readme: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let (mut fenced, mut generated) = (false, false);
    for (i, line) in readme.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed == START {
            generated = true;
        } else if trimmed == END {
            generated = false;
        } else if trimmed.starts_with("```") {
            fenced = !fenced;
        } else if fenced
            && !generated
            && trimmed.starts_with("yt-upload ")
            && !trimmed.contains(['<', '['])
        {
            out.push((i + 1, trimmed));
        }
    }
    out
}

/// Error for an example the current CLI rejects; `--help`/`--version` count as accepted.
fn check_example(line: &str) -> Option<String> {
    match Cli::try_parse_from(split_words(line)) {
        Ok(_) => None,
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => None,
        Err(e) => Some(e.to_string().lines().next().unwrap_or("").to_owned()),
    }
}

#[test]
fn readme_usage_is_current() {
    let readme = read_readme();
    let updated = replace_usage(&readme, &usage_markdown()).unwrap();
    if updated == readme {
        return;
    }
    if std::env::var_os("UPDATE_README").is_some() {
        std::fs::write(readme_path(), updated).unwrap();
    } else {
        panic!("README usage is out of date; run: {REGENERATE}");
    }
}

#[test]
fn readme_examples_parse() {
    let readme = read_readme();
    let failures: Vec<_> = examples(&readme)
        .into_iter()
        .filter_map(|(n, line)| check_example(line).map(|e| format!("line {n}: {line}\n  {e}")))
        .collect();
    assert!(
        failures.is_empty(),
        "README examples the CLI rejects:\n{}",
        failures.join("\n")
    );
}

#[test]
fn usage_section_is_stable() {
    let usage = usage_markdown();
    assert!(usage.contains("Usage: yt-upload [OPTIONS] <COMMAND>"));
    assert!(usage.contains("Usage: yt-upload upload [OPTIONS]"));
    assert!(usage.contains("[env: YT_UPLOAD_CLI_PRIVACY]"));
    assert!(!usage.contains(".exe"));
    // Env values never leak, whatever the environment of the test run.
    for env in usage.split("[env: ").skip(1) {
        let name = env.split(']').next().unwrap();
        assert!(!name.contains('='), "env value shown: {name}");
    }
}

#[test]
fn replaces_only_between_markers() {
    let readme = format!("intro\n{START}\nold\n{END}\noutro\n");
    assert_eq!(
        replace_usage(&readme, "NEW").unwrap(),
        "intro\nNEW\noutro\n"
    );
    assert!(replace_usage("no markers", "x").unwrap_err().contains("exactly once"));
    let twice = format!("{START}{END}{START}{END}");
    assert!(replace_usage(&twice, "x").unwrap_err().contains("exactly once"));
    let reversed = format!("{END}{START}");
    assert!(replace_usage(&reversed, "x").unwrap_err().contains("before"));
}

#[test]
fn splits_quoted_words() {
    assert_eq!(
        split_words(r#"yt-upload upload v.mp4 --title "My video"  -p public"#),
        ["yt-upload", "upload", "v.mp4", "--title", "My video", "-p", "public"]
    );
    assert_eq!(split_words(r#"a """#), ["a", ""]);
}

#[test]
fn finds_and_checks_examples() {
    let readme = format!(
        "yt-upload outside a fence\n```\nyt-upload auth\nyt-upload upload <VIDEO> [OPTIONS]\n\
         yt-upload upload v.mp4 --title T --visibility public\n```\n{START}\n```\n\
         yt-upload ignored --in-generated\n```\n{END}\n"
    );
    let found = examples(&readme);
    assert_eq!(
        found,
        [
            (3, "yt-upload auth"),
            (5, "yt-upload upload v.mp4 --title T --visibility public")
        ]
    );
    assert_eq!(check_example(found[0].1), None);
    assert!(check_example(found[1].1).unwrap().contains("--visibility"));
    assert_eq!(check_example("yt-upload --help"), None);
}
