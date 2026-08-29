# Farhelm implementation specification

NOTE: This documents implementation _choices_ in service of SPEC.md, together with the motivation for each choice so
future changes are made with the original reasoning in hand. It is not a build plan — sequencing, milestones, and PR
breakdown live elsewhere. SPEC.md defines what the product does; when this document and SPEC.md disagree about
observable behavior, SPEC.md wins.

Standing rule: the user-facing CLI surface described here (command names, subcommands, flags a user actually types) must
be kept in sync with SPEC.md wherever SPEC.md references it. Changing a command here means updating SPEC.md's mentions
of it in the same change.

## Language and runtime

Rust throughout, async on tokio, one cargo workspace. The only sanctioned non-Rust runtime code is the xterm.js terminal
island and the thin JS interop around it (see GUI), plus Playwright test code in TypeScript.

Motivation: single language across supervisor, helm, CLI, and UI maximizes shared types and lets one test suite exercise
real components. tokio because the chosen web stack (axum, tungstenite) lives there; no exotic async needs exist that
would justify anything else.

## Workspace layout

- `crates/farhelm` — the single multi-call binary (see CLI).
- `crates/farhelm-supervisor` — session management, tmux driver, agent-kind integrations, SQLite state.
- `crates/farhelm-helm` — the host registry (SPEC.md's term: the helm's record of registered hosts and their SSH
  destinations), SSH transport, aggregation, axum API, static UI serving.
- `crates/farhelm-proto` — wire types and protocol version, shared by both ends and by tests.
- `crates/farhelm-ui` — the Dioxus application, built for web (wasm32) and desktop from the same crate.
- `crates/farhelm-desktop` — the macOS webview shell (D6): a `main` that calls farhelm-ui's desktop entry point and
  nothing else. It exists as its own package because only a package can enable farhelm-ui's `desktop` feature
  unconditionally, and it is excluded from the workspace's `default-members` so that no ordinary
  `cargo
  build`/`test`/`clippy` compiles WebKit. `-p farhelm-desktop` is consequently the only thing that compiles it.

Motivation: the proto crate is the seam that keeps helm and supervisor honestly decoupled (they meet only over the wire,
even in-process). `farhelm` remains the single multi-call artifact provisioning ever has to move — farhelm-desktop is a
second release binary for the Mac desktop alone, never provisioned to a host.

## GUI: Dioxus

Dioxus, version pinned at the workspace level, rendering the same component tree in two targets: web (wasm32, real DOM,
served by the helm) and desktop (wry webview wrapping the identical DOM). No dioxus-fullstack / server functions — the
UI is a pure client of the helm's HTTP/WS API.

Motivation: the project's standing constraints — recorded here, because SPEC.md deliberately stays
implementation-neutral and does not contain them: as much Rust as possible, one implementation for web and native, and a
GUI that agents can test visually without a human in the loop. Those force a DOM-based Rust framework. Canvas-rendering
toolkits (egui, Iced, Slint) fail the testing constraint: Playwright against a canvas is blind screenshot-diffing with
no semantic selectors. Among DOM-based Rust options, Dioxus is the most active and has a first-party desktop story;
Tauri+Leptos would mean gluing two frameworks for no clear gain. Skipping dioxus-fullstack keeps the API a first-class
tested surface (the spawn CLI and test fixtures need it anyway) and avoids the framework's most churn-prone part.

The session list's chosen ORDER is a per-client preference, kept outside the filter state. The browser keeps it in
localStorage under `farhelm.sort`, beside the `farhelm.last-selected` record, as the bare word the helm's `?sort=`
takes; an absent or unrecognized value reads as the UI default (`activity`). Falling back rather than failing matters
because that storage outlives the build that wrote it: a word a later build stored is one the helm answers with a 400,
so passing it through unchecked would leave the sidebar unable to list anything until someone cleared their browser
storage. It is written only when the control changes, so a client that never touches it never writes.

Desktop keeps both that word and the selection's same `{helm, id}` record in the bootstrap state file
`desktop-client.json`, alongside (not instead of) its two device credentials. Missing fields decode as absent so a file
from an older build remains valid. At first authentication the native side sends the two values through the existing
desktop-auth eval exchange, which seeds the SAME localStorage keys the browser uses. localStorage was chosen over a new
page-only global because it preserves one record shape, key vocabulary, and invalid-value fallback across both engines;
native Rust also mirrors the values in process memory because it cannot synchronously read the webview's storage. The
page attempts both localStorage updates before sending `ready`; storage errors are swallowed, so only the native mirrors
are guaranteed to contain the seed. `AppBody` does not mount before `ready`, and therefore the sort signal and
auto-select effect see those native remembered values on their first run — there is no frame that selects the newest
session and then corrects itself.

On a user selection or sort change, the native-rendered component updates that synchronous mirror immediately and queues
the browser-shaped record. Native coalesces a burst for 150 ms, then runs one one-shot eval that writes the matching
localStorage values and echoes the update to native; serialized batches prevent an older round trip from landing after a
newer click. Native then read-merges the acknowledged fields into `desktop-client.json` under the same state-file lock
as auth: selection cannot discard sort, and neither preference can discard credential material or the webview-auth
generation. The round trip and state-file replacement are best-effort: IPC and native-file failures are logged under
`desktop_preferences` and never surface in the UI or roll back the current choice, while JavaScript deliberately
swallows localStorage failures before acknowledging the attempt. The earlier implementation declined this round trip
because a nicety alone did not justify owning an eval channel's failure modes; desktop authentication now requires and
monitors that channel already, so preference persistence adds no new failure class, only a silent loss of next-launch
convenience when the owned bridge or disk write is unavailable. An ordinary app shutdown drains the native pending and
in-flight records directly because its webview is already going away; a hard kill can lose the last choice until the
state-file merge completes, including the debounce, eval, and blocking atomic replacement rather than only the first 150
ms.

Keeping the order out of `SessionFilter` mirrors the helm's own split, and on this side the argument is about
reconciliation rather than about caches: what a reply COVERS is keyed to the filter — whether the banner may say the
list is filtered, whether a session's absence means it left the fleet, whether an optimistic rename may be retired — and
not one of those answers can change because the same rows arrived in a different sequence. Reply ADMISSION is the one
question that does depend on both: a listing walked under the previous order is a correct list of the wrong sequence
arriving under a control that names another one, so it is refused exactly as a listing answering a stale filter is. The
UI names the order on every read instead of leaning on the helm's `created` default, which is what keeps the control on
screen and the rows beneath it from disagreeing. Changing it restarts the walk, since a cursor names a position in one
order and the helm refuses one replayed under another.

Two consequences are worth recording because nothing on screen shows either. The first is the auto-select fallback
(SPEC.md's "newest-created non-archived session", for a client with no remembered selection): it can no longer be
assumed to be the first row of the listing, because the first row is now whatever the chosen order put there. It picks
by the session's `created_at` instead, which is why that field is decoded by the UI at all, and treats a missing stamp
(an older helm) as unknown rather than as 1970 — a fleet with no stamps degrades to the listing's own first non-archived
row. When the listing is INCOMPLETE and the applied order is not `created`, even that is not enough, since the newest
session may lie past the cut; the fallback then asks the helm directly, with a one-row creation-order request under the
default filter.

The second is that a listing can now come back underfilled without any flag saying so. Sort keys are mutable — activity
advances when a session prints, a title changes when someone renames it — so a live row can cross the cursor's position
between two pages of one walk and be served by neither, while the walk ends normally with no ceiling hit and no
`next_cursor`. Only the counts show it. "Short" is therefore one predicate with three readers rather than a rule each
place restates: it is exactly the condition the count banner already prints "showing N of M" for, and the same answer
now decides whether an absence may be read as a departure (otherwise the missing row's rename is retired, its editor
closed, and — if it is the selected one — its pane torn down and replaced) and whether the auto-select fallback may
trust its prefix. A UI that tells the user its list is incomplete and then reasons as though it were complete would be
disagreeing with the one line whose job is to be believed.

What a sidebar row SHOWS was narrowed on 2026-08-23, reversing part of BUGS_BURNDOWN.md's "Decisions (interviewed
2026-08-13)" list, which called for title, status badge, host, working directory and invocation on every row. Two of
those turned out to cost a line of height each while saying nothing on the common fleet. The host line now renders only
when the session is NOT on the helm's own machine — locality is decided by comparing the session's host id against the
registry's `HostKind::Local` row, never by name. Every unknown (an old helm sending no host, a hosts read that has not
landed) answers "not local": unknown locality never SUPPRESSES an available host label, it only ever leaves the row free
to show one it already has. Legacy rows without a host name at all necessarily show none regardless — locality answers
whether a name would be shown, not whether one exists to show. The invocation is rendered compactly: the profile's
snapshotted name when the session was created from one, otherwise the program's basename plus a marker for an
unattended-mode flag (`claude · skip-perms`). The working directory is tilde-folded against the `/home/<user>` and
`/Users/<user>` shapes, since no home directory is on the wire to fold against properly. Every one of those
abbreviations is lossy, so the untouched string rides along in a `title` attribute — the row is a summary, and the full
truth stays one hover away.

The status badge's dot-or-word split (SPEC.md's Status section) is decided in Rust, in `status::status_badge`, not in
CSS: the badge carries its word on every path and a flag saying whether that word is shown or only left for a screen
reader, so the badge element's text content is the status word on every status, live or ended. That is deliberate beyond
accessibility — the browser suite's status oracles read text, and a design where a live status existed only as a color
would have cost every one of them.

The split is NARROWER than the TODO entry that asked for it, and deliberately so. That entry said "replace the
`running`/`idle`/`exited` text with a color-coded dot", naming an ended status among the words to remove; the decision
taken before the work started was to convert the LIVE statuses only. A dot is a good trade for a live status because
there is nothing to lose: `running`, `waiting`, and `idle` are one word each carrying no information the color does not
already carry. An ended status is not: its badge also carries the exit code, the "stopped by user" annotation, and the
launch shim's exec-failure detail — facts no dot can hold and no tooltip should be the only home for, since they are
usually the reason the row needs attention at all. Rendering them somewhere else on the row was considered and rejected
as spending more of the density the refresh was buying than the words cost. SPEC.md's Status section is the
authoritative statement of the resulting rule; this paragraph only records why it is not what the TODO said.

The relative age beside it needs a `now`, and there is no honest one on the wire. `last_activity_at` is written by the
session's HOST and compared against the VIEWER's wall clock, which on a remote helm is a different machine and in a
multi-host fleet is several of them at once. The UI corrects for none of that. The one reference a client could obtain —
the helm's own clock, off an HTTP `Date` header — would fix at most one of the N edges involved and would lend the rest
a precision they do not have, so the code instead refuses to print nonsense (a stamp in the future reads `now` rather
than a negative age) and keeps the raw stamp one hover away. The `Session` mirror decodes `last_activity_at` for this
and applies the proto's own fallback rule, `last_activity_at` when positive and `created_at` otherwise, copied into
`Session::effective_activity` rather than shared: this crate mirrors the HTTP contract rather than depending on proto
internals, but the helm ORDERS an activity-sorted page by its copy of that rule, so a client rendering ages by a
different one would print a column contradicting the order it was printed in. A zero means "this helm predates the
field" and renders no age at all rather than an age counted from 1970. The viewer's end of the subtraction can go
missing too — a platform clock that will not answer, or one sitting at or before the epoch — and that is carried as an
absent value rather than as a zero, because subtracting a good host stamp from a zero "now" would clamp every session in
the fleet to `now` and paint a dormant fleet as a busy one.

Ages advance on a dedicated 30-second tick — one page-wide signal, written by a component mounted beside the
invalidation feed and read by the list and the open session's header. The listing's fallback poll was the obvious thing
to reuse and is the wrong one: it runs only while the feed is DOWN, so on a healthy page it never fires at all. The
signal outlives the component that writes it, so the component republishes the current time at MOUNT before starting its
loop: it is unmounted and remounted whenever the authenticated tree is rebuilt, and without that first write the page
would spend a full tick — or, after a reauthentication that followed a long idle, much longer — rendering ages against a
reading from before the gap. The list formats each row's age itself and hands the row a finished string, which is what
keeps the tick from re-rendering rows whose displayed age has not moved — an `8h` row survives sixty ticks comparing
equal.

Styling is the single hand-written application stylesheet, `crates/farhelm-ui/assets/app.css`: plain CSS, with no
preprocessing and no framework transformation, though Dioxus still registers it as a packaged asset and decides its
served path the same way it does every other asset. xterm.js's own look is a separate vendored stylesheet
(`vendor/xterm.css`), loaded alongside app.css rather than folded into it. Its colors, font stacks, and non-zero corner
radii are declared once as CSS custom properties in a `:root` block at the top of the file and referenced as
`var(--token)` everywhere else; the rule is that no use site holds a literal, with one carve-out — a structural `0`,
where a corner has to stay square because it joins a neighboring control, names no design value and stays a literal. The
tokens are named for the role a value plays — surface levels (`--bg-*`), foreground levels (`--fg-*`), one accent
family, `--ok`/`--warn`/`--danger` with their fill and border variants, `--radius-*`, `--font-ui`/`--font-mono` — rather
than for the color it happens to be, so a restyle is an edit to one block instead of to every rule that mentioned the
same hex. The palette is dark-only today; a light theme lands as a second `:root` block redefining the same names, which
is the arrangement the no-literals rule exists to protect.

Two of those roles are design constraints and not merely names. The first is the surface ladder: exactly three levels
are in use — the ground (the page, which is pure black because that is what xterm.js paints and the terminal is not ours
to restyle), the chrome one step above it (sidebar, main header, tab strip), and the floating level one step above that
(menus, dialogs, forms, bands that interrupt a pane). Which level an element sits on is recorded in the `:root`
comments; a `--control-hover-bg` and a `--chip-bg` token fill a bordered control's hover state and a small chip
respectively, named for that specific role rather than folded into the `--bg-*` surface family, so neither one reads as
a fourth and fifth level to lay something out on. The second is that there is ONE accent, and what it may be spent on is
a closed list rather than a palette to decorate with: selection, `:focus-visible`, the one filled primary control a
surface is allowed, and any PRESSED disclosure control — a trigger wearing the accent for exactly as long as the thing
it opened is showing. That last entry covers the session row's actions-menu toggle and the header's own archive and
restart triggers alike; they are one category, not a rule plus exceptions, and the accent is what separates "this one is
open" from the hover fill every ghost control already takes. The filled-primary entry is scoped per SURFACE, not per
screen: the sidebar's resting chrome carries exactly one filled control (`new session`), and each dialog that floats
over it — create session, add a host, rename — supplies its own submit as THAT dialog's one primary, since a dialog is
read as its own surface rather than counted against the sidebar's. Everything else, on any surface, stays ghost,
including destructive actions, which mark themselves with red text rather than a red fill. SPEC.md requires the sidebar
to mark the selected session's row readably at a glance, so anything joining that list has to be a place where the
accent means "this is where you are" — the same thing the other entries say — because an accent spread across ordinary
decoration would leave nothing to make the selection readable. Both constraints have a contrast floor under them: the
quiet foreground tokens are set so that metadata stays at WCAG AA against the brightest surface it lands on, which is
what caps how light the selected row's fill may go.

