# Plan: Shift-Cmd-click a file to copy its contents to the clipboard

Slug: `shift-cmd-click-copy-file`

## Goal

Add **Shift-Cmd-click** on macOS (**Shift-Ctrl-click** on Linux) on hovered terminal file paths to stream the file's
contents directly into memory (up to 10 MB, without touching Downloads) and copy them onto the clipboard: valid UTF-8
text on macOS and Linux, plus an explicit allowlist of GPUI-supported raster image formats on macOS, refusing binary or
unsupported formats loudly in the terminal's top status line.

### Acceptance criteria

1. **PR 1 (`farhelm-helm-client`):**
   - `Transfers::download_bytes(&self, session: &str, path: &str, max_bytes: usize) -> Transfer<Vec<u8>>` posts to the
     existing `POST /api/sessions/{id}/files/download` route with `save_to_downloads: false, open: false`, enforces
     `max_bytes` while reading the stream without buffering past the cap, and returns a dedicated
     `TransferError::TooLarge { max_bytes }` when exceeded (never misreporting it as `TransferError::BadReply`).
   - Reuses factored HTTP/status/capped-read helpers in `crates/farhelm-helm-client/src/transfers.rs` (handling helm's
     plain-text `http_error` bodies and 401 `CREDENTIAL_REFUSED`) rather than duplicating request/status code, and adds
     `FakeTransfer::DownloadBytes` plus unit and real-helm tests.
   - Zero helm, supervisor, wire-protocol, or vendored GPUI changes.
2. **PR 2 (`farhelm-gui`, specs, docs, changelog, `TODO.md`):**
   - File hover disclosure card line 3 appends ` · ⇧⌘-click to copy contents` on macOS
     (` · Shift-Ctrl-click to copy contents` on Linux) for eligible files: readable regular files with
     `size <= 10_000_000` (10 MB decimal), and on Linux omitting the hint for extensions in the raster-image allowlist
     (`png`, `jpg`, `jpeg`, `webp`, `gif`, `bmp`, `tif`, `tiff`, `ico`).
   - Exact 3-way modifier handling (`Download` / `Open` / `Copy`): plain click downloads, Cmd/Ctrl-click without Shift
     downloads and opens allowed types, Shift-Cmd-click (macOS) / Shift-Ctrl-click (Linux) copies contents.
   - Shift-Cmd-click / Shift-Ctrl-click on a ready or held-back file link clears any existing terminal selection (via
     `view.set_selection(None, cx)`) and does not start or extend a `SelectGesture`, so an existing selection never
     swallows the click; uses `self.platform` (not `THIS_PLATFORM`) so cross-platform GUI tests stay deterministic.
   - Clicking an oversized (`> 10_000_000` bytes) or Linux raster-image file fails fast before transferring; otherwise
     streams bytes through `Transfers::download_bytes(..., 10_000_000)` on the existing `FileLinks` task map, status
     line, and 5-second fade timer (preserving `!hover.staged`).
   - Pure format classifier in `farhelm-gui` using `gpui::ImageFormat` directly (no duplicate enum, no new crate
     dependencies):
     - Explicit raster-image extension allowlist (`png`, `jpg`, `jpeg`, `webp`, `gif`, `bmp`, `tif`, `tiff`, `ico`)
       verified by magic bytes on macOS (`ClipboardItem::new_image`), refusing loudly on magic mismatch (never falling
       back to text) and refusing loudly on Linux (plain text only for now).
     - Otherwise valid UTF-8 text without NUL bytes (`std::str::from_utf8(&bytes).is_ok() && !bytes.contains(&0)`,
       including `.svg`, with no C0 control-char rejection so `.log`/ANSI files still copy).
     - Everything else (binary files, `.avif`, archives, etc.) refuses loudly with a clear status message.
   - Top-of-terminal status line: `Copying…` -> `Copied to clipboard` (fades after 5s) / `Copy failed: <reason>` (stays
     until next press, with empty-body refusal fallback `"Copy refused"` rather than `"Download refused"`, and host/helm
     error strings routed through `HostText::from_host`).
   - Covered or not-visible disclosure refusals say `cannot copy: …` for copy clicks in both `files.clicked` and
     `attached.rs`'s `press.file_held_back` branch.
   - Updates `SPEC.md` ("Terminal experience", "Local authority and trust between hosts"), `SPEC_impl.md` ("Session-host
     file reads"), `website/src/content/docs/docs/using/work-in-a-session.mdx` (leaving the existing download-hover
     screenshot untouched), `terminal_file_disclosures` golden (`files::golden_displays`), `files/tests.rs` hint
     assertions, changelog fragment, and removes the covered entry from `TODO.md`.

## Requirement sources

### User request and maintainer decisions

- **Original TODO entry:** "Shift-cmd-click a file to copy it to the clipboard. Add a 'click to copy into clipboard' for
  files on hover. So today we have click to download, cmd-click to open; add a shift-cmd-click to copy to clipboard."
