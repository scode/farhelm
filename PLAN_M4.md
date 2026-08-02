# Farhelm M4: attachments and terminal tabs

NOTE: This is the plan for milestone 4 only. It builds toward SPEC.md using the choices in SPEC_impl.md; where this
document and those disagree, they win. The overall motivation and the coarse milestone ladder live in PLAN.md; only the
current milestone is planned at this level of detail.

## Goal

Give a session more than one terminal, and give every one of those terminals paste-and-drop that lands files where the
agent can read them. SPEC.md's session-view and attachments sections are the contract: additional terminal tabs are
plain shells in the session's working directory that survive disconnects and supervisor restarts exactly like the agent
terminal, and pasting or dropping a file or image into ANY of the session's terminals transfers it to the session's host
and inserts the resulting host-side path at the cursor — unconditionally intercepted, never dropped into the workspace,
never silently lost.

Tabs come first, deliberately: the attachments contract quantifies over "any of a session's terminals — the agent's or a
tab's", so building tabs before attachments means the attachment work ships testable at its full scope instead of
growing a tab dimension afterwards.

## User-visible outcome

- A session view has a tab strip: the agent terminal, plus any number of plain shell tabs opened in the session's
  working directory. A tab is a real terminal with the same fidelity, replay, and flow-control guarantees as the agent
  terminal. Detach, reconnect a day later, and the tab is still there with its scrollback — supervisor restarts
  included. After a host reboot or an archive, tabs are gone and nothing recreates them; the user re-adds what they
  need.
- Closing a tab kills that shell and its processes — the whole per-tab operation set in v1. Stopping the agent leaves
  tabs running; delete and archive take everything down.
- Opening a tab when the session's working directory has vanished fails with a clear error naming the directory; the
  session itself is untouched.
- Pasting or dropping a file into any terminal of a session uploads it to the session's host and inserts the host-side
  path at the cursor, so the agent picks it up with no manual copying. A pasted screenshot does the same with a
  generated name. Plain text still pastes as text — including text that merely looks like a path. Dropped directories
  are rejected with a visible error.
- Transfer never blocks typing: the terminal stays interactive while an upload runs, and the path lands at whatever
  cursor position is current when it completes. Failures are visible — an upload that dies says so where the user can
  see it, never silently.

## Scope

### In

1. **One protocol bump to 6, all M4 wire vocabulary upfront.** The M2.5/M3 rule stands: new tagged-enum variants are
   connection-fatal to older decoders, so every wire shape lands in one proto PR — vocabulary first, handlers later. The
   vocabulary: a terminal selector on attach (the agent terminal or a tab by id — today's attach implicitly means window
   0), a session-attachment lease — a per-client identity carried on every terminal attach, because the supervisor sees
   only channels multiplexed over the helm connection and needs to know which channels belong to one client before it
   can enforce the one-attached-client rule across all of a session's terminals (item 3) — tab open/close requests and
   replies, the tab list carried on session detail, the attachment-upload control shapes (begin with session id,
   proposed filename, and declared size; a commit/abort close; a reply carrying the host-side path or the error), and
   the error vocabulary the handlers will need — tab-not-found, and attachment-specific failures (declared-size
   mismatch, storage failure, stalled transfer). The vanished-working-directory refusal reuses M3's error shape if it
   fits tab-open unchanged; if it needs widening, that widening is part of this bump. Attachment bytes themselves ride a
   data channel, chunked with a bounded in-flight window — control frames are size-capped (M2 defused oversize frames to
   per-request errors), a screenshot must not be one giant frame, and the window keeps bulk bytes from queueing ahead of
   latency-sensitive terminal input and flow-control frames on the shared connection. Golden and both-direction
   tolerance tests within 6; the version-skew handshake test grows the new boundary.