`--font-ui` and `--font-mono` name the same vendored face — JetBrains Mono Nerd Font, described below in the xterm.js
island section — rather than two different ones. The chrome (`--font-ui`) and the terminal (`--font-mono`, the stack
terminal.js hands xterm.js) used to differ, chrome sitting on the platform's `system-ui` face; unifying them means
chrome-only pages that never open a terminal (the auth screen, an empty session list) now load the face too, and chrome
text reads as the same typeface as whatever the agent prints instead of pairing a generic UI font against a distinctive
monospace one. That extra load is a WOFF2 fetch rather than the vendored TTF's — a lossless re-encoding at roughly 40%
of the TTF's size — and once either surface has fetched it, the browser serves the other from cache rather than fetching
it a second time.

The open session's chrome is ONE header row — title, `{cwd} — {invocation}`, status badge, archive and restart — sized
at about 40px, with the tab strip beneath it and nothing else in the steady state. It used to be four stacked bands
costing roughly 170px before the terminal started, on a surface whose entire point is the terminal. Two of those bands
had to go somewhere rather than merely shrink. The restart offer's explanation became the restart button's tooltip and
its `aria-describedby` target: SPEC.md's "restart says so and offers that same fallback or a fresh launch" is carried by
the button's accessible name (`aria-label` and, alongside the further elaboration, `title`) — naming the offer
(`resume conversation`, `restart (fresh launch)`, `restart with the configured resume command`) rather than the action —
because the VISIBLE glyph is the compact "restart" every header action uses. The row's supported minimum width (~320px,
the main pane's own floor) has no room for the longest offer's ~320px of text on the button's face, so "says so" now
reaches a user through the accessible name and the hover tooltip rather than through the glyph itself. The archive and
restart confirmations became popovers anchored under the button that opened them, still confirm-in-place with focus on
cancel; the consequence sentence they lead with is the one line standing between a click and a killed process tree, and
a header that kept it in flow would have to either wrap or truncate it. Everything conditional — a refused restart's
prose, the archived notice, the host-unreachable notice and its last-known-status band, the "helm stopped listing this
session" line — is still a full-width band, because a band that only appears when it has something to say costs the
steady state nothing. A classified status renders in at most one place: the header normally, the stale notice's own
metadata band for a stale session (where SPEC.md's title/directory/last-known-status triple is assembled), and nowhere
at all for a session nothing has classified yet.

Every per-session action lives in one floating actions menu behind the row's `⋯`, and four decisions about it are
contract rather than styling. **Anchor:** the panel hangs below-LEFT of the toggle that opened it — its top-right corner
at the toggle's bottom-left, clamped inside the viewport — so that the toggle COLUMN of every row below stays uncovered
and clickable. The tidier flush-under-the-toggle placement was written and rejected: it puts the panel's `stop` and
`delete` exactly where the neighbouring rows draw their own `⋯`, turning a click aimed at another session's menu into a
destructive action on this one. Because the panel therefore floats over rows that look just like its own, ownership is
carried by three cues instead of by proximity alone — the toggle holds a pressed accent state, its row holds a tint, and
the panel is a raised surface with a shadow. **One at a time:** at most one row's menu is open, and it closes on any
layout change that could have moved the row it was measured against (a sidebar scroll or resize, the hosts panel or
filter bar opening, the create form, the row reordering under a refresh), because the panel's coordinates are a one-time
snapshot. **Keyboard:** it is a real `role="menu"` and behaves like one — opening it (pointer, Enter, Space, ArrowDown)
lands focus on the first command and ArrowUp opens onto the last; arrows step and wrap, Home/End jump; the whole menu is
a single tab stop via roving `tabindex`, so Tab leaves rather than walking the commands; Escape closes; and every close
that took the menu away from a focused item hands focus back to the toggle rather than dropping it on the document body.
An item made inert by an in-flight operation stays focusable and refuses on activation (`aria-disabled`) rather than
going natively `disabled`, because a browser cannot focus a disabled control and a menu that went busy under the user
would otherwise swallow every navigation key. **Confirm in place:** a destructive item swaps the panel's own contents
for the consequence line and a confirm/cancel pair with focus on cancel, rather than opening a second surface; that
sub-state is a `role="dialog"` inside the same positioned box, and it survives the panel closing, which is why it
deliberately does not answer Escape.

Clone (the row menu's newest item) reuses the create form rather than a second submit path: the click builds a
`CreatePrefill` snapshot of the row's `Session` and hands it to the SAME `CreateSessionForm`, tagged with a monotonic
generation the list view mints per click. A `use_effect` inside the form compares that generation against the last one
it applied and reseeds every field — including the raw invocation, for a profile-backed clone too, since the command
input is merely disabled while a profile is chosen, not emptied, and leaving it stale would surface an unrelated command
the moment the user switches modes — whenever the two disagree; comparing generations rather than mere presence is what
makes cloning the SAME row twice in a row reseed a second time, since an unrelated rerender of that effect (a host
reconnect, a catalog refresh) must not overwrite an edit in progress. The profile choice is trusted only when the row's
own profile snapshot is `Present` — the catalog still holds that id under the SAME name — which is deliberately STRICTER
than an ordinary create's remembered-default rule (an id that merely still exists, under a new name, is not evidence
that cloning it again is what today's catalog would still offer); every other answer falls back to the raw command.
Trusting the id at all is still a snapshot decision, not a live one: submitting a profile-backed clone resolves that id
against whatever definition the catalog holds at that moment, exactly like any other profile-backed create.

The clone's host is put through the SAME install-identity comparison SPEC.md's ordinary creation default uses (a
`HostId` is a registry row that outlives a retarget or an adopt) before either the selector or the agent choice trusts
it; a row whose install this client cannot currently confirm is left at the ordinary default with a note explaining why,
rather than risking a stale command — or, worse, a colliding starter-profile id — landing on a successor install. That
identity check is not a one-shot gate: `CloneHostState` (`list::create_form`) tracks it across renders so a clone opened
before the FIRST hosts read lands keeps retrying once the registry answers, instead of giving up permanently because the
form's separate text-field reseed only ever runs once per clone generation; and a clone whose host DID pass the check is
re-checked on every later pass, withdrawing the selection back to the ordinary default the instant a retarget or an
adopt changes the installation behind it while the form stays open. A row that names no host at all (a session from a
helm too old to report one) is treated as permanently unconfirmable rather than retried: its agent is left unresolved
with its own note, since there is no install to check at all and applying a raw command or a profile id sight-unseen
onto whatever host the ordinary default picks could run it on a machine the row never named. An explicit host or agent
interaction takes the decision away from all of this automatic reconciliation outright, for the rest of that clone
generation.

A cross-host clone that DOES pass the identity check still cannot select its host and apply its agent choice in the same
render: the form's own profile catalog is scoped to the LIST VIEW's chosen-host effect, which only catches up one render
pass after the clone moves `chosen_host`. The agent choice is queued (bound to the target's full install fingerprint,
not merely its host id, so a retarget landing during the wait cannot be mistaken for the install the choice was actually
queued for) and applied the instant the catalog's target matches it — on the SAME render when the clone's own host was
already current, and on a later one otherwise. Because that handoff spans a render the user can act inside of, any
explicit host or agent interaction cancels a still-queued choice outright, so an answer given before the handoff catches
up is never silently overwritten once it does.

A clone's working directory, invocation and title are peer-relayed text (SPEC.md's clone rule copies them off another
session, and a remote supervisor under `--ssh` is the one this client does not control) going into editable controls, so
they get the profile editor's escaped-display / raw-seed / edited-flag treatment (`profiles::submitted_field`) rather
than being written in raw: shown escaped while untouched, so a directional override or an invisible character cannot
make the field say something different from the bytes a submit would send, and an untouched submit still sends those
ORIGINAL bytes rather than the escaped spelling on screen.

Known risks, accepted deliberately:

- API churn between Dioxus 0.x releases. Mitigation: pin, avoid internals, budget for migrations.
- Desktop is WKWebView on macOS while tests drive Chromium (no usable WebDriver exists for WKWebView on macOS).
  Mitigation: the tested surface is the web build; desktop-only glue is kept as small as possible and is the one
  manually-verified path.
- Clipboard and drag-drop are where WKWebView diverges from Chromium, and paste interception is a headline feature. The
  default is the same DOM paste/drop event path on both targets — WebKit does deliver file/image data on those events,
  but with documented engine-specific restrictions (pasted-HTML sanitization, gesture gating on the async clipboard
  API), so the honest framing is "same event model, engine differences expected", not "same as Chromium". No special
  desktop solution is built until a real deficiency shows up in our actual flows; native-side wry hooks are the known
  fallback. One concrete thing to check early rather than debug late: wry's own file-drop handling swallows DOM drop
  events unless configured not to. (The browser path is a secure context on loopback, so web clipboard APIs are fully
  available there.) Also established the hard way during M2 dogfooding: wry implements NO native JS dialogs on macOS —
  `window.confirm()` silently does nothing — so any confirmation or prompt the UI needs must be in-page DOM, never a
  browser dialog. SPEC.md's confirmation language is deliberately mechanism-agnostic; this is the constraint that picks
  the mechanism.
- Blitz (Dioxus's native renderer) is not production-ready. The plan assumes webview desktop indefinitely; nothing may
  depend on Blitz landing.

## Terminal widget: xterm.js island

The terminal is xterm.js, vendored as a static asset (no CDN — the UI must be fully self-contained, consistent with
SPEC.md's no-public-relay, no-third-party-services posture and the loopback deployment), mounted as a JS island inside
the Dioxus tree. PTY bytes flow WebSocket → `term.write()` directly, bypassing Dioxus state entirely. Dioxus owns
everything around the terminal (tabs, status, dialogs), not the terminal's content path.

JetBrains Mono Nerd Font is vendored alongside xterm.js for the same self-contained reason, and terminal.js sets it as
xterm's `fontFamily` — but it is no longer terminal-only: `app.css`'s `--font-ui` token (see the design-tokens paragraph
above) applies the identical vendored face to the rest of the chrome, so the whole app reads as one typeface. Chrome and
terminal share the same two cached `.woff2` files rather than each vendoring its own copy; whichever surface asks first
pays the fetch, and the other reads it back from the browser's cache.

Motivation: xterm.js is the only battle-tested embeddable terminal (VS Code) and full escape-sequence fidelity is a
SPEC.md requirement. Routing high-frequency PTY output through a reactive framework would be a performance disaster, so
the bypass is load-bearing, not an optimization. A pure-Rust wasm terminal (alacritty_terminal grid + canvas renderer)
was rejected: it is a project in itself and reintroduces the untestable-canvas problem inside the most important widget.

The bypass alone is not sufficient (audited): `term.write()` is non-blocking with a hard ~50MB buffer that silently
discards beyond the cap, and xterm.js parses at roughly 5–35 MB/s while a PTY can produce far faster. The terminal path
therefore carries end-to-end backpressure — write-completion callbacks drive watermark pause/resume messages over the
WebSocket, and the supervisor throttles its pane reads accordingly. Interactive agent output never approaches these
rates; `cat` of a huge file must degrade to slow, never to silent data loss. Precisely (sharpened while planning M2.5,
when the original sentence met tmux's actual flow-control mechanics): no code Farhelm owns may ever drop a terminal byte
— every Farhelm-side bound is backpressure or a visible detach, never discard.

What "degrade to slow" is allowed to slow includes, on one of the two tmux behaviors below, the AGENT's own writes for
the duration of a viewer's pause. SPEC.md's stall bullet states that bounded-slowdown contract directly, and this
document's job is only to record the mechanism: nothing here throttles the agent deliberately, and the block is bounded
by the flow-control window and ultimately by the stall detach. The permission is deliberately left standing even though
the supervisor no longer takes it up (see the session sink, below) — it is what keeps the layers above free of any
assumption about which way tmux answered.

The producer-side bound is tmux's, and tmux implements it in one of two ways. With `pause-after` set on the supervisor's
control client, a client that stops reading gets EITHER of these (audited 2026-07-29 on 3.3a, 3.4, and 3.7b, both with a
standalone control client and through the full supervisor stack):

- **tmux throttles the pane.** It stops reading the PTY, the agent's own `write` blocks, and nothing is queued or
  dropped. On resume, delivery continues from exactly where it stopped — a genuine end-to-end degrade-to-slow, with no
  recovery needed.
- **tmux reads ahead into history and pauses the client's stream.** The agent free-runs into scrollback (tmux server RSS
  stays flat), the bytes queued for the stalled client age past `pause-after`, and tmux then cuts that client's stream
  with `%pause` and discards what it had queued for it. Recovery is replay from retained history, exactly like a
  reattach.

Which one happens is NOT a property of the tmux version — an earlier draft of this paragraph claimed it was, and the
audit does not support that. All three versions were observed taking both paths across repeated identical trials; the
deciding factor is how far tmux happens to have read ahead of the client at the moment it stalls, which in turn depends
on how fast that client was consuming beforehand. Both paths satisfy the contract, so nothing above this layer may
depend on which one occurs, and the supervisor implements both (it honors `%pause` whenever it arrives and simply keeps
reading when it does not).

The first path is nonetheless one the supervisor now prevents from arising against a Farhelm session, and the reason is
the multi-terminal shape tabs introduced rather than any change of heart about degrading to slow. tmux stops reading a
pane when no attached client is able to consume it, and that judgement is about the PANE, not about the stalled client's
own terminal — so once a session has several terminals, a stalled viewer on a background tab could block the agent's
writes, which is a very different bargain from a viewer slowing the terminal it is itself looking at. Two further
measurements sharpened it (2026-08-02, tmux 3.4 and 3.7b): the block is not bounded by `pause-after` at all (observed
persisting for a full 45-second window, ending only when the stalled client went away), and it reproduces only at high
output rates, which is why an audit can honestly report it as intermittent. So every session with a live attachment now
also carries one always-drained control client of its own — a session sink — whose only job is to be somebody tmux can
always deliver to. With it attached, only the second path remains reachable, and the per-terminal clients additionally
turn the session's other panes off for themselves (`refresh-client -A <pane>:off`), which is safe only because the sink
is there to keep those panes readable. Nothing above this layer changes: `%pause` is still honored whenever it arrives,
and code may still never assume which path tmux took.

One qualification belongs with that claim rather than in a footnote, because it is the only hole left in it: a sink is a
process, and a process can die. From the moment one does until its replacement has attached — a process spawn and one
control-mode round trip, retried with exponential backoff capped at a few seconds, forever, for as long as any terminal
of that session is attached — the session's terminals still have their foreign panes filtered off with nothing holding
those panes readable, so a pane nobody is watching can stop being read for that window. The window is bounded by the
backoff cap and is not otherwise defended against: closing it entirely would mean keeping a second sink permanently
attached to every session, paying a certain cost against an uncertain one. An attach that arrives during such a window
waits for the sink to come back rather than installing filters into it, which is the one case where the gap must not be
allowed to widen.

The xterm.js scrollback capacity is therefore sized to at most the tmux history floor (both currently 12,000 lines) — an
invariant tests must pin — which makes the replay-based catch-up's end state observably equivalent to lossless slow
delivery: every byte still within the terminal's own retention is present, and bytes beyond it would have been evicted
from scrollback even had they been delivered one at a time.

Outbound key delivery has one non-obvious mechanic: for input small enough to fit one protocol frame and one `send-keys`
command, message boundaries are pty write boundaries. The transport frames input at 32KiB and the supervisor chunks each
frame into `send-keys -H` commands of at most 256 bytes, so larger input is deliberately split — nothing may depend on
atomic delivery of an arbitrary message. But a message at or under one command IS one command, and tmux flushes each
command to the pane as its own write — measured on the pinned 3.7b: two commands landed as two reads ~7ms apart, never
coalesced. For almost all input none of this matters, but SPEC.md's Shift+Enter chord (ESC CR, two bytes, always one
frame and one command) is exactly the sequence where the boundary is meaning: split across two writes it reads as
Escape-then-Enter to boundary-sensitive line editors, which is how the chord shipped broken for Codex while bash's 500ms
readline timeout hid the split. The implementation therefore merges the pair into ONE message at the source
(`shift-enter-key.js`, whose header owns the full design rationale): the chord's keydown arms the merge and returns
`true` so xterm's own Enter path — scroll-on-input, selection dismissal, hidden-textarea cleanup, accessibility — runs
untouched, and the `term.onData` wrapper prepends the pending ESC onto the `\r` xterm emits synchronously for that same
keystroke. The arm's lifetime is that one synchronous dispatch (expired in a microtask queued at arm time), so a fizzled
chord leaves no stale arm behind and the ESC is never flushed alone: a stray lone ESC is the byte shape the merge exists
to never emit.

## Terminal substrate: private tmux server

Each supervisor runs a dedicated tmux server on a private socket (`~/.local/state/farhelm/tmux.sock`) with a locked-down
generated config: status bar off, `history-limit` sized to SPEC.md's replay floor, `remain-on-exit on`. One tmux session
per Farhelm session; window 0 is the agent terminal in practice, additional windows are the terminal tabs. Neither is
identified by position: the supervisor stamps each window it creates with a tmux user option — the agent's window with
the session id, a tab's window with a minted tab id that is also that tab's whole record. The agent terminal is
identified by its durable pane record first, with the marker as the recovery aid for a session whose record is empty;
tabs have no durable record at all and are rediscovered from their markers alone, because a pane's own processes inherit
`TMUX` and can conjure windows a positional scan would adopt. The user's own tmux usage and config are untouched.

Farhelm requires tmux at or above a version FLOOR that is, by policy, the exact release the output-client teardown
regression suite (`scripts/test-tmux-pinned-shutdown.sh`) runs against — 3.7c as of this writing, pinned in
`.github/release/source-pins.env`, with the supervisor's floor constant tested to equal that pin so the two cannot
drift. This replaced the original "any tmux ≥ 3.3" policy on 2026-08-22 (the decision record lived in TODO.md until the
floor shipped). The original policy treated versions above 3.3 as interchangeable, and experience said otherwise: the
supervisor's driver is full of behavior audited per version (3.3a, 3.4, and 3.7b each differ in ways that shaped real
code), a production distro 3.4 server died in BUGS.md's `fatal()` abort shape on 2026-08-19, and BUGS.md records the
same abort class reproduced on distro 3.6. The floor is therefore DESIGNED to exclude many current distro packages
(Ubuntu 24.04 ships 3.4, 26.04 about 3.6, Debian 13 3.5a; some Fedora releases already ship 3.7c): on Linux this is
close to always-bundled in practice. Always bundling was considered and rejected all the same: it loses distro security
patching of tmux and libevent (where the 2026-08-16 3.7b segfault lived), concentrates a bad build's blast radius on
every host at once, needs a static build per platform (the darwin one has never completed), and a from-source install
has no bundle anyway — so the version check is the policy and the bundle is one way of meeting it. Versions above the
floor are accepted in tmux's own release spelling (`major.minor` plus at most one patch letter); a version newer than
the pinned one earns a one-time warning that it is unaudited, never a refusal. Anything the parser cannot read exactly —
`next-3.8`, release candidates, distro-decorated strings — is refused rather than guessed about: a version Farhelm
cannot name is one nobody audited it against.

How the binary is chosen: the supervisor selects its tmux program once at startup — `--tmux <path>` on
`farhelm
supervisor run`, else `FARHELM_TMUX` from its environment, else the bare name `tmux` — and every invocation
goes through that one value. A bare name is resolved against `PATH` by the operating system at each spawn, as it always
was; only the spelling is fixed, and the refusal message reports the `PATH` entry it would resolve to. Whatever was
chosen is version-checked and refused by name (binary path, version found, floor) when too old. The check is applied
TWICE, because a tmux server outlives the supervisor by design: once to the client executable before any server is
started, and once more to the server the supervisor adopts on a socket that already has one — the server is the
component that holds sessions and the component that has crashed, so a 3.7c client driving a pre-floor server left over
from before an upgrade is exactly the case the floor exists for. A below-floor adopted server is refused without being
killed: the message names the socket, both versions, and the floor, and the operator drains or kills that server
deliberately. The override is "you own the substrate": a way to run something newer or differently built, not a
supported configuration. Linux releases bundle a private musl tmux build per architecture, which provisioning installs
only when the host has nothing acceptable (Linuxbrew's tmux is the documented alternative); either way the unit
provisioning writes names the accepted binary through `FARHELM_TMUX`, so a private build left behind by an earlier
installation cannot shadow a host tmux that was accepted later. The Mac app bundles none: the desktop app takes an
ambient `FARHELM_TMUX` as given, otherwise probes `/opt/homebrew/bin`, `/usr/local/bin` (Homebrew on the two
architectures) and `/opt/local/bin` (MacPorts) in that order — because GUI apps do not inherit the shell `PATH` — and
hands a hit to its managed supervisor through `FARHELM_TMUX`; with no hit it sets nothing and the supervisor's ordinary
`PATH` lookup applies. Homebrew's tmux is the recommended way to meet the floor on a Mac, not the only one the code
accepts.

The desktop app additionally owns a PREFLIGHT of its own: before it spawns its managed supervisor — never before, and
never for a supervisor it merely discovers already answering — it probes the SAME candidate it is about to hand that
child through `FARHELM_TMUX` and applies the version floor itself (`farhelm_ui::desktop::run_tmux_preflight_or_exit`). A
missing or below-floor result prints one plain stderr message naming what was tried and how to fix it, then exits 1,
before the state directory holds anything beyond the bare private directory itself (created up front so discovery's own
probe has a valid path to operate against), before the embedded helm starts, and before the managed supervisor is ever
spawned. Any OTHER probe failure this preflight has no tailored wording for (a permission-denied spawn, a nonzero `-V`,
unparseable `-V` output) falls through to the desktop's ordinary bootstrap-error path instead. Discovery runs FIRST
specifically because an answering supervisor is an ownership boundary: it may be driving a perfectly good tmux selected
by its own `--tmux`, its own `FARHELM_TMUX`, or a login-shell `PATH` this Finder-launched process never sees, and
refusing startup over a dependency this process does not need would reject a supported setup. The supervisor still
performs its OWN two floor checks after being handed a tmux this way (the client-executable probe and the adopted-server
check described above) — the desktop's preflight is a user-experience improvement specific to the one path where this
app starts the substrate itself, not a replacement for the supervisor's own checks, which remain the authority for the
case this preflight cannot see: a private server already running on the socket from before an upgrade.

Historical note on what the floor made moot: below 3.7 the supervisor warned once at first attach and lost bracketed
paste restoration (`bracket_paste_flag` arrived in 3.7), and on 3.3a `capture-pane -N` dropped trailing styled padding
from a stop snapshot's dead-pane frame (found 2026-07-29 during M2.5's 3.3a validation). The first is a fallback the
driver still carries (a missing `bracket_paste_flag` format) that the floor makes unreachable; the second was old tmux's
own behavior under the same `capture-pane -N` the driver issues today, not a Farhelm code path. Both are recorded here
so nobody rediscovers them as bugs.

tmux is a headless PTY holder and history store. The supervisor's only client is a non-rendering control-mode client
(`tmux -C`, the interface iTerm2's tmux integration is built on; `pipe-pane` is the fallback shape). Sizing (audited on
tmux 3.7): a control-mode client is an attached client, but tmux ignores it for window sizing until it declares a size
via `refresh-client -C` — the supervisor never declares one, so tmux ignores it for sizing entirely, and geometry comes
from explicit `resize-window` calls tracking the attached GUI client's dimensions (`resize-window` sets
`window-size manual` on the window it touches, which is where that setting comes from). NOTE: setting
`window-size manual` globally in the config crashes the tmux 3.4 server outright — the version Ubuntu 24.04 ships — so
it must stay out of the generated config; the two mechanisms above make it redundant anyway. The supervisor streams raw
pane output to the client; input goes in as `send-keys -t <pane> -H <hex bytes...>` commands written to the same
attached control-mode client's stdin that streams that attachment's output. An earlier design tried `load-buffer -` over
stdin followed by `paste-buffer -d -r` instead, specifically to keep input bytes off a process's argv (see below) — and
had to be abandoned: verified empirically against tmux 3.7b, `paste-buffer` caret-escapes control bytes on the way into
the pane (DEL arrives as the two literal characters `^?`, ESC as `^[`, ctrl-C as `^C`), silently breaking backspace,
arrow keys, and ctrl-C. Keystrokes are not pastes, and no `paste-buffer` flag changes that. `send-keys -H` delivers
bytes verbatim instead (also verified against 3.7b) and keeps the security property that motivated stdin delivery in the
first place: hex-encoded input never touches a process's argv, because it rides an _already-running_ process's stdin
rather than a freshly spawned `tmux send-keys` command's arguments — the earlier concern was a spawned process's argv
being world-readable via `/proc/<pid>/cmdline`, which matters because input includes credentials typed at agent prompts,
and that risk never applied to bytes written to a pipe. Each `send-keys` command is chunked at 256 bytes because tmux
rejects a command carrying on the order of ~1000 arguments as "command too long" and each input byte becomes one hex
argument; every command's `%begin`/`%end` reply rides back on the same stdout the client's other notifications use,
which is safe to ignore because the output-streaming loop already discards every notification it has no use for (see
below). Passthrough sequences (audited): the control-mode pane-output stream carries `\ePtmux;...\e\\`-wrapped payloads
still wrapped, regardless of the `allow-passthrough` option — that option only gates forwarding to rendering clients,
which Farhelm has none of — so the supervisor unwraps passthrough payloads itself before they reach xterm.js. Reconnect
replay prefills xterm.js from `capture-pane -e` history, then continues with live bytes from the same control client —
that is how the 10,000-line floor is met without a gap between the two. The handoff ordering is load-bearing: a separate
tmux command process targets the incumbent control client by its tmux-assigned name and switches it back to `no-output`;
only after that process succeeds is the incumbent's stdin closed and the process reaped. The acknowledgement cannot
share the output client's protocol stream because cancellation may leave older positional command replies unread there.
tmux applies `no-output` by discarding all pending pane blocks for that client and refusing new ones, so this is a
client-wide boundary rather than a racy list of panes that existed when teardown began. Closing or killing tmux 3.7b's
client while one of those blocks remains can abort the whole private server with `fatal: not enough data`; the
acknowledged transition is therefore part of the handoff contract, not cleanup polish. Pane modes, a history snapshot, a
visible-screen snapshot, and a final `refresh-client -f !no-output,pause-after=N` are submitted as one
semicolon-separated command group through that replacement. The matching `%end` for the final refresh block is the
cutover: earlier pane bytes are represented by the snapshot, later ones arrive as live output, and `no-output` advances
rather than queueing a second copy for delivery. Normal-screen replay selects the history snapshot; alternate-screen
replay selects the visible snapshot so normal history is not mixed into a full-screen app.