- **Clipboard semantics (settled):** File contents only — UTF-8 text and explicit GPUI-supported raster images on macOS,
  UTF-8 text only on Linux for now, no Downloads file touched, no Finder/Nautilus file references, no vendored GPUI
  patches. Maintainer's exact words: _"Yes, everything about the intent of these to-do items was about the content. ...
  Going the other direction, it was all about getting the contents of the file in all cases and make sure we're only
  doing files, text files, images, and that there's no weird random other thing going on here. Only a clear, loud list
  of specific formats that make sense."_
- **Size limit (settled):** Cap at 10 MB (`10_000_000` decimal bytes).
- **Linux shortcut & parity (settled):** `Shift-Ctrl-click` on Linux, plain text only for now (no vendored Linux GPUI
  patch).
- **Hover hint & status wording (settled):** Show hint on eligible files; maintainer's exact instruction: _"hint should
  say 'copy contents' not just 'copy'"_ (`· ⇧⌘-click to copy contents` / `· Shift-Ctrl-click to copy contents`). Status:
  `Copying…` -> `Copied to clipboard` (fades after 5s) / `Copy failed: <reason>`; covered-card refusal:
  `cannot copy: …`.
- **Queue state & review gate (settled):** Planned as `[held]`; per-PR review gate is a single fresh-context Opus 5.5
  agent at high effort with the general review charter.

### Binding repository constraints

- `SPEC.md` ("Terminal experience" file-path bullet & clipboard best-effort rule, "Local authority and trust between
  hosts") and `SPEC_impl.md` ("Session-host file reads"). Note `Copy failed: <reason>` reports Farhelm-side failures
  before calling GPUI's `write_to_clipboard` (which returns `()` and remains best-effort per `SPEC.md`, including
  Wayland unfocused-window drops; do not patch GPUI).
- `scripts/check-crate-layering.py`: `farhelm-helm-client` does not depend on `gpui`.
- `.agents/docs-media.md`: do not regenerate or edit the `work-in-a-session/download-hover` docs screenshot.
- `website/AGENTS.md` governs website documentation edits (unattended runs start no preview server and name changed
  pages in the report).
