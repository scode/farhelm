# Managed checkouts: a visible type, a branch glyph, and a trash for archived checkouts

Written against main at de457dc7 on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan. It covers two TODO.md entries, merged into one plan with the maintainer's
agreement because they change the same surfaces.

## The goal

Make sessions that live in a checkout Farhelm made for them understandable. Today a `gh:` launch clones into a new
folder named after the session, and deleting the last session using it silently moves the folder into
`farhelm-archived-working-copies` under the working-copy root. The user is never told where it went, and the archive is
never cleaned. After this plan:

- Such a checkout is called a **managed checkout** everywhere the user reads about it (D2).
- The session list and the session view mark sessions in a managed checkout with a **branch glyph** and show the
  repository where other sessions show a folder (D3).
- The launcher and the template editor offer a visible choice of destination type, **folder** or **managed checkout**,
  in the same segmented control, with an explanation of what a managed checkout does (D4).
- Archiving stays, and becomes visible as a **trash** beside New: a count when it holds anything, a dialog listing the
  archived checkouts per host with sizes and where they are on disk, and permanent deletion per host or everywhere (D1,
  D5, D6).
- A Delete that archived a checkout plays a short cue: the row flies into the trash, which wiggles (D7).

The maintainer reviewed a live mockup of all of this; the Appendix describes it precisely enough to build from.

Acceptance criteria:

**Naming and explanation (D2, D4).**

- Every user-facing string that names this kind of checkout calls it a "managed checkout": the launcher, the template
  editor, Clone's description, Delete's confirmation, the approval card text that mentions "fresh checkout of"
  (`approvals.rs`), helm- or supervisor-produced messages that reach the user (such as the teardown notice), SPEC.md,
  and the docs website. Internal names (Rust types such as `TemplateDestination::Github`, `WorkingCopyInfo`, the
  protocol, the `gh:` search label) stay as they are; stored template JSON does not change.