Setting `pause-after` on that same cutover (M2.5) changes the dialect the client then reads, which the parser must
handle rather than discard: pane bytes arrive as `%extended-output <pane-id> <age> ... : <data>` instead of `%output`,
and `%pause`/`%continue` notifications appear. Both output dialects are accepted unconditionally and decoded
identically, including across a switch mid-stream, because the passthrough decoder carries state between notifications.
`%pause` is acted on — it means tmux cut this client's stream, and the dropped bytes are recoverable only by replaying
history — while `%continue` is discarded like any other chatter, since it arrives inside the reply block of the command
that requested it and nothing waits on it. The extensible-argument rule matters too: everything between the age and a
lone `:` field is reserved for future tmux versions and is skipped by scanning for that separator field rather than by
counting fields, so a future argument cannot silently shift the payload.

This boundary was checked against tmux 3.3a, 3.4, and 3.7b with `scripts/check-tmux-cutover.py` under a continuously
busy pane. The corresponding tmux source has the same ordering in all three versions: one input line appends the
complete command group, synchronous `capture-pane` and `refresh-client` commands drain from that queue before the server
loop returns to pane reads, and `CLIENT_CONTROL_NOOUTPUT` advances the client offset instead of queueing a backlog. Each
command still produces its own `%begin`/`%end` block, so the parser keeps their numeric identities and does not declare
the stream live at an earlier block. A command `%error`, partial EOF, timeout, output before cutover, or missing
matching marker fails the attach rather than replaying an incomplete snapshot.

Content alone is not enough: pane modes (alternate screen, bracketed paste, mouse reporting, application cursor keys,
cursor position) are read from tmux pane format variables and re-synthesized into xterm.js after the prefill — without
that, a reattached full-screen agent silently loses paste bracketing and mouse reporting. Replay remains bounded by
tmux's retained history. One deeper tmux limitation also remains: `capture-pane` serializes rendered cells, not an
in-progress terminal escape parser. If reconnect lands after tmux has consumed only a prefix of one escape sequence, the
snapshot cannot serialize that hidden parser state for xterm.js; a later application repaint repairs the display.
Farhelm does preserve split printable output and keeps its own passthrough decoder across live pane-output notification
boundaries (resetting it only when a `%pause` catch-up abandons the stream it belonged to). The supervisor enforces
SPEC.md's one-attachment rule itself.

Exited-session semantics: `remain-on-exit on` keeps dead panes viewable per SPEC.md, and exit codes come from the dead
pane's status. Exec failure versus ran-and-died cannot be told apart by exit code alone (a missing command yields 127
and a non-executable file 126 — both indistinguishable from a program exiting with that code), so classification does
not rely on exit codes: the shell execs `farhelm internal launch`, a shim that always exists, which resolves and execs
the profile invocation and, on exec failure, writes a sentinel with the errno detail to a per-launch status file (named
by session and launch generation, so a sentinel left by a failed earlier launch can never describe a later relaunch)
before exiting. The supervisor classifies **error** on that sentinel; the one sentinel-less error path is a
cgroup-scoped launch whose `systemd-run` wrapper died before the shim ever ran, recognized only by its full evidence
shape (dead pane, launch spec still unconsumed, no sentinel) so it can never claim an agent that actually started. NOTE:
a sentinel written by the shell after a failed `exec` was audited and rejected — interactive bash survives a failed
exec, but zsh terminates on it in every mode, so shell-side code after `exec` never runs for zsh users; the shim works
identically under any `$SHELL`.

