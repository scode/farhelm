# Plan: Shift-Cmd-V to paste via temporary file upload

Slug: `shift-cmd-v-paste-upload`

## Goal

Add a terminal keyboard shortcut (**Shift-Cmd-V** on macOS, **Ctrl-Alt-V** on Linux) that uploads clipboard text as a
securely allocated temporary file on the session's host — reusing Farhelm's existing per-session attachment pipeline —
and inserts its shell-escaped host path into the terminal instead of pasting the text directly.

### Acceptance criteria

1. Pressing `Shift-Cmd-V` (`cmd-shift-v`) on macOS or `Ctrl-Alt-V` (`ctrl-alt-v`) on Linux in a focused live terminal
   uploads clipboard plain text verbatim as UTF-8 bytes (`pasted-<n>.txt`, sharing the per-terminal `pasted-<n>`
   counter) through the existing per-session attachment upload pipeline (`<state_dir>/attachments/<session-id>/`,
   private `0700` dir, `0600` staged exclusive file, atomic no-clobber publish, removed on session Delete) and pastes
   its shell-escaped host path plus trailing space into the terminal via bracketed paste.
2. No protocol, helm, or supervisor changes are introduced; the existing streaming `Transfers::upload_bytes` /
   `POST /api/sessions/{id}/attachments` path is reused as-is across local, SSH, and sandbox hosts.
3. Behavior is uniform for non-text and empty clipboards: if the clipboard holds file references or an image, upload
   them just like ordinary Paste; if the clipboard is empty, do nothing silently; if the terminal is detached or inert,
   show the existing attachment refusal.
4. Holding the shortcut triggers only one upload (`ONCE_PER_PRESS` suppresses key auto-repeat).
5. On macOS, the **Edit** menu includes **Paste via File Upload** (showing `⇧⌘V`), enabled only when a terminal is
   focused (disabled in focused text fields).
6. The in-app **Keyboard shortcuts** dialog lists **Paste via file upload** in its **Terminal** group (with `⌘+Shift+V`
   on macOS and `Ctrl+Alt+V` on Linux), and the website docs
   (`website/src/content/docs/docs/using/keyboard-shortcuts.mdx` and `work-in-a-session.mdx`) document both platform
   chords.
7. `SPEC.md` is updated for the new paste-via-file-upload shortcut and narrowly for listing this terminal-focused
   shortcut in the Keyboard shortcuts dialog (without pulling `Paste`, `Copy`, `Select All`, or `Shift+PageUp/PageDown`
   into the dialog).
8. The covered entry in `TODO.md` is removed and a changelog fragment is added per repository rules.

## Requirement sources

### User request and maintainer decisions

- **Original TODO entry:** "Shift-cmd-v to paste via file upload. Shift-cmd-v to 'paste via file upload' so text can be
  pasted but instead of directly pasted, upload a file to a securely allocated temporary file and paste the path to it
  into the terminal." (Clarified in planning: the point of uploading to a file when pasting is to avoid pasting giant
  blocks of text directly into an agent TUI.)
- **Host storage (settled):** Reuse the existing per-session attachments directory (no separate OS `/tmp` allocation, no
  protocol/helm/supervisor changes).
- **Shortcuts (settled):** `Shift-Cmd-V` on macOS, `Ctrl-Alt-V` on Linux (since `Ctrl-Shift-V` is already ordinary Paste
  on Linux).
- **Clipboard contents, encoding, filename & repeat (settled):** Uniform paste-via-upload: verbatim UTF-8 bytes uploaded
  as `pasted-<n>.txt` (sharing the `pasted-<n>` counter); files/images upload normally; empty clipboard is a silent
  no-op; suppress held-key auto-repeat.
- **Discoverability (settled):** macOS Edit menu (enabled only when a terminal is focused) + in-app Keyboard shortcuts
  dialog + website docs.
- **Queue state & review gate (settled):** Planned as `[held]`; per-PR review gate is a single fresh-context Opus 5.5
  agent at high effort with the general review charter.

### Binding repository constraints

- `SPEC.md` ("Terminal experience", "Attachments", "Desktop window chrome") and `SPEC_impl.md` ("Actions and keymap",
  "Bulk data stays separate"). Note `SPEC.md` currently states the Keyboard shortcuts dialog lists app-wide shortcuts;
  update that wording narrowly for this maintainer-requested terminal-focused shortcut.