- `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", `.agents/validation.md`,
  `.agents/test-evidence.md`, and `.agents/test-authoring.md`.
- `plans/AGENTS.md` and `plans/.agents/executing.md` govern claim checks, record pushes, blocking, and reporting.

### Planner-proposed implementation choices (fresh-context reviewed)

- **Clean two-PR split:** PR 1 adds bounded `Transfers::download_bytes` and `FakeTransfer::DownloadBytes` strictly in
  `crates/farhelm-helm-client`; PR 2 puts the format classifier directly in `crates/gui/farhelm-gui` (using
  `gpui::ImageFormat` with zero duplicate enums and ~20 lines of pure magic-byte prefix checks — no new `image`/`infer`
  production dependencies) along with GUI wiring, golden/tests, specs, docs, changelog fragment, and `TODO.md` removal.
- **Factor `transfers.rs` helpers cleanly with a distinct `TransferError::TooLarge`:** Share `post_file_request`,
  `check_status`, and cap-parameterized `read_capped` across `Stat`/`Download`/`DownloadBytes`, while keeping
  `TransferError::TooLarge { max_bytes }` distinct from `TransferError::BadReply` so oversized files report a clear size
  cap message instead of a corrupt-reply error.
- **Reuse `FileLinks` transfer/status machinery:** Keep one `downloads: HashMap<u64, Task<()>>`, status line, `Leaves`,
  and 5s fade timer for download, open, and copy; parameterize the empty-body refusal fallback (`"Download refused"` vs
  `"Copy refused"`).
- **Exact 3-way `ClickAction` and both `Hidden` refusal sites:** Replace `download::opening` with
  `ClickAction::{Download, Open, Copy}` checking exact modifiers, and route both `attached.rs`'s `press.file_held_back`
  branch and `files.clicked` through `"cannot copy: ..."` (`Hidden::copy_words`) for copy clicks.
- **Clear existing selection on copy-click press:** In `attached.rs::press`, when a copy-gesture click lands on a ready
  or held-back file link, call `view.set_selection(None, cx)` and return without starting `SelectGesture`, so
  `link_released`'s `selected_text().is_none()` guard does not silently swallow the click.
- **Hover hint gating without a binary-extension blocklist:** Gate the copy hint on `size <= 10_000_000` (and omit on
  Linux for the already-defined raster image extensions). Do not add a separate general binary-extension blocklist
  (`.pdf`, `.zip`, etc.), which would duplicate `file_icon`'s table and always be incomplete; binary files under 10 MB
  refuse loudly after click-time byte inspection. Do not add a `Content-Disposition` / `same_type` check (bytes are
  verified directly). Keep `from_utf8` + `!contains(&0)` without special-casing uncompressed ASCII `.pdf` or 0-byte
  files (log a brief note/test on Wayland's 0-byte no-op behavior).
- **Docs scope:** Update only `website/src/content/docs/docs/using/work-in-a-session.mdx` (`keyboard-shortcuts.mdx` does
  not list file mouse clicks).

## Implementation outline

Two stacked PRs:

1. **PR 1 (`plan/shift-cmd-click-copy-file/01-helm-client-download-bytes`):**
   - In `crates/farhelm-helm-client/src/transfers.rs`, factor `post_file_request`, `check_status`, and parameterized
     `read_capped`; add `TransferError::TooLarge { max_bytes: usize }` (handled in `TransferError::upload_outcome` and
     Display); add `Transfers::download_bytes(&self, session: &str, path: &str, max_bytes: usize) -> Transfer<Vec<u8>>`
     posting `save_to_downloads: false, open: false`.
   - Add `FakeTransfer::DownloadBytes` in `crates/farhelm-helm-client/src/fake/transfers.rs` (and wire any test helpers)
     plus unit and real-helm tests for success, exact cap boundary, oversized stream refusal, empty-body/plain-text
     refusal, and 401 handling.
2. **PR 2 (`plan/shift-cmd-click-copy-file/02-gui-shift-cmd-click-copy-contents`):**
   - Replace `download::opening` with exact 3-way `ClickAction` (`Download` / `Open` / `Copy`) in
     `crates/gui/farhelm-gui/src/terminal/files/download.rs`.
   - Add the pure copy-contents format classifier in `crates/gui/farhelm-gui/src/terminal/files/` using
     `gpui::ImageFormat` and ~20 lines of magic-byte checks (macOS: 7 raster formats verified by magic bytes, or valid
     UTF-8 without NUL including `.svg`; Linux: valid UTF-8 without NUL only; no fallback from image extension to text).
   - Update `crates/gui/farhelm-gui/src/terminal/files.rs` and `disclosure.rs` for hover hint
     (`· ⇧⌘-click to copy contents` / `· Shift-Ctrl-click to copy contents`), fast-fail on `> 10_000_000` bytes or Linux
     raster extension, status messages (`Copying…`, `Copied to clipboard`, `Copy failed: <reason>`, `"Copy refused"`
     fallback, `Hidden::copy_words` `"cannot copy: ..."`), and `cx.write_to_clipboard`.
   - Update `crates/gui/farhelm-gui/src/terminal/attached.rs` to clear any existing selection
     (`set_selection(None, cx)`) and skip `SelectGesture` on copy-gesture presses over ready/held-back file links (using
     `self.platform`), and pass copy intent into the `press.file_held_back` refusal path.
   - Update `files::golden_displays` (`terminal_file_disclosures` golden), `files/tests.rs`, `attached/tests.rs`,
     `SPEC.md`, `SPEC_impl.md`, `website/src/content/docs/docs/using/work-in-a-session.mdx`, add changelog fragment in
     `releasing/changelog.d/`, and remove the covered entry from `TODO.md`.

## Scope reassessment

Before implementing a substantial departure from the outline (for example touching helm/supervisor/protocol, patching
vendored GPUI, adding an external image-decoding crate, adding a binary-extension blocklist, or writing files to
Downloads/temp), or after repeated corrective reviews of the same component, run a fresh-context review at the executing
session's own model and effort with this charter, supplying the original request, decisions, outline, current diff, and
proposed departure:

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
   record directory with `cargo xtask records path plan/shift-cmd-click-copy-file` (`plans/.agents/outside-files.md`,
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

- Base the stack on `main@origin` (or on this plan's own open `plan/shift-cmd-click-copy-file/*` PRs when resuming,
  never on another plan's PRs), set up per `plans/.agents/executing.md`, "Executing one plan".
- Use `jjstack` with bookmark names `plan/shift-cmd-click-copy-file/<nn>-<short-name>`.
- Keep every PR a draft; never mark a PR ready and never merge.
- Restructure a PR in place instead of stacking a corrective PR on top (including open PRs built by an earlier run).
- Never edit anything under `plans/` in code PRs.
- Before every push of a plan bookmark, follow `plans/.agents/executing.md`, step 9 (`queue status` claim check, push
  records, push bookmark, log pushed commits).
- Follow `AGENTS.md`, "Conventional Commits", "Changelog fragments", and "Tests and validation" (plus
  `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, `website/AGENTS.md`, and
  `plans/.agents/executing.md`, step 8 for noting any departure from `## Release note` in the changelog fragment).
- Remove the covered `TODO.md` entry in the last code PR (PR 2).

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

- **Status file:** `farhelm-plan-shift-cmd-click-copy-file-watchdog.json` in the parent directory of the executing
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
criteria (including removing the covered `TODO.md` entry in PR 2), has passed its Opus 5.5 high review gate, and
satisfies the latest `## Decisions` entry in this plan file (if any). Delivering the report and closing out the claim
happen after the goal is achieved, per `plans/.agents/executing.md`, steps 11–12.

## Release note

You can now Shift-Cmd-click a file path in the terminal to copy its contents to your clipboard. Plain click still
downloads the file and Cmd-click still opens allowed types.
