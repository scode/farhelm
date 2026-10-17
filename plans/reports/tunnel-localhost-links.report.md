### What this was about

A localhost link printed in a remote session opened the address on the Mac, rather than the service running on the
session's host. The Mac app now offers Cmd-click to tunnel and open that service, and an eligible agent link card offers
**Tunnel and open** first. Ordinary clicks and **Open** retain their existing meaning.

Tunnels use Farhelm's existing connection to the host, support raw TCP and WebSockets, and listen on IPv4 loopback on
the Mac. They prefer the remote port number and choose a free port when it is occupied. Reopening the same target reuses
its tunnel. The session view lists open tunnels with close buttons, including while the host is offline.

### Things you should know

The hover, Cmd-click, card button, warning, Settings switch and strip still need an interaction check in the Mac app
with an updated remote host. Linux validation exercised the native controller and compiled the desktop code, but could
not verify actual Mac browser opening, modal focus or hover geometry. The devbuild flow can provide that check.

Tunnels require acceptance of a prominent warning. That consent is shared by clients of the same helm and starts off.
Tunneled pages use `127.0.0.1` on the Mac: cookies are not isolated by port, browsers treat them as local pages for
private-network checks, and any program on the Mac can connect to the unauthenticated listener. Turning tunnels off in
Settings closes both listeners and active connections. Same-site navigation hardening also means ordinary links from
another local port into a standalone helm are refused; the address bar and bookmarks still work.

Remote hosts need updating for protocol 54. Helm schema 55 stores the consent choice. Open tunnels remain only in
memory: closing the app, deleting or replacing their session, or explicitly closing them ends their connections.
Stopping or restarting the agent preserves them. After a host reconnects, new connections use its current supervisor
connection; broken TCP streams are not replayed. Creating tunnels from Farhelm's web client, tunneling local-host
sessions and automatic login-callback forwarding remain unsupported. Opening a tunneled service in the Mac's browser is
supported. The Work in a session, Approve what agents do and Security model pages describe the feature.

### Open questions and possible follow-ups

The Mac interaction check above remains platform verification. There is no outstanding implementation decision.

### The PRs

