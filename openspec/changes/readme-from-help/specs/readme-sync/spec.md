# Spec Delta

## Purpose

Keeps README.md's documentation of how to use yt-upload identical to the CLI's real help and free
of commands the CLI no longer accepts, so new usage is documented and legacy usage disappears at
the moment the code changes.

## ADDED Requirements

### Requirement: Generated usage section
README.md SHALL contain exactly one section delimited by `<!-- usage:start -->` and
`<!-- usage:end -->` whose content is the CLI's top-level help followed by the help of the
`upload` command, each in a fenced code block. The generated text SHALL be identical on Linux,
Windows and macOS and independent of the environment: the program name is always `yt-upload`,
lines wrap at a fixed width, and environment variables appear by name without their current
values. Everything outside the markers SHALL be left untouched by generation.

#### Scenario: Help and README agree
- **WHEN** the README section is regenerated on any supported OS
- **THEN** it equals the output of `yt-upload --help` and `yt-upload upload --help`, with the program name shown as `yt-upload`

#### Scenario: Environment values are not leaked
- **WHEN** `YT_UPLOAD_CLI_PRIVACY=unlisted` is set while the section is generated
- **THEN** the README shows `[env: YT_UPLOAD_CLI_PRIVACY]` without `=unlisted`

#### Scenario: Prose is preserved
- **WHEN** the section is regenerated
- **THEN** every byte of README.md outside the markers is unchanged

### Requirement: Stale README fails the build
The project's test suite SHALL fail when the generated section in README.md differs from what the
current CLI produces. The failure message SHALL state the command that regenerates it.

#### Scenario: New flag without README update
- **WHEN** a developer adds an option to `upload` and runs `cargo test` without regenerating the README
- **THEN** the test fails and tells them to run the regeneration command

#### Scenario: Missing markers
- **WHEN** README.md lacks the markers or has them more than once
- **THEN** the test fails saying the markers are missing or duplicated

### Requirement: README examples use current syntax
Every command line in a fenced code block of README.md that starts with `yt-upload ` SHALL be
accepted by the current CLI parser. Placeholders in angle brackets (`<VIDEO>`) or square brackets
(`[OPTIONS]`) SHALL be excluded from this check, and lines inside the generated section are not
examples. Otherwise the test suite SHALL fail and name the offending line.

#### Scenario: Legacy flag in an example
- **WHEN** the README shows `yt-upload upload v.mp4 --title T --visibility public` and `--visibility` does not exist
- **THEN** `cargo test` fails naming that line

#### Scenario: Valid examples pass
- **WHEN** all README examples use existing commands and flags
- **THEN** the check passes

### Requirement: Pre-commit regeneration
The repository SHALL provide a versioned pre-commit hook that regenerates the usage section and
adds README.md to the commit. It SHALL refuse the commit, without changing anything, when
README.md has unstaged modifications, so partially staged edits are never committed by accident.
It SHALL work in Git for Windows' shell, on macOS and on Linux, and contributors enable it with
`git config core.hooksPath .githooks`.

#### Scenario: Commit after changing a flag
- **WHEN** the hook is enabled and a developer commits a change to a flag's help text
- **THEN** the commit includes the regenerated README usage section

#### Scenario: Unstaged README edits
- **WHEN** README.md has unstaged changes at commit time
- **THEN** the hook aborts the commit with a message asking to stage or stash README.md

#### Scenario: README already current
- **WHEN** regeneration produces no change
- **THEN** the commit proceeds and README.md is not modified

### Requirement: Contributor rules
AGENTS.md SHALL document that the usage section is generated, how to enable the hook and
regenerate manually, and that any change to how the CLI is used SHALL also update the README's
prose and examples: document new usage, remove usage that no longer exists, and correct statements
that changed.

#### Scenario: Agent changes CLI behavior
- **WHEN** an agent reads AGENTS.md before changing a command or flag
- **THEN** it finds the instruction to regenerate the usage section and review the README prose in the same PR
