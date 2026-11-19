### What this was about

An exited session could stay on “Session unavailable — reconnecting” after its terminal host died, for example after a
container was stopped outside Farhelm. The app tried again every 30 seconds even though there was no terminal to attach
to. The agreed fix was to have the supervisor report the missing terminal in the session listing and let the desktop use
that fact to show the existing recovery card. An attach refusal can also occur while a Restart is bringing a terminal
back.

### Things you should know

- The recovery card applies when the host is reachable and the session is still available for lifecycle actions. It
  offers Restart when Farhelm can resume the conversation, and Replace to start fresh. Legacy launches use Replace with
  to open the launcher with the old command. Interrupted sessions show the reboot or update explanation. An unreachable
  host or a container session already marked retired shows its own card.
- Other terminal tabs are hidden while the card shows, as agreed. A separate shell can keep running while its tab is
  hidden and can be out of reach until a replacement agent terminal is available.
- An exited agent can still have a terminal with its final screen and scrollback. That terminal stays viewable. If a
  Restart reply reports that a new terminal is available, the app can show it before the session list refreshes. A newer
  reply that still reports absence brings recovery back. The last listed status can still say Running, so the card asks
  for the usual consent before a Restart that could stop a running agent.
- The helm and supervisor use protocol 79 for the new field. A helm and supervisor on different protocol versions refuse
  the connection.

### Open questions and possible follow-ups

None.

### PRs

- [#4235](https://github.com/scode/farhelm/pull/4235/changes) carries terminal presence from the supervisor through the
  helm.
- [#4244](https://github.com/scode/farhelm/pull/4244/changes) shows the desktop recovery card and stops automatic attach
  attempts while it is open.

### Checks run, reused and skipped

- Focused desktop tests passed 208 cases through the recorder (run `948935cc-1945-46af-b501-f2f6e3c84921`). Command
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^session_pane::/) | test(/^session_list::restart::/)'`. These cover
  missing-terminal discovery, no attach while recovery shows, recovery on a listing update, the ordinary confirmations
  and the Restart reply sequences.
- Reused seven passing wire, cache and real-terminal tests from run `2f61e720-4d2b-4d1b-b9e4-0bba584f6a39` on the
  initial protocol patch in the working copy over `96e4ffb1`. The subsequent protocol review changed keyed replay and
  added held-list coverage without changing the tested list, Rename, Restart, cache or wire behavior. The test ends a
  real terminal host and removes its handle with the test fixture before verifying the listings and replies. A separate
  test also verifies that an exited agent's final screen stays viewable. Command
  `cargo nextest run -p farhelm-proto -p farhelm-supervisor -p farhelm-helm -p farhelm --lib --test e2e -E 'test(/session_terminal_missing_defaults_to_false_and_round_trips|protocol_version_is_pinned_at_79|unknown_control_message_tag_fails_decode|unknown_control_message_tears_down_the_connection|mutation_cache_takes_current_terminal_availability|terminal_availability_follows_the_published_host_across_restart|a_host_agents_exit_is_recorded_once_its_child_exits|wrong_protocol/)'`.
- Reused four passing protocol handshake, keyed replay and held-list tests from run
  `53283b26-deac-4fef-be04-41333653d31a` on the reviewed protocol patch now at `fbbe36c8`. The desktop change leaves
  those code paths intact. Command
  `cargo nextest run -p farhelm-supervisor -p farhelm-proto --lib -E 'test(/a_listing_during_a_restart_keeps_the_session_as_last_listed|generation_zero_error_cleanup_preserves_create_replay|reconciliation_error_cleanup_preserves_replay_and_read_only_artifacts|handshake_refuses_protocol_mismatch/)'`.
- Clippy passed with
  `cargo clippy --locked -p farhelm -p farhelm-proto -p farhelm-supervisor -p farhelm-helm -p farhelm-gui --all-targets -- -D warnings`.
  Rust formatting, scoped dprint, changelog format, test-delay annotations and whitespace checks passed. The instruction
  checker found zero errors and four warnings outside the changed files.
- The real Linux desktop smoke passed 49 steps (run `3188a6a6-d569-44bc-8f4b-ec969ef53e2a`), including normal attach,
  replay, Restart and Replace. The headless clipboard case was skipped because the compositor gives the app no clipboard
  ownership. The first attempt, run `acc22943-0744-4b4f-9cb3-5c63bfbb4efe`, was interrupted during compilation. The
  passing command was `python3 -B scripts/gui-e2e-smoke.py --no-build`, after building the app, CLI, fixtures and
  automation bridge from this stack.
- The Linux renderer wrote a complete set of 268 screenshots with `python3 scripts/check-gui-goldens.py --update`
  (capture run `68ff1196-0599-4d56-a857-19eb8cca902d`). The recorder was interrupted but the renderer finished and wrote
  its completion marker. All 266 existing images matched their original hashes, and both new recovery-card images were
  inspected. After publishing and fetching the references, all 268 captures matched the fetched files byte for byte. The
  scene-list and batch checks passed all 47 cases with `python3 -B scripts/test-gui-screenshot-batch.py` (run
  `81f41ab5-a950-499f-97de-cdc78b16ba49`).
- Validation ran locally on Linux. The targeted wire, host and GUI checks plus the real desktop smoke covered the
  changed paths, so there was no full workspace run. Native macOS validation was outside the local Linux environment.

### Review gate

One independent high-effort GPT-6.2 Sol reviewer checked each draft. The protocol review found an absent-terminal case
in Create replay and missing assertions for held listings and replay. Those were fixed. The desktop review found two
Restart cases and a missing explanation in the copy. The card now asks for consent when the cached status is Running, a
later reply can clear the earlier success mark, and the text includes the case where the host could not find the
terminal.

### Landing

#### What landed with it

It landed alone in its round. Since its base, main gained round 258's plans: gui-ux-enter-and-focus (keyboard focus
after dialogs, confirmations and bulk actions, including a step that moves focus off a control that is focused but no
longer drawn) and settings-app-version (a Version row in Settings → App), plus a repair of the triage archive. That
focus step covers this plan's card too: when the card replaces a terminal or a shell tab that had focus, focus moves to
the card. The independent reviewer found no other interaction, and nothing else on main uses protocol 79.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The protocol and supervisor
side is proportionate: one field, set from whether the supervisor holds the session's terminal, in the places that build
a session reply. The card reuses the interrupted card's layout and code, with a second reason for showing it. What the
review found on the desktop side is fixed below.

