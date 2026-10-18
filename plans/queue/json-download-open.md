# Cmd-click opens a user-chosen list of file types, with warnings for risky ones

Written against main at d526038e on 2026-10-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records the maintainer's new decision that replaces a confirmed SPEC.md decision (below):
updating SPEC.md to that decision is part of the goal.

This plan has no dependency on another plan.

## The goal

In the desktop app, Cmd-click on macOS (Ctrl-click on other desktop builds) on a file path in a session's terminal saves
the file into Downloads and then opens the saved copy with its default app, but only for `.md` and `.txt`. The
maintainer wants `.json` to open too, and widened the entry while choosing it: the list of file types becomes a setting,
a free-form comma-separated text field in the settings dialog, defaulting to `md, txt, json`. The helm enforces the
list. Any type the user adds opens; types not known to be safe get a warning beside the field that nobody could miss,
with a specific warning for HTML-like types.

TODO.md entry this plan covers (Near term): "**Open JSON downloads with Cmd-click.** Add `.json` to the desktop app's
allowed extensions for opening a completed file download with Cmd-click (Ctrl-click on non-macOS desktop builds). Keep
the final-extension check case-insensitive. Update SPEC.md and SPEC_impl.md, which currently limit opening to `.md` and
`.txt`."

### Acceptance criteria

- The helm decides, on both the hover's open hint (`can_open` in the stat answer) and the open request itself, from a
  list stored by the helm. With no stored choice, the list is `md, txt, json`, so a `.json` save opens out of the box
  for existing and new installations alike.
- The check stays on the saved file's final extension only, ASCII case-insensitively. The same-type check stays: the
  saved file must have the same final extension as the path the hover disclosed, or it is saved and not opened.
- There is no deny-list: whatever the list names opens.
- The refusal message for a type not on the list names the configured types (today it is the fixed "only .md and .txt
  files can be opened"), and says when opening is turned off entirely because the list is empty.
- The desktop app's settings dialog has the list as a free-form text field with an explicit save. Commas separate
  entries; surrounding spaces, one leading dot per entry and letter case do not matter. A malformed list is refused with
  a visible reason beside the field and changes nothing.
- Beside the field, one warning per listed type that is not on the safe list (`md`, `txt`, `json`, `log`), shown while
  the field holds that type (the draft as typed, and the saved list when the dialog opens). `html`, `htm`, `xhtml` and
  `svg` get the HTML-specific warning; every other unsafe type gets the generic warning. A list of only safe types shows
  no warning at all. The warning is impossible to miss: a red-bordered box on the dim danger fill, a warning icon, a
  bold heading naming the type, a slow glow animation (see the planner proposals for reduced motion).
- The HTML warning states only claims verified against how browsers treat a page opened from a local file (see the
  verification step in the Outline); a DECISION in the working log records the final text and the evidence for each
  claim.
- SPEC.md, SPEC_impl.md and the docs pages `website/src/content/docs/docs/using/work-in-a-session.mdx` and
  `website/src/content/docs/docs/how-it-works/security-model.md` describe the new behavior. The shipped v0.29.0
  `CHANGELOG.md` line is not touched; a `feat` changelog fragment describes the change.
- The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-18):**

- Cmd-click (Ctrl-click on non-macOS desktop builds) on a completed terminal file download in the desktop app opens it
  with the default app when its final extension is in a user-editable list. The list is "Free-form with simple comma
  separation that you can edit at in a text field" in the settings dialog. The default is `md, txt, json`. The final
  extension check stays case-insensitive, and the existing same-type check stays.
- The helm enforces the list, not only the page: any signed-in device of the desktop helm can ask it to open a file, so
  a setting checked only in the page would protect nothing.
