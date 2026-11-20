# Bash and Zsh shell sessions

## What this was about

Opening a standalone shell used to require a custom command. The launcher now has a **Shells** group on the **Agent**
tab. Type `bash` or `zsh` in the launcher and press Enter twice to pick and launch it. Shell sessions have their own
icons and no permission mode. They appear in recent setups and can be saved in launch templates. Clone opens another of
the same type. Stop ends the shell, and Replace starts a fresh shell. Restart is unavailable because there is no
conversation to resume.

## Things you should know

The selected program must be on the PATH where the session runs. Farhelm uses the ordinary launch path and applies any
wrapper prefix. A shell launch needs no YOLO confirmation. An agent's request uses the usual session-creation approval
rules. If the host asks before YOLO launches, Farhelm asks for explicit approval when an agent supplies a wrapper
command for a shell. The shell keeps the session's credentials, so `farhelm spawn` still works from inside it.

The helm and supervisor use protocol version 81 for the new launch choices. Use matching builds when trying the feature.

## Open questions and possible follow-ups

No open decisions. The local tests verified the Zsh launcher and command composition. Zsh process startup remains
untested because the local test host lacks Zsh.

## PRs

- Product behavior, protocol, icons and regression tests. [#4333](https://github.com/scode/farhelm/pull/4333/changes)
- Documents how to launch, save and manage shell sessions on the Shell sessions, Supported agents, Start a session,
  Launch templates, Session list and Stop, restart and resume pages. Removes the completed TODO.
  [#4334](https://github.com/scode/farhelm/pull/4334/changes)

## Checks

- After the rebase, `cargo test --locked -p farhelm-gui --lib -- --test-threads=1 <5 filters>` passed both shell
  shortcuts, Stop and suspend, and terminal tab restoration after switching sessions or reconnecting
  (`198d445e-8525-4062-b31d-afe4475e7c87`). `bun run build` in `website/` built all 39 pages and checked every internal
  link. `cargo fmt --all -- --check` and `dprint check <changed docs>` passed.
- Reused `<built farhelm-proto libtest> --test-threads=1 <5 filters>` for wire and template behavior
  (`beb83b36-b707-461d-98bd-3feb2f3bf560`), `<built farhelm-helm libtest> --test-threads=1 <7 filters>` for launch
  compilation, ordinary approval and launch memory (`dfbc28e0-9e5b-4a76-8819-e839a57b0e02`), and
  `<built farhelm-gui libtest> --test-threads=1 <14 filters>` for launcher and approval UI, assets and lifecycle wording
  (`be8a012b-e8c9-4753-a560-8355be843dbc`). These ran on the working tree based on `cb055d21`. Main added remembered
  terminal tabs and Antigravity conversation verification. Both changes leave shell selection, compilation and
  permission handling intact, and the post-rebase tests cover tab restoration.
- Reused `python3 -B scripts/gui-e2e-smoke.py --no-build` on the same pre-rebase tree's just-built binaries. It passed
  49 of 50 steps (`9f42c9af-3890-492b-99f0-e98afdf001c5`). The clipboard case skipped because the headless compositor
  provides no clipboard ownership. `python3 scripts/check-gui-goldens.py --update` produced all 298 captures and each
  matched its recorded image (`03b5c2df-c1cc-4f2f-b030-1cff6942606b`). Main kept the screenshot scenes and reference
  manifest intact. The captures do not restore a previously chosen tab or handle agent conversation callbacks.
- Reused
  `cargo clippy -p farhelm -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm-gui --all-targets -- -D warnings`,
  `python -B scripts/check-test-sleeps.py` and `python3 releasing/check-changelog.py format` from the pre-rebase tree.
  Main left this stack's Rust changes, test waits and shell changelog fragment intact.
- Skipped the full Rust and cross-platform release batteries. The local selections cover the changed launch and
  lifecycle paths, and the post-rebase checks exercise the overlapping desktop behavior.

## Review gate

The required independent review ran for both PRs. The implementation review caught search precedence when a remembered
model shares a shell's name, a template that tried to set a shell as a command's agent, and surface-specific wording and
asset omissions. Those were corrected. The docs review corrected the shortcut, template, remembered permission and
Restart wording. The PR messages passed a separate cold read.
