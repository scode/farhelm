## What this was about

Two security hardenings where a remote host reaches the user's own machine, from three findings you decided on the
2026-10-22 triage page.

- **Link opening.** When an agent asks Farhelm to open a link, the link counts either as public or as pointing at your
  own machine or network, and each kind has its own host permission to open without asking. Some IPv6 addresses that
  name something local counted as public: IPv6 spellings that carry a local IPv4 address (IPv4-compatible addresses, the
  NAT64 prefix and 6to4), the local-use NAT64 prefix and the old site-local range. With only public links allowed
  without asking, such a link opened without a card. You chose "Treat IPv4-embedding IPv6 ranges as local".
- **Localhost tunnels (Mac app).** A tunnel listened on the remote service's own port number when that number was free
  on the Mac. A remote service on 7433 then sat at the browser helm's default address on the Mac, where a browser that
  ever signed in to a browser helm keeps its sign-in, so the remote page could read it. And the listener allowed address
  reuse, which on macOS may let it take connections meant for another program serving the same port on all addresses.
  You chose "Always use a port the operating system picks; never reuse the remote number", which reverses the 2026-10-18
  same-port preference. The two tunnel findings share one fix, so they share one PR.

## Things you should know

- **Which IPv6 links now count as local.** IPv4-compatible, NAT64 well-known-prefix and 6to4 addresses are judged by the
  IPv4 address inside them, so a public NAT64 or 6to4 address stays public; the triage entry allowed this or treating
  the whole ranges as local, and this keeps ordinary public addresses from asking needlessly. The local-use NAT64 prefix
  and the old site-local range count as local whatever follows: site-local is scoped to one site, and a local-use NAT64
  address goes through a translator on your own network and does not put the IPv4 address in one fixed place, so its
  destination cannot be read from the text. Classification still never looks names up.
- **Tunnels now always use a port your Mac picks.** The tunnel's row and the opened link already used the port actually
  bound, so nothing else relied on the two numbers matching; users see a different port number on the Mac. If the
  operating system happens to pick the remote number, the helm binds again so that it never uses it. Address reuse is
  off for this bind too: on BSD-derived systems, a reusable socket's free-port search only avoids exact-address clashes,
  so it could still be given a port another program serves on all addresses.
- **What users may notice.** A remote app that writes its own `localhost` address and port into links or redirects no
  longer reaches the tunnel that way, since the tunnel's port differs. The changelog entry says to open it through the
  tunnel's link instead. The security warning shown before tunnels are allowed no longer mentions the remote port
  number, and neither do SPEC.md, SPEC_impl.md or the docs pages "Work in a session" and "Security model".
- **Not verified on a Mac.** The macOS address-reuse behaviour the second tunnel finding describes could not be
  exercised: Linux, the only system available, refuses that bind either way. The fix does not depend on it.
- **Changelog.** Link permissions shipped in an earlier release, so the link fix has its own new entry. Tunnels shipped
  in v0.30.0, but the entry about the old port warning has not, so that entry was rewritten to describe the new
  behaviour instead of adding a second one.
- **Size.** The link PR is about 93 changed lines, within the 100-line gate. The tunnel PR is 127 changed lines on a
  strict count, about 25 of which are the spec paragraphs being rewrapped around the edited sentences, so about 100
  lines of real change; I treated that as within "about 100".

## Open questions and possible follow-ups

- No test pins that the tunnel's socket is bound without address reuse; on Linux every test would still pass if that
  regressed. A test reading the socket option would take about 12 more lines, which would have pushed the tunnel PR well
  past its gate. I recommend it as a small follow-up.
- Two more IPv6 forms carry an IPv4 address and still count as public: Teredo (`2001::/32`) and the obsolete translated
  form `::ffff:0:0:0/96`. Neither was in the triage decision. Possible follow-up if you want them covered.

## The PRs

- #2630 fix: treat IPv6 links that embed a local IPv4 address as local
- #2635 fix: let the OS pick every localhost tunnel's port on the Mac

Each finding's `TRIAGE_OUTCOMES.md` Execution field names its change, bookmark and PR.

## Checks

Run now:

- Recorded `cargo nextest run -p farhelm-helm --lib -E 'test(browser::)'`: run 8ee1b19a-aea1-4d11-bd9f-86c010233931, 3
  of 3 passed (link classification, including the new per-range test).