- No deny-list: whatever is listed opens.
- Warnings, in the maintainer's words: "Let's make sure that it warns about anything that we don't directly recognize as
  a safe type, as opposed to trying to identify unsafe types. Let's make the warning be such that if you only have
  believed to be safe types, it doesn't say anything, and if you pick any type that isn't believed to be safe, then we
  show a very obvious, maybe red or animated or something, that you can't miss warning. Let's make it so that for
  specific types, especially just html right now, we warn about the cross-site stuff and ports and the local host being
  considered same and cookies and stuff like that. So we can have specific warnings for specific extensions, and let's
  at least do that for html. But if it is any extension that we don't have on the safe list, we give a generic warning
  about how there is no protection about what happens when you open it."
  - Safe list (no warning): `md, txt, json, log`.
  - HTML-specific warning for `html, htm, xhtml, svg`: they open in the browser and can run scripts outside Farhelm's
    link permissions.
  - Every other type not on the safe list: a generic warning that nothing limits what the default app does with the
    file, and that a file an agent wrote could run code.
  - Shown in the settings dialog next to the field, one per offending type. The agreed mockup: a red-bordered box on the
    dim danger fill with a slow glow animation, a warning icon and a bold heading, for example "html: an agent's page
    would run in your browser" or "command: no protection".
  - The HTML warning's claims must hold for a page opened from a local file (`file://`), which browsers treat
    differently from a page served on `127.0.0.1`. The cookie, port and same-site findings of the localhost-tunnel work
    (`plans/reports/tunnel-localhost-links.report.md`, and SPEC.md's tunnel warning paragraph) were about pages served
    on `127.0.0.1`; do not assume they carry over. The maintainer named cross-site behavior, ports, localhost and
    cookies as topics to cover where true. Verify, and log a DECISION on the final text.
- This replaces SPEC.md's "Confirmed 2026-10-14" decision in the security section ("only an explicit Cmd-click … only
  for final .md or .txt extensions … HTML is deliberately excluded …"). The maintainer made the new decision on
  2026-10-18; rewrite that paragraph as a decision confirmed on 2026-10-18 rather than leaving the old one standing.

**Binding repository constraints:**

- SPEC.md: "Terminal experience" (the terminal file-click paragraph that limits opening to .md or .txt), "Session list"
  (the settings dialog and what the shared preference row carries), and "Local authority and trust between hosts" (the
  2026-10-14 paragraph above, and the tunnel warning paragraph after it). SPEC_impl.md's paragraph on the desktop file
  opener ("After publication, one pure final-extension predicate admits only md and txt …", and `can_open` coming "from
  that same predicate").
- The opener runs only after a successful native save and never sees a caller-supplied path; a refused or failed open
  never turns a successful save into a failed download. Keep both.
- Root `AGENTS.md`: "Changelog fragments", "Tests and validation", "The live install is off-limits", "Talking to the
  user". `website/AGENTS.md` and `website/EDITORIAL_RULES.md` (read both in full before editing anything under
  `website/`): docs edits riding along in this plan are incidental, so start no preview server and name the changed
  pages in the report.

**Planner proposals (implementation choices, each with its reason):**

- Storage: a new nullable column in the helm's shared preference row (`preferences` in `helm.db`, with a schema
  migration in `crates/farhelm-helm/src/store.rs`), exposed through `GET`/`PUT /api/preferences` as one string field
  (for example `open_extensions`). Reason: the helm must enforce the list, so it must be stored where the helm reads it,
  and the embedded desktop helm's `helm.db` is that place. The preference row already carries a security-relevant choice
  the helm enforces (`tunnel_consent`), the client already loads the row after authenticating, and adding a column
  follows the established migration pattern (see the `color_terminal_frames` and `remembered_wrapper_prefixes` steps).
  The other storage kinds do not fit: per-device webview storage and the app installation's own state file are not where
  the helm reads its policy, and a new table or route would duplicate what the preference row already provides. NULL
  means "never chosen" and reads as the default list; this is what makes `.json` open out of the box without rewriting
  anyone's row.
- An empty saved list means no type opens (Cmd-click only saves), stored as an empty string distinct from NULL. Reason:
  for a security setting, clearing the field reading as "use the default" would surprise the user who cleared it.
- One shared pure module in `crates/farhelm-proto/src/` (beside `host_appearance.rs`, which plays the same role for host
  appearance) parses and normalizes the list, holds the default, the safe list and the HTML-family list, and classifies
  an extension as safe, HTML-family or other. The helm validates writes and checks extensions with it; the page uses it
  to show warnings for the draft as typed. Reason: one vocabulary, so the warning the page shows and the rule the helm
  enforces cannot drift.
