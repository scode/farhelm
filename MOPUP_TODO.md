# Mop-up TODO: everything known to remain after the M7 run

This file aims to be self-contained: each item carries enough context and motivation to act on without the working logs,
which live outside this repository and will not follow it. Where a repository file is itself the living record for an
item (a checklist with result fields, a ledger paragraph), this file names that file rather than duplicating it — the
pointer is part of the item, and only out-of-repo dependencies count as broken self-containment. Written 2026-08-08 at
the close of the run that built M6.5 + M6.75 + M7 as one 29-PR stack (#96–#124) off `main`, plus PR #125 carrying this
file; nothing merged at time of writing.

**Merging the stack** (the one step preceding everything below): the previous stacked-milestone merge landed 21 PRs in
one motion rather than 21 merge-and-CI cycles — retarget every PR's base to `main` (`gh pr edit --base main`), then
fast-forward push `main` to the stack tip; the linear chain is already one commit per PR, GitHub auto-closes every PR as
merged, and the push bypasses the queued `require-main-base` check (admin bypass, expected — that check exists to block
out-of-order merges, and a fast-forward of the whole chain is the in-order case). Afterwards: delete the stack bookmarks
locally and remotely, cancel any still-running PR-branch CI, and let the single `main` CI run be the verdict.

NOTE (2026-08-09): #125 stopped being the stack tip after this file's original snapshot. The flake-fix series added
review units above it, followed by the tmux output-client teardown fix, and this document now sits at the top again. Do
not use the old PR range as a merge inventory: derive the current linear bookmark and PR chain from `jj log` and GitHub
immediately before merging.

## Release engineering (CI-side; no Mac required)

1. **Finish the macOS release bundle.** The release workflow's `macos` job (workflow_dispatch-gated, builds an
   aarch64-apple-darwin `Farhelm.app` with an embedded helm, managed supervisor, private tmux, and the CLI at
   `Contents/MacOS/farhelm`, publishing it as a release asset) has never completed. It was parked after two failed
   attempts under a standing 180-minute total macOS-runner budget, of which 23 minutes are spent. Attempt 1 failed
   installing ncurses' terminfo database on the runner's case-insensitive APFS (fixed: the darwin build now uses
   `--disable-db-install` and points at the system terminfo). Attempt 2 got past that and failed because tmux 3.7b's
   configure on darwin insists on an explicit utf8proc decision (`must give --enable-utf8proc or --disable-utf8proc`).
   The believed-complete next step is adding `--disable-utf8proc` to the darwin tmux configure in
   `scripts/build-private-tmux.sh` (utf8proc is not among the pinned sources and tmux runs without it on the Linux
   side); budget for the possibility of one more such configure-iteration stop behind it. Resume recipe, named precisely
   because this is a stacked-history operation: the script belongs to PR #123's commit (bookmark `m7-packaging`); squash
   the fix into that commit, which rebases every descendant through the current stack tip (the original #124/#125 pair
   is no longer the whole descendant set); force-push every moved bookmark; move tag `v0.0.3-rc.1` to the REWRITTEN
   `m7-packaging` head (`gh api -X PATCH repos/<owner>/<repo>/git/refs/tags/v0.0.3-rc.1 -f sha=<full-sha> -F force=true`
   — the workflow checks out and verifies that tag, so a stale tag builds the wrong commit); then
   `gh workflow run release.yml --ref v0.0.3-rc.1 -f release_tag=v0.0.3-rc.1` and count the macos job's minutes against
   the remaining 157. If the stack has already merged, tag the merged commit instead and skip the stacked mechanics. The
   Linux half of the workflow is proven: it has published real Linux artifacts to the prerelease twice.
2. **Dispose of prerelease v0.0.3-rc.1 — the release AND the tag.** Both were minted purely to exercise the release
   workflow: not because GitHub releases need a pre-existing tag (`gh release create` can mint one), but because THIS
   repository's workflow deliberately checks out and verifies `refs/tags/<release_tag>` before building, so exercising
   it required a real tag on the stack's packaging commit. The prerelease carries real Linux assets built from that
   open-stack commit. Once a real release exists — or sooner — either delete both (`gh release delete v0.0.3-rc.1` and
   `gh api -X DELETE repos/<owner>/<repo>/git/refs/tags/v0.0.3-rc.1`) or keep both consciously; deleting only the
   release would leave a tag pointing into abandoned pre-merge history.

## Blocked on a human with a Mac