Motivation for never rendering through a normal attach: a rendering tmux client takes over the outer terminal on the
alternate screen and draws everything itself, which kills native scrolling — xterm.js would accumulate no scrollback,
wheel scrolling would need tmux `mouse on` copy-mode with tmux-flavored selection UX, and the capture-pane prefill would
land in a buffer the alt screen makes unreachable. Streaming raw pane bytes instead lets xterm.js own scrollback,
selection, and search natively; inner alt-screen apps (vim in a tab) still render correctly because their escape
sequences pass through in the stream; and mouse-reporting apps still work because xterm.js's mouse sequences are
forwarded as input. tmux's own UI (status bar, prefix key, copy-mode) never appears anywhere.

Motivation: tmux delivers exactly the hard guarantees SPEC.md makes — processes and terminal state survive supervisor
restarts, scrollback retention, screen re-render on reattach — with a decade of hardening, and the approach is validated
by herdr. The rejected alternative (per-session Rust holder daemons owning PTYs plus our own terminal-grid engine for
replay) buys independence from tmux at the cost of owning a terminal state machine's bugs; not a v1 trade. The
supervisor's internal terminal interface stays narrow (create, attach-cutover, resize, input, kill) so a Rust holder
could replace tmux behind it later without touching anything above.

Consequence to keep in mind: tmux sits in the escape-sequence path. Fidelity issues (new terminal features, passthrough
sequences) get debugged at the tmux layer first; the generated config is the knob.

## Helm ↔ supervisor transport: system ssh + stdio protocol

The helm shells out to the user's `ssh` binary (tokio::process), one ControlMaster per host (`ControlPersist`) so
interactive latency stays low and reconnects are cheap. The supervisor is reached by executing `farhelm internal stdio`
on the remote side, which proxies stdio to the supervisor's unix socket. Supervisors listen on that unix socket only —
no network port, exactly as SPEC.md requires.

The ssh child's stderr is piped and relayed as bounded, control-escaped tracing events attributed to the host, not
inherited. Inheriting is defensible for the single-host path a user started by hand; for a registered host it hands a
remote party a direct, unbounded write channel to the operator's own terminal, escape sequences included. The relay
drains continuously (a full stderr pipe would wedge the child), caps each line, stops logging after a per-connection
budget while still draining, and Debug-escapes what it does log — the same treatment the supervisor gives tmux's exit
reasons, for the same reason. Peer-supplied error text is normalized the same way wherever it is logged or retained in a
host's state, and repeated identical failures stop being logged after a few, with the suppressed count reported by the
next different one — a host that is down, or a peer that errors on every refresh, must not be able to write the log
indefinitely.

The hello's two free-text fields carry generous LENGTH caps checked at handshake decode (256 bytes each): both are
retained for the connection's whole life by the peer's counterpart, so an unbounded one is a memory cost a peer chooses
for the other side and re-chooses on every reconnect. Over-long is refused, never truncated — a shortened identity would
map two distinct hosts onto one claim. Length only, never shape: an identity is opaque to every consumer by design, so a
format check would invent a compatibility rule nothing else has.

The ssh argv puts its option terminator (`--`) BEFORE the destination, and that placement is a security boundary rather
than a style choice: a destination is user-supplied text, and one shaped like `-oProxyCommand=...` is read by OpenSSH's
own option parser and executed locally — a command injection with no ssh connection involved at all — for as long as the
terminator sits anywhere after it. The registry additionally refuses option-shaped destinations when they are
registered, so the user gets a clear error rather than a puzzling ssh failure; the argv ordering is the actual guard,
since it also covers callers that never go through the registry.

On top of that byte pipe: a multiplexed framing protocol — length-prefixed frames carrying a channel id and a type tag;
control messages are serde_json, terminal data channels are raw bytes. The same protocol runs over the unix socket
locally, so "local host" and "remote host" differ only in transport. Connection setup exchanges protocol and build
versions; it refuses protocol-version incompatibility per SPEC.md's version-skew rule, while build versions travel for
diagnostics only — mixed builds with a compatible protocol are the normal steady state SPEC.md describes.

Motivation: SPEC.md promises provisioning and transport ride "the user's keys, agent, and config" — only the real ssh
binary honors ~/.ssh/config fully (ProxyJump, Match blocks, agent forwarding, ControlMaster). russh was rejected for
exactly that: partial config support would quietly break the promise. JSON control frames keep the protocol debuggable
by eye; raw binary data channels keep PTY throughput off the JSON path.

`SessionInfo` carries a `last_activity_at` (unix seconds) beside `created_at`: the last time the supervisor observed
that session's agent pane change. It was added WITHIN protocol version 11 rather than bumping it, per the running rule
every version since 3 has followed — a new optional field with a decode default, whose omission a receiver can ignore
harmlessly, is additive; a new tagged variant or a required field is not. Absent means the sender predates the field and
decodes to 0, which a receiver reads as "unknown, fall back to `created_at`" and never as an instant in 1970. This is
recorded here because the per-version changelog in `lore/` is frozen at the moment it was written and is not maintained
as the protocol grows. The identity hook's pair — `ControlMsg::ReportConversation` and `ConversationReported` — went the
other way and took the protocol to version 12, since two new tagged variants are exactly what an older decoder refuses
outright instead of ignoring.

Version 13 adds the one shape on this wire that travels UPWARD as a request: `ControlMsg::AgentRequest`, answered by
`AgentResponse`. Both legs of its journey carry the same pair. An agent inside a session dials its own supervisor's
socket with the per-session credential — exactly as `farhelm spawn` does — and the supervisor forwards the request to
the helm, because the session has no route, address, or credential back to the machine the helm runs on. Nothing else
changed direction: the helm still learns about sessions by drain, so version 10's "no supervisor-edge push channel"
holds for everything but this. The supervisor picks the helm that holds an attachment to the asking session — well
defined by the one-attachment-per-session rule, and by construction the helm the user is looking at, which is also the
rule that stays correct if several helms per supervisor are ever supported. Request ids are per connection on this
protocol and stay that way: the asking process numbers its own leg, the supervisor numbers the upcall from a counter it
keeps per helm connection, and the relay holds the mapping for one round trip.

The failure vocabulary is two kinds, split by whether a retry is free. `ErrorKind::Unavailable` means nothing is holding
the request: no helm is attached, its connection died before or during the request, the request could not be delivered
onto that connection at all, or the helm itself is not in a state to answer — still starting up, shutting down, or
serving a connection with no fleet behind it. `ErrorKind::Timeout` means the opposite and only that: the request was
QUEUED for delivery on the helm's connection and no answer arrived within the supervisor's budget. It may or may not
have reached the helm — the supervisor observes its own writer queue accepting the frame, never the writer transmitting
it — so a retry is neither provably free nor provably duplicative, and the message says exactly that rather than
claiming a delivery nobody watched. Closing that residual gap needs a per-frame transmission receipt from the
connection's writer, with the answer budget starting at the receipt and a write failure before it reported as
`Unavailable`; that is the known refinement, deferred because the queue is one `mpsc::Sender<Frame>` shared by every
attachment site in the supervisor and a receipt changes that type at all of them. Both kinds are emitted by the relay
and by the helm; the earlier rule that only the supervisor emits them was too narrow, because the helm has its own
transient states and a bare `Internal` would tell a caller nothing about retrying. The DELIVERY leg gets its own short
budget (5 seconds) separate from the helm's answer budget (30 seconds) precisely to keep this distinction honest: a
request that spent its whole budget waiting for room on a full writer queue was never sent, and reporting that as
`Timeout` would invert the one thing the two kinds exist to say. Both budgets live on the supervisor because it is the
only party that can tell them apart; the asking CLI blocks with no deadline of its own so that the specific answer
reaches it.

The trust boundary is the CONNECTION, not the message. The supervisor authenticates the per-session credential and
refuses a peer asking as a session it is not; from there the helm accepts the forwarded `session_id`, and the claim that
the connection it arrived on belongs to that session's host, without re-verification — it never sees the credential, so
there is nothing on its side to check against. That is sound because a full-authority supervisor connection is the
helm's own provisioned install, holding complete authority over every session on its host: a helm that could not trust
it for a fleet-wide read could not route a single operation to it either. What the helm does check is that the
connection is still the CURRENT one for that host row, since registry rows outlive the machines behind them. So 13 is
the current protocol version, and the frozen changelog stops at 11.

The two read verbs are answered from the helm's own listings, narrowed to what an agent can name and act on. Two
narrowings are contractual rather than incidental. The session listing is drained by cursor under two ceilings — a fixed
row cap (5,000) and a cumulative encoded-byte allowance across every page it walks (6 MiB, leaving the reply's envelope
room under the 8 MiB frame limit) — and carries a `truncated` flag when either cuts it, because a partial fleet listing
is otherwise shaped exactly like a complete one and "that session does not exist" would be indistinguishable from "that
session is past the cut". The byte allowance is the load-bearing one: the pagination it drains from applies its own
budget per page, so rows alone bound nothing about the assembled reply, and a fleet of legally fat records would
otherwise produce an answer no frame could carry — discarded whole, reaching the agent as `Internal` rather than as the
partial listing the verb promises. It includes archived sessions, flagged, since an agent has no archive switch to flip.
And the per-session `agent` field is a non-secret label — the source profile's snapshotted name, or the invocation's
program basename — never the raw command line. Users put credentials in command lines, this listing is readable with any
one attached session's credential, and its reader is a model that will quote what it read, so arguments must not cross
this wire at all.

The session list is cursor-paginated on this wire (protocol 8, M6). The contract, recorded here because the milestone
plan that settled it is history the moment M6 closes: pages walk a total order — creation time descending, session id
ascending as the tiebreak — over columns that never change for a live session, so an issued cursor stays a valid resume
point for as long as its holder cares to use it. The cursor is opaque: it encodes the last-returned entry's ordering
key, callers store and replay it verbatim, and an undecodable cursor is refused as an invalid request. A decodable
cursor is simply an ordering key, and deliberately so — cursors carry no authority in a single-user supervisor, any
well-formed key is a valid resume position (which is also what lets a deleted session's cursor resume cleanly), so there
is nothing a forged cursor obtains that honest paging does not. Resumption is strictly-after, which is also what makes a
cursor whose own session was deleted resume cleanly. Under concurrent mutation the walk promises no duplicates and no
tearing, and deliberately NOT completeness: a session created mid-walk can land behind the cursor (same-second creations
tie-break by id; a clock rollback places new sessions mid-order) and is only guaranteed by the next walk from the start.
Both page cuts — the count limit and the frame-size budget — carry the same continuation cursor, so truncation is a
resumable state rather than M2's terminal flag, and the reply's total keeps reporting the full count before any cut. One
case is neither cut: a record too large to ship even alone (nothing bounds a title below the byte budget) is an explicit
error, never a fake exhaustion — an empty page with no cursor would otherwise claim the walk was done when a session
remains that can never be represented on any page.

## Supervisor internals

- State in SQLite (rusqlite) at `~/.local/state/farhelm/supervisor.db`: sessions and their metadata (SPEC.md's
  supervisor-authoritative list), agent profiles and each session's profile snapshot taken at creation (SPEC.md's
  snapshot rule shapes the session schema), spawn idempotency keys, captured conversation identities, host identity, and
  the boot id last seen. Comparing the stored boot id against the current one (`/proc/sys/kernel/random/boot_id`;
  `kern.bootsessionuuid` on macOS — a per-boot UUID, chosen over `kern.boottime` because the kernel rewrites boottime on
  clock steps and a boot id must never change mid-boot) is how "interrupted" is classified per SPEC.md.
- Host identity: generated once at first run, stored in the db.
- Agent profiles live in a `profiles` table in that same db, bounded on both axes — 128 profiles per host, 8 KiB of
  caller-supplied text per profile — so the unpaginated catalog reply can never outgrow a frame. That bound is not
  tidiness: the listing is also how a client finds the profile it wants to delete, so a catalog too large to list would
  be one nobody could trim back. The starter profiles SPEC.md promises (Claude Code and Codex, each in a plain and a
  permission-skipping "yolo" variant, four rows total) are seeded by the schema migration that creates the table, not by
  a check at startup. A migration step runs exactly once per database, so a deleted starter stays deleted and an edited
  one stays edited, with no "already seeded" flag that could disagree with the table it describes and re-seed what the
  user threw away. A profile names its kind explicitly (`generic` is the spelling for "no kind"), and an absent resume
  template means the kind's own default, derived at create time from that profile's invocation. Substituting `{cwd}`
  into an invocation or a resume template happens in the single spawn seam (`spawn_agent`), after validation and before
  hook injection, because the value is the directory tmux is handed for that launch and that seam is the only place that
  knows it on every path — create, retry, and every restart mode alike. A session records the id and name of the profile
  it was created from and nothing mutable; whether that profile still exists, and whether it has since been renamed, is
  derived by one catalog lookup when a reply is built (one per reply, not one per session), so an edit or a delete never
  rewrites historical rows and there is only one copy of existence truth. Creating a session from a profile that has
  since been deleted fails as a precondition — before any launch, with no session left behind — and never falls back to
  another profile.
