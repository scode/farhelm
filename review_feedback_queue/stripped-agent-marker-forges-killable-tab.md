# A stripped agent marker lets a forged tab kill the live agent

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A rogue program running in any terminal can plant a fake tab in another session whose close button kills that session's
live agent — or gets the agent's window destroyed automatically when it exits.

## Details

Source: pre-pr-review-swarm, area supervisor-teardown, 2026-09-17. Confidence: possible (needs the forge plus one user
click or a natural exit; definite mechanism).

`tabs_from_pane_states` excludes agent windows via `state.agent.is_some()` (terminals.rs:224), and the docs promise "An
AGENT-marked window is never a tab, whatever else is written on it" (terminals.rs:186-189) — explicitly naming the
threat ("Nothing stops a pane from adding a tab marker to the agent's own window, and adopting it would offer a 'tab'
whose close would reap the agent"). But the defense assumes the AGENT marker STAYS: `state.agent` comes from
`parse_marker`, which returns `None` for anything not UUID-shaped — including a marker that was unset or overwritten
with garbage (tmux/control_codec.rs:89-91). Window options are writable by any process holding `TMUX` (the codebase's
own documented forger, terminals.rs:180-183). So a pane process — in the victim session or any other session on the
private server — runs `tmux set-option -w -u @farhelm-agent` plus `set-option -w @farhelm-tab <fresh-uuid>` against the
victim's agent window. The duplicate-id rule (terminals.rs:239-241) does not fire (fresh uuid, single claimant), and the
agent window is adopted as a tab; listings show a bogus tab. Nothing downstream re-checks: `resolve_terminal_for_close`
(terminals.rs:374-380) resolves the forged id to the agent's pane, and `close_tab_window` (core.rs:9528+) has no guard
comparing the resolved pane against the entry's agent pane — it reaps with `TabReapAnchor::PaneIfLive` (pane root =
agent pid → PPID closure kills the agent tree) and `kill_window`s the agent window. Trigger: the victim user closes the
unfamiliar tab (kills a live agent), or the agent exits on its own and the ticker's `reap_dead_tabs`
(ticker.rs:1394-1461) auto-reaps the now-dead forged tab (destroying the retained window and its exit evidence).
Teardown's own use of the same rediscovery is unaffected (a forged uuid derives a nonexistent scope unit — harmless
no-op). This is the mirror image of the attack the duplicate rule was built for (terminals.rs:190-194, forged marker
redirecting close onto the wrong window) — but via marker REMOVAL plus a forger-minted id, which no filter covers.
Cross-session: any pane on the shared private server can forge against any session's agent window; the threat model
explicitly covers forged markers (the duplicate rule exists for them).

To verify, unset the agent marker and set a fresh tab marker on a live agent window from another pane, then list tabs
and close the bogus one.

Suggested fix: positively exclude the entry's recorded agent pane from tab adoption — pass the agent pane id into
`tabs_from_pane_states` (or filter at its call sites: `resolve_terminal_inner` at terminals.rs:506, `reap_dead_tabs` at
ticker.rs:1421) and drop any claim whose window contains that pane. Pane ids are tmux-assigned ordinals, not
pane-writable, so unlike the window option the forger cannot move them; a stale recorded pane is simply absent from the
live states map and excludes nothing.
