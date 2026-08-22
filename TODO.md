# TODO

A running list of things the maintainer wants fixed or built, in no particular order. This is intent, not history: an
entry is REMOVED in the same PR that addresses it, so the file only ever describes what is still wanted. It is not a
roadmap and carries no priorities unless an entry says so itself.

- Finish the macOS release bundle. The release workflow's `macos` job (workflow_dispatch-gated; builds an
  aarch64-apple-darwin `Farhelm.app` with embedded helm, managed supervisor, and the CLI at `Contents/MacOS/farhelm`)
  has never completed, which means the README's primary quickstart references an artifact that does not exist. The tmux
  floor decision (SPEC_impl.md's "Terminal substrate" section) already dropped the job's private darwin tmux build, its
  cache, and the bundle-assembly copy — the app now requires Homebrew's tmux and probes for it
  (`crates/farhelm-ui/src/desktop.rs`) — so what remains is dispatching a run and seeing the never-run darwin steps
  through for the first time: `dx bundle` and the bundle assembly's `find` for dx's `.app` output. One product gap found
  in review and not yet closed: when the managed supervisor refuses its tmux (missing, or below the floor), its message
  goes to inherited stderr and the app exits before a window exists, so a Finder launch shows nothing — the refusal the
  quickstart promises needs capturing and presenting (an alert or a minimal startup-error window) before the Mac build
  is called done. Both prior failures (23 minutes spent of a standing 180-minute macOS-runner budget) were inside that
  now-removed tmux build: attempt 1 died on ncurses terminfo installation under case-insensitive APFS (fixed —
  `--disable-db-install` + system terminfo); attempt 2 died because tmux's darwin configure demands an explicit utf8proc
  decision — deliberate upstream and still present in 3.7c/master, which auto-tries utf8proc on darwin and falls back to
  the same error. Should the private darwin build ever come back (a bundled fallback, say), the decisions already made
  for it stand: `--disable-jemalloc` (3.7c's darwin configure refuses to guess about it, as it already did about
  utf8proc; the build links nothing but its own prefix), `--disable-utf8proc` (macOS's stale `wcwidth(3)` may draw newer
  Unicode a cell off in Mac-local sessions; enabling means a fourth pinned static source, installed as an archive only
  and linked by path), plus an `otool -L` assertion that every load command is under `/usr/lib/`, because macOS cannot
  link a fully static executable, `ld` prefers a `.dylib` over a `.a` in the same directory, and a leaked dynamic
  library yields a tmux that works on the runner and dies on every user's Mac with `dyld: Library
  not loaded` — the
  darwin leg of the script's prefix isolation has never executed on a Mac. Then tag the release commit and
  `gh workflow run release.yml --ref <tag> -f release_tag=<tag>`, counting the job against the remaining 157 minutes.
  Unblocks the manual Mac checklist (`docs/manual-mac-checklist.md`) and the README quickstart.

- Make the never-started verdict say which link died. When a scoped launch dies before farhelm's exec shim, the
  supervisor's `wrapper_failure_detail` (launch_artifacts.rs) records "the agent was never started: the launch never
  reached farhelm's exec shim, so something before it — the transient cgroup scope wrapper, or the login shell itself —
  exited first", which names two suspects and separates neither. The wrapper's stderr is still sitting in the dead pane
  under `remain-on-exit`, so a `capture-pane` at classification time could say which. A first attempt (2026-08-23)
  appended the pane's last words to the durable `LastOutcome::Error` detail and was withdrawn in review for three
  reasons any retry must design around: (1) SPEC.md's terminal-retention contract — terminal content lives only as long
  as the host-side terminal, with no separate history store — which a durable excerpt of startup/rc output contradicts,
  so either the quote must not be persisted (log it, or surface it only while the pane exists) or the spec must
  authorize a bounded exception first; (2) the pane is reused across relaunches and keeps its scrollback, so a
  generation N+1 that died before printing anything would quote generation N's conversation unless the capture is fenced
  to text written after the wrapper started; (3) the ownership-and-deadness check and the capture are separate steps
  with no lifecycle claim across them on the list path, so a same-pane restart in between would quote a live later
  generation — revalidate atomically with the capture, and budget the capture so N never-started rows cannot cost N tmux
  timeouts on the hot list. The e2e harness already has `wait_for_agent_ready` (harness.rs), whose failure text shows
  the same pane text for a test's own diagnosis; that is the non-durable shape to start from.

- Review the two security-relevant resolutions made during the M7 run, and record a dated verdict (a note in
  SPEC_impl.md or a review comment on PR #117). (a) A product-spec/impl-spec conflict over where the web token is stored
  was resolved in the product spec's favor, with the impl spec amended in #117 — confirm the amendment says what the
  code does. (b) The web credential's transport was redesigned mid-run from an HttpOnly cookie to localStorage + an
  Authorization bearer header + a WebSocket subprotocol, because cookies are host-scoped rather than port-scoped:
  another local user's loopback-port server could lure the browser and replay the cookie under a forged Origin. The
  deliberate trade: giving up HttpOnly means in-origin XSS can read the credential, judged acceptable because in-origin
  XSS already has the API authority the credential grants. The review is completable from this checklist alone — verify
  that: the master token is stored recoverably server-side and `farhelm helm token show|rotate` both work after a helm
  restart; device secrets are stored hash-only; the browser credential lives in localStorage with no ambient cookie sent
  or honored anywhere; REST carries it as a bearer header and WebSockets as a subprotocol; and rotation both 401s every
  existing device session and drops already-open feed/terminal sockets. Implementation surfaces: the helm's auth and
  middleware modules, the UI's auth/api modules. Spec surfaces: SPEC.md's token section, SPEC_impl.md's auth/storage
  section as amended in #117.

- Dispose of prerelease v0.0.3-rc.1 — the release AND the tag together — once a real release exists. Both were minted
  only to exercise the release workflow (it checks out and verifies `refs/tags/<release_tag>` before building, so a real
  tag was required). Not before a real release exists: rc.1 currently carries the only published Linux artifact, so
  deleting it today would leave the README's install path pointing at an empty releases page. Deleting only the release
  would leave a tag pointing into abandoned pre-merge history — remove both (`gh release delete v0.0.3-rc.1` and
  `gh api -X DELETE repos/<owner>/<repo>/git/refs/tags/v0.0.3-rc.1`).

- Decide the reservation tombstone scope for interactive creates, then do the work the verdict leaves standing. When a
  client attaches an intent key to a create, the supervisor records it in `create_reservations` so a retried request
  returns the already-created session instead of double-launching an agent. The durability-era decision made these
  reservations PERMANENT for interactive creates (spawn's are session-bounded), which makes `create_reservations` the
  only store table that grows without bound — every interactive create ever made adds a row nothing deletes. Two
  follow-on debts, deliberately distinct: (a) digest the reservation fingerprint — rows currently retain enough of the
  original request to match retries, i.e. request plaintext (titles, cwds, invocations) retained forever; hashing bounds
  each row and ends the plaintext retention but does nothing about row count; (b) expiry/pruning — actually bounds the
  count, at the deliberate cost that a pruned key becomes reusable after the horizon (a very late retry could
  double-create). The digest half is worth doing under either verdict. If the verdict bounds the scope (session-lifetime
  — defensible, since a retry outliving the session it protects is protecting nothing), the pruning half mostly
  evaporates; that reverses a durability-milestone decision, so record the reversal where that decision lives. The store
  module's own docs describe both debts.

- Run the review-cap residue pass: one targeted review (test-quality and docs lenses only) over the three largest M7
  surfaces — auth, provisioning, packaging. During the M7 stack's reviews, the test-quality lens (often docs too) was
  still producing ACCEPTED findings at the hard three-pass cap on every large PR (#114, #115, #117, #118, #119/#120,
  #121, #122, #123) — those reviews ended because the budget ran out, not because the reviewers ran dry, so there are
  almost certainly real accepted-grade findings never surfaced in security-critical code. Treat a pass that returns zero
  accepted findings as saturation finally reached; anything it does return gets the normal fix-or-reject treatment.
  Cheap to run, and the difference between "reviewed until done" and "reviewed until the meter ran out" — do it before
  declaring the first real release final.

- Close the HostId-reuse create-default window. The create dialog defaults its host field to the selected session's host
  BY ROW ID, but a host row id survives a retarget (or an adopt where a new install takes over) while the machine behind
  it changes: look at a session on the old machine, have the row retargeted, open the create dialog within one
  listing-refresh interval, and the create lands on the successor install. The request's own install-incarnation check
  passes — the request was genuinely built against the successor — so the system does what it was told while the user's
  intent lands on the wrong machine. Raised as a definite security finding in #156's review; accepted there as residual
  because selection reconciliation already narrows the window to one refresh interval. The full fix: the helm's listing
  must denormalize install identity per session so the client binds its create default to the install the user was
  actually looking at, not the row id. Not urgent (needs a concurrent retarget plus a one-interval race), but "accepted
  residual" should not quietly become "permanent".

- Run the manual Mac checklist (`docs/manual-mac-checklist.md` — that file IS the record; its "Observed:" fields are the
  state, all "not run"). Blocked on the macOS release bundle above and a human with a real Mac. Not covered by any CI:
  Playwright's WebKit is not WKWebView.

- Reassess the host row's component-prop shape: the earlier "regroup when props are actively growing" condition has
  fired (provisioning grew it to 20 props). Only with a memoization-preserving grouping — state-only structs, never
  callback structs (the framework's callback-prop memoization does not survive struct nesting; the session row learned
  this) — and with a host-row render-count regression test like the session row's.

- UI refresh (brainstormed 2026-08-22 from screenshots of the web UI): a set of chrome tweaks to make the shell read as
  modern rather than as the "minimal M1 chrome, nothing decorative" placeholder app.css still declares itself to be. The
  terminal itself is off-limits throughout — fidelity to the agent's real TUI is the product — and the sidebar stays
  fixed-width (a recorded decision; denser rows solve the same pain). Each sub-item is its own PR; the token item goes
  first (see it for why):

  - Introduce design tokens in app.css — RECOMMENDED FIRST, before any other entry (here or elsewhere) that tweaks the
    UI's look, since every later visual change otherwise pays a scattered-edit tax. Today the file has no CSS custom
    properties and ~40 distinct hex literals inline (`#8a919e` 24 times, a long tail of one-offs that are probably
    unintended near-duplicates). Replace with a `:root` block of named roles — surface levels (`--bg-0/1/2`), foreground
    levels, one accent, ok/warn/danger, radius, and `--font-ui`/`--font-mono` — and `var()` at use sites.
    Pixel-identical refactor; audit the one-offs into roles as part of it. Same file, no framework, no build change; a
    light theme later becomes a second `:root` block.

  - Finish the JetBrains Mono conversion. The chrome font is still `system-ui` (app.css's html/body rule) and the
    vendored face is applied only through xterm's `fontFamily`; app.css's own header says nothing else uses it. The
    earlier intent was everything in JetBrains Mono. Use the already-vendored Nerd Font face for chrome too (its Latin
    glyphs are identical to upstream JetBrains Mono, and the browser has already fetched it for the terminal); do NOT
    vendor a second non-Nerd copy. If UI-size line spacing looks off, that is a `line-height` fix, not a font-file one.

  - Sidebar row hierarchy. Rows are four near-equal-weight lines (title, host, full cwd, full invocation), ~120px each.
    Drop the host line entirely when the session is on the helm's own machine (show it only for remote hosts); tilde-
    abbreviate the cwd and, if it still does not fit, hard-cut on the LEFT (right-most segments are the informative
    ones) with the full path on hover; render the invocation compactly (profile name or `claude ·skip-perms` style
    badge) rather than the full command line. Target roughly half the current row height.

  - Status as dot plus timestamp, not a word. Replace the `running`/`idle`/`exited` text with a color-coded dot beside
    the title (pulsing for running) and put a relative last-activity timestamp (`2m`, `1h`) where the word was — it
    makes the "recently active" sort visibly meaningful.

  - Surface layering. Sidebar, rows, header, and terminal all sit on essentially the same background and the selected
    row is barely distinguishable. Use two or three surface levels and a single accent (selection edge/tint, focus
    rings, primary button) — sparingly; this is stared at all day, so hover transitions stay ~100ms and the status pulse
    is the only animation.

  - Consolidate the main-pane header. Title/cwd line, archive button, restart banner, and tab strip currently stack into
    ~170px of chrome before the terminal. Fold title, cwd, status dot, and actions (archive, restart, overflow menu)
    into one ~40px row with tabs beneath; the restart explanation becomes a tooltip or dismissible inline note rather
    than a permanent band.

  - Two-tier buttons. Everything is an outlined gray box. Keep one filled primary (`new session`); everything else
    becomes ghost (no border, hover background). Per-row kebab menus reveal on hover/focus instead of sitting as eight
    identical boxes down the column.

  - Row overflow menus look like a stack of outlined buttons, floating centered over the row with no tie to the `⋯` that
    opened them; with several `⋯` in view there is no way to tell which one is open. Make it an actual menu: one raised
    surface (1px border, soft shadow) anchored to the opening button's corner, full-width left-aligned rows that
    highlight on hover with no per-item borders, a divider before the destructive `delete` (red text, no red box), and
    the `profile: …` line demoted to a small muted footer under its own divider. The opening `⋯` stays in a visible
    pressed/active state and its row gets a subtle highlight for as long as the menu is up. Check that Escape and
    arrow-key navigation work while at it.