- `crates/gui/farhelm-gui/src/actions.rs`: `ChordModifiers` intentionally omits `alt` to enforce the `PHYSICAL_KEYS`
  invariant ("Option (Alt) and Fn are never part of one"). Do not add `alt` to `actions::ChordModifiers`.
- `crates/farhelm-input-mapping`: pure port of pinned `grid-input.js` key-encoding policy; do not add GUI-keymap
  carve-outs inside `farhelm-input-mapping`.
- `website/AGENTS.md` governs website documentation edits (unattended runs start no preview server and name changed
  pages in the report).
- `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", `.agents/validation.md`,
  `.agents/test-evidence.md`, and `.agents/test-authoring.md`.
- `plans/AGENTS.md` and `plans/.agents/executing.md` govern claim checks, record pushes, blocking, and reporting.

### Planner-proposed implementation choices (fresh-context reviewed)

- **Single cohesive PR:** Total production diff is ~60–80 lines inside `crates/gui/farhelm-gui` plus docs/spec/tests;
  keeping it in one PR keeps the feature, macOS menu item, shortcuts dialog row, spec updates, docs, changelog fragment,
  and `TODO.md` removal atomic.
- **Reuse `attachments::classify` unchanged:** Do not add a parallel `classify_as_upload` or a `paste_as_file: bool`
  parameter to `attachments::classify`. Keep `classify` returning `Paste::Attach | Paste::Text | Paste::Nothing`, add
  `Offered::Text(String)` sharing the `self.pasted` + `Source::Bytes` path with `Offered::Image` in
  `Attachments::attach`, and share one private `AttachedTerminal::start_paste(as_file: bool, cx)` /
  `pasted(..., as_file)` path on the existing `self.paste` task slot
  (`Paste::Text(text) if as_file => self.offer(vec![Offered::Text(text)], cx)`).
- **Keep `actions::ChordModifiers` untouched:** Handle `alt: bool` privately inside
  `crates/gui/farhelm-gui/src/shortcuts.rs` (formatting `Ctrl+Alt+V` on Linux and `⌘+Shift+V` on macOS) and map it in
  `shortcuts/tests.rs` (asserting `!chord.alt` on `PHYSICAL_KEYS` rows).
- **Do not extend `components::EditCommand`:** `EditCommand` bridges `TextInput`-shared items with `OsAction`. Add a
  plain `MenuItem::action("Paste via File Upload", ...)` directly in `crates/gui/farhelm-gui/src/desktop/menu.rs` after
  `Paste`, and update `desktop/tests.rs` to check that it is bound in `TERMINAL_KEYS` and **disabled** in a focused text
  field.
- **Keep `shortcuts/tests.rs` drift check strict for app-wide rows:** Distinguish terminal-scoped rows in `Runs` (e.g.,
  `Runs::TerminalAction`) so app-wide rows stay checked strictly in `[CONTEXT]` while the new terminal-scoped row checks
  `[CONTEXT, TERMINAL_CONTEXT]` and its `TERMINAL_KEYS` binding on both platforms.
- **Handle the Linux `ctrl-alt-v` test collision cleanly:** `farhelm-input-mapping` maps Linux `ctrl-alt-v` to
  `Decision::Send`, while GPUI's `TERMINAL_CONTEXT` binding and `AttachedTerminal::key` claim it first at runtime.
  Allowlist `(Platform::Linux, "ctrl-alt-v")` in `the_keymap_and_the_input_mapping_agree_on_terminal_keys`
  (`terminal/attached/tests.rs`) with an explanatory comment instead of modifying `farhelm-input-mapping`, and also
  exclude the new action's chords from `app_and_terminal_keys_never_reach_the_program` (alongside `paste_chords()` so
  macOS clipboard contents from earlier in the loop do not trigger an upload).

## Implementation outline

One PR (`plan/shift-cmd-v-paste-upload/01-paste-via-file-upload`):

1. **Action & keymap (`crates/gui/farhelm-gui/src/actions.rs`, `actions_tests.rs`):** Declare the new action (e.g.
   `PasteViaFileUpload`), add it to `ONCE_PER_PRESS`, and bind `cmd-shift-v` (macOS) and `ctrl-alt-v` (Linux) in
   `TERMINAL_CONTEXT` via `TERMINAL_KEYS`.
2. **Attachments & terminal handler (`src/terminal/attachments.rs`, `src/terminal/attached.rs`):** Add
   `Offered::Text(String)` reusing `GENERATED_NAME_PREFIX` (`pasted-<n>.txt`), `self.pasted`, and `Source::Bytes` in
   `Attachments::attach`. Route `Paste` and `PasteViaFileUpload` through a shared `start_paste(as_file: bool, cx)` /
   `pasted(..., as_file)` path on `AttachedTerminal`, keeping `attachments::classify` unchanged.
3. **macOS Edit menu & Keyboard shortcuts dialog (`src/desktop/menu.rs`, `src/desktop/tests.rs`, `src/shortcuts.rs`,
   `src/shortcuts/tests.rs`):** Add plain `MenuItem::action("Paste via File Upload", ...)` after `Paste` in the Edit
   menu (asserting it is disabled in focused text fields and enabled in a focused live terminal). Add the row to
   `shortcuts.rs`'s Terminal section with private `alt` support on `shortcuts::Chord` and a terminal-scoped drift check
   in `shortcuts/tests.rs`.
4. **Tests (`src/terminal/attachments.rs`, `src/terminal/attached/tests.rs`):** Unit-test `Offered::Text` upload naming,
   verbatim UTF-8 bytes, counter sharing with images, uniform file/image/empty behavior, detached/inert refusal,
   `ONCE_PER_PRESS`, and the Linux `ctrl-alt-v` allowlist and `left_out` exclusion in `attached/tests.rs`.
5. **Specs, docs, changelog fragment, TODO removal:** Update `SPEC.md`,
   `website/src/content/docs/docs/using/keyboard-shortcuts.mdx`, and
   `website/src/content/docs/docs/using/work-in-a-session.mdx`; add a changelog fragment in `releasing/changelog.d/`;
   remove the covered entry from `TODO.md`. (`SPEC_impl.md` should need no change unless an implementation note is
   warranted.)

## Scope reassessment

Before implementing a substantial departure from the outline (for example a new supervisor/protocol endpoint, modifying
`farhelm-input-mapping`, widening `actions::ChordModifiers` or `EditCommand`, or pulling other terminal-focus shortcuts
into the shortcuts dialog), or after repeated corrective reviews of the same component, run a fresh-context review at
the executing session's own model and effort with this charter, supplying the original request, decisions, outline,
current diff, and proposed departure:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

Apply the decision boundary in Unattended fallback below. Give any same-model sub-agents bounded units within the
outline; unapproved new mechanisms are outside their task.

## Checkout and resume protocol

1. **Checkout check:** Run inside the checkout the executing session runs in — the nearest Git worktree or jj workspace
   containing the working directory. Confirm that at least one remote targets `scode/farhelm` (comparing host, owner,
   and repo name). If not, stop before the resume protocol, write nothing, and report the mismatch. All project paths in
   this file are relative to that checkout root.
2. **Working log (`agent-resumeable`):** After claiming the plan per `plans/.agents/executing.md`, resolve the plan's
   record directory with `cargo xtask records path plan/shift-cmd-v-paste-upload` (`plans/.agents/outside-files.md`,
   "Files outside the repository"). Its `log.md` is the working log (never create a log beside the checkout). Invoke the
   `agent-resumeable` skill with that `log.md` path: if it already exists, read all of it, cross-check against GitHub
   and VCS state, and run the resume check per `plans/.agents/executing.md`, step 7; otherwise start fresh. Push records
   at the cadence required by `plans/.agents/outside-files.md`, "When records are pushed".

## Execution mode

No-workhorse mode (explicit). Do not send any unit of the executing session's own decomposition to a weaker model or a
lower effort than the executing session's own. Do not invoke, load, or follow `scode-galaxy-brain` or
`scode-model-routing` at any point; if `scode-galaxy-brain` is already active when the run starts, this goal file is the
express instruction to stop it.

Same-model sub-agents at the executing session's own model and effort are allowed at the executor's judgment (using
separate worktrees or jj workspaces if concurrent writers might conflict), while the executing session keeps ownership
of the `jjstack` stack and every PR. Launch any model not available as a native sub-agent type (including the per-PR
reviewer) through `scode-harness-shellout`.

The read-only fresh-context sub-agents required by `plans/AGENTS.md`, "Sub-agents this file requires" (the resume check,
the cold read of a Blocked question, and the cold read of the report) are always required and exempt from any delegation
restriction. The scope reassessment review runs at the executing session's own model and effort.

## Stack and PR discipline

- Base the stack on `main@origin` (or on this plan's own open `plan/shift-cmd-v-paste-upload/*` PRs when resuming, never
  on another plan's PRs), set up per `plans/.agents/executing.md`, "Executing one plan".
- Use `jjstack` with bookmark names `plan/shift-cmd-v-paste-upload/<nn>-<short-name>`.
- Keep every PR a draft; never mark a PR ready and never merge.
- Restructure a PR in place instead of stacking a corrective PR on top (including open PRs built by an earlier run).
- Never edit anything under `plans/` in code PRs.
- Before every push of a plan bookmark, follow `plans/.agents/executing.md`, step 9 (`queue status` claim check, push
  records, push bookmark, log pushed commits).
- Follow `AGENTS.md`, "Conventional Commits", "Changelog fragments", and "Tests and validation" (plus
  `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, `website/AGENTS.md`, and
  `plans/.agents/executing.md`, step 8 for noting any departure from `## Release note` in the changelog fragment).
- Remove the covered `TODO.md` entry in the last code PR.

## Review gate

Before finishing any PR, delegate an independent review of its changes to a **single fresh-context Opus 5.5 agent at
high effort** (explicit demand for this model and effort, launched natively or through `scode-harness-shellout`; do not
hardcode a launch command). Supply the checkout root, commit range or bookmark, a private findings file in the
executor's scratch directory, and this complete charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved; report findings to the named file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix; edit nothing and touch no VCS state.

Address every valid finding and re-verify before finishing the PR.

## Resource watchdog

Right after the resume protocol and before any delegation or heavy build, start the `scode-build-goal` resource watchdog
(resolved via `realpath` from `scripts/watchdog` next to the installed `scode-build-goal` skill's `SKILL.md`; requires
`deno`).

- **Status file:** `farhelm-plan-shift-cmd-v-paste-upload-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`, "Placing the files"), never inside the `plan/<slug>` record directory.
- **Watched paths (`--path`):** the executing checkout root, the home directory, `/tmp`, the parent directory holding
  the status file, the executing session's scratch directory under `/tmp/farhelm-tmp/<id>/`, and any worktree or build
  directory on another filesystem. Interval: 60 seconds, default alert levels (filesystem under 5 GiB free or under 25%
  for filesystems < 20 GiB, or under 10% free while also < 20 GiB; memory/cgroup under 10% available while also < 4
  GiB).
- **Harness mechanics, verification, acting on alerts, and lifecycle:** Follow `scode-build-goal`, "Resource watchdog"
  (Claude Code two-Monitor setup, Codex `skillette-codex-wake-on-exit` setup, or background-sampler + `check` fallback),
  including throwaway `--interval 5` verification before starting the real monitors, stopping any stale prior session's
  monitors on resume, acting immediately on `ALERT`/`WORSE`/`STALE`/`ERROR`, running `watchdog check` before launches
  and at natural boundaries, recording handles/paths in `log.md`, and stopping owned monitors at close
  (`plans/.agents/executing.md`, step 12).

## Decision logging and unattended fallback

- Log consequential design and implementation decisions in `log.md` as they happen with a scannable `DECISION` label and
  the alternatives considered.
- The executing session runs unattended and **never asks the maintainer anything in conversation** — no direct
  questions, no harness question-tool prompt, no approval prompt, no closing "should I...?" or menu of optional next
  steps (`plans/AGENTS.md`, "Unattended means no questions").
- Resolve routine implementation forks within the agreed scope and log them as `DECISION`. Remove unnecessary
  planner-invented machinery when requested behavior and repository constraints still hold.
- When remaining work cannot proceed without a material scope change, weakened guarantee, omitted required behavior, or
  other decision only the maintainer can make, push everything that is consistent and block per
  `plans/.agents/executing.md`, step 10 (`queue block` after a fresh-context cold read). Put things the maintainer
  should know and optional follow-ups into the report rather than stopping or blocking on them.

## Done criterion

The goal is achieved when every PR in the stack exists as a draft PR on GitHub, collectively meets all acceptance
criteria (including removing the covered `TODO.md` entry in the last PR), has passed its Opus 5.5 high review gate, and
satisfies the latest `## Decisions` entry in this plan file (if any). Delivering the report and closing out the claim
happen after the goal is achieved, per `plans/.agents/executing.md`, steps 11–12.

## Release note

**You can now press Shift-Cmd-V to paste clipboard text as a temporary file.** Farhelm uploads the text to the session's
host and inserts the file path into the terminal instead of pasting the text directly.