3. **Run the manual Mac checklist** (`docs/manual-mac-checklist.md` — that file IS the record: its per-section
   "Observed:" fields are the done/not-done state, and everything reads "not run" as of this writing). Four sections,
   all requiring a real Mac and the release-candidate .app from item 1: the seven-step native-app close-out
   (provisioning, sessions, restart/reboot durability, web/native handoff, spawn), the clipboard-name fact capture (with
   its privacy redaction rules), the remote-paste latency measurement, and the terminal-selection-dismissal confirmation
   carried from M5. None of this is covered by current CI: Playwright's WebKit is not WKWebView and no WebDriver for
   WKWebView exists today, so native-webview behavior is uncovered until someone builds macOS-native automation; the
   latency measurement is intrinsically manual (it needs a real Mac-to-host link and a subjective bar). For orientation,
   manual testing that HAS already happened, so nobody re-litigates it: the maintainer ran the five-step M4 desktop
   attachment pass on a real Mac (2026-08-03) — file drops, tab-targeted drops, screenshot paste, and folder rejection
   all passed; the two items deferred from that pass with approval (Finder-copied-image naming, remote-paste latency)
   are exactly what grew into this checklist's clipboard and latency sections. The M5-era selection-dismissal check is
   the one old manual item that never ran at all.

## Decisions the maintainer owes (each changes or unblocks work)