- Normalization: split on commas, trim spaces, drop one leading dot, lowercase ASCII, drop empty entries (a trailing
  comma is harmless), collapse duplicates, keep the user's order. A dot inside an entry is refused: only the final
  extension is ever compared, so `tar.gz` could never match, and a refusal beats a silently dead entry. That is the only
  firm rule. As a default, also allow only ASCII letters and digits in an entry; you may loosen that (for example to "no
  dot, comma, whitespace or path separator, non-empty", which admits `c++`) and log the choice as a DECISION. Cap the
  entry count and entry length at modest values the executor picks and logs (the preference handler already caps
  free-text fields so one request cannot park a large string in the shared row). The helm stores the normalized form
  (`md, txt, json`). A stored value the parser refuses (a downgrade or a hand edit) fails closed: nothing opens.
- The write is an acknowledged save, not the silent best-effort preference queue: a dedicated `PUT` like
  `write_tunnel_consent` in `crates/farhelm-ui/src/api.rs`, with the helm's refusal text shown beside the field. Reason:
  a free-form security setting needs visible validation, and the settings dialog's explicit-save fields (checkout
  folder, post-clone command, ntfy alerts) are the existing shape for that. Update the dialog footer ("Most choices save
  immediately. Ntfy alerts and checkout fields use save.") to include this field.
- The field is shown only in the desktop app (`#[cfg(native_desktop)]`), in the "terminal links" section next to the
  native tunnel switch, following `NativeTunnelSettings` in `crates/farhelm-ui/src/settings.rs`. Reason: only the
  desktop helm has an opener, and browsers already never see the desktop-only controls.
- The helm reads the list on each stat and each open request (`state.store.preferences()`, as the tunnel routes do), so
  a saved change takes effect on the next hover and click with no cache to invalidate. The open request rechecks even
  when the hover said the file could open.
- Glow animation on the box's border or shadow only; the text holds still, and under `prefers-reduced-motion: reduce`
  the animation is off and the static red box remains. Reason: `crates/farhelm-ui/assets/app.css` records the convention
  that "a warning that has to be read should hold still" (the link-mismatch hover), and the maintainer asked for
  animation as one way of being impossible to miss, not for moving text.
- Warning text drafts. The generic warning: nothing limits what the default app does with the file; an agent can write
  such a file, and opening it may run code on this Mac. Farhelm deliberately saves downloads without the macOS
  "downloaded from the internet" mark (SPEC.md, security section), so the Mac will not ask first; include that only if
  the code still saves without a quarantine flag, and phrase it so it is true on Linux too. The HTML warning comes out
  of the verification step.
- The hover in the terminal does not gain its own warning, and saving an unsafe list needs no extra confirmation: the
  maintainer asked for the warning beside the field and nothing more.

## Outline

Two PRs, in this order. The browser verification for the HTML warning is independent of PR 1 and can run in parallel
with it (a same-model sub-agent is a reasonable fit).

**PR 1, `feat`: the helm enforces a stored list of openable types, defaulting to md, txt and json.**

- The shared module in `crates/farhelm-proto/src/` with unit tests: parsing, normalization, refusals, the default, the
  classification.
- `crates/farhelm-helm/src/store.rs`: the schema migration (next free `SCHEMA_VERSION`), the column in fresh creation,
  read and sparse write, and the column added to the migration test's rewind list ("Future columns must leave this
  rewind too"). `crates/farhelm-helm/src/preferences.rs`: validation of the new field, with the module docs updated.
- `crates/farhelm-helm/src/downloads.rs`: `may_open` takes the list; `stat_file` and `download_file` read it; the
  refusal message names the configured types or says opening is off. Rewrite the doc comments on `may_open` and
  `same_type` (today they say the policy excludes executable types and that HTML was dropped), and the module doc.
  Update `only_final_document_extensions_may_open`,
  `native_open_uses_only_published_allowlisted_save_and_preserves_failures` (its refusal assertion matches the old
  message) and `stat_exposes_open_hint_only_with_native_capability`; add cases for `.json` opening by default, a stored
  list widening and narrowing what opens, an empty list, and a stored value the parser refuses.
- The client's `Preferences` mirror in `crates/farhelm-ui/src/api.rs` gains the field in PR 2, where a control first
  reads it; nothing in PR 1 needs it.
- SPEC.md: the terminal file-click paragraph and the security decision paragraph, rewritten for a helm-enforced list
  with the default and the same-type check (the settings field and its warnings come in PR 2); the "Session list"
  paragraph naming what the preference row carries. SPEC_impl.md: the opener paragraph's predicate and `can_open`
  sentences, and the column.
- A changelog fragment under `releasing/changelog.d/` (`kind: added`), written for someone running Farhelm, per
  `releasing/EDITORIAL_GUIDANCE.md`.

**HTML warning verification (before PR 2's warning text).**

Establish how a page opened from a local file behaves, only as far as a two- or three-sentence warning can use: time
spent here is capped by what the warning text will say. Run one small page in Chromium and WebKit through the Playwright
install the repository's `e2e/` directory already uses (read `.agents/machine.md` before installing anything or creating
files outside the checkout). This is a throwaway experiment in your scratch directory, not a repository test, so it is
neither committed nor recorded through `scripts/record-test-run.py`. Bind any server it starts to `127.0.0.1` on a port
the kernel chooses. The questions, each answered yes or no with evidence:

- Does the page's script run, and can it navigate the browser, open windows, and send requests to arbitrary addresses,
  including `127.0.0.1` on any port and local-network addresses, without a prompt?
- Do requests the page starts to a service on `127.0.0.1` carry that service's cookies? One `SameSite=Lax` and one
  `SameSite=None` cookie, tried with a top-level navigation and a form POST, are enough.
- Do Chromium's local-network (private-network) access restrictions apply to a page opened from a local file?

Answer definitional questions from documentation rather than the experiment: a page opened from a local file has an
opaque origin and no site, so it cannot read another origin's cookies through `document.cookie` and is not same-site
with anything on `127.0.0.1`. Whether the page can read a sibling file in the same folder (Downloads holds other
downloads) is worth adding only if it costs one more line on the same page; it is a planner addition and must not grow
the step.

Back each answer with the experiment and, where it exists, authoritative documentation (MDN, the Chromium and WebKit
projects, the relevant specs). A Linux headless Chromium or Playwright WebKit is not the user's Safari or Chrome on
macOS: say in the DECISION and the report which claims rest on documentation alone. Then write the HTML warning from the
claims that hold, covering the maintainer's topics (cross-site, ports, localhost, cookies) only where they hold for a
local file, and log a DECISION with the final text, the claims kept, the claims dropped and why. If the evidence is
inconclusive, use only the claim SPEC.md already establishes and that is easy to confirm: an opened page runs scripts in
the browser and can open web addresses outside Farhelm's link permissions, without the localhost-tunnel warning.

**PR 2, `feat`: the settings field and its warnings.**

- `crates/farhelm-ui/src/settings.rs` (and a small module of its own if the field outgrows a component there): the
  desktop-only field, label and help in the dialog's existing tone, the acknowledged save, the inline refusal, the live
  warnings from the shared classifier, one per offending type, and the footer text. The platform modifier word in the
  label and help (Cmd on macOS, Ctrl elsewhere) follows the existing Rust precedent,
  `cfg!(all(native_desktop, target_os = "macos"))` in `crates/farhelm-ui/src/window_chrome.rs`.
- `crates/farhelm-ui/assets/app.css`: the warning box from the app's danger tokens (`--danger-strong`, `--danger-bg-dim`
  and their neighbours), the glow, and the reduced-motion rule. The warning icon is the text "⚠" the app already uses
  for warnings (`crates/farhelm-ui/src/app_updater.rs`, around line 331); no `icons.rs` change.
