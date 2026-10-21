# Proposal

## Why

Manual tests, such as uploading to a real channel or checking the live GitHub Pages site, need the
maintainer's credentials, browser or a merge. Agents currently stop and wait for them: PR #6 waited
on a manual upload test until the maintainer chose to skip it, and PR #7 is still a draft for the
same reason. Automated tests (mock server, 95% coverage gate, CI on three OSes) are the merge gate.
Manual tests are extra confidence and should not hold up the flow.

## What Changes

- AGENTS.md "Testing and coverage" gains a rule that **manual tests never block the flow**. An
  agent that cannot run a manual test, for lack of credentials, a browser or a pending merge, does
  not pause. It:
  - leaves the task unchecked, with a note `(manual, not run: <reason>)`;
  - lists the manual test as pending in the PR description, with the exact steps for the
    maintainer;
  - opens the PR ready for review (not as a draft) once automated checks pass, and continues with
    the next tasks or changes.
- Archiving a change whose only incomplete tasks are manual tests is allowed. The pending manual
  tests are named in the archive summary.
- `openspec/config.yaml` gains a `tasks` rule so new plans label manual test tasks as
  `(manual, non-blocking)`, keeping them easy to spot.
- Existing safety rules for manual tests are unchanged: `--privacy private` and cleanup afterwards.
- Assumption: "fluxo" means the agent workflow (apply, PR, archive). The maintainer still decides
  whether to merge without the manual test.

## Capabilities

### New Capabilities

None: contributor guidelines only, no change to the CLI's behavior (`skip_specs: true`).

### Modified Capabilities

None.

## Impact

- Files: `AGENTS.md`, `openspec/config.yaml`.
- No code, tests, dependencies or user-facing behavior change.
- Release: `docs:` PR.
- Follow-up enabled by this rule: PR #7 (`multiple-accounts`) can leave draft with its manual
  test (task 4.2) listed as pending.
