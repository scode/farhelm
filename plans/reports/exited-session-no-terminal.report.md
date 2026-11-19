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