- [#2219](https://github.com/scode/farhelm/pull/2219/changes): reject navigation into the helm from other local pages.
- [#2227](https://github.com/scode/farhelm/pull/2227/changes): relay remote loopback TCP over the supervisor connection.
- [#2229](https://github.com/scode/farhelm/pull/2229/changes): own listeners by session and enforce shared consent.
- [#2231](https://github.com/scode/farhelm/pull/2231/changes): terminal and card actions, warning, Settings, strip,
  specs, docs and removal of the covered TODO entry.

### Checks run, reused and skipped

The Rust child commands below ran through `scripts/record-test-run.py`, with pinned nextest, four slots, zero retries
and failure on an empty selection. Recorder-injected configuration paths are omitted. The real-supervisor run required
the pinned tmux substrate; pure in-process selections did not use tmux.

- `cargo nextest run --locked -p farhelm-helm --lib -E 'test(middleware::tests::)'`: 12 passed, recorder
  `e4d9999e-773a-461b-97e1-5d77194d4c92`.
- `cargo nextest run --locked -p farhelm-proto --lib -E 'test(tcp_relay) | test(protocol_version_is_pinned_at_54)'`:
  protocol state, wire correlation and version pin, 5 passed, `37428585-7163-4bfd-854d-7ad03d6c8920`.
- `cargo nextest run --locked -p farhelm-helm --lib -E 'test(client::tcp_relays::)'`: final cancellation and
  connection-loss selection, 3 passed, `3cc1ee81-6017-48e3-8a80-9be3f579ce5f`.
- `cargo nextest run --locked -p farhelm-helm -p farhelm-supervisor --lib -E 'test(client::tcp_relays::) | test(restricted_dispatch_refuses_an_off_list_message_and_any_create)'`:
  3 passed, `60b7c3b9-43ac-4e1d-8398-3307d5c483de`. Only its restricted-session refusal result is reused; that
  supervisor code is unchanged, while the final client run above supersedes its client coverage.
- `cargo nextest run --locked -p farhelm --test e2e -E 'test(tcp_relays::)'`: 5 passed,
  `7700f5a2-73f2-4422-b1d4-595f3810ab77`. It covers short HTTP requests without half-close, a WebSocket through
  IPv6-only loopback, traffic exceeding the credit window, invalid targets and channel cancellation. The owned
  supervisor and pinned tmux ran; there were no runtime skip markers.
- `cargo nextest run --locked -p farhelm-helm -E 'test(tunnels::) | test(tunnel_consent)'`: manager and consent, 18
  passed, `8a235500-7a2b-4f5a-b262-7f3e2d3a362f`. It covers real admitted-socket EOF on close, revocation, deletion,
  replacement and shutdown; failed persistence and deletion; cancelled consent callers; reconnect recovery;
  binding/reuse; and consent migration.
- `cargo nextest run --locked -p farhelm-helm -p farhelm-proto -p farhelm-ui -E 'test(approvals::) | test(browser::) | test(agent_origin_cannot_follow) | test(the_remembered_permissions_word_seeds)'`:
  approval, browser-handler, protocol and UI selection, 33 passed, `1a656491-d5a3-408f-8762-4d6ed4d8b420`. This includes
  a controlled ownership change while tunnel creation waits, refusal without settling ineligible cards, and exactly one
  raw or tunneled native open.
- `cargo nextest run --locked -p farhelm-ui --features desktop -E 'test(source_retirement_cannot_replace)'`: 1 passed,
  `138dd67d-2d62-4e6e-a139-48ceb2dd3c92`, covering queued cancellation and view retirement during an unresolved consent
  save. No runtime skip markers appeared in these Rust selections.
- The final combined JavaScript harness (`node --test` in `crates/farhelm-ui/js-tests`) passed all 289 cases,
  `47fcb929-b39b-4536-bd9a-6654a4da88c9`, including the actual exported tunnel classifier, gesture and dispatch helpers.
  The final docs build (`bun install --frozen-lockfile && bun run build`) produced 32 pages and validated internal
  anchors.
- Browser verification passed 96 cases, 48 each in Chromium and WebKit, recorder `f70644cb-f6bd-4c0d-8b7c-4f53d8e5dbd9`.
  The Playwright selection was terminal links, terminal link clicks, agent relay and Settings; the Settings pattern also
  selected notification settings. One worker, zero retries, no skips or failures. The recorded child invocation was
  `playwright test --workers=1 --retries=0 --repeat-each=1 --forbid-only --fail-on-flaky-tests --max-failures=0 --update-snapshots=none 'terminal-links\.spec\.ts' 'terminal-link-clicks\.spec\.ts' 'agent-relay\.spec\.ts' 'settings\.spec\.ts'`
  in `e2e`, using the repository config and recorder-owned reporters/output paths.
- Formatting, changed-document dprint, instruction links, changelog formatting and the test-delay checker passed; the
  delay checker inspected 303 calls and found no missing rationale. `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop` and the Dioxus release web bundle build passed on the final combined
  tree. Shipped-binary Clippy and the CLI/fixture build passed before the final UI-only rebase and remain applicable.

Evidence from the first three units was collected before the rebases. Their patches are unchanged by those rebases: the
earlier origin unit was `9e009c5e`, relay `f59ceb6e`, and manager `eee58efb`. Later approval integration retains the
manager's ordinary terminal path and adds the separately tested agent-origin constraint. The native controller method
tested above is unchanged by the final keyboard-access attributes on its warning. Its retained source fingerprint is
`c78e694b10f0`; the approval selection's is `e317d498da24`, and the manager selection's is `2d50de51c6a8`. These
identify the tested working trees, including edits above the recorded Git parent.

The browser run covered the fourth unit at `85fb3af5`, source fingerprint `278e2a1cdf59`, after the keyboard-scroll
rebase and before the quick-launch rebase. Both incoming changes were read in full. Keyboard-scroll's optional
approval-text property and CSS compile with this work; the warning body now also has a named Tab stop while saving
disables its buttons. Quick-launch's existing visible-dialog guard and modal-opening cancellation include the tunnel
warning. All four unit patches had empty interdiffs across the quick-launch rebase. The final 289-case JavaScript run,
desktop compile, web bundle and website build cover that combined tree. Repeating the 96 browser cases would add no
evidence for changed tunnel behavior, so their successful result is reused.

Two recorder substrate refusals and two deliberately interrupted pre-execution builds remain retained and are not
counted as passing evidence: `5400429c-bc27-4621-9201-7066874af004`, `afb0b43d-e4c8-4d32-b7d1-d328e56843be`,
`b1e8243f-68a2-4f5f-955f-8f7bd57f9e01` and `80c32fea-d9ad-4ec1-8c35-a7b29f69ddab`. Initial lint and native compile
errors were corrected before their successful reruns. The full workspace/browser batteries, installer and release checks
were skipped because these focused selections cover the changed transport, authority, lifetime and link behavior. Actual
Mac interaction was unavailable, as noted above.

### Review gate outcome

Each unit received the required single fresh-context gpt-6.1-sol review at high effort, including the full
test-authoring checklist where tests changed. Review found that a stalled local recipient could prevent a tunnel from
closing, that an agent's pending request could tunnel to a newly assigned host, and that leaving or cancelling a warning
while consent was saving could interfere with a later warning. The corrections make connection retirement interrupt
stalled writes, authorize the exact host used to create the tunnel, and retain the pending warning until its save
completes. Focused regressions passed for each correction. Follow-up review confirmed the host correction and prescribed
the final Cancel guard; the executor applied that guard and verified the extended native regression.

All required code reviews are complete and their findings are resolved. A separate review at the executing model
confirmed that the UI corrections fit the agreed scope without another coordination mechanism. Commit and PR wording
passed separate cold reads. The completion report passed the planning flow's independent cold read after revision.

### Landing

Landed in round 85, as #2219, #2227, #2229 and #2231, merged bottom-up, first of the round's two plans, and unchanged
from what the report describes.

#### What landed since the stack was based, and whether it interacted

Since this stack's base, main gained the session action shortcuts (#2218, #2221, #2230): Escape for header
confirmations, Cmd+W or Ctrl+Shift+Backspace to delete the open session, Cmd+L or Ctrl+Shift+L to rename it, and a
rebuilt Mac menu. The round's other plan, which keeps rapid arrow presses in menus from stepping from the wrong item
(#2232), lands after this one. The landing monitor read them against this stack, and a fresh-context reviewer did the
same independently. Neither found an interaction:

- The tunnel consent warning is a modal dialog, so the delete and rename shortcuts do nothing while it is open, and
  Cmd-click on a link does not match either shortcut. Deleting a session closes its tunnels.
- The quick-launch key sequence's existing guard against open dialogs also covers the consent warning.
- The terminal link changes keep the earlier link-safety rules: clicks and drags across links, the setting that hands
  link clicks to the terminal program, the strict web-address rule, and the wrapped-link mismatch warning still apply,
  with the tunnel hint added after them. A tunnel request never falls through to opening the raw link.
- Neither plan branches on a particular agent harness; tunnel checks depend on whether the session's host is remote.

The landing rebase was clean, and every PR's diff is identical to the one the executor delivered.

#### Upgrade and security

#2227 raises the protocol from 53 to 54, because older hosts cannot read the new relay messages, and the helm's database
moves to schema 55, which adds one consent setting that starts off. Neither 53 nor 54 has shipped (v0.29.0 carries
protocol 51), so the next release already requires remote hosts to be updated. The changelog entry and the report both
say so. An older helm refuses the newer database, as after every schema change.

The reviewer checked the security boundary and found nothing unsafe or contradicting SPEC.md. The Mac listeners bind
`127.0.0.1` only. Tunnels work only in the Mac app; a standalone helm refuses them, and sessions on the local host are
refused. Consent is checked when a tunnel is created, and turning it off closes every listener and connection. An
agent's tunnel request is tied to the exact host connection that asked. Both the helm and the supervisor confine the
target to loopback, and only the helm itself can open a relay, never a session's own connection. #2219 refuses
cross-port navigation into the helm, and SPEC.md and the security page describe the new trust boundary accurately.

#### Fixes made during the landing

None.

#### A decision for the maintainer

When planning this feature, the maintainer chose that a tunnel takes the same port number on the Mac as the service on
the host, when that port is free. That number comes from the link, and an agent decides which links it prints, so an
agent can pick which port on the Mac a tunnel occupies. If a tunnel is still open when a program on the Mac later
expects that port, for example a login flow that sends the browser to `127.0.0.1:<port>` to finish signing in, the
browser's request and whatever code it carries go to the session's host instead of that program. Every tunnel still
takes the user's own Cmd-click or **Tunnel and open**, even after consent, and link permissions never create one. The
warning says any program on the Mac can connect to a tunnel, but not that a tunnel can take a port a Mac program wanted.

The options: (a) keep the port choice and add that sentence to the warning, a wording-only follow-up; (b) always use a
free port Farhelm picks, which takes the choice away from the agent but breaks pages whose own links or redirects name
their original port, since that port on the Mac is not the tunnel; (c) both. The landing monitor recommends (a): it
keeps the behavior already chosen and closes the gap in what the warning tells the user.

#### Things to keep in mind

- Tunnels are gated on any desktop build rather than the Mac specifically, so a desktop build on another system would
  show the Mac wording. Only the Mac app ships, so this is a nit.
- The changelog entry mentions the new refusal of cross-port navigation into the helm only in a later paragraph; the
  release curation should keep it visible.
- As the report says, the Mac interaction has not been checked: the hover, Cmd-click, card button, warning, Settings
  switch and strip, with an updated remote host. That check should also confirm the new session shortcuts stay inert
  while the warning is open.

#### Checks

- Run now, on main with this stack and the round's other plan merged together locally: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, and the UI JavaScript
  harness (`node --test`, 293 passed: the report's 289 plus the shortcuts' new cases). The harness was run directly, not
  through the recorder, so it has no run id. These show the combined code builds and its JavaScript units pass. Whether
  the shortcuts stay inert under the tunnel warning rests on the code reading above, not on a test.
- Reused: every passing recorded run the report lists, including the relay end-to-end run
  `7700f5a2-73f2-4422-b1d4-595f3810ab77`, the tunnel manager and consent run `8a235500-7a2b-4f5a-b262-7f3e2d3a362f`, the
  approval and browser-handler run `1a656491-d5a3-408f-8762-4d6ed4d8b420`, and the browser run on Chromium and WebKit
  `f70644cb-f6bd-4c0d-8b7c-4f53d8e5dbd9` (96 passed), at the revisions the report gives. The Rust runs still apply
  because nothing that joined the base touches the helm, the supervisor or the protocol. The browser run predates the
  shortcuts; it is reused because the tunnel interface is Mac-only, so that run only shows browsers are unchanged by
  this stack. The shortcuts' own browser tests were not re-run on the combined tree.
- Skipped: everything the report skipped, for the reasons it gives.

Nothing in the executor's report became untrue at landing.