#### Fixes made while landing

- **Two end-to-end tests failed.** The tests for a session whose terminal is gone after a supervisor restart compare the
  whole listed session with the one that was created, and the listing now says the terminal is missing. The plan never
  ran that test file; the workspace run on the boats caught it. The tests now expect the missing-terminal report, which
  also checks the new field on the real path, while the session that kept its terminal is still checked not to report
  it.
- **The card's Restart asked a false question.** Farhelm keeps a session's last definite status when its terminal
  disappears, so the card can show for a session that still reads Running. There, the card's Restart opened "Still
  running — restarting stops the agent and its whole process tree first" and waited for Confirm restart. But with no
  terminal no agent is running, and the supervisor never asks for consent to restart a session that has no terminal. The
  plan's own reviewer had asked for the question, and the plan added a second question path to the card for it. I
  removed it: the card's Restart is one click again, as the interrupted card's is and as SPEC_impl.md describes. If an
  agent were somehow running after all (a terminal brought back since the card was drawn), the supervisor would refuse
  that Restart and the card would show the refusal, so nothing is stopped by mistake.
- **The card's opening sentence was reworded.** It read "This session's terminal is gone, which can happen if it ends
  outside Farhelm (for example when its container stops), never starts, or cannot be found on this host", which read as
  a general hypothetical, and "this host" can sound like the user's own Mac. It now reads "This session's terminal is
  gone: it ended outside Farhelm (for example when its container was stopped), never started, or could not be found on
  its host." I published the two re-rendered reference screenshots and looked at both.
