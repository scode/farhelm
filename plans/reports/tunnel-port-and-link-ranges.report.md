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