4. **Reservation tombstone scope for interactive creates** (PLAN_M7's review question 1, still open). Interactive
   creates' idempotency reservations are permanent (a decision from the durability milestone); only spawn's are bounded.
   The counter-argument: `create_reservations` is the one store table that grows without bound, and making every scope
   session-lifetime would close that — at the cost that a long-deleted session's create key becomes reusable. If
   permanence stands, item 7's bounding work is real and unowned; if it falls, that work disappears. Decide, then either
   schedule item 7 or record the reversal.
5. **The count banner versus the include-archived toggle** (PLAN_M7's review question 3). The session-list banner reads
   "N matching of M sessions" where M is the whole fleet, but the default view now excludes archived sessions, so the
   two numbers can disagree out of the box. The shipped choice keeps M as the fleet total on the grounds that archived
   sessions are still fleet members; the alternative is M excluding archived so the default view is self-consistent. A
   UX judgment, cheap to flip, worth an explicit verdict.
6. **Review two security-relevant resolutions made during the run.** (a) A conflict between the product spec and the
   implementation spec over where the web token is stored was resolved in favor of the product spec, with the
   implementation spec amended in the auth PR (#117). (b) The web credential's transport was redesigned mid-run from an
   HttpOnly cookie to localStorage + an Authorization header + a WebSocket subprotocol, because cookies are host-scoped
   rather than port-scoped: another local user's loopback-port server could lure a browser and replay the cookie with a
   forged Origin. The trade is deliberate — giving up HttpOnly means in-origin XSS can read the credential, judged
   acceptable because in-origin XSS already owns the API. What the review should verify, so it is completable from this
   list alone: the master token is stored recoverable server-side and `farhelm helm token
   show|rotate` work after a
   helm restart; device secrets are stored hash-only; the browser credential lives in localStorage (port-scoped, no
   ambient cookie is sent or honored anywhere); REST carries it as a bearer header and WebSockets as a subprotocol;
   rotation 401s every existing device session AND drops already-open feed/terminal sockets. Implementation surfaces:
   the helm's auth and middleware modules and the UI's auth/api modules; spec surfaces: SPEC.md's token section and
   SPEC_impl.md's auth/storage section as amended in #117. Record the verdict as a dated note in SPEC_impl.md or a
   review comment on #117.

## Real engineering work, parked with reasons

7. **Bound `create_reservations`** — conditional on item 4 keeping permanence, and genuinely two debts, not one
   interchangeable pair: a DIGEST of the reservation fingerprint bounds each row's size and stops retaining request
   plaintext (privacy + per-row bound, does nothing about count); EXPIRY or another pruning policy bounds the row COUNT,
   at the idempotency cost that a pruned reservation's create key becomes reusable after the horizon. The
   unbounded-table problem is only closed by the second; the store module's own docs describe both.
8. **Design cleanup-on-abnormal-exit for the e2e harness's state tempdirs.** Each browser-suite run builds ~65 MB state
   directories under /tmp; killed test stacks orphan them. One long multi-session day accumulated 423 orphans (~22 GB)
   and hit disk-full, killing unrelated work. A backstop ALREADY exists — `e2e/start-stack.sh` sweeps `/tmp/fh-e2e.*`
   directories older than 60 minutes at stack startup, with four deliberate safety guards (directories only, owned only,
   age-gated, NUL-delimited) — so name what the incident proved about it: it matches only the `fh-e2e.*` prefix while
   much of the leaked volume sat in `tempfile`-default `.tmpXXXXXX` names it never touches, and it runs only when a NEW
   stack starts, so a long-lived session accumulates without bound. The design pass must extend coverage to everything
   the harness creates (a shared, sweepable naming scheme beats chasing tempfile defaults) while preserving those safety
   guards and never reaping a CONCURRENT run's live state.
9. **The seam-requiring test debts.** Canonically recorded in-repo in PLAN.md's M6.5 ladder entry (the ledger paragraph)
   — that list is the authority; this is its summary. Assertions deliberately unwritten because each needs a seam the
   code intentionally lacks, and adding a seam purely for a test was judged worse than parking the test:
   attach-races-rename interleaving, takeover-before-marker (held-replay seam), rename-commit cancellation (seam between
   durable write and map install), claim-released-before-reply-probe (pausable tmux probe), supervisor-side mid-replay
   proof, outcome-transition failure during a rename's reply build, deterministic ack-enqueued observation, and the
   scope probe's timeout-versus-retry path (no fake-tools seam; the retry decision is pinned as a pure function only).
   Building any of them starts with deciding its seam is now worth owning.
10. **Component-prop growth in the session and host rows.** Two separate facts, previously conflated: (a) the SESSION
    row's props were partially grouped during this run — the state half became one struct, but a planned callback
    grouping was withdrawn when it turned out the framework's callback-prop memoization cannot survive struct nesting;
    callbacks stay direct props on stable handles, pinned by a render-count regression test. (b) The HOST row was left
    ungrouped by an explicit assessment decision whose condition was "regroup where props are actively growing" — and
    that condition has now fired: provisioning work grew it to 20 props. Reassess the host row's shape, but only with a
    memoization-preserving grouping (state-only structs, never callback structs) and a host-row render-count regression
    test like the session row's.

## Watch items (no action until the trigger fires)

11. **Rotating load-class test flakes on stacked-PR CI.** The original root-cause claim here was wrong: matching libtest
    threads to the runner's four vCPUs is a useful process and memory bound, but serial fresh-process runs later
    reproduced the tmux server death. The actual trigger was closing an output-bearing tmux 3.7b control client while
    pane bytes were still queued for it. Farhelm now establishes an acknowledged client-wide `no-output` boundary from a
    separate tmux process before closing or reaping that client, including cancellation and failed-open paths. CI keeps
    the four-thread cap as a resource bound and separately runs the focused teardown scenarios against a checksummed
    tmux 3.7b build. PLAN.md's M6.5 amendment carries the detailed evidence and discriminator. Any recurrence after
    those review units is new evidence; do not classify it as oversubscription without a mechanism.
12. **Playwright flood-harness sightings.** The second-occurrence trigger fired, but it was not a recurrence of the
    original `drain socket closed before FLOOD-DONE` error. WebKit instead exhausted a fixed 45-second completion budget
    while the in-page verifier was still making correct forward progress. The whole-stream test now fails corruption
    immediately, renews a bounded stall budget when progress advances, and retains an independent hard cap for a
    producer that only limps forever. The original premature socket-close shape has still occurred once. Record and
    investigate another occurrence of either exact shape rather than folding the two mechanisms together.
13. **WebKit engine-process crashes.** A final-tip run lost both terminal and feed WebSockets when WebKit reported
    `Network process crashed`; the helm and supervisor were still healthy. The old one-project-per-engine layout had
    kept one browser alive for all 294 WebKit cases. Playwright now starts a fresh browser for each spec file while
    keeping the one-worker shared-stack contract. The failed case passed 11 fresh-process reruns and the complete
    588-case suite, whose longest remaining browser lifetime was the 21.4-minute WebKit terminal file. If this recurs
    inside a fresh per-file project, retain the trace and treat it as new evidence. Do not fold it into the
    flood-harness timeouts or a server-side disconnect unless the matching process evidence supports that mechanism.
14. **Playwright route-handler teardown failures.** A later WebKit terminal run finished the test's assertions, then
    failed because an invalidation-driven host refresh had entered `route.fetch()` just before Playwright disposed the
    page's request context. The trace showed the second GET begin before after-hooks and its JSON read fail only after
    context teardown started. Every terminal test now unregisters its page routes after the case and waits for handlers
    already in flight; the failed case passed 21 standalone fresh-process runs and the complete 588-case suite
    afterward. A recurrence is a route-lifetime problem only if the trace again shows an intercepted request crossing
    into context teardown — do not group a WebKit process crash, a server disconnect, or a request that genuinely failed
    before teardown under this heading.
15. **Review-cap residue.** The review swarms ran a hard three-pass cap, and the test-quality lens (often docs too) was
    still producing accepted findings AT the cap on every large PR: #114 (plan), #115 (proto), #117 (auth), #118
    (spawn), #119/#120 (grouping+archive), #121 (provisioning), #122 (UI provisioning), #123 (packaging) — the cap, not
    saturation, ended those reviews. Trigger: after the stack merges and before the first real release is declared
    final, run one targeted review pass (test-quality and docs lenses) over the three largest surfaces — auth,
    provisioning, packaging — and treat a pass that returns zero accepted findings as saturation reached.

## Local/infra hygiene (not product work)

16. **Build-cache disk growth on the dev box.** `target/debug/incremental` regrows ~30 GB per heavy day and, combined
    with sccache, double-caches; it caused one disk-full incident. Recorded suggestion: `CARGO_INCREMENTAL=0` where
    sccache is the wrapper, or a periodic sweep.

## From the 2026-08 bugs-burndown stack (#151–#159; sidebar redesign + tab fixes)

Added 2026-08-15 at the close of the burndown run (its out-of-repo working log does not follow the repository; these are
the items with a future). All five burndown issues shipped; BUGS_BURNDOWN.md carries the per-issue record.

17. **Desktop cross-restart selection memory.** Browser clients remember the last-selected session across reloads
    (localStorage, a `{helm, id}` record keyed by the local host row's install identity); the v1 desktop app remembers
    only within a process, so a relaunch auto-attaches the newest-created session instead of the one last used. SPEC.md
    says exactly this, so nothing is wrong — but it was deferred purely because the webview's localStorage is not
    synchronously reachable from native Rust, not because the behavior is right. If desktop use makes the fallback
    annoying, persist the record native-side (the desktop bootstrap already owns a state dir) and restore it before the
    fallback runs in `list.rs`'s auto-select effect.

18. **Create-default can follow a reused HostId across a retarget window.** The create dialog defaults to the selected
    session's host by row id; a retarget/adopt keeps the row id while changing the machine, and within one listing
    refresh a create prepared against the stale selection lands on the successor install (the request's incarnation
    check passes because the request was built against the successor). Mitigated: selection reconciliation clears a
    vanished session quickly, so the window is one refresh interval. Closing it fully needs the helm listing to
    denormalize install identity per session so the client can bind its create default to the install, not the row.
    Raised as a definite security finding in #156's review; accepted as residual there.

19. **Archive-from-detail keeps the archived view — revisit only on complaint.** #159's review asked for detail-pane
    archive to clear the selection like the row path does. Rejected deliberately: the archived detail view
    (archived-notice + restart) is the ONLY unarchive path while archived rows are hidden by default, so clearing would
    strand the user. If include-archived browsing ever grows a proper unarchive affordance, this decision can flip.

## E2E fixture contracts (write tests against these, or lose an afternoon)

Learned during the burndown's test migrations; the helpers' docstrings in `e2e/tests/helpers/fleet.ts` carry the
details, this is the map:

- **Fabricated helm replies need three things** or the client rejects/misreads them: the `x-farhelm-build` header (probe
  the real helm for its stamp — a missing stamp latches skew and the page stops reading), COMPLETE serde enum variants
  (`HostPhase` is `tag="phase"` with required per-variant fields; a bare `{phase: "connected"}` fails decode and renders
  the compact strip's error line instead of chips), and a `matching` count (a missing `matching` reads as an old helm
  and takes the count banner's incoherence wording).
- **Auto-select changes every test's page load**: a session attaches at `goto` before any click. Tests that stage route
  holds or stubs around a specific session's first reads must `pinAutoSelect(page, otherId)` BEFORE `goto` (the helper
  fetches the helm identity and writes the keyed record — a bare id is silently ignored). `__farhelmTermReady` is true
  from load, so it no longer gates "the session I just acted on is up" — use title-based completion signals.
- **On-demand surfaces need their open helpers first**: `openRowMenu` / `openHostsPanel` / `openFilterBar` each await
  their surface's mount; bare-DOM `querySelector().click()` on a not-yet-mounted control is a silent no-op.
- **"Leaving" a session means selecting another** (no back button): bounce through the shared `e2e-session` row, and
  prove teardown via stashed handles (socket readyState, instance identity) rather than gone-entirely windows — the
  replacement mount owns the globals immediately.
- **`created_at` is second-granular with an id tiebreak**: two sessions created in the same second have arbitrary merged
  order, so "newest" assertions need a >1s gap between creates.
- **Playwright's fake clock ticks between `install()` and `pauseAt()`**: pause at a strictly future instant or a loaded
  box throws "Cannot fast-forward to the past".