- **Smaller fixes.** A misleading code comment was corrected, and the changelog entry now says the session's other
  terminal tabs are hidden while the card shows.

#### Things you should know that the report does not say

- Two sentences of the report are no longer true: under "Things you should know", "the card asks for the usual consent
  before a Restart that could stop a running agent", and under "Review gate", "The card now asks for consent when the
  cached status is Running". See the second fix above.
- Any session on a reachable host whose terminal the supervisor no longer holds gets this card, not only exited ones.
  That includes a session whose launch failed (it reads Error) and one that was still starting when its supervisor
  restarted and came back without a terminal. Both used to sit on "Session unavailable — reconnecting"; they now show
  the card with its "This session's terminal is gone" sentence. That follows from the plan's rule, not from the landing.
- A rare flicker: when the app retries a session create whose first attempt actually succeeded, the answer can say the
  terminal is missing while the new session's terminal is still being set up. The new session then shows the recovery
  card until the next session list from its host, normally a few seconds. I do not think it needs a follow-up unless you
  see it.

#### Checks

The plan changes the protocol, the supervisor, the helm, the command line's end-to-end tests and the desktop app's GUI
crate, so the landing ran the whole Rust workspace suite, the desktop app's own tests, the app's end-to-end smoke and
the reference screenshots (goldens). Builds ran locally; the workspace suite and the smoke ran on boat sandboxes (cloud
VMs), from binaries built locally.

On the plan as delivered, rebased onto main without conflicts:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy --locked -p farhelm-gui --all-targets --features agent -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`: passed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats
  (`20c05e4a-f2f3-4d06-ae70-d0dfba6c00ee`, `d0e0a75d-a057-4210-95b7-553b585bf1db`,
  `ad84d079-63d8-4a92-9348-af9f8f8a2b5e`): 8075 of 8078 passed, with no runtime skips. Two end-to-end tests failed
  because of the plan (the first fix above). The third failure was the macro compile-contract test, which always fails
  on a boat because it resolves crates from the local cargo cache. It passed locally
  (`cargo nextest run -p farhelm-testtrace-macros`, `125a5d99-306f-4d8b-8c90-8c188f16df8e`).
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`386177ac-4d19-4eeb-9708-8631cc6f3a21`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed, and the clipboard
  paste step was skipped, as always on Linux (`26a2eab6-bf15-4584-b0bc-7cd554be2ec5`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`1fdee873-fd9f-4102-8718-d759be9e3e99`).

On the fixed tree (the two end-to-end tests, the card's Restart and opening sentence, comments and the changelog entry):

- `cargo fmt --all -- --check`, the sleep check, `dprint check`, the changelog check,
  `cargo clippy --all-targets -- -D warnings` and the GUI clippy run with `--features agent`: passed.
- The restart-gap and host-terminal end-to-end tests,
  `cargo nextest run -p farhelm --test e2e -E 'test(/restart_gap/) | test(/^host_terminals::/)'`, locally: 14 passed,
  with no runtime skips (`10a001f8-d63a-4583-9f43-28921f732725`).
- The session pane, Restart, row menu and focus tests,
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^session_pane::/) | test(/^session_list::restart/) | test(/^session_list::menu/) | test(/^focus_tests::/)'`,
  locally: 302 passed (`cf1099b0-4a11-4347-9f3f-41d161bf70f5`).
- `python3 scripts/check-gui-goldens.py --update`, locally (`2faac9be-7389-4b30-b288-6a6f59a3ffc3`): every capture but
  the two recovery-card pictures matched its reference byte for byte. I published those two, and the goldens then passed
  against the published references (`dcf8e413-9e3a-4bcc-a6b5-598f3382a2ab`).

The rest of the workspace suite, the desktop app's tests and the smoke were reused for the fixed tree. After them, only
three things changed: the card's Restart click and its text, which the focused GUI tests and the goldens cover; the two
end-to-end tests' expectations, which ran again; and comments and the changelog entry.

Skipped:

- The Mac-only checks: no Mac-specific code changed.