- Recorded `cargo nextest run -p farhelm-helm --lib -E 'test(tunnels::)'`: run ae67faae-87c3-432a-9ee9-1b010d62d1ad, 19
  of 19 passed.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, `cargo fmt --all -- --check` and
  `cargo check -p farhelm-ui --features desktop` (the warning dialog's code is desktop-only): pass.
- Docs website build (`bun install --frozen-lockfile`, `bun run build`): pass.
- `python3 releasing/check-changelog.py format`, `dprint check` on the changed Markdown files, and
  `python -B scripts/check-test-sleeps.py` (314 test delays, none without a rationale): pass.

Reused: none. Skipped: the rest of the helm's tests, the UI tests and browser tests. The changes are confined to link
classification and the tunnel bind, both covered by the selections above, plus the removal of one sentence from a
desktop-only dialog, which nothing tests.

## Review gate

Each PR passed a fresh-context Opus 5.5 review at high effort, with no blocking findings. Applied: for the link PR,
stronger test cases (a 6to4 case that only the right embedding position classifies correctly, and a case pinning the
local-use NAT64 prefix's width), corrected comments and a changelog entry that names the setting users see; for the
tunnel PR, renamed tests and comments that still described the old fallback, the explanation of why address reuse stays
off for a kernel-chosen port, the rebind that makes "never the remote number" hold, an assertion that the opened link
uses the bound port, and a changelog entry that states what was exposed and the caveat above. Not followed: folding one
existing IPv6 check into the new helper (the standard library's version reads clearer), and the socket-option test above
(size gate).

### Landing

Landed as #2630 and #2635, merged bottom-up, with the content the report describes and one documentation fix described
below. It was the first of three plans in this landing round, before triage-docs-oct22 and resume-target-fixes-oct22.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained sign-in-preservation-sweep (a browser that signs in again reloads the page, and a
failed GUI preference save is reported and discarded) and two TODO.md commits. A fresh-context reviewer and I read this
plan against them and against the other two plans in the round:

- The sweep changes only the GUI's preference saves, sign-in and activity times. The reason this plan gives for never
  taking the remote port (a browser's sign-in to a browser helm lives at that helm's own address, `127.0.0.1:7433`)
  still holds after it: the sweep changed how a browser recovers from a refused sign-in, not where the sign-in is kept.
- No text anywhere still describes the old behavior of preferring the remote port number: SPEC.md, SPEC_impl.md, the
  warning dialog and the docs pages are updated, and no browser test ever described it.
- The tunnel offer's own check of a link only refuses malformed links, so the wider local-network classification does
  not change which links offer a tunnel. The web client has no link classification of its own that would need the same
  change.
- The plan edits the existing changelog entry for the tunnel warning rather than adding one, which is right: that entry
  has not shipped yet. The link classification entry is new and fixes behavior that shipped in earlier releases.
- The other two plans in the round touch neither tunnels nor links.

#### Fixes made during the landing

- The "Open a remote localhost link" section of the "Work in a session" docs page gained one sentence, in #2635: the
  port on the Mac is one the Mac picks, never the host's port number, so an app that puts its own `localhost` address
  and port into links or redirects does not reach the tunnel that way, and should be opened through the tunnel's link.
  The changelog entry already gave this caveat; the page, which never promised the same port, did not, so a user who
  hits it had nowhere to look it up.

#### Checks

The round's three plans change the helm's link classification and localhost tunnel port, the supervisor's Restart with
and its checks of which Grok or OMP process reports a conversation, and specification text. This round ran the helm's
and the supervisor's own tests, the browser test for clicking links in a terminal, and the source checks, on all three
plans stacked together on this round's main (which already held sign-in-preservation-sweep). Everything below except the
last item ran before the round's documentation fixes (in tunnel-port-and-link-ranges and resume-target-fixes-oct22),
which change only three docs pages and one changelog fragment:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`
  (tunnel-port-and-link-ranges removes a sentence from the desktop-only tunnel warning), the isolated test-sleep check,
  `dprint check` on the changed Markdown files, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (one warning, about a citation main already had), the docs website build, the
  workspace debug build and the UI's release web build: all passed.
- `cargo nextest run -p farhelm-helm` through the test recorder with pinned nextest and tmux, four slots, no retries,
  run `57e5a94a-29d0-4261-b9a5-697446134c41`: 1291 passed. This covers the link classification and tunnel tests
  tunnel-port-and-link-ranges added or changed, and the rest of the helm.
- `cargo nextest run -p farhelm-supervisor --lib`, same recorder settings, run `8604dc02-2297-49e6-a824-76be3cebf57d`:
  1258 passed, 4 skipped by design (helper entries that only run as child processes of other tests). This includes the
  Restart with tests that drive a real restart and every Grok and OMP process check.
- `npx playwright test tests/terminal-link-clicks.spec.ts` in `e2e/` on Chromium and WebKit, three workers each with its
  own helm and supervisors, no retries, run `6f350fd5-b9c0-42d3-a88a-f098d376cce2`: 30 passed.
- After the documentation fixes: the docs website build, `dprint check` on the changed Markdown files, the changelog
  format check and the instructions check passed again.
- Reused: none. Skipped: the rest of the Rust workspace (the farhelm binary's end-to-end tests and the UI tests; no code
  outside the helm and supervisor changed apart from removing one sentence from a dialog), the rest of the browser suite
  (only link classification reaches the browser), the doctests (no documentation example changed), the desktop smoke
  test, the native Mac app (not runnable here) and the installer (unchanged).

The report's checks ran the helm's link and tunnel tests and the website build; this round ran the whole helm crate, the
terminal link-click browser test on both engines and the website build on the plan as it landed. Nothing in the
executor's report became untrue at landing.