- Sessions launch through the user's shell as an interactive login shell inside the PTY —
  `$SHELL -l -i -c 'exec farhelm internal launch ...'` as the window's command, with the shim doing the final exec of
  the profile invocation (see exited-session semantics) — evaluated per launch. The `-i` is load-bearing, by different
  mechanisms per shell (audited): zsh sources `.zshrc` directly when interactive; bash login shells never source
  `.bashrc` themselves under any flags — only the profile chain — and `-i` matters because it puts `i` in `$-`, so the
  stock Debian/Ubuntu `.bashrc` interactivity guard doesn't bail out when the profile chains it. Either way the sourced
  file set matches an SSH-and-type session, which is the contract. When `$SHELL` is unset (user-manager services on
  systemd older than 255 don't set it), the supervisor falls back to the passwd database, then `/bin/sh`.
- Status heuristics: periodic sampling of tmux pane activity and captured tail content, sharpened per agent kind (see
  below). Sampling must never sit on the attach/input path — SPEC.md forbids status from gating interaction. The
  supervisor's own ticker takes the samples; classification is a pure read of the sample beside the durable outcome, and
  sits BELOW the recorded-error and dead-pane rules in the existing precedence, so a heuristic can only ever choose
  among the live statuses. The generic baseline is observed output alone, counted in a session's OWN samples rather than
  in elapsed time: three consecutive samples showing an unchanged screen reads idle, anything else live reads running,
  and a session not yet sampled twice reads running since that is what a session that just launched is. Counting samples
  rather than seconds is load-bearing — the sampler works through live panes on a budgeted round robin, so a session's
  real sampling period grows with the fleet, and any wall-clock window would eventually report a continuously-working
  agent as idle because the HOST was busy. Waiting is never derived from activity at all (a blocked agent and a finished
  one are equally quiet); it comes only from per-kind sharpening.
- Last-activity timestamp: the same ticker that samples for status also DATES the changes it sees, into a
  `last_activity_at` column on the session row and onto the wire. It is the ordering key a "most recently active"
  session list needs, seeded to the session's creation time so one that has never produced output sorts by age rather
  than landing at the epoch, and restored verbatim on supervisor restart. Persisting it does not contradict the rule
  that liveness is never persisted: a status is a claim about NOW and rots the instant the process it describes moves
  on, while this is a claim about a past instant that the passage of time cannot falsify. The two must not be conflated
  in the other direction either — classification still reads sample COUNTS and never this clock, for the
  population-dependence reason above. The value advances only when the observed change is at least a minute newer than
  what is already stored, and the reason is blast radius rather than resolution. Two costs, scaling differently: a
  durable `UPDATE` per session per crossing, which without the quantum would be a write per busy session every two
  seconds; and a fleet-wide UI wake, which is COALESCED — the helm detects a changed session by comparing whole
  serialized `SessionInfo`s, but bumps the invalidation feed at most once per host refresh that found anything
  different, however many sessions moved. So the wake is bounded per refresh while the writes are bounded per session,
  and without a quantum a single busy agent would re-render every connected client on every drain. No user distinguishes
  two sessions whose last output was twenty seconds apart. Writes are monotonic in SQL as well as in memory, so a
  backwards clock step cannot walk a visibly busy session down the sort; a lost write costs sort precision until another
  observed change crosses the quantum, and nothing else.
- Agent-kind integrations live in the supervisor as a small trait (`AgentIntegration`; `AgentKind` is the wire enum
  naming the kind itself): status sharpening over the sampled tail, and conversation-identity capture. Sharpening is a
  DEFAULTED trait method that may only promote a live baseline to waiting, never invent liveness, and never panic on
  arbitrary terminal bytes; the default is "no sharpening", which is deliberately different from the no-integration case
  (generic sessions still get the baseline). Recognition is conservative by design — a vendor question phrase AND a
  rendered menu of numbered answers, both at the bottom of the screen — because a status that reads waiting at a working
  session teaches users to ignore the column, while a missed prompt merely reads idle. Claude Code: watch
  `~/.claude/projects/<munged-cwd>/` for the session record. Audited specifics that shape this: the record appears at
  first prompt submission, not at launch, so correlation keys on first-input time and tolerates an unbounded
  launch-to-first-input gap; the cwd munging is non-injective (`/`, `.`, `_` all become `-`); and per-line JSON fields
  (sessionId, cwd, timestamps) are the reliable correlators — file birth times can postdate content after rewrites.
  Codex: same approach against `~/.codex/sessions` rollout files. An identity is claimed only when the correlation is
  unambiguous — two near-simultaneous launches in one cwd stay uncaptured, which triggers SPEC.md's explicit fallback
  instead of a silent wrong guess. Plain resume appends to the existing record under the same id for both agents
  (audited on current versions; a new id appears only on explicit forks — `--fork-session`, `forked_from_id`), so a
  captured identity survives restarts; the watcher treats appends as the resume signal and cheaply re-verifies identity
  after each restart rather than baking in either behavior. Re-verification is a scan-only affair, because only a
  scan-derived claim carries the record locator an append can confirm. A hook-reported identity carries none and is
  never re-verified — nothing on disk can improve on the agent's own answer — so it simply stays durable, and the next
  launch's own hook reports again from inside the new process. The scan is no longer the only identity source, though it
  is still the only one that works without vendor cooperation — see the hook paragraph below.

  **The per-launch identity hook.** Scanning cannot see a conversation being replaced inside a live process: Claude
  Code's `/clear` and Codex's `/new` both mint a new conversation id with nothing on disk pointing back at the record
  they replaced, so a scan-derived identity keeps resuming the conversation the user just threw away. Both vendors fire
  a `SessionStart` hook whose payload carries that id, and both accept a hook supplied on the command line for a single
  launch, so farhelm appends itself as that hook (`farhelm internal hook`, reporting over the supervisor's one shared
  `supervisor.sock` and authenticating with the per-session credential the launch already carries) and lets the agent
  state its own identity. Claude takes it as `--settings <json>`; Codex takes
  `--dangerously-bypass-hook-trust -c features.hooks=true -c hooks.SessionStart=…`. Per-launch is the whole point:
  nothing is written to `~/.claude` or to Codex's active configuration home (`$CODEX_HOME` when set, `~/.codex`
  otherwise), no trust state is left behind, and flags cannot outlive the process they were passed to — which is what
  keeps SPEC.md's no-agent-configuration rule intact rather than merely bent. The costs are accepted deliberately, and
  both are scoped to the launches that actually carry the injected flags rather than to Codex launches in general: on
  those, Codex prints a hook-trust warning line above its composer, and with trust bypassed any hook the user has in
  that same configuration home but has not trusted runs too. Codex fires `SessionStart` at the first prompt rather than
  at process start, so a Codex session's identity arrives only once the user has typed something, where Claude's arrives
  at startup. And three invocation shapes disqualify a launch, which is skipped with a logged reason rather than made to
  work: an argv that already carries `--settings` (Claude honors only the last one, so injecting ours would silently
  drop the user's), an argv already steering Codex's own hook configuration (a second bypass flag risks a rejected
  command line, and the `hooks.`/`features.hooks` tables are the user's once they touch them), and — for either vendor —
  an argv containing a bare `--` (our flags would become prompt text). `FARHELM_AGENT_HOOKS` in the supervisor's
  environment — `all`, `none`, or a comma list of kinds — turns injection off wholesale or per kind, read once at
  supervisor start and carried as a seam value. The scan is untouched by all of this and remains the fallback wherever
  no report has been accepted — an unhooked launch, but also a hook that failed, timed out, or was refused; it is never
  the override. A reported identity dominates every scan-derived state, the ambiguous verdict included, because it is
  not evidence about which record is ours — it is the agent's own answer. `docs/agent-hook-injection.md` is the
  user-facing account of the same mechanism.
- Per-session spawn credential: random token in the session's environment (`FARHELM_SESSION_ID`,
  `FARHELM_SESSION_TOKEN`, socket path), checked by the supervisor on the unix socket.
- Process-tree ownership (SPEC.md's stop/reap promises): killing the tmux pane is not enough — tmux signals the
  foreground process group, and daemonized descendants escape it. M2 ships the portable sweep: enumerate the pane's
  descendants by walking /proc PPIDs, unioned with a scan for processes whose environment carries the session's
  `FARHELM_SESSION_ID` marker (which catches daemons that already reparented to init), then SIGTERM, a short grace,
  SIGSTOP-quiesce, re-enumerate, SIGKILL — with process start-time validation so a recycled pid is never signaled.
  `systemd-run --user --scope` cgroup scopes layer on top as the Linux hardening (M3): where a functional systemd user
  manager exists — probed once by actually running a trivial transient scope and then showing, killing, and confirming
  the collection of it, not by `which`, and through absolute binary paths so a login shell's `$PATH` cannot substitute
  what the probe approved — each launch is wrapped in its own generation-named scope (audited on systemd 255: the
  wrapper execs in place, so the pane's process tree, exit codes, and liveness checks see exactly the unwrapped shape),
  the per-launch SELECTION is recorded durably as a boolean while the unit name is re-derived from session id plus
  generation at every use (a stored name would let a tampered row aim a kill at another session's unit), and stop kills
  through the scope first — SIGTERM, the same grace the sweep gives, SIGKILL, then confirming the unit was actually
  collected, because `systemctl kill` returning only proves delivery. The sweep ALWAYS runs afterwards as the backstop,
  and is the whole mechanism where no user manager exists — a missing manager never degrades stop below the sweep's
  guarantees, and neither does a broken one: the sweep's verdict is the answer, and the scope's troubles are diagnostic.
  A wrapper that fails runs before the shim can write its exec-failure sentinel, so the supervisor classifies that shape
  (a launch spec nothing ever consumed, on a dead pane, for a scoped launch) as **error** rather than letting it
  masquerade as a plain exit.

  **What the cgroup does and does not promise.** It targets ACCIDENTAL daemonization — the dev server, MCP server, or
  build watcher that double-forks and execs away its environment marker, which is exactly the shape the sweep provably
  cannot find. It does NOT contain a deliberately adversarial descendant: one that runs `systemd-run --user --scope` on
  itself migrates into a sibling unit under the same user manager, and with its marker scrubbed is then invisible to
  both mechanisms (reproduced, not theorized). Containing that needs a delegation boundary — a parent slice the
  supervisor owns, with the manager refusing migrations out of it — which v1 does not build and SPEC.md does not
  promise. Agent descendants run with the user's own privileges by design, so a descendant determined to outlive its
  session can always arrange to; the honest claim is that stop reaps what a normal program leaves behind. macOS has no
  /proc, so the three reads the sweep needs — the same-euid process table, one pid's parent/start-time/zombie state, and
  one pid's environment — go through a platform seam that answers them with `sysctl` there (`KERN_PROC_ALL`,
  `KERN_PROC_PID`, and `KERN_PROCARGS2`) and with /proc on Linux; every decision above it, and therefore everything stop
  promises, is one shared implementation. The Mac marker source is deliberately narrowed to the environment region of
  `KERN_PROCARGS2` with argv discarded, so that neither platform can claim a process for marker text that merely appears
  on its command line. One Mac residual on top of the shared ones: macOS 26+ withholds that environment region for Apple
  platform binaries even from a same-uid parent (observed on real hardware, pinned by a macOS-only test), so a
  reparented descendant exec'd into `/bin/sh` or another platform binary escapes the marker scan there; the PPID closure
  still reaps it while it remains in the pane's tree. The planned close is a session-id membership channel — tmux panes
  are session leaders and a SID survives fork, exec, and reparenting — deferred until the gap proves to matter in
  practice. See lore/2026-07-27-m2-process-tree-stop.md for the alternatives as they looked when this was decided.
- Attachments land in `~/.local/state/farhelm/attachments/<session-id>/`, deleted with the session. There is no size cap
  in v1: the bytes are the user's, on the user's own machine, and every hop streams them under a credit window, so a
  large file costs time rather than memory. A disk that fills up is therefore a failed upload with nothing published and
  a visible error, never a truncated file at the published path.
- The rest of the state directory: `supervisor.sock` (the unix socket that is the supervisor's only doorway — mode 0600,
  inside a 0700 directory, because reaching it means running commands as the user), `tmux.sock` and `tmux.conf` for the
  private tmux server, and `launch/` holding one 0600 JSON spec per session. A launch spec carries the agent's full
  command line, which users put credentials into, so the shim unlinks it as soon as it has read it, creation removes it
  if the session never starts, and the supervisor sweeps leftovers at startup.
- Symlink TOCTOU hardening of the state directory is intentionally absent. The directory create, chmod, lock, socket,
  and sweep operations are plain path-based calls that follow symlinks; making them airtight means `O_NOFOLLOW` opens,
  dir-fd-relative operations, and ownership verification throughout. Exploiting the gap requires write access to a
  parent of the state directory — the user's own home — and an attacker with that already runs arbitrary code as the
  user, so the rewrite buys nothing against any attacker this tool could plausibly face. Decided won't-fix during the M1
  review. Revisit only if the state directory ever moves somewhere group- or world-writable. (The one place symlink
  safety is load-bearing anyway, launch-spec creation, uses `O_EXCL` and is safe.)

## Helm internals

- State in SQLite at `~/.local/state/farhelm/helm.db`: host registry (SSH destinations, host identities), last-known
  session cache (survives helm restarts per SPEC.md), recoverable web token, hashed browser device sessions, remembered
  defaults (last-used profile per host).
- The host registry (PLAN_M6.md item 3) reserves one row for the machine running the helm itself: auto-created at `open`
  if absent, never user management surface, never removable. It exists specifically so the local host has a cache row to
  serve stale sessions from when its own supervisor is down — the plan's first draft made this row optional, and review
  caught that a row-less local host would have nowhere to cache into, breaking the very promise (stale sessions survive
  a down host) the cache exists to keep. An SSH row also carries optional `remote_farhelm`/`remote_state_dir` fields
  (the argv fields M1's `--remote-farhelm`/`--remote-state-dir` carried), `None` meaning "use the remote's own default",
  not "unset for now". Two distinct SQL mechanisms enforce two distinct invariants here, not one: the `hosts` table's
  own `CHECK` constraint is what pins the local row's NULL destination/remote-field shape, while separate partial unique
  indexes enforce at most one local row, uniqueness of an SSH row's destination among SSH rows, and — see below — at
  most one row claiming any given host identity.
- A host's `host_identity` is `NULL` until first contact ever succeeds for that row — including the local row, which is
  minted with no identity and learns one the same way any other host does. Recording it is split into two operations so
  silent identity merging is structurally impossible at the storage layer (SPEC.md: never silently merge): first contact
  writes only when the stored identity is still `NULL`, or is a no-op when it already matches what was just reported: a
  DIFFERENT stored identity is refused outright, changing nothing, with the mismatch surfaced as a value the caller acts
  on rather than an error. Adoption is a separate, explicit compare-and-swap that only a user's adopt choice may invoke.
- At most one registry row may hold a given identity, and that is a SCHEMA invariant (a partial unique index), not a
  check the connection manager performs before writing. The difference is not stylistic: with a check-then-record shape,
  two entries reaching one freshly installed supervisor can both see no twin and both record, and at the next helm start
  each sees the other as its twin and both freeze as duplicates — so a live host appears zero times. First contact and
  adoption therefore resolve the claim inside the same transaction as the write, and a loser gets a typed outcome naming
  the row that holds it, which the manager renders as the ordinary duplicate state. Databases predating the constraint
  are resolved by its migration: the lowest host id keeps the claim, later rows are demoted to unclaimed and lose the
  cache that was only meaningful under it, so they re-learn at next contact and freeze as duplicates properly.
- SPEC.md's install-bound create default rides on a denormalization of that recorded identity: every session listing and
  detail row carries `host_identity` (the registry's value for the row's host), the UI snapshots it together with the
  host id at selection time, and the default holds only while the row still reports the same identity — the identity,
  not the client-side fingerprint or the connection token, because only it changes exactly when the install does (an
  address-only retarget or a reconnect must not evict the default). The key is serialized even when `null` so a client
  can tell "records no identity" from "predates the field", and only the latter degrades to the row-id-only comparison.
  The identity is a second read beside the session snapshot and cannot be atomic with it, so both assembly paths read
  the registry FIRST: a retarget straddling the reads then yields a stale identity on fresh content — mismatch, safe
  fallback — rather than a fresh identity on stale content, which would be a false match onto the wrong machine. The
  identity-less residual SPEC.md accepts shows up here as `null == null` passing; a host frozen in the identity-mismatch
  phase is disqualified outright, since its recorded identity still names the predecessor.
- Both identity writes also carry the connection-defining configuration the attempt was DIALED under (destination,
  remote farhelm, remote state dir) and are refused if the row no longer matches. A hello that crossed the wire while
  the user was retargeting a row describes the old endpoint, and committing its identity under the new configuration
  would durably attribute one machine's identity to another. Tearing down in-flight attempts on an edit narrows that
  window; checking the dialed configuration inside the write's own transaction closes it.
- Each host's session cache is replaced wholesale on every successful list refresh — delete then insert in one
  transaction, never a partial mix — so a session dropped from a host's live list is dropped from its cache too, and
  ordering never depends on parsing every cached row's JSON (created_at and session id are extracted as columns at write
  time). Removing a host cascades its cache rows (SPEC.md's disposal rule). Adopting a new identity at a known
  destination purges that host's cache in the same transaction as the identity write: the old identity's cached sessions
  describe a dead install, and carrying them forward under the new identity would misattribute one install's history to
  another. A cache write also carries the identity it was produced under, checked against the stored value in the same
  transaction: this closes the window where a refresh already in flight when a user adopts a new identity could land
  after the adoption's purge and repopulate the cache with the dead install's sessions by a side door. Reads of the two
  tables disagree on purpose: a cache row that no longer decodes is skipped and logged rather than failing the read (it
  is last-known display data, not authority), while a corrupt registry row still fails `list_hosts` loudly (the registry
  is authority for which hosts exist at all).
- One connection actor per registry row, the local row included (PLAN_M6.md item 4), each owning its transport
  connection, its reconnect state machine, and its slice of the session cache. A row's connection is always in exactly
  one of six states, and the last three exist because folding them into "unreachable" would throw away the only
  information that makes the situation fixable: **connecting** (active retries in progress), **unreachable-reprobing**
  (the active window is spent, background probes continue forever, no give-up), **connected**, **version-skew** (the
  hello was answered and refused; carries both protocol versions, the peer's build, and the remediation text, since
  SPEC.md demands actionable rather than merely diagnostic errors), **identity-mismatch** (frozen, carrying both
  identities, connecting nothing until the user adopts or fixes the destination), and **duplicate** (this entry's
  identity is already another entry's; connects nothing while it stays one, so the HOST appears exactly once under the
  twin while the entry stays visible as something to resolve). The local row's unreachable state additionally
  distinguishes "no supervisor is running on this machine" from a generic transport failure, because that is the one
  case whose remedy is a command on the machine the user is already sitting at — a manual-path hint, never an offer to
  install (provisioning is M7's). A seventh state exists that is not about the host at all — **retired** — for an entry
  whose actor has stopped: a panicked task, or one that outlived its own registry row. Without it, an actor's last
  published status stands forever after the actor is gone, so a task that died mid-connection would leave the entry
  reading connected, with a routable client, and nothing left running to ever correct it. Each actor is therefore
  supervised by the task the manager actually holds, which publishes the retired state (client dropped) when the actor
  it wraps finishes for any reason other than being cancelled on purpose.
- A host's state and its live connection are read TOGETHER, from one borrow of the actor's published status. The pair
  has an invariant — a client exists exactly while the state is connected — and session routing is built on it, so two
  separate reads straddling a transition would let a caller refuse an operation against a host that is up, or route one
  onto a connection that is already gone.
- Shutting the manager down is terminal: the flag it sets is checked in the same lock hold that reconciliation does its
  insertions in, so a reconcile that read the registry just before the shutdown becomes a no-op instead of repopulating
  the map with actors nothing can stop.
- Cadences (user decision 2026-08-04, "snappy"), all injectable so tests can drive real transports without waiting out
  production timescales: active retries wait 1, 2, 4, 8, 15 and 30 seconds between attempts — an immediate attempt plus
  six, spread over about a minute — and then background re-probing takes over at 45 seconds, forever. The whole point of
  those numbers is that a host which comes back is noticed within about a minute of returning, while a fleet of down
  hosts costs a little over one connection attempt per host per minute. A connected host's session list refreshes every
  3 seconds, matching the UI's own poll interval, so multi-host aggregation does not make the visible list staler than
  the single-host path already is. The two regimes are distinct rather than one repeating ladder: a re-probe is a SINGLE
  attempt, and a fresh active window is granted only where something changed — startup, a connection that was up and was
  lost, or the resolution of a freeze. A re-probe also leaves the host's existing state alone while it dials, so an
  entry that has been unreachable overnight reads as unreachable instead of flickering into "connecting" every 45
  seconds. Version-skewed and duplicate entries ride the same 45-second cadence: the first so an upgraded host
  resurfaces by itself, the second to re-ask the registry whether the collision is still there. Identity-mismatch is
  deliberately the one state with no timer at all, because no amount of waiting answers a question only a user can.
- Two DEADLINES bound what a peer can do to an actor by saying nothing, both injectable alongside the cadences. One
  connection attempt (dial and hello together) is bounded at 20 seconds, and expiry is an ordinary failed attempt so the
  ladder and the re-probe cadence carry on unchanged; one cache refresh is bounded at 30 seconds, and expiry drops the
  connection so the actor re-enters its normal loss handling. Without them a transport that accepts and then goes silent
  parks the whole state machine indefinitely while every layer below looks healthy — no error, no EOF, and a host that
  reads as connecting or connected forever.
- Editing a registry row's connection-defining fields RECONNECTS the host rather than waiting for its current connection
  to end on its own: the connection is torn down, a non-connected state is published together with the new row (so a
  hosts list can never pair an edited destination with the old connection's state), and the actor gets a fresh active
  window, which is the same treatment resolving a freeze earns. An explicit retry is the same restart without the fresh
  window — a user clicking retry is not evidence that a down host is back, so it makes one attempt and returns to the
  re-probe cadence, while a connected host's retry is a genuine reconnect rather than an early poll.
- A cache write refused because this connection's identity is no longer the row's also ends the connection. Every later
  refresh on it would be refused identically, so keeping it up would show a host as healthily connected while its stale
  list silently stopped advancing; dropping it re-asks the identity question against the row as it now stands.
- A connected host's cache refresh is drain-then-replace: follow the supervisor's `next_cursor` to exhaustion, then
  replace that host's whole cache slice in one identity-bound write. The page limit is left unset so the supervisor
  applies its own default cap, which is sized so an ordinary host's entire list arrives in one page — that matters
  beyond round-trip count, because the supervisor's conversation-capture sweep rides the `ListSessions` handler and
  therefore runs once per page, so a smaller limit would multiply whole-host scans for every host on every refresh. A
  failed refresh records the failure and keeps the previous cache, never wiping it: the cache's whole job is to answer
  "what did this host have, last we knew" while the host is unavailable, so clearing it on failure would destroy the
  answer exactly when it becomes the only one available, and would make a transient failure look identical to "this host
  genuinely has no sessions". A host whose supervisor reports no identity at all connects and serves live but writes no
  cache, since the identity binding has nothing to bind to. The walk's termination is never the peer's to decide: it is
  bounded by pages followed, by sessions accumulated (ten of the supervisor's own default pages), and by a refusal to
  follow a cursor identical to the one that produced it — each catching a shape the others cannot, and all three landing
  as an ordinary failed refresh that keeps the previous cache.
- The served session list is a MERGE, and it is served from what the helm has already recorded rather than from the
  hosts. Every connected host's actor drains its supervisor's paginated list to exhaustion into helm.db; the list
  endpoint then merges what is there — live hosts' latest refresh and down hosts' last-known entries alike — into one
  order, tagging each row with its host and marking it stale unless that host is connected right now. A host being
  connected changes only that flag, never where its rows are read from.
- The list is served in one of THREE orders, chosen by the request (`?sort=`, defaulting to `created` when absent so
  every client written before there was a choice keeps its behavior; an unrecognized word is a 400, like an unknown
  status). `created` is `created_at` DESCENDING, then session id ascending, then HOST ID ascending — the first two are
  the wire's own total order and the third is what keeps the merged order total even in a database whose one-owner index
  is absent. `activity` leads with the session's effective activity stamp descending, and `title` with its collated
  title ascending; both then fall into that same creation-order tail. Every order therefore ends in the same total
  order, which is not decoration: a cursor over an order that left equal ranks unordered can skip one row and repeat
  another when two of them swap between page fetches. The effective activity stamp is `last_activity_at` when the sender
  supplied one and `created_at` when it did not, so a session that has produced no observed output — or whose supervisor
  predates the field — sorts by its creation time rather than piling up at the epoch. The title collation is Rust's
  `str::to_lowercase` compared as code points: Unicode's locale-independent FULL lowercase mapping (it can lengthen a
  string, as `İ` does), which is neither SIMPLE lowercasing nor case FOLDING — so `ß` and `SS` stay distinct keys. The
  result is case-insensitive and otherwise ordinal, deliberately not locale-aware (that is an ICU dependency and a
  per-user setting this milestone does not have), and deliberately not SQLite's `NOCASE`, which folds ASCII only and
  would case-fold half of a non-English user's alphabet. The folded key is CUT to 128 characters, because it is also an
  index key on every cached row and the leading component of a cursor the browser replays in a query string, and the
  only bound on a title anywhere is the supervisor's 64 KiB create-field cap. Cutting is sound only because it happens
  where the key is minted rather than where the cursor is written: the cut value IS the row's position, so two titles
  sharing a 128-character prefix tie and fall through to the shared tail, exactly as two identical titles do.
- Keyset cursors are stable under insert and delete because the key they name does not move — and under `activity` and
  `title` that is no longer unconditionally true. `created_at` is immutable, so the old order was immune; an activity
  stamp advances and a rename rewrites a folded title, so a row can cross the cursor between two page fetches of one
  walk and be shown twice or not at all. There is deliberately no snapshot or generation machinery against that: it
  would mean per-walk server state and an expiry policy, for a list the user re-reads constantly anyway. Two things
  already in place bound the damage instead. The supervisor advances an activity stamp at most once a minute
  (`ACTIVITY_STAMP_QUANTUM`) rather than per byte of output, so a row can only cross that rarely; and the UI's own
  coherence checks re-read the whole list when the numbers stop agreeing, saying so on screen ("the list changed while
  it was being read; refreshing"). A duplicated or missed row is a stale read of a visibly moving list, not a lost
  session.
- Both new ordering keys are denormalized into `session_cache` columns extracted at write time (schema version 11,
  backfilled from the existing payloads, with one HOST-LEADING index per new order — version 11 adds two ordering
  indexes, bringing the total to four for the three logical orders). `created` alone keeps a global/host-leading pair,
  inherited from before the page query became per-host; `activity` and `title` get only the host-leading index, because
  by the time they were added nothing would ever read a global one. The host-leading index is what a page actually
  walks, at every fleet size and under every order: an `IN`-list of more than one host makes SQLite abandon a global
  ordering index and sort the whole cache into a temp b-tree, so the page query is written as one single-host SELECT per
  host merged by `UNION ALL`, and each branch is a range scan over that host's slice. That was true of `created` before
  the other two orders existed; it is fixed for all three together. Same reason `created_at` and the archive flag are
  columns: an ORDER BY must not mean decoding every payload in the fleet. The activity column stores the EFFECTIVE value
  so the fallback rule is applied in exactly one place, while the payload keeps the raw field untouched — a synthesized
  value written back would be indistinguishable from an observation at the next merge. The title is folded in Rust
  rather than by a SQL collation so that the one definition of the collation also serves the in-memory rows an ordered
  page is merged with. Nothing new is needed for change detection: both columns are derived from the payload on the way
  in, so a session that produced output or was renamed already flips the cache's changed flag and reaches open clients
  on the invalidation feed.
- A cursor is bound to its ORDER exactly as it is bound to its filter, and replaying one under a different `sort` is a
  400 telling the caller to start a fresh walk. A resume point names a place in one sequence; applied to another it
  resumes mid-list and silently drops everything that sorts before it. The token carries the order it was taken in and
  only the key components that order compares, so a creation-ordered cursor omits both new components and stays
  fixed-size (version 3 grew it by the order word and the longer domain tag, not by anything that scales with a peer's
  text) while a title-ordered one pays for the title only where the title is what it is ordered by; a token naming an
  order without its leading component is refused rather than defaulted, since every default is itself a real position in
  the order — where that position falls differs per order, so the damage is a silent skip or a silent repeat depending
  on which. The cursor's domain tag is versioned for exactly this, so tokens minted before the order was named fail
  cleanly.
- Ordering is not filtering, and the two are kept apart: a sort changes the sequence, never the membership, so neither
  `total` nor `matching` moves with it and the helm's per-filter matching-count cache stays valid across a sort change.
- One host does not fit the cache rule and cannot be made to: a supervisor reporting NO identity, against a registry row
  that has none on record either, has nothing for the identity-bound cache write to bind to. Its refreshes are kept in
  the connection manager's memory and merged into the list and the owner lookup from there; they serve while it is
  connected and vanish when it is not, because with no durable copy there is nothing to stand behind. A row that HAS a
  recorded identity meeting an identity-less hello is a different situation entirely and fails closed — see below.
- The REST list is paginated with a helm-level cursor that is deliberately DECOUPLED from the wire cursor underneath it.
  Composing per-host wire cursors into the REST cursor would tie one browser page fetch to N live host round trips, so a
  single flapping host would break a page walk that has nothing to do with it and a slow host would set every page's
  latency. Draining into the cache first makes the REST cursor a plain resume point over local data — an opaque
  base64url-JSON ordering key, resuming strictly after the last row a page returned, so pages are stable under
  concurrent creation and deletion for the same reason the wire cursor is. The decoupling is enforced rather than
  intended: the helm's cursor carries a domain-and-version tag and spells its key's components differently, so neither
  decoder can read the other's tokens — without that they were byte-compatible and each silently resumed at a position
  the other had named.
- The page is a PAGE all the way down. The resume predicate and the limit go into one indexed query against helm.db's
  merged-order index, so a poll reads and JSON-decodes only the rows it returns; the alternative, loading the whole
  fleet's cache per request, made a full walk quadratic in the fleet's size. Two independent cuts apply, mirroring the
  supervisor's own list discipline: the caller's limit (capped, with an over-large request refused rather than silently
  clamped) and an encoded-byte budget that shrinks a page of fat records rather than oversizing the reply. The ordering
  key carries the host id as its final component so the order is total even where the one-owner index below is absent —
  a cursor over a non-total order can skip or repeat rows.
- A listing reply carries two counts, and they answer different questions. `matching` is how many rows satisfy the
  caller's filter across the whole merged view, and it is present exactly when a predicate is active. `total` is how big
  the VIEW is — the denominator the UI's "N matching of M sessions" prints — and it deliberately does not move when the
  user types, because a denominator that tracked the filter would compare a number against itself. The one thing that
  does move it is the archive-inclusion switch: that switch selects which list is being served rather than narrowing
  one, so the default view's rows and its total are both about the non-archived fleet and `include_archived=true` widens
  both. The flag is denormalized into a `session_cache.archived` column (schema version 10, backfilled from each
  payload) for the same reason `created_at` is: counting must not mean decoding every blob. A row whose payload no
  longer decodes stays inside the `total` of whichever view its stored flag names, and outside `matching` in both, which
  is what keeps "showing 4 of 5" reading as one unshowable entry rather than as data loss. The flag is what it was when
  the row was written, so such a row keeps the classification it was filed under rather than reverting to active; the
  one place an unreadable payload does land active is the version-10 backfill, which has no stored flag to keep and
  reads the archive member out of the JSON text — a payload SQLite cannot parse at all is counted rather than hidden.
  Changed 2026-08-22: `total` used to count archived rows in every view, so out of the box the default list showed ten
  rows above a count of twelve, with no filter typed and nothing on screen able to explain the gap. The accepted
  consequence is that the ordinary list now reads as unfiltered — "M sessions" — and the filtered wording belongs to
  filters a person applied.
- At most one HOST may cache a given session id, as a schema invariant. Session ids are supervisor-minted UUIDs, so two
  hosts naming one is either a bug or a hostile supervisor claiming a session it does not own — and the consequence is a
  routing decision, not a display one: owner lookup would resolve one host while the list showed another's row, so a
  stop aimed at one machine could land on a different one. The first claim holds and the later claimant's row is
  dropped, so the LIST stays coherent; but while both hosts keep reporting the id, ROUTING fails closed naming both,
  because the helm has no basis for choosing which one the user meant. That contest is per-host REFRESH STATE,
  reconstructed from each drain's own evidence rather than remembered: it clears itself when a claimant stops reporting
  the id, goes with the host when it is removed, goes with the cache when an adoption purges it, and needs no schema to
  survive a restart — a restart forgets the marker and the next drains re-observe the collision if it is still real,
  which costs one refresh interval in which a genuine collision routes to the cached owner. A host that lists one
  session id twice in a single reply is a different failure: a list that contradicts itself is refused whole, and the
  previous cache is kept.
- The wire order is VALIDATED, not assumed. A drain rejects a list that is not creation-time descending with the session
  id ascending — within a page and across page boundaries — because this side does not merely display that order: an
  identity-less host's list is binary-searched for a resume point and merged in lockstep with the persisted page, and
  both are meaningless over an unsorted sequence. The failure would otherwise be silent pages that skip or repeat
  entries. Session ids are bounded at every peer ingress for a related reason: an id near the frame limit produces a
  cursor no client could replay, which would strand a walk at that row forever. The served `sort` does not reach this:
  it is a property of the helm's own merged cache, so hosts go on listing in creation order and the drain goes on
  validating exactly that. The consequence for the one host that serves from memory rather than from the cache is that
  its list arrives in creation order whatever was asked for, and the merge re-orders it per request — a k-way merge is
  only correct over sources that are each already ordered.
- Every mutation whose result changes what the helm has RECORDED records it before answering: a create seeds its new
  session, a restart and a rename store the reply's fresh `SessionInfo`, and a delete forgets the row. The merged list
  and the owner lookup are both served from those records, so a mutation that recorded nothing leaves the list
  contradicting the answer the caller just got — a session that cannot be operated on, a restart that still reads
  `exited`, a deleted row sitting beside its replacement. All of it is best effort and none of it can fail the mutation:
  the operation succeeded, and reporting a success as a failure is the one outcome SPEC.md's creation contract rules
  out. Each write carries the CLAIM its operation was routed under — a manager-wide connection token that is never
  reused, plus the identity — and is dropped if the connection has changed since, so a delayed reply cannot file one
  install's session under another's name. Writes are serialized against the host's own refresh, and a refresh whose
  drain predates one of them declines to commit rather than erasing it.
- One field of such a reply is NOT taken as given: a status of `unknown` never overwrites a definite one. The protocol
  is explicit that `ListSessions` is the only reply computing a real liveness answer and that everywhere else `unknown`
  means "not yet known" rather than "not running" — a create's and a restart's replies carry it deliberately, because at
  the instant they are built the pane exists but the agent's own exec inside it has not been observed. Recording that
  verbatim answered a successful restart with a badge saying the helm had no idea, for a session it had definite
  knowledge about a moment earlier. Keeping the previous value would leave it stale until the next refresh computes the
  truth, so such a write also WAKES that host's refresh — a refresh-only wake that cuts short the wait between drains
  and touches nothing else (distinct from the retry verb, which drops the connection and re-enters the retry ladder).
  The definite answer then arrives in one `ListSessions` round trip rather than one cadence interval, which is what
  keeps a restart of an exited session from reading `exited` afterwards. The wake is sent after the write's own epoch
  bump, so the drain it provokes is a post-write one and commits rather than declining as a pre-write snapshot would.
- Identity-less serving is only for a row with NO identity on record. A row that HAS one, meeting a peer that reports
  none, FREEZES in its own non-connected state (`identity-unverified`) and connects nothing. There is no identity to
  compare, so the mismatch check cannot see the situation at all — and connecting anyway would put an unverified peer in
  charge of a host whose cache, written under the recorded identity and still in scope for the list, describes a
  different install: the silent merge SPEC.md forbids, arriving through the one door that check cannot cover. The old
  cache stays and serves stale like any other non-connected host's, which is the honest reading — it is still the last
  thing this helm actually verified. Distinct from `identity-mismatch` because the remedy differs and offering the wrong
  one would be worse than offering none: nothing was presented, so there is nothing to ADOPT, and the ways out are
  fixing the host, retargeting the entry, or removing it. Re-probed automatically, unlike a mismatch, because there is
  no human decision available to wait for.
- Session operations route by OWNER LOOKUP in that merged view — from the cache's COLUMNS, never from the stored
  metadata, so a row whose payload no longer decodes still routes and a live session is never made unreachable by a
  corrupt copy of its own details. A session whose host is in any non-connected state is refused with the state named
  and nothing queued. Unreachable is not special-cased; a version-skewed, identity- mismatched, duplicate, or retired
  host refuses identically, because a caller that handled four of six would silently mis-handle the rest. The routing
  decision reads the host's state and its live connection from a SINGLE borrow of the actor's published status — split
  across two reads it could pair a fresh `Connected` with a dead connection, which is precisely how an operation gets
  routed onto a corpse. Creation takes the target host in the body (defaulting to the local row, the tail of SPEC.md's
  own creation default) and refuses a non-connected one as a precondition failure. Reading a session's DETAIL is the one
  route a non-connected host does not refuse: SPEC.md requires a stale session's metadata to be viewable behind the
  host-unreachable notice, so that read is served from the cache and marked stale, while a reachable host's detail is
  always fetched live — the cache exists for the stale list, not as a general serving layer. The live path drains the
  owner's list to exhaustion rather than reading one page, since a session sitting past the supervisor's default page is
  exactly the case a busy host has most of.
- Host management commits durably first and converges the live actors after, so each verb states how it fails closed:
  add rolls its row back if no actor could be started (a registered host with no actor is invisible and un-dialed, while
  its destination is taken); retarget converges instead of rolling back, because the durable write is what the user
  asked for and the actor can be told to reconnect through a path that cannot fail; remove tears the actor down by the
  id it just committed, needing no registry read that could fail. Retry reports whether it found a host, and a RETIRED
  host's retry respawns its actor from the current row — nothing else ever restarts one, so without that an actor that
  panicked left its host permanently dark. Adopting names the identity the user was shown and is refused if the host has
  since started reporting a different one, because a re-probe between the decision and the request would otherwise adopt
  something nobody approved.
- `--ensure-hosts <file>` is a JSON5 floor under the registry, applied through the same registration path as a REST add
  before serving begins and never consulted again. It adds what is missing and touches nothing else: an already
  registered destination keeps its fields and its learned identity, because helm.db is the durable authority and a
  startup file that overwrote user edits every boot would make the two fight. Validation is all-or-nothing — a malformed
  file, an unusable destination, or a destination listed twice fails startup with the entry named and nothing written,
  since a helm that came up with three of five guaranteed hosts looks healthy and is not.
- axum serving: REST for CRUD (sessions, profiles, hosts), a WebSocket event stream for live session-list updates, a
  WebSocket per attached terminal, and the static UI bundle. Loopback bind enforced — refuses non-loopback per SPEC.md.
- Web token: random 128-bit value minted on the helm's first run and stored recoverably in helm.db so `token show` can
  print it. Browser auth exchanges it once for a random 128-bit device secret returned in the response body; the browser
  keeps that secret in origin-scoped localStorage, whose origin includes the loopback port, and sends it explicitly as a
  Bearer credential on REST requests and a credential-bearing WebSocket subprotocol during upgrades. The helm stores
  only the device secret's SHA-256 hash, and rotation deletes all device sessions. This deliberately gives up HttpOnly:
  script execution in the authenticated origin can read the secret, but such a script can already drive the same API,
  while port scoping prevents an unrelated loopback service from receiving an ambient host-scoped credential. The
  loopback Origin guard remains defense in depth; no ambient browser credential remains, so this flow has no CSRF edge.
- The native app embeds farhelm-helm in-process; the Linux helm is the same code behind `farhelm helm run`. The local
  supervisor is a separate process either way — the app discovers one that already answers and leaves it alone, or
  starts `farhelm supervisor run` from its sibling binary and owns that child for its own lifetime.

## Logging

`tracing` everywhere, with `tracing-subscriber` env-filter semantics. The intended mature shape uses spans carrying
session and host ids, so SPEC.md's required diagnostic trails (creation, PTY lifecycle, attachment transfer,
reconnection, resume) fall out of structured context rather than ad-hoc log lines.

M1 emits structured lifecycle and failure events, attaching session or channel fields where that context exists. The
host half of the span discipline above now exists: M6's connection manager runs every actor inside a span carrying the
host id, kind, and destination, so the reconnection trail SPEC.md requires — connection attempts, phase transitions,
hello refusals, identity decisions (first contact, mismatch, adoption, duplicate), refresh outcomes, and recovery —
falls out of that context rather than out of per-call-site discipline. Two decisions are made by the manager rather than
by an actor, and therefore outside that span: adopting an identity and reconfiguring an edited host. Both attach the
same host metadata explicitly, so the trail has no gap where a user's decision should be. The destination is attached
per event rather than carried in the span, because a span's fields are fixed at creation and a retargeted host would
otherwise keep being described by the address it no longer uses — including in the very lines about reconnecting to its
new one. Phase transitions are logged when the phase actually changes, never on every republish, so a connected host
refreshing on its poll cadence does not bury the handful of lines that describe what happened to it. The session half
and resume's own trail are still later milestones.

Logs go to stderr, and deliberately not stdout: under `farhelm internal stdio` the process's stdout IS the protocol
channel, so a stray line there corrupts frames. File logging under `~/.local/state/farhelm/logs/` with rotation
(tracing-appender) and a `--log-level` flag are the intended shape but are not built yet — today verbosity is
`RUST_LOG`-style env only.

One log source is not native code: `POST /api/client-log` (PLAN_desktop_web_bug_triage.md) lets the desktop webview's
console shim forward its errors and warnings into native tracing under the `webview_console` target, because the webview
is the one layer with neither tracing nor devtools and a dead eval bridge (MT-5) otherwise erases its own evidence. The
route is device-session-authenticated with the desktop-webview CORS layering, and treats the page as a peer, not a
friend: an envelope-shaped body with unknown fields refused, a route body limit, per-field byte caps with the same
bound-and-escape treatment as every other peer string, a shared fixed-window accept budget, and at most one
dropped-count warn per window so the endpoint's own reporting cannot amplify the failure loop it exists to observe. Only
the desktop build ships a sender; browsers have devtools and forward nothing. The desktop app also runs a native
eval-bridge watchdog (`webview_watchdog` target): a 15-second one-shot-eval heartbeat that logs exactly one error line
per continuous outage when the bridge stops answering (the MT-5 class — the shim cannot report a failure of the very
bridge that armed it) and one recovery line if it resumes; log-only by explicit decision, never a reload or an exit.
Both target names are grep contracts: `docs/desktop-web-triage.md` is the triage recipe built on them, and
`scripts/desktop-smoke.sh` asserts the whole pipeline (a marker through the real capture path, for the first launch and
the restarted process alike) plus watchdog silence on every non-skipped run.

Motivation: tracing is the ecosystem standard, and span context is the cheap way to make "logs are available for X" a
property of the architecture instead of a discipline.

## CLI

clap (derive), one multi-call binary named `farhelm`, clean subcommand grammar. The user-facing surface:

- `farhelm helm run` — run the helm (flags: `--port`, `--state-dir`, `--ui-dist`, `--ensure-hosts <file>`,
  `--payload-dir <dir>` (env `FARHELM_HELM_PAYLOAD_DIR`), `--release-base-url <url>` (env `FARHELM_RELEASE_BASE_URL`)).
  The last two select where "add host" provisioning payloads come from — an operator-staged directory (verified not at
  all, D3) or a download source other than the default GitHub release (D2); `--payload-dir` wins if both are given
  (D18). It takes no session or transport flags: M1's `--ssh`, `--cwd`, `--agent`, `--title`, `--remote-farhelm`, and
  `--remote-state-dir` were dropped with M6's registry (user decision 2026-08-04). A helm drives every registered host
  at once, so a flag naming one of them could only ever have meant the wrong thing; the last two live on as per-host
  registry fields, and creation is `POST /api/sessions`, which is where the host selection belongs. A release build
  compiles its own web UI in (`FARHELM_UI_DIST` at build time); `--ui-dist` still overrides it at runtime, and an
  ordinary developer build with neither serves the API alone.
- `farhelm helm setup [--state-dir DIR] [--port N] [--tmux PATH] [--no-supervisor] [--dry-run]`, and
  `farhelm helm setup --uninstall [--dry-run]` — write, enable, and remove this machine's systemd user units for the
  helm and its supervisor. Linux only (it exits 2 elsewhere, pointing macOS at the desktop app). One state directory is
  resolved from setup's own environment and pinned into both units, so a shell-only `XDG_STATE_HOME` cannot leave the
  two services on different trees, and both installing and uninstalling refuse when the running user manager reads units
  from a different directory than the one this environment selects. The units are rendered from the templates in
  `crates/farhelm-helm/units/`, the same ones remote provisioning fills in, and every file setup writes starts with
  `# managed-by: farhelm helm setup`: setup overwrites or removes only files carrying that marker, and refuses anything
  else rather than replacing it. It never installs tmux — a tmux below the floor, or none at all, is a refusal naming
  what it found. The matching rule on the helm side is that the hosts panel never installs or updates a supervisor on
  the helm's own machine at all: an absent one is answered with "run `farhelm helm setup` here", a unit already running
  that same binary is named as setup's or its author's and left alone, and a supervisor that ANSWERS is discovered and
  registered exactly as a remote one is.
- `farhelm helm token show|rotate` — web-token bootstrap and rotation.
- `farhelm supervisor run` — run the supervisor in the foreground; this is SPEC.md's "run the binary with arguments in a
  terminal" path.
- `farhelm spawn --cwd <dir> [--title ...] [--agent ...] [--parent ...]
  [--idempotency-key ...]` — the in-session
  spawn CLI from SPEC.md.
- `farhelm agent hosts|sessions` — the in-session ASKING CLI from SPEC.md, on the same injected credential spawn uses.
  It prints an aligned table on stdout, `*` marking the asking session and its host, and puts a refusal on stderr with a
  non-zero exit exactly as spawn does. A table rather than JSON because the reader is a model quoting its own shell
  output. Every dynamic cell is escaped to one printable line and every non-final column is capped at 48 characters:
  these values are fleet-wide user text printed straight to a terminal, so a raw newline forges a row, an ESC drives the
  terminal, and one long title would otherwise be padded onto every other row. A cut listing prints its rows on stdout
  and one warning on stderr, so a script capturing stdout still gets nothing but the table. It has no timeout of its
  own: the supervisor bounds the relay and is the only party that can distinguish its two failures (see the transport
  section's version-13 paragraph).

Internal commands live under a hidden-from-help `internal` namespace — `farhelm internal stdio` is the ssh-exec stdio
proxy. (An underscore prefix like `_stdio` was considered; it is not a recognized convention, while an explicit
`internal` namespace is self-describing and gives future internal commands a home.)

Motivation: one binary is one provisioning artifact and guarantees the spawn CLI exists inside every session (the
supervisor puts its own binary on the session PATH). clap-derive because it is the standard and keeps the grammar
declared next to the types.

## Native app packaging

Dioxus desktop (wry) wrapping farhelm-ui, shipped as a BARE BINARY rather than a `.app` bundle: `farhelm-desktop`, a
thin crate (`crates/farhelm-desktop`) whose `main` is one call into farhelm-ui's desktop module, built by cargo-dist
alongside `farhelm` and installed next to it. The two are one artifact pair, not one bundle — the shell embeds
farhelm-helm from the same workspace version and reaches supervisor code by finding its CLI sibling next to its own
executable, discovering a local supervisor that already answers or spawning `farhelm supervisor run` when none does.

The bundle went away because a bare binary has nowhere to put a `Resources/` directory, and Dioxus's `asset!()` files
were the only thing that needed one. They are served instead from the UI tree compiled into `farhelm-helm`, through a
`dioxus-desktop` asset handler registered on `/assets/*`, handed to the webview over the `dioxus://` scheme. By default
— and in every release build — those are the same bytes the helm serves to a browser, since both read the compiled-in
tree; `FARHELM_DESKTOP_UI_DIST` breaks that identity deliberately, pointing only the loopback helm at a directory on
disk while the window keeps rendering from the embedded tree. Registering a handler for a path prefix takes precedence
over dioxus's own filesystem resolver, so there is no bundle-directory fallback at all; the price is that the desktop
build's asset set and the web bundle's must be identical, which `scripts/check-desktop-assets.sh` enforces on every
change.

A releasable `farhelm-desktop` must be produced by `dx`, not by Cargo alone. The `asset!()` macro emits a placeholder
into a `__ASSETS__` link section and dx rewrites those symbols with content-hashed names after linking; a plain
`cargo build` links and launches but requests placeholder paths, so every asset 404s even with the web bundle embedded.
Release production therefore has to run or consume `dx build --package farhelm-desktop --platform desktop --release`
with `FARHELM_UI_DIST` set; that it must happen is not negotiable, because the failure is invisible to a build that
succeeds.

How that is wired: cargo-dist cannot be asked to run dx over a package it built, and it has no post-build hook — so the
shell is declared to it TWICE, and exactly one of the two descriptions ships. `crates/farhelm-desktop` (the Cargo
package) carries `[package.metadata.dist] dist = false`, which is what stops cargo-dist from publishing its own
placeholder-bearing Cargo output. `packaging/farhelm-desktop/dist.toml` declares a generic (non-Cargo) dist package of
the same name in the hybrid workspace `dist-workspace.toml` defines, and its `build-command` is
`scripts/build-desktop-binary.sh`, which runs dx, refuses a binary still carrying placeholder asset names, checks the
requested set against the bundle it embedded, and hands the result back for archiving. Neither half is redundant: delete
the generic package and the Mac loses its window; delete the `dist = false` and the release grows a second
`farhelm-desktop` archive that renders nothing. The cost of the arrangement is a version number duplicated into that
`dist.toml`, which a unit test in `assets.rs` holds to the workspace's.

Native glue (dock/menu integration, plus whatever proves genuinely necessary — see the clipboard note in the Dioxus
risks) lives behind a feature flag in farhelm-ui, kept deliberately thin.

## Provisioning

Implemented in the helm over the same system-ssh access: sftp the cross-compiled `farhelm` binary (plus a private static
tmux build when the host has no tmux) into `~/.local/lib/farhelm/`, write user-level systemd units,
`systemctl --user enable --now`, `loginctl enable-linger` as the optional-step (proceed-without-if-privileged per
SPEC.md). Discovery-first: probe for a running supervisor via `farhelm internal stdio` before proposing any of this, and
show the full concrete action list before touching the host.

ADD and UPDATE both retain that concrete plan behind an opaque, one-use confirmation id. Planning is inspection-only;
confirmation consumes the id, revalidates the host and registry facts the plan relied on, and only then admits the
host-scoped run. Discovery records the resolved supervisor binary, state directory, and identity together so a later
helm dials the same installation that answered the probe. UPDATE starts from those recorded coordinates rather than
assuming the standard layout.

Artifacts land under temporary names in their final flat directories and are atomically renamed into place. There are no
version directories or `current` symlinks: a failed transfer leaves the installed file intact, while a running binary
keeps its old inode until the explicit supervisor restart. Hash checks skip identical payloads and unit files are
written only when their content differs, so rerunning provisioning converges from wherever an earlier run stopped.
Matching content also repairs mode drift, and provisioning creates or repairs its directories with explicit modes; the
supervisor state directory is private to its user (`0700`).

The supervisor unit uses `KillMode=process`. Sessions started through Farhelm belong to the private tmux server that the
supervisor launches, so systemd's default `control-group` policy would kill that server and every session whenever an
explicit UPDATE restarts the supervisor. Limiting the unit stop to its main process preserves the same ownership model
as running `farhelm supervisor run` manually: stopping the supervisor detaches management, while tmux continues to own
the session processes and terminals until the user deletes them or the host reboots.

Motivation for shipping tmux ourselves when absent or too old: apt needs root, and SPEC.md forbids requiring it; a
static tmux under our own lib dir keeps the no-root promise without asking the user to install anything.

The provisioning payloads — linux-musl `farhelm` binaries for both architectures plus the static tmux builds — are no
longer embedded in the helm's own distribution (D2). This REVERSES the earlier "provisioning must work with no
third-party downloads" posture: a release-shaped build (D13 — one that embedded the web UI) downloads them, on demand,
from the GitHub release matching its own version, verifies them, and caches them under helm state before pushing them
over SSH exactly as before. A developer build defaults to no payloads at all (`NoPayloads`, D13) rather than to a
download, and `--payload-dir <dir>` (env `FARHELM_HELM_PAYLOAD_DIR`) selects an operator-staged directory instead —
files in an explicitly selected local directory are treated as operator-trusted and are not verified, on ANY build,
developer or release. The downloading source is `ReleasePayloadSource` (`provisioning/release_payloads.rs`), which
caches one release's assets, their extracted binaries, and the signed checksum file under
`<state_dir>/payloads/v{version}-<first 12 hex of sha256(base_url)>/` — keyed by version so an upgraded helm never
reuses the previous release's binaries, and by base URL so a mirror or test server can neither read nor poison what the
real release wrote. Motivation for reversing the embedding: bundling every target's binaries inside every platform's own
artifact cost tens of MB per download and coupled a payload's presence to whichever platform happened to embed it; a
download keeps the size cost where it belongs (paid once, by the host actually being provisioned) while keeping the same
"provisioned host runs exactly what the provisioning helm expects" property, because the default download always names
the provisioning helm's own version.

Verification chain (D3): CI writes one `SHA256SUMS` covering the six release binaries/archives and signs it with an
unencrypted minisign secret key held as a repository secret, passing `-t "farhelm $TAG"` so the release version lands in
the signature's trusted comment; the `farhelm` binary embeds the matching public key and refuses any download whose
`SHA256SUMS.minisig` does not verify, whose trusted comment names a version other than the helm's own, or whose per-file
SHA-256 does not match. The trusted comment is not decoration: the signature otherwise authenticates only the CONTENTS
of `SHA256SUMS`, which name no version, so whoever serves the release URL could replay an older release's valid manifest
and assets at a newer version's URL and downgrade every host that helm provisions. A release tag is `vX.Y.Z` and already
carries the `v`, so the comment is `farhelm` plus the tag verbatim: signing without `-t`, or with a second `v`
(`farhelm v$TAG` → `farhelm vv1.2.3`), produces a release no helm can install. `--payload-dir` is the one path that
skips all of this — nothing there is downloaded, so nothing there is checked. SPEC.md's "no public relay, no third-party
rendezvous service" line still holds: GitHub is a download source the helm's own machine reaches directly, never a relay
or rendezvous point sessions or connections pass through.

Release signing key. The key pair behind that chain is the project's one long-lived secret, and its handling is
deliberately minimal. The public half is committed twice — `MINISIGN_PUBKEY` in `release_payloads.rs` and
`crates/farhelm-helm/src/provisioning/farhelm-release.pub`, with a test that they agree. The secret half exists only as
the `MINISIGN_SECRET_KEY` repository secret: it was generated locally, stored with `gh secret set`, and the file
destroyed; it is never committed, never printed, and never present on a developer machine. Only the `sign` job of
`sign-sums.yml` receives it, after a secretless `validate` job has already checked the assets, so the generated dist
workflow and the build jobs never see it. Neither minisign keys nor repository secrets expire; rotation happens when the
maintainer chooses. Rotating is one PR: `minisign -G` a fresh pair, `gh secret set MINISIGN_SECRET_KEY` from the new
secret file, shred it, replace both committed copies of the public key, and cut the next release. Nothing in the field
notices, because under D2 a helm only downloads the release built from the same commit as itself, which is signed by the
key that commit compiled in; old helms keep verifying their old releases with the old key. The one future feature that
changes this is a cross-version download such as an auto-updater: it would verify the next release with the key it
already carries, so a rotation would then need a transition release signed by the old key but carrying the new one.
Sequencing rotation before shipping such a feature, never in the same release, is the whole rule. Note also what the key
does not protect: `install.sh` runs on a machine with nothing to pin a key in, so installing trusts GitHub over TLS and
the `SHA256SUMS` served beside the archive; the signature guards what a running helm provisions onto other hosts, not
the first download of the helm itself.

A release also carries cargo-dist's own metadata, none of which is signed and none of which Farhelm reads:
`dist-manifest.json`, a `<archive>.tar.gz.sha256` beside each of the four archives, and a lowercase `sha256.sum` over
what dist built. That last one is worth naming explicitly because it looks like the file that matters and is not it:
`SHA256SUMS` — uppercase, six entries, the one `SHA256SUMS.minisig` authenticates — is what the helm and `install.sh`
verify against. The metadata is nonetheless part of the release contract rather than incidental: the signing job
REQUIRES the manifest and the four per-archive checksums to be present (a future Homebrew formula, deferred but meant to
be a config flip, consumes the manifest) and treats `sha256.sum` as optional. Anything else appearing on a release fails
it, so no published asset can sit outside both the signed set and that list.

## Cross-compilation and targets

Supervisor-side artifacts: `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, static, built by cargo-dist on
a native runner per architecture with `musl-tools` for the C half of the link (audited: rusqlite-bundled static musl
builds work for both). Cross-compiling them with cargo-zigbuild, which the retired hand-written release workflow did, is
no longer necessary now that GitHub's arm64 Linux runners are generally available; zig remains the toolchain for the
private tmux builds, which are C and cross-compile cleanly. The Mac artifacts (`aarch64-apple-darwin`: `farhelm` and
`farhelm-desktop`) are built on a macOS runner with the native toolchain — cross-building them from Linux was audited
and rejected: zig cannot link the Apple frameworks wry needs (WebKit, AppKit) without a licensed macOS SDK, and cross
ships no darwin images. Dropping the `.app` bundle removed a third reason (`dx bundle` produces bundles only on the
native platform) but not those two, and it added one: `farhelm-desktop` is built through `dx build`, which patches the
asset names into the linked binary and so has to run where that binary is linked. The macOS runner stays. Signing and
notarization are deferred (D11) and the artifacts ship unsigned, so signing is not among today's reasons — it would
become one if that changes.

Motivation: musl-static sidesteps glibc version skew across Ubuntu releases, which matters because provisioning drops
binaries on machines we do not control; "predicated on whatever cross-compiled binaries are available" in SPEC.md
becomes exactly this target list.

## Testing

The testing story is a first-class requirement — agents verifying GUI behavior without a human is a standing project
constraint (see the GUI section's motivation), not an afterthought:

- **Playwright (TypeScript) drives the web build in headless Chromium** against a real helm and real supervisor on
  Linux. DOM assertions and screenshots both work because the UI is real DOM. This is the canonical GUI verification
  path for agents.
- **A fake agent** — `farhelm internal fake-agent --script basic|altscreen|binary|mouse-modes|spawn`, a hidden
  subcommand of the one binary rather than a separate artifact — stands in for Claude Code/Codex across this suite's
  integration and e2e tests. Its deterministic scripts cover prompt/echo input, terminal modes, alternate-screen
  rendering, byte-clean live output, and mouse-mode reporting, without vendor auth. Later milestones extend this fixture
  with fake on-disk records for status heuristics, conversation capture, and resume. The spawn suite also has an
  automated real-Claude leg that creates a jj workspace and spawns into it; CI leaves it gated because vendor
  credentials and network access are absent, and a developer enables it manually with `FARHELM_REAL_AGENT=1`.
- Rust integration tests exercise supervisor+tmux directly (CI provides tmux) and the framing protocol with golden
  cases; farhelm-proto keeps wire compatibility testable.
- **`node --test` unit-tests the asset-JS layer's pure functions**, under `crates/farhelm-ui/js-tests/` (outside
  `assets/` for source/test separation — bundling itself is by explicit `asset!` registration, so placement alone
  neither includes nor excludes a file). It currently covers `term-bytes.js`'s byte-domain conversion for
  `term.onBinary` — the byte-for-byte contract pinned at the boundaries (0x00, 0x7f, 0x80, 0xff), empty input, and a
  mouse-report-shaped sequence. Node's built-in runner over vitest/jest: node is already a CI requirement for
  Playwright, so this is zero new dependencies, and the asset-JS layer has no bundler for a module-tooling-heavy runner
  to pay for (PLAN_M6_5.md item 1).
- **A CentOS Stream 9 container stands in for a host that is not the CI runner.** Provisioning accepts any Linux host
  with a usable systemd user manager, but every provisioning integration test dials `localhost`, and both CI and the
  release gate run on Ubuntu — so the case the retired `ID=ubuntu` gate existed to forbid, a helm installing onto a
  different distribution, was the one case nothing covered. `scripts/test-provision-centos.sh` boots
  `quay.io/centos/centos:stream9` with systemd as PID 1, publishes its sshd on loopback, and points the existing ssh
  provisioning test at it through an alias in the user's own ssh config, so the transport, the PAM stack, the user
  manager, and `/etc/os-release` under test are all the container's. What it pushes is what a release publishes: the
  musl-static `farhelm` for `x86_64-unknown-linux-musl` and the pinned static tmux, both put through
  `scripts/check-static-elf.sh` first. That choice is forced — the workspace's glibc debug binary cannot exec on CentOS
  9's older glibc — and it is also the point, since it makes this the one test of the artifacts users actually receive
  on a machine that is not the one that built them. Known limit: the container runs under the host's SELinux, and the
  host is an Ubuntu runner where it is not enforcing, so RHEL-family hosts with SELinux enforcing remain untested.
- The desktop shell's native glue is the acknowledged manual-test gap (see GUI risks); everything else must be coverable
  without a human.

## Version and skew

One version number across the workspace; the protocol hello carries protocol and build versions; incompatibility refuses
with a clear error at the edge (helm↔supervisor connect, client↔helm load) per SPEC.md. Protocol version bumps with any
incompatible change — which includes a field whose omission changes what the receiver DOES, not only changes to frames
and message sets. A serde-additive field can still be semantically load-bearing: the non-displacing attach is the worked
example (a peer that ignores it displaces a client it was asked to leave alone, silently, on both ends), and decode
tolerance is why such a bump is required rather than why it is unnecessary.

The client↔helm edge has no hello to refuse at, so the helm stamps its build on every reply and the UI compares it
against the one compiled into its bundle. A mismatch — including a helm that reports no build at all — surfaces a reload
prompt and, more importantly, withdraws every UNATTENDED behavior that depends on the helm honoring this milestone's
vocabulary: the terminal heartbeat and automatic reconnect both stop, while anything the user explicitly asks for keeps
working.