- The launcher's destination section (`crates/farhelm-ui/src/list/create_form.rs`) starts with a segmented control,
  using the launcher's existing `.launch-composer-segmented` style, with two segments: "folder" (a folder glyph) and
  "managed checkout" (the branch glyph). Folder shows today's folder controls. Managed checkout shows a repository field
  (with today's repository suggestions) and the checkout preview, which names the new folder's path and explains, in the
  Appendix's words or close to them, that Farhelm clones the repository into a new folder named after the session and
  that the folder moves to the trash when the last session using it is deleted. Selecting a `gh:` search result still
  works and simply selects the managed checkout type with that repository. "use existing folder" becomes selecting the
  folder segment. Every existing launcher behavior around checkouts (preview retry, the root-missing message,
  path-too-long refusal, Clone's prefill) keeps working.
- The template editor's destination field (`crates/farhelm-ui/src/list/templates.rs`) uses the same segmented control,
  names, glyphs and explanation instead of today's "folder" / "fresh GitHub checkout" pill buttons.

**Session list (D3).**

- A session whose current working copy is a managed checkout (`SessionInfo.working_copy` is set, which includes sessions
  that reuse another session's checkout) shows the branch glyph followed by the repository (`owner/name`) on the row's
  meta line, in place of the folder path. In compact rows, which have no meta line, the glyph follows the session name.
  Hovering the glyph shows a tooltip naming it a managed checkout of the repository, its folder path, and that the
  folder moves to the trash when its last session is deleted. The session view's header shows the same mark beside its
  folder. Sessions in ordinary folders are unchanged.
- The glyph is drawn the way `crates/farhelm-ui/src/icons.rs` draws its glyphs (12-unit view box, `currentColor`,
  `aria-hidden`, paired with a visually hidden word), from the Appendix's path.

**Trash (D1, D5, D6).**

- A trash button sits immediately left of New in the session list header (`list/view.rs`), exactly as tall as New.
  Empty, it is the outline can in the faint foreground. Holding anything, it is the "lid pops open" can in the primary
  foreground with the count beside it. Its tooltip explains the trash in both states (Appendix). The header must still
  fit at the sidebar's minimum width; tighten the header's spacing or controls if needed rather than letting it clip.
- The count is the number of archived checkouts on reachable hosts whose folders still exist. The client refetches each
  host's list (without sizes) on load, after its own Delete that archived, after emptying, when the dialog opens, and
  when a session in a managed checkout drops out of the session list (which also covers Deletes made in other clients).
  No new poll. Accepted gap: when another client empties the trash, this client's count stays high until its dialog
  opens (planner proposal; see Outline).
- Clicking the trash opens a modal dialog (the app's existing modal pattern and isolation, `modal_isolation.rs`) laid
  out as in the Appendix: an explanation; one boxed group per host that has archived checkouts, each with the host's
  name, count, total size and a "delete all on <host>" button, then its checkouts newest first (folder name, repository
  with the branch glyph, size, when archived), then where they are on that host's disk
  (`<root>/farhelm-archived-working-copies/`); and a footer with the total for reachable hosts and a global "delete
  all". When empty, the dialog says so and explains what lands there.
- A host that is not connected is shown greyed out with a note that it cannot be reached, its button disabled; the
  global delete skips it. Its archived checkouts cannot be listed while it is unreachable, because the records live on
  that host (planner proposal; the mockup showed them listed, which would need the helm to cache listings).
- Sizes are measured when the dialog opens, per host, showing "measuring…" until a host answers. Measurement is disk
  usage, does not follow symlinks, and is bounded in time; a checkout whose size could not be measured in time shows no
  size rather than a guess, and the host total says it is partial.
- Every delete asks once, in place, stating how many checkouts on which hosts and their size, and that work in them that
  was never committed or pushed is lost permanently. Then it deletes exactly the archived folders Farhelm recorded and
  nothing else (D6): for each recorded archive, the supervisor resolves
  `<recorded root>/farhelm-archived-working-copies/
  <recorded destination name>`, verifies the root and the directory
  against the recorded identity with the existing `same_directory`/`verified_root` rules, and only then removes it
  recursively without following symlinks. A directory that is missing or fails the identity check is not touched, and
  its record is dropped from the trash; a mismatch is reported in the dialog, naming the folder left in place, so the
  trash can still empty. An archive that contains the working folder of a live session (a user who started a session
  inside the archive to recover something) is refused and reported rather than deleted. Records whose folder is already
  missing when the list is read are dropped and never counted. Other contents of the archive folder are never touched.
  Failures are reported per checkout in the dialog, never silently. The working-copies module's own rule ("NO recursive
  removal anywhere in this module") stays true: put the deletion where that rule does not apply, or update its
  documentation to name this one exception.
- After a Delete that archived a checkout, in the client that issued it, the deleted row's outline flies into the trash
  and the trash wiggles once, about a second in all (Appendix, D7). Nothing plays for a Delete that archived nothing,
  and nothing plays in other clients. The Delete reply says whether the checkout was archived (an optional field on the
  supervisor's delete reply and the helm's JSON, alongside today's `notice`). The cue respects `prefers-reduced-motion`
  by skipping the flight and the wiggle.
- Delete's confirmation sentence for a session in a managed checkout says the checkout moves to the trash when its last
  session is deleted, replacing "moves it into the working-copy archive; no files are deleted".

**Specs, docs and tests.**

- SPEC.md's "Fresh GitHub checkouts" section and every reference to it use the managed-checkout name and describe the
  trash, the type choice in the launcher and templates, and the session list mark. Its rule that archiving is "a
  no-overwrite move, never recursive deletion" stays true for archiving; add the trash's permanent delete as the one,
  explicitly user-confirmed exception, limited to recorded archives that pass the identity check. SPEC_impl.md records
  the protocol additions, the identity verification, the size measurement and its bound, how the count stays current,
  and how "archived at" is known. SPEC.md's template description ("a folder or a fresh GitHub checkout") follows the
  rename.
- The docs website explains managed checkouts and the trash: the launcher's section on fresh GitHub checkouts in
  `website/src/content/docs/docs/using/start-a-session.mdx`, Delete in `stop-restart-resume.mdx`, and the session list's
  marks and header in `session-list.mdx`. `docs/github-checkouts.md` follows the rename, describes the trash, and drops
  its stale claim that a failed archive move asks the user to retry Delete (SPEC.md now says archiving never blocks
  Delete).
- Tests per Validation. Supervisor unit tests cover listing (recorded archives only; missing and identity-mismatched
  directories), deletion (only verified recorded archives; symlinks inside are not followed; unrecorded neighbours in
  the archive folder survive), size measurement and its bound, and the delete reply's archived field. Browser tests
  cover the type choice in the launcher and the template editor, the branch glyph and its tooltip, the trash count and
  dialog, per-host and global delete with their confirmation, an unreachable host, and that the cue plays only for a
  Delete that archived. Existing specs that match the old strings (`e2e/tests/github-checkouts.spec.ts`,
  `github-checkout-composer.spec.ts`, and any other the rename touches) are updated.
- The last code PR removes both TODO.md entries this plan covers: "Make checkout archiving understandable, or drop it."
  and "Make `gh:` launches less magical." (Near term).
- A linear stack of draft PRs, each having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's requests (TODO.md, Near term, the maintainer's words):**

- "Make checkout archiving understandable, or drop it. Deleting the last session using a fresh GitHub checkout moves the
  checkout into `farhelm-archived-working-copies` under the working-copy root. The only explanation is one line in
  Delete's confirmation ("moves it into the working-copy archive"), which does not say where that is or what it is for,
  and the docs site does not mention it. Decide whether archiving should stay at all; if it does, work out how a user
  learns that it happens, where the archived checkout went, and that cleaning the archive up is theirs to do."
- "Make `gh:` launches less magical. In the maintainer's words, "the gh: stuff is kinda magical right now". A session in
  a fresh GitHub checkout behaves differently from one in an ordinary folder: a `gh:` launch clones into a new directory
  under the working-copy root and names it after the session, deleting the last session using it archives the checkout,
  and Clone is planned to make another checkout. Consider how the launcher and the session list could better surface
  what is going on, such as that a session lives in a checkout and of which repository, and what a `gh:` launch or an
  action on such a session is about to do. Related to the checkout archiving entry above." (Clone into a new checkout
  has since landed, in #1684.)

**The user's decisions (2026-10-09), reached over four rounds of a live mockup:**

- D1. Keep archiving, and make it visible as a trash: "a 'trash can' sitting to the left of the 'new' button in the side
  bar. it goes 'full looking' (and different color) when there is something to delete. on hover it explains it. when you
  click, you get a list of archived sessions in 'most recently archived first' and you can 'empty trash'. no click flow
  to restore or anything, just telling people hey there are archived, here are their names, when they were archived and
  you can either do nothing or empty trash. it should tell the user where these are on disk if they need to recover
  something."
- D2. The name is "managed checkout", a general term rather than a GitHub-specific one.
- D3. Mark sessions in a managed checkout in the session list with a general indicator, not a GitHub-specific one: the
  branch glyph.
- D4. The launcher should show the type the way the template editor does, consistently: "currently it has a 'fresh
  checkout' and 'folder'. we should probably have something like 'managed checkout' or similar instead in that dialog.
  we should make it feel consistent." Settled on the launcher's segmented control in both dialogs, with the branch glyph
  for managed checkout.
- D5. Trash design: the "lid pops open" shape when full, bright (primary foreground) with a count, as tall as the New
  button. The list is a separate modal dialog, "an actual visual group per host with a list of sessions under it, and a
  'delete all' button for the whole host, and a global 'delete all' across all hosts". Sizes are shown. A host that is
  not reachable is "greyed out or similar indicating it's not reachable".
- D6. Deleting covers "what we archived and we should only delete precisely that".
- D7. After a Delete that archived, the cue is the row flying into the trash.
- D8. Review gate: a fresh-context Opus 5.5 agent at high effort with the general charter, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

The maintainer approved the live mockup at `https://snippets.scode.org/s/farhelm-checkout-trash/` (round 4); fetch it
and look at it before building the UI, and compare your result with it. Its design is also carried by the Appendix;
where the mockup and this file differ, this file wins.

**Planner proposals** (challenge them through Scope reassessment rather than treating them as requirements):

- "Archived at" is derived from the recorded destination name, which the supervisor writes as
  `<original basename>-<YYYYMMDDTHHMMSSZ>[-<hex>]`, rather than a schema migration adding a column. The store already
  keeps `original_basename` beside `archive_destination`, so the timestamp parses unambiguously; a name that does not
  parse shows no time.
- The trash's records are the store's `retired` working-copy rows that carry an `archive_destination`
  (`working_copies.rs` `AllocationState`, `store.rs`). Deleting an archive drops its row; no new "purged" state.
- Exactly two new supervisor requests, one to list a host's archives (with an optional flag to measure sizes) and one to
  delete a given set of them, following the browse-directory chain end to end (UI `api.rs` → helm route and handler →
  helm client → `farhelm-proto` request/reply → supervisor handler and a blocking worker in `service/core.rs`). New
  variants earn the next `PROTOCOL_VERSION` per its doc comment. The UI calls the host-scoped endpoint once per host, so
  a slow host does not hold up the others.
- The count comes from the list request itself, refetched at the moments the acceptance criteria name, rather than
  riding on the session listing: the helm serves the session list from its own cache, so carrying a count there would
  cross every layer for a cross-client freshness the user did not ask for.
- Recursive removal uses `std::fs::remove_dir_all` on a path verified as above, the same primitive `repo_cache.rs` and
  `farhelm-teststate` use, which does not follow symlinks.
- Size is disk usage summed from `symlink_metadata` over the tree, in the supervisor's blocking worker, with one
  deadline per request, reusing the bounded-worker pattern directory browsing uses (`handle_browse_directory`).

**What the planner found (at de457dc7).**

- Archiving: `crates/farhelm-supervisor/src/service/teardown.rs` (`archive_last_reference`, ~700–770, ~926; the
  `ArchiveOutcome::Archived { destination }` arm only logs today), `working_copies.rs` (`archive_move`,
  `ARCHIVE_DIR_NAME`, `rename_exclusive_into`, `same_directory`, `verified_root`), `store.rs` (the `working_copies`
  table with `path_device`, `path_inode`, `path_birth_ns`, `allocation_state`, `archive_destination`; rows go
  `allocated` → `archive_pending` → `retired` and are kept). The archive move is a same-device no-replace rename, so
  identity survives it; `reconcile_archive` relies on that already. Nothing ever cleans the archive. Repository
  discovery skips the archive folder.
- Delete reply: `ControlMsg::SessionDeleted { req_id, notice }` (`farhelm-proto/src/lib.rs`), built from
  `teardown_session`'s return, forwarded by helm `client.rs` `delete_session_with` and `sessions.rs` `delete_session` as
  `{"notice": …}`, read by the UI's `api.rs` `delete_session` / `DeleteReply` and `list/view.rs` `do_delete` /
  `DeleteNotice`. Replace also publishes the notice.
- Protocol: `PROTOCOL_VERSION` is 43, pinned by `protocol_version_is_pinned_at_43`; new variants must also be added to
  `reply_req_id`, `request_req_id` and the name match. SPEC.md: no compatibility across protocol versions, and the
  "Upgrade compatibility and client scale" rule that no change may break any installation's update path.
- Hosts: `/api/hosts` → UI `Host { state: HostPhase }`; helm `manager.rs` `HostState::is_connected`. A host-scoped helm
  handler refuses unless the host is connected (`host_client`).
- UI: session row meta in `list/row.rs` (~1651, `session-cwd`), session view header in `session_view.rs` (~2057), Delete
  confirmation sentences in `list/row.rs` (~1915) and `session_view.rs` (~2377), `DeleteTarget::needs_confirmation` in
  `list/shared.rs`. Launcher checkout mode, `gh:` results and preview in `create_form.rs` (~1809, ~5017, ~5192–5316) and
  `launch_composer.rs` (`SearchScope::Github`, `ComposerSearchResult::Github`). Template destination in
  `list/templates.rs` (~688–702). Header in `list/view.rs` (~3260–3360) with CSS `.session-heading` in `assets/app.css`;
  the stylesheet's token comments say what each color may be spent on.
- Docs: `website/src/content/docs/docs/using/start-a-session.mdx` ("Start from a fresh GitHub checkout"),
  `stop-restart-resume.mdx`, `session-list.mdx`; `docs/github-checkouts.md` ("What deletion does").
- Design history: `lore/2026-09-17-github-clone-launch-brainstorm.md`. Archiving was chosen as a stand-in for real
  deletion, so that a bug in the logic would show up before anything was deleted.

**Binding repository constraints:** root `AGENTS.md`: Finishing work (targeted validation; browser specs on Chromium and
WebKit through the recorder; the JS harness; the test-sleep check; `scripts/check-desktop-assets.sh` if an asset file is
added), Releases and the changelog (fragments for `feat`/`fix`), Docs website (read `website/AGENTS.md` and
`website/EDITORIAL_RULES.md` first; the Overview page and its intro SVGs are not edited without the maintainer's
request), Harness-specific code, Testability, Sharing the machine, Agent scratch space, The live install is off-limits.
SPEC.md "Upgrade compatibility and client scale" (the protocol bump must not break any installation's update path).
`.agents/test-authoring.md` for any test change.

## Outline

The PR slicing below is a proposal; reshape it if the code argues for it, without churn. Each PR updates SPEC.md,
SPEC_impl.md and the docs for what it changes.

### PR 1: call them managed checkouts and mark them in the session list

`feat:` with a changelog fragment. The rename of user-facing strings (including Delete's confirmation, Clone and the
approval card), the branch glyph component, the session row and compact row mark with its tooltip, the session view's
mark, SPEC.md wording, the docs pages' wording, and the e2e string updates the rename forces.

### PR 2: a visible destination type in the launcher and the template editor

`feat:` with a changelog fragment. The segmented folder / managed checkout control in both dialogs, the repository field
and explanatory preview, `gh:` results selecting the type, removal of "use existing folder" in favour of the segment,
and browser tests for both dialogs.

### PR 3: the supervisor and helm side of the trash

`feat:` with a fragment of kind `none` if nothing is user-visible yet. The list and delete requests, the protocol bump,
"archived at" derivation, size measurement with its bound, identity-verified deletion with the live-session guard, the
archived field on the delete reply, helm routes, SPEC_impl.md, and supervisor and helm tests.

### PR 4: the trash in the UI

`feat:` with a changelog fragment. The header button and its states, the count, the dialog with host groups, unreachable
hosts, sizes and confirmations, the delete cue, SPEC.md and the docs pages, and browser tests. Remove both TODO.md
entries.

### Out of scope

Restoring an archived checkout, opening or deleting single entries from the trash, automatic or age-based cleanup,
deleting checkouts that were not archived by Farhelm, the helm caching listings for unreachable hosts, supporting
repositories other than GitHub, and renaming internal types, protocol fields or the `gh:` search label.

### Validation

Per root `AGENTS.md` Finishing work, choose targeted checks per PR: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` when the supervisor or helm
changes, `cargo check -p farhelm-ui --features desktop`, the affected crates' unit tests through the recorder (the proto
crate's version pin test, the supervisor's working-copy, teardown and store tests, the helm's sessions tests, the UI
crate's tests), `cd crates/farhelm-ui/js-tests && node --test` if asset JS changes, the relevant Playwright specs (the
GitHub-checkout specs, clone, delete, templates, and the new ones) on Chromium and WebKit through the recorder after
`cargo build` and the `dx` web build, the test-sleep check,
`cd website && bun install --frozen-lockfile && bun
run build` when docs pages change, `dprint check`, and
`python3 releasing/check-changelog.py format`. Look at the launcher, the template editor, the session list and the trash
dialog yourself in a real browser at the sidebar's minimum and default widths, and compare them with the mockup.

## Appendix: the design

Colors are the app's tokens (`crates/farhelm-ui/assets/app.css`). Glyphs use a 12-unit view box, stroke `currentColor`
at 1.2, round caps and joins.

- Branch glyph:
  `<circle cx="3.5" cy="2.8" r="1.2"/><circle cx="3.5" cy="9.2" r="1.2"/><circle cx="8.5" cy="4.2"
  r="1.2"/><path d="M3.5 4v4M8.5 5.4c0 2-5 1.4-5 2.6"/>`.
  Drawn at 12px on rows, in `--fg-1`, with the repository in `--fg-0`.
- Folder glyph (launcher and template segment):
  `<path d="M1.5 3.2v5.6a.8.8 0 0 0 .8.8h7.4a.8.8 0 0 0
  .8-.8V4.6a.8.8 0 0 0-.8-.8H6L5 2.4H2.3a.8.8 0 0 0-.8.8z"/>`.
- Trash, empty:
  `<path d="M1.5 3h9M4.6 3V1.6h2.8V3"/><path d="M2.6 3l.6 7.3a.9.9 0 0 0 .9.8h3.8a.9.9 0 0 0
  .9-.8L9.4 3"/><path d="M5 5.2v3.8M7 5.2v3.8"/>`,
  at 16px, in `--fg-2`.
- Trash, full ("lid pops open"):
  `<path d="M2 2.6l7.6-1.9M5 1.9l-.3-1 2.4-.6.3 1"/><path d="M2.6 4.2l.6 6.1a.9.9 0 0 0
  .9.8h3.8a.9.9 0 0 0 .9-.8l.6-6.1z" fill="currentColor"/><path d="M4 4.2l.8-1.2 1.4.6 1.6-.9.6 1.5"/>`,
  at 16px, in `--fg-0`, with the count beside it at 11px semibold. The button has New's border width and vertical
  padding, so the two are the same height.
- Trash tooltip. Empty: "trash: empty. When you delete the last session using a managed checkout, its folder moves here
  instead of being deleted." Full: "trash: N archived checkouts. A managed checkout moves here when the last session
  using it is deleted. Click to see them, or to delete them for good."
- Row tooltip: "managed checkout of <owner/name>", the folder path, then "Farhelm made this folder for the session. When
  the last session using it is deleted, it moves to the trash."
- Launcher preview, under the path: "Farhelm clones the repository into a new folder named after the session, under this
  host's checkout folder. When the last session using it is deleted, the folder moves to the trash." The template editor
  prefixes "Each session from this template gets its own managed checkout."
- Trash dialog: title "trash" with the glyph; explanation "Folders of managed checkouts whose last session was deleted.
  They stay here, untouched, until you delete them. To get something back, copy it out of the folder shown under its
  host." Host group: a bordered box with a header band (host name semibold, "N checkouts · size", and an outlined
  danger-tier "delete all on <host>" button at the right), entries as two-line rows (name and size; branch glyph with
  repository, and "archived <when>"), and a footer band "on disk: <path>". Unreachable host: the whole box at reduced
  opacity, the note "not reachable, so they can't be checked or deleted", the button disabled. Dialog footer: "N
  checkouts · size on reachable hosts" and a filled danger-tier "delete all". A confirmation replaces nothing: it
  appears in place, in a red-tinted band under the header it belongs to (or above the footer for the global one), with
  "cancel" and "delete permanently".
- Delete cue: a dashed outline the size of the deleted row moves from the row's position to the trash button over about
  650 ms, shrinking and fading; then the trash rotates through a short decaying wiggle of about 900 ms.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-managed-checkout-trash-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/managed-checkout-trash/<nn>-<short-name>`.
- Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack deletes. Within this run, if
  a PR needs correcting, restructure it rather than stacking a correction on top; that applies to all of this plan's own
  open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entries this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
that PR's changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review
swarm: a fresh-context agent on Opus 5.5 at high effort, shelled out to the harness that serves that model when the
executing one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. For PR 3, also ask the reviewer to check
specifically that deletion can never remove anything other than a verified recorded archive. Where you disagree with a
finding, decide on the merits and log the DECISION. Do not write a launch command here or in the log; the
harness-shellout skill owns launch mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (a schema migration, a helm-side cache of host
listings, a new background poll for the count, restore or per-entry actions, deleting anything not recorded as archived;
these are examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run
a fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions above, this
outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative
was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the protocol shapes, how the count stays current, how "archived at" is known,
what happens to a record after its archive is deleted, the size bound, how identity is verified before deletion, how the
launcher's type control replaces "use existing folder", what the header gave up to fit the trash at minimum width, and
every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. In
particular, never widen what deletion may remove beyond verified recorded archives. If the work needs such a decision,
record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10). Because the PRs form one
linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the question, and close the plan
as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