2. **Supervisor terminal tabs as tmux windows, rediscovered rather than stored.** A tab is a new window on the session's
   tmux session, running the user's shell through the same interactive-login contract as the agent launch
   (`$SHELL -l -i`, per-launch evaluation — the SSH-and-type contract applies to tabs too), started in the session's
   working directory after the same vanished-cwd check restart makes, and carrying the session's environment marker plus
   a per-tab marker of its own. Opening a tab needs the session's tmux session to exist: on a session whose terminals a
   reboot (or archive) erased, tab open refuses with a clear error saying to restart the session first — building a
   tab-only tmux session for an agent-less session would be a strange half-alive state, and SPEC.md's "nothing recreates
   them automatically" already puts re-adding tabs after the user's own restart. No shim and no launch spec, but launch
   failure is still not silent: a tab pane already dead when open would reply is a refused open with the pane's last
   words as the error detail (window cleaned up), per SPEC.md's every-failed-operation rule — while a shell that starts
   and later exits is just a dead pane (`remain-on-exit` keeps it viewable like any other), because established tabs
   have no error-vs-exited story to tell. Tabs are NOT rows in supervisor.db — SPEC.md says they are not durable
   metadata, and the honest implementation of that is rediscovery — but rediscovery cannot be positional: the pane's own
   processes inherit `TMUX` and can create windows on our private server, so a bare "windows 1+" scan would adopt
   foreign windows with the wrong cwd and teardown semantics. Farhelm marks the windows it creates with a tmux user
   option (the tab identity) and the agent window likewise, rediscovers only marked tab windows, and finds the agent by
   its marker rather than by index (window 0 remains the agent in practice, as SPEC_impl.md describes — the marker
   hardens identification without changing the layout, and SPEC_impl.md picks up that refinement when the tabs PR lands
   it). A supervisor restart thus preserves tabs by the same mechanism that preserves the agent terminal; a reboot or
   archive erases them with nothing left to clean up; an unmarked window someone conjured is ignored rather than
   misreported. Close reaps in M2's stop ordering, never kill-window-first: the tab's tree is enumerated and quiesced
   while the live pane still anchors the descendant walk, the kill goes through the tab's own scope where one exists,
   the window dies, and a re-enumeration catches survivors — killing the window up front would orphan the walk's root
   and leave only the weaker marker scan. The scope is M3's layering applied to tabs: each tab launch is wrapped in its
   own `systemd-run --user --scope` where a user manager exists — the marker sweep provably cannot find a descendant
   that scrubbed its environment, which is exactly why agent launches got scopes — and the pane-descendant walk unioned
   with the per-tab marker scan runs as the backstop, and as the whole mechanism where no manager exists. "Kills that
   shell and its processes" is a promise about daemonized children too. Tabs force a marker split that M3's stop
   machinery did not need: stop and restart sweep by session marker today, and once tab processes carry that marker the
   sweep would reap them — directly against SPEC.md's "terminal tabs keep running" after stop and "restart touches the
   agent terminal only". So the agent's own launches gain an agent-scoped marker, stop and restart select agent-marked
   processes (unioned with the agent pane's descendants, as today) plus a legacy bucket — session-marked processes
   carrying no kind marker at all, which keeps daemons from pre-split launches covered — and the session-wide marker
   sweep remains exactly the delete/archive semantic, which now also covers every tab. Two rules sharpened during
   implementation ride with this: every launch boundary scrubs the OTHER kind's inherited markers, because a supervisor
   dogfooded inside a farhelm tab would otherwise hand its agents an ambient tab marker that exempts them from their own
   stop sweep; and tab selection anywhere requires the session marker and a non-empty minted tab value together, never
   the tab variable's mere presence. A test pins stop and restart leaving a tab's shell and its daemonized child
   untouched, and another pins the ambient-marker agent still being reaped.
3. **Per-terminal attach channels under session-scoped ownership.** SPEC.md's one-attached-client rule is per SESSION:
   the attached client owns all of the session's terminals, and a takeover detaches every terminal channel the previous
   client held, as one visible event. Item 1's lease is what makes that enforceable — the supervisor groups terminal
   channels by the lease identity they attached under, and a different identity attaching to any of the session's
   terminals detaches every channel of the old one. Below that, each attached terminal gets its own control-mode tmux
   client and its own WebSocket/protocol channel — the M1 cutover and M2.5 flow-control machinery, generalized from "the
   session's pane" to "a window's pane". Per-terminal control clients are load-bearing, not an implementation
   convenience: `pause-after`/`%pause` operate per control client, so sharing one client across terminals would let one
   stalled tab viewer pause the agent's stream. With one client per terminal, a slow tab slows only itself, and the
   stall detach fires per terminal — an interpretation this plan settles deliberately: SPEC.md's stalled-viewer bullet
   defines the SURFACE (the takeover-detach banner, reattach as an ordinary reconnect), and a genuinely wedged client
   hits every terminal's stall bound and converges to a whole-client detach on its own, while a live client with one
   stalled terminal loses only that terminal. The partial state is well-defined — the stalled terminal shows the stall
   banner and reattaches like any reconnect — and detaching a working agent view because a background tab wedged would
   punish exactly the terminal the user is using. Resize likewise goes per window (`resize-window` already targets one
   window). The incumbent-kill step of the attach cutover scopes to the terminal being attached; the session-level
   takeover is the lease check above it.
4. **Supervisor attachment storage.** Uploads land in `~/.local/state/farhelm/attachments/<session-id>/`, the directory
   SPEC_impl.md already names. The upload path is receive-to-temp, then rename into place, and the reply carries the
   final path — under M3's atomicity policy this class is best-effort atomic: a torn file must never be observable at
   the published path, but a crash mid-upload just loses the upload, which the client surfaces as a failed transfer (the
   reply never came). The declared size is verified against the received bytes before rename — a short or long stream is
   a mismatch error, never a published file. Naming keeps the original filename recognizable — agents read paths, and
   `screenshot.png` beats a hash — but reduced to a SHELL-SAFE basename: ASCII alphanumerics plus `.`, `_`, and `-`,
   everything else (spaces included) mapped to `_`, because the path is inserted as terminal input and a name a shell
   would split or expand breaks exactly the flow attachments exist for. A name that sanitizes to nothing gets a
   generated fallback name (the pasted-image scheme) rather than a refusal — SPEC.md rejects only directories, never a
   file for its name. Numeric suffix on collision, and publication is no-clobber atomic — two concurrent uploads that
   both picked the same free name must both publish, under distinct paths, never one silently replacing the other. Temp
   files have a full lifecycle, not just a happy path: an abort or channel loss cleans the upload's temp immediately,
   and a startup sweep removes any orphans a crash left behind (M3's orphaned-temps rule, applied to this directory).
   There is no size cap in v1: the bytes are the user's, on the user's machine, and the transfer is streamed and
   backpressured end to end, so a large file costs time, not memory — a full disk surfaces as a failed upload with
   nothing published, and this recorded no-cap-plus-visible-failure policy lands in SPEC_impl.md alongside the
   implementation. Two more obligations SPEC.md attaches to this path land with it: it is a long-lived paste path, so a
   transfer that stays connected but stops progressing is detected (per-hop progress timeout), aborted with its temp
   file cleaned, and surfaced as a stalled-transfer error rather than a silent forever-pending upload; and it emits the
   structured diagnostic trail SPEC.md's logging section requires for attachment transfer — begin, publish, abort, and
   failure events carrying session and transfer identifiers and byte counts, never contents. Session delete removes the
   directory, and deletion is serialized against the upload lifecycle: delete aborts in-flight transfers, a commit that
   races deletion fails with the session-gone error rather than publishing, and nothing recreates the directory after
   removal — "removed when their session is deleted" must hold even when the delete lands mid-upload. Archive does not
   touch the files (archive shuts down processes; the attachment files die with the session row).
5. **Helm plumbing for both features.** Tabs: REST to open and close a tab on a session, the tab list on the session
   detail the UI already fetches, and the existing terminal WebSocket generalized by a terminal selector (agent or tab
   id) — one WebSocket per attached terminal, per SPEC_impl.md's helm shape. Attachments: an upload endpoint on the
   session (multipart or raw body with the filename alongside — chosen at implementation time by what the browser side
   makes robust), streamed through to the supervisor's begin/chunks/commit shapes, replying with the host path or the
   error. No direct client-to-supervisor path exists, so the helm relays; it must stream rather than buffer whole files,
   for the same no-size-cap reason as item 4, and it shares item 4's transfer obligations — the progress timeout covers
   the client-to-helm hop too, and the relay emits its own leg of the transfer diagnostic trail.
6. **UI terminal tabs.** A tab strip on the session view: the agent terminal first and unclosable, one xterm.js island
   per open tab, an add button, a per-tab close with an in-page confirm (wry has no native dialogs — established M2
   ground). Every open tab's terminal stays attached concurrently — switching tabs is a CSS visibility change, not an
   attach cutover — because each terminal has its own channel and flow control (item 3), and background tabs consuming
   their own streams is exactly what per-terminal backpressure makes safe. Attach-on-select was considered and rejected:
   every switch would pay a full cutover replay (the visible re-scroll PLAN.md already records as an M5 problem) and
   exercise the takeover machinery on every click for no gain; the resource cost of staying attached is one xterm
   instance, one channel, and one control client per tab, and tab count is bounded by the user opening them by hand, so
   v1 adds no artificial cap. A tab whose WINDOW is gone — closed from another client, or erased by a reboot or archive
   — renders the session view's existing no-terminal explanation; a tab whose shell merely exited stays viewable like
   any dead pane, exactly as the agent terminal does (`remain-on-exit` is the contract for both). The tab list refreshes
   by the same polling M2 settled for the session list, so a tab opened or closed from another client appears without a
   reload — SPEC.md's changes-appear-automatically rule, on the interim mechanism; M5's live push replaces both polls
   together. Tab labels are positional ("Terminal 1", "Terminal 2") — SPEC.md gives tabs no names and close is their
   only operation, so v1 invents no naming surface.
7. **UI paste and drop interception, classified by flavor.** On every terminal of the session view, paste and drop
   events classify in SPEC.md's precedence order: actual file objects first, then image data, then plain text — pasted
   text that looks like a path is still text, and plain text passes through the existing paste path untouched. Files and
   images are intercepted unconditionally (remote and local alike) and uploaded via item 5; on completion the host-side
   path is inserted through the same code path a plain text paste takes — bracketed-paste aware for free, at whatever
   cursor position is then current, into the terminal that received the drop. Pasted images get a generated name
   (`pasted-<n>.png`-shaped, extension from the MIME type). Dropped directories are rejected with a visible error —
   SPEC.md's rejection rule is unconditional, so it cannot hinge on any one API: the drag-entry API detects directories
   up front where the engine provides it, and where it does not, a directory masquerading as a `File` fails when its
   bytes are read, which rejects the drop visibly before anything uploads — no engine gets a silent pass, and nothing
   directory-shaped ever publishes. If a supported engine turns out to deliver directory drops as readable `File`s
   (neither detectable nor read-failing), that is exactly the "real deficiency in our actual flows" SPEC_impl.md's
   clipboard risk reserves the native wry hooks for — the rejection promise stays unconditional; only the mechanism
   escalates. Multi-file drops upload each file and insert each path in completion order, space-separated — the agent
   sees N paths, which is what N dropped files mean. An upload in flight shows an unobtrusive indicator; a failed upload
   surfaces an error the user cannot miss, and never inserts a path. The desktop target rides the same DOM event path
   per SPEC_impl.md's decision, with its wry file-drop-interception config checked as part of this work (the known
   foot-gun: wry swallows DOM drop events unless told not to); engine differences beyond that wait for a real deficiency
   observed in our flows, per the recorded risk posture.

### Out (deliberately)

Tab renaming, reordering, and any per-tab operation beyond close (SPEC.md: close is the whole v1 set). Tabs as durable
metadata — nothing recreates a tab after reboot or archive, by contract. Attachment management UI (listing, deleting
individual attachments — the files live and die with the session in v1). Upload progress percentages (an indicator, not
a meter). Paste-history or multi-item clipboard handling beyond the flavor precedence. Status heuristics, profiles,
rename, live list push (M5); helm-side persistence and multi-host (M6); archive itself, spawn, web auth (M7). The
attachments directory's cleanup on session delete is M4's only new delete obligation; nothing else about lifecycle
changes.

## Testing decisions (settled while planning)

Proto changes get the same golden and tolerance coverage every bump has gotten, plus the version-skew boundary at 6.
Supervisor tab behavior is Rust integration tests against real tmux: open (cwd check, dead-at-reply refusal, and the
restart-first refusal on a session with no tmux session), close (the tab-scoped reap verified with a deliberately
daemonized child, same technique as M2's stop tests; the scope path additionally pinned against an environment-scrubbing
double-fork where a user manager exists, loudly skipped where not — M3's pattern), list-by-rediscovery across a
supervisor restart including an unmarked foreign window that must be ignored, and per-terminal attach cutover and flow
control on a tab (a stalled tab client must not pause the agent's stream — pinned, because it is the reason per-terminal
control clients exist). A tab is the same terminal machinery as the agent pane, and the tests say so rather than assume
so: the existing terminal conformance coverage — replay, alternate-screen selection, pane-mode restoration, resize,
binary-clean output — is parameterized over both targets instead of duplicated or skipped for tabs. The tab launch
contract gets M3's environment treatment too: an rc-file change in a fixture HOME between two tab opens is visible to
the second (never by mutating the test process's environment — repo rule), and the unset-`$SHELL` passwd fallback is
exercised through the existing seam. Attachment storage is exercised at the supervisor integration level: size-mismatch
refusal, collision suffixing (including two concurrent uploads of the same name), sanitization including the empty-name
generated fallback, abort and orphan-sweep temp cleanup, and delete-removes-the-directory. The torn-file window is
tested through M3's fault-injection seam, which grows a streaming variant for this consumer: M3's seam wraps whole-file
staged writes, and an attachment must stream through the same write-flush-rename-cleanup-injectable stages without ever
materializing in memory — a bounded-memory large-upload test pins that the seam extension did not quietly buffer the
file.

Playwright covers the user-visible contracts end to end against the fake agent: open a tab and run a real shell command
in the session cwd; reload the browser and find the tab still attached with scrollback; close a tab behind the in-page
confirm; drop a synthesized file into the agent terminal and see the inserted path; drop or paste into a TAB and see the
path arrive in that tab's input with the agent terminal untouched — the "any of a session's terminals" words made
executable; paste text that looks like a path and see it arrive as text; a directory drop rejected visibly. Browser
reality check, recorded honestly: synthesized `DataTransfer` objects carry files fine, but the drag-entry API that
detects directories does not reproduce faithfully under synthesis in every engine — if the directory-rejection e2e
proves flaky to synthesize, the rejection logic gets a unit-level test at the classification seam and the e2e pins the
file path only, with the gap recorded in M6.5's backfill list rather than silently absorbed. The no-entry-API fallback
(read-failure rejection, item 7) gets its own test at that same seam regardless. Image-paste insertion is pinned e2e by
synthesizing the paste event with image data at the event seam, not through the async clipboard API — that API's
permission and gesture gating make real-clipboard automation engine-specific, so the synthetic event is the
deterministic path, and real clipboard behavior belongs to the manual desktop pass below. The classifier itself gets
table-driven coverage of SPEC.md's precedence order — a payload carrying file+image+text and one carrying image+text
each produce exactly one winning interpretation and exactly one insertion — and the clipboard-borne FILE reference (a
file object pasted, not dragged — SPEC.md names it explicitly) is pinned at that same seam, with a recorded manual
engine check standing in where browser automation cannot synthesize it faithfully.

The one manual-verification item is the desktop (wry) attachment path, consistent with SPEC_impl.md's standing decision
that the tested surface is the web build — but it is a capability check, not a configuration check: one recorded manual
pass on the desktop build covering a real file drop AND a real image paste end to end (upload lands, path inserts at the
cursor), with the wry file-drop-interception config verified as part of it. A config review alone would not verify the
capability parity SPEC.md promises the two client forms. The same recorded pass notes the observed
paste-to-path-inserted latency for a representative screenshot against a REMOTE session — SPEC.md promises "for a
typical screenshot this is imperceptible", which no local Playwright run can vouch for; a number in the record keeps the
promise honest without inventing a CI gate for a subjective bar. SPEC.md's Mac-screenshot acceptance walkthrough itself
still lands with M7's Mac app bundling; this pass covers the desktop build on the platform we have.

## Order of work

Each step leaves something runnable; later steps only add. Tests ride with their step.

1. This plan, plus the PLAN.md header update it implies.
2. Proto: the complete M4 wire vocabulary, one bump to 6.
3. Per-terminal attach generalization under the lease — split out of the tabs step while building it, because it is the
   riskiest machinery change of the milestone and deserves review in isolation against the existing agent-only behavior
   (tab selectors refuse with not-found until the next step delivers tabs).
4. Supervisor tabs: windows, open/close/list-by-rediscovery, the marker split that keeps stop and restart off tabs, the
   tab-scoped reap; the two-terminal same-session forms of the stall-scope, input-routing, resize-targeting, restart,
   and delete tests land here, where a second terminal first exists to test against.
5. Stream-isolation hardening — found while reviewing step 4: every per-terminal control client receives the whole
   session's pane traffic, and under tmux's nondeterministic pane-throttle path a stalled terminal's client can
   transiently slow the session's other panes until the stall detach fires, which is bounded but weaker than this plan's
   isolation claim. The fix is one always-drained per-session sink client (so tmux never stops reading any pane) plus
   foreign-panes-off on per-terminal clients — safe only in that combination, which is why step 4 shipped the honest
   qualification instead of a quick flag.
6. Helm tab plumbing: REST open/close, tab list on session detail, terminal WebSocket selector.
7. UI tabs: the strip, concurrent attachment, close-with-confirm, Playwright coverage.
8. Supervisor attachments: the storage path, size verification, naming, delete cleanup.
9. Helm attachment upload: the streaming relay endpoint.
10. UI interception: classification, upload, path insertion, failure surfacing, Playwright coverage; the wry drop-config
    check rides here.

## Acceptance

M4 is done when all of the following hold, pinned by automated tests except where a manual run is named:

1. A tab opened in a session runs a shell in the session's working directory; a command typed there shows the cwd.
2. Kill -9 the supervisor with two tabs open and restart it: both tabs are listed, attachable, and their shells never
   noticed. Reload the client mid-session: every tab reattaches with scrollback intact.
3. Closing a tab kills its shell AND a deliberately daemonized child of that shell; the agent terminal and other tabs
   are untouched. Stopping or restarting the agent leaves a tab's shell and its daemonized child untouched — the
   stop-path sweep must never reach tab processes. Deleting the session takes agent, tabs, and every daemonized
   descendant.
4. Opening a tab after the working directory vanished fails with an error naming the directory; the session survives.
   Opening a tab on a session whose tmux session no longer exists fails with the restart-first error; a tab whose shell
   is dead by open-reply time is a refused open with the pane's output as detail, not a silently "successful" dead tab.
   An unmarked window created behind the supervisor's back never appears as a tab.
5. A stalled viewer on one tab pauses only that tab's stream: the agent terminal and other tabs keep flowing, and the
   stalled terminal alone takes the stall detach. Attaching from a second client detaches every terminal channel the
   first client held, as one takeover event.
6. A file dropped on any terminal — agent or tab — lands under the session's attachments directory and its host path is
   inserted at the cursor of that terminal; typing stays live during the transfer — including during a deliberately
   throttled large upload — and the path lands at the cursor position current at completion. A pasted image does the
   same under a generated name. N dropped files insert N paths.
7. Pasted text passes through as terminal input even when it looks like a path; a dropped directory is rejected with a
   visible error; a failed upload (induced) surfaces visibly and inserts nothing; a transfer wedged mid-stream (induced)
   is aborted with a visible stalled-transfer error and its temp file cleaned.
8. Two uploads named identically coexist under distinct paths — including two launched concurrently; a shell-hostile
   filename publishes under its sanitized shell-safe name; a declared-size mismatch publishes nothing; deleting the
   session removes its attachments directory, and a delete racing an in-flight upload (at begin and at commit) leaves no
   file and no directory behind.
9. The desktop-build manual pass is recorded: a real file drop and a real image paste both land and insert, the wry
   file-drop configuration is verified as part of it, and the remote-screenshot paste latency is noted in the record.
10. The full CI gate is green on every PR.

## Risks retired by this milestone

- The attachments pipeline — the headline paste-a-screenshot flow — stops being a paper promise and meets the two
  engines' real clipboard/drop behavior, where SPEC_impl.md has recorded divergence risk since M1.
- The attach/flow-control machinery generalizes from "the one pane" to "any window's pane" — proven now, with the
  one-stalled-viewer isolation property pinned.
- The one-attachment rule gets its multi-terminal interpretation settled (session-scoped ownership, per-terminal
  channels) before M5's live-push and M6's multi-host layers build on attachment semantics.
- Frame-size discipline meets its first bulk-transfer consumer: the chunked upload shape proves the protocol can move
  megabytes without violating the bounded-frame rule M2 established.
