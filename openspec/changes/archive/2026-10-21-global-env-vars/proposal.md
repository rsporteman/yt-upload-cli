# Proposal

## Why

Today only `--config-dir` can come from the environment (`YT_UPLOAD_CLI_HOME`), and only when the
variable is already in the process environment. Users who set a variable "globally" (Windows
`setx` or System Properties, macOS `launchctl setenv`, Linux `/etc/environment` or
`~/.config/environment.d`) often run `yt-upload` from a terminal, scheduler or agent that was
started before the change, so the value is silently ignored. Scripts and agents that call
`yt-upload` unattended also have to repeat the same defaults (privacy, category, playlist...) on
every call.

## What Changes

- Upload options that work as reusable defaults get an environment variable with the
  `YT_UPLOAD_CLI_` prefix: `YT_UPLOAD_CLI_PRIVACY`, `YT_UPLOAD_CLI_CATEGORY`,
  `YT_UPLOAD_CLI_TAGS`, `YT_UPLOAD_CLI_PLAYLIST`, `YT_UPLOAD_CLI_MADE_FOR_KIDS`.
  Per-video values (video path, `--title`, `--description`, `--thumbnail`, `--publish-at`) stay
  command-line only, so a stale global value can never end up on the wrong video.
- When one of the supported variables (including the existing `YT_UPLOAD_CLI_HOME`) is missing
  from the process environment, the CLI reads its persisted, OS-level definition:
  - Windows: user then system environment in the registry (`HKCU\Environment`, then
    `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`), expanding `%VAR%`.
  - macOS: the launchd user environment (`launchctl getenv`).
  - Linux and other Unix: `~/.config/environment.d/*.conf`, then `/etc/environment`.
- Precedence: command-line flag > process environment > persisted OS environment > built-in
  default.
- `yt-upload --help` shows the variable for each supported option; README documents the
  variables and how to set them globally on Windows, macOS and Linux.
- Not in scope: reading shell startup files (`.bashrc`, `.zshrc`, PowerShell profiles). Sourcing
  them would mean running user scripts; a variable exported only there still works once the
  shell has loaded it, as today.
- No breaking change: existing flags and `YT_UPLOAD_CLI_HOME` behave as before when set.

## Capabilities

### New Capabilities

- `env-config`: configuring the CLI through environment variables, including which options
  accept them, precedence against flags and defaults, and fallback to the OS-persisted
  (global) environment on Windows, macOS and Linux.

### Modified Capabilities

None (no specs exist yet).

## Impact

- Code: `src/lib.rs` (clap `env` attributes, a parse entry point that injects persisted values),
  a new `src/env.rs` module (lookup per OS plus pure, testable parsers), `src/main.rs` (call the
  new parse function).
- Dependencies: `winreg` on Windows only (`[target.'cfg(windows)'.dependencies]`) to read the
  registry; nothing new on macOS/Linux.
- Privacy: values are read locally and never printed or sent anywhere; no new network calls, no
  OAuth scope change, so `docs/privacy.html` needs no update.
- Docs: README "Setup" and "Usage" sections.
- Release: `feat:` PR, minor version bump.
