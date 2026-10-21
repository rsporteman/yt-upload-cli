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
