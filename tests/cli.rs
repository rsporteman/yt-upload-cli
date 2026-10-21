//! Runs the real binary to check environment-variable handling done by clap, without changing
//! the environment of the test process.

use std::process::{Command, Output};

const VARS: &[&str] = &[
    "YT_UPLOAD_CLI_HOME",
    "YT_UPLOAD_CLI_PRIVACY",
    "YT_UPLOAD_CLI_CATEGORY",
    "YT_UPLOAD_CLI_TAGS",
    "YT_UPLOAD_CLI_PLAYLIST",
    "YT_UPLOAD_CLI_MADE_FOR_KIDS",
    "YT_UPLOAD_CLI_ACCOUNT",
];

fn run(args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_yt-upload"));
    for var in VARS {
        cmd.env_remove(var);
    }
    cmd.args(args).envs(env.iter().copied()).output().unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn config_dir_comes_from_the_environment() {
    let out = run(&["config-dir"], &[("YT_UPLOAD_CLI_HOME", "env-home")]);
    assert!(out.status.success());
    assert_eq!(stdout(&out), "env-home");
}

#[test]
fn flag_beats_environment() {
    let out = run(
        &["--config-dir", "flag-home", "config-dir"],
        &[("YT_UPLOAD_CLI_HOME", "env-home")],
    );
    assert_eq!(stdout(&out), "flag-home");
}

#[test]
fn empty_variable_is_ignored() {
    let out = run(&["config-dir"], &[("YT_UPLOAD_CLI_HOME", "")]);
    assert!(out.status.success());
    assert!(stdout(&out).ends_with("yt-upload-cli"));
}

#[test]
fn upload_help_lists_the_variables() {
    let help = stdout(&run(&["upload", "--help"], &[]));
    for var in &VARS[1..] {
        assert!(help.contains(var), "{var} missing from help");
    }
    assert!(!help.contains("YT_UPLOAD_CLI_TITLE"));
}

#[test]
fn invalid_privacy_lists_allowed_values() {
    let out = run(
        &["upload", "v.mp4", "-t", "T"],
        &[("YT_UPLOAD_CLI_PRIVACY", "secret")],
    );
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("private, unlisted, public"), "{err}");
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// A config folder with these accounts (`channel` is `(id, title)` or none) and no OAuth client,
/// so any attempt to reach Google would fail loudly.
fn config_with(accounts: &[(&str, Option<(&str, &str)>)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (alias, channel) in accounts {
        let account = dir.path().join("accounts").join(alias);
        std::fs::create_dir_all(&account).unwrap();
        std::fs::write(account.join("token.json"), "{}").unwrap();
        if let Some((id, title)) = channel {
            let json = format!(r#"{{"id":"{id}","title":"{title}","handle":null}}"#);
            std::fs::write(account.join("channel.json"), json).unwrap();
        }
    }
    dir
}

fn run_in(dir: &tempfile::TempDir, args: &[&str]) -> Output {
    let mut full = vec!["--config-dir", dir.path().to_str().unwrap()];
    full.extend_from_slice(args);
    run(&full, &[])
}

#[test]
fn lists_accounts_sorted_and_tab_separated() {
    let dir = config_with(&[("work", Some(("UCw", "Work"))), ("personal", None)]);
    let out = run_in(&dir, &["accounts", "list"]);
    assert!(out.status.success());
    assert_eq!(
        stdout(&out),
        "personal	-	-	-
work	Work	UCw	-"
    );
}

#[test]
fn empty_account_list_hints_on_stderr() {
    let dir = config_with(&[]);
    let out = run_in(&dir, &["accounts", "list"]);
    assert!(out.status.success());
    assert_eq!(stdout(&out), "");
    assert!(stderr(&out).contains("yt-upload auth --account"));
}

#[test]
fn removes_accounts() {
    let dir = config_with(&[("work", None), ("personal", None)]);
    let out = run_in(&dir, &["accounts", "remove", "work"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).contains("myaccount.google.com/permissions"));
    assert_eq!(
        stdout(&run_in(&dir, &["accounts", "list"])),
        "personal	-	-	-"
    );
    let out = run_in(&dir, &["accounts", "remove", "work"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("no account named 'work'"));
}

#[test]
fn upload_requires_an_account_when_several_exist() {
    let dir = config_with(&[("work", None), ("personal", None)]);
    let out = run_in(&dir, &["upload", "v.mp4", "-t", "T"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("personal, work") && err.contains("--account"),
        "{err}"
    );
    assert!(
        !err.contains("client_secret"),
        "must fail before any OAuth work: {err}"
    );

    let out = run_in(&dir, &["upload", "v.mp4", "-t", "T", "--account", "nope"]);
    assert!(stderr(&out).contains("no account named 'nope'"));
}

#[test]
fn upload_names_the_target_channel() {
    let dir = config_with(&[("work", Some(("UCw", "Work Channel")))]);
    let out = run_in(&dir, &["upload", "v.mp4", "-t", "T"]);
    // No OAuth client in this folder, so it stops right after naming the target.
    let err = stderr(&out);
    assert!(
        err.contains("Uploading to Work Channel (account 'work')"),
        "{err}"
    );
    assert!(err.contains("client_secret.json"), "{err}");
}

#[test]
fn migrates_the_legacy_token() {
    let dir = config_with(&[]);
    std::fs::write(dir.path().join("token.json"), "{}").unwrap();
    let out = run_in(&dir, &["accounts", "list"]);
    assert!(stderr(&out).contains("Moved the saved token to account 'default'"));
    assert_eq!(stdout(&out), "default	-	-	-");
    assert!(!dir.path().join("token.json").exists());
    // config-dir never touches accounts.
    std::fs::write(dir.path().join("token.json"), "{}").unwrap();
    assert!(run_in(&dir, &["config-dir"]).status.success());
    assert!(dir.path().join("token.json").exists());
}

#[test]
fn auth_rejects_invalid_account_names() {
    let dir = config_with(&[]);
    let out = run_in(&dir, &["auth", "--account", "My Channel"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("invalid account name"));
}