- Tests: pure tests of the warning list a draft produces (safe-only lists produce none; each unsafe type produces one,
  HTML-family ones the specific warning). They cover the logic the maintainer cares about. A render test under the
  desktop feature is optional: add one only if it fits the pattern in `crates/farhelm-ui/src/app_updater.rs` (around
  line 577, `VirtualDom::new_with_props` and `rebuild_to_vec`, then inspecting the rendered nodes) cheaply.
- SPEC.md: "Session list" gains the field (desktop app only, explicit save, empty means nothing opens, the safe list and
  the two kinds of warning). Say that it is explicitly saved but kept in the shared preference row, like tunnel consent;
  do not put it in the sentence about the ntfy alerts and checkout fields, which SPEC.md describes as explicitly saved
  helm configuration outside the preference row. The security paragraph gains the warning behavior and the HTML
  warning's verified claims. SPEC_impl.md if the field's storage or save path needs a sentence there.
- Docs: `website/src/content/docs/docs/using/work-in-a-session.mdx` (the Cmd-click paragraph: the default types, the
  setting, where it lives) and `website/src/content/docs/docs/how-it-works/security-model.md` (the Cmd-click sentence,
  and that adding types other than the safe ones removes Farhelm's protection, as the warnings say). Check whether the
  settings dialog's description in `website/src/content/docs/docs/using/session-list.mdx` ("Terminal links") or
  `website/src/content/docs/docs/using/manage-hosts.mdx` needs a sentence. Read `website/AGENTS.md` and
  `website/EDITORIAL_RULES.md` in full first; no preview server.
- Edit PR 1's changelog fragment to cover the setting and the warnings rather than adding a second fragment.
- Remove the TODO.md entry.

### Validation

Choose per `.agents/validation.md` and `.agents/test-evidence.md`; at least:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo check -p farhelm-desktop`.
- Through the recorder: the new `farhelm-proto` module's tests, `farhelm-helm`'s `downloads`, `preferences` and `store`
  tests (the store's migration and fresh-schema comparison tests matter most for the new column), and in PR 2 the
  `farhelm-ui` desktop-feature tests for the field (`cargo nextest run -p farhelm-ui --features desktop` with a filter).
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`,
  `python3 releasing/check-changelog.py format`, `dprint check`, and for PR 2's docs edits
  `cd website && bun install --frozen-lockfile && bun run build`.
- No browser end-to-end tests: the web UI does not render the field, and `e2e/tests/terminal-link-clicks.spec.ts`
  answers stat requests with a fixed `can_open`, so it covers nothing this plan changes. Run it only if you end up
  changing `crates/farhelm-ui/assets/terminal-files.js`.
- Do not launch the desktop app to look at the field: its default state may be the maintainer's live installation (root
  `AGENTS.md`, "The live install is off-limits"). The report says the field and a real Cmd-click were not seen running,
  and that the change adds a `helm.db` schema migration, so trying it on the Mac (a devbuild) migrates that helm's
  database.

## Fallbacks

- HTML warning claims that cannot be verified are left out, down to the minimum in the verification step. Name the
  dropped topics in the report so the maintainer sees which of their topics the warning does not cover and why.
- Another change landing a `helm.db` schema version first: take the next free version during the careful rebase and keep
  the migration's column-presence guard, as the existing steps do.
- Without a render test, the pure tests of the warning list carry the coverage; say in the report that the rendered
  field was not tested.

Examples of departures that need the scope reassessment: a deny-list or any type refused regardless of the list,
per-device or per-installation storage, a new table or route for the setting, a confirmation step when saving an unsafe
list, warnings in the terminal hover, showing the field in browsers or on server helms, caching the list in the helm.

Decisions to log: the column and field name, the characters an entry may hold and the normalization caps, the empty and
unparseable cases, the module's home, the field's placement and label, the final HTML and generic warning text with the
verification evidence, the glow and reduced-motion treatment, whether a render test was added.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-json-download-open-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/json-download-open/<nn>-<short-name>` (for example `plan/json-download-open/01-helm-open-types` and
  `plan/json-download-open/02-open-types-setting`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the outline;
- for PR 2, the verification DECISION with its evidence, so the reviewer can check every claim the HTML warning makes.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
