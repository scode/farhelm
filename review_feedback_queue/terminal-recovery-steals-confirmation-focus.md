# Terminal recovery can take a confirmation's keyboard input

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

An unanswered Delete, Replace or Restart question can lose keyboard focus when terminal recovery finishes or a selected
tab disappears. Enter intended to answer the question can instead submit input already waiting in the live terminal.

## Details

Suggested bucket: `highest`, provisionally, because unintended submission to a live program could affect user work.
Confidence: static focus guards and input forwarding confirmed; the user-visible input consequence is likely and was not
runtime reproduced. Provenance: retained `browser-terminal-1/f1`, corroborated by `ui-session-detail-1/f2`, both through
their final second-turn synthesis. Proposed drop: none. No destructive command was reproduced. Keep that uncertainty
visible when finalizing priority; modal dialogs and the explicitly protected nonmodal controls are unaffected.

`keyboardHeldElsewhere` protects ARIA modals, focused approval cards, the sidebar separator and notification controls
but omits header confirmations (`crates/farhelm-ui/assets/terminal.js:1111–1118`). `takesFocus` reads the current
focused element at reveal time and deliberately permits ordinary buttons to yield (:4471–4485). Focusing the question's
button while replay runs therefore does not protect it. Reveal shows the terminal before focusing it (:4419–4426),
availability recovery retries the same guard after removing stale-pane inertness (:4492–4495), and automatic
selected-tab fallback shares the incomplete guard (:3049–3053).

Header questions are nonmodal divs (`crates/farhelm-ui/src/session_view.rs:2381,2481,2585`), with one-frame focus
placement when mounted (`lib.rs:232–238`). Header Delete/Replace confirmation state is independent of connection/detail
state (`session_view.rs:696–706`); fresh detail and sync (:918–967,1955–1980) do not close them. The visibility cleanup
at :1995–2012 applies only to interrupted-card Replace. The lifecycle gate excludes mutations, not terminal input:
`terminal.js:5120–5122,5147–5152` forwards input with an open socket. Terminal inertness follows staleness, not
lifecycle busy state (`session_view.rs:3128,3192`).

The session-detail reviewer independently traced automatic fallback: the view filters the selected tab against current
tabs (`session_view.rs:1837,2036`) and emits agent `focus=true` when the selection disappears (:1885). It need not wait
for tab cleanup to finish: `crates/farhelm-supervisor/src/service/status.rs:446–466` omits dead tabs from listings,
while `service/ticker.rs:1840–1936` reaps separately and may defer for a busy session. The header question's lifecycle
claim therefore does not prevent the tab-list update or fallback. After focus moves out, Escape also misses the header's
bubbling cancellation handler.

Open a header Delete question while terminal replay/recovery is pending, leave its answer focused, wait for recovery to
finish, then press Enter. Another trigger is a selected shell disappearing while a header question is focused. Automatic
readiness, availability recovery and tab fallback must preserve a still-open confirmation's keyboard ownership; explicit
terminal selection can retain select-then-type behavior. A focused browser check should cover those automatic handoffs
and verify that no Enter reaches the terminal while the question owns focus.

Possible cover: none established. SPEC.md:1123–1127 gives confirmations keyboard focus; SPEC_impl.md:478–487 requires
recovery to preserve deliberate focus in other keyboard owners. SPEC.md:1279–1284 and the ledger's
“pending-menu-arrows-can-ignore-independent-focus.md” accept near-simultaneous manual navigation, not arbitrarily
delayed automatic recovery; their bound also excludes unintended commands. The queued
“update-check-replacement-loses-focus.md” concerns replacing a focused update button, with no terminal input
consequence. No exact Planned, BUGS or filter match was found. SPEC.md:1211–1215 permits automatic tab removal, not
redirection of a confirmation's keyboard input; SPEC_impl.md:501–513 protects the separate Restart-with modal, which is
outside this finding. The session-detail corroboration describes the same keyboard-ownership issue, not a separate item
per automatic focus call. This does not bypass the header operation's own confirmation; harm depends on the terminal's
contents.
