# Link classification treats some IPv6 forms of local IPv4 addresses as public

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

When you let a remote host open public links without asking but keep local-network links behind a card, an agent on that
host can ask to open an IPv6 address that embeds a private or loopback IPv4 address in a form other than the usual
mapped one. Farhelm counts it as public, so the desktop app opens it in your browser without a card. Whether such an
address actually reaches your own machine or network depends on the network (an IPv6-only network with NAT64 is the
plausible case), and that has not been confirmed.

## Details

Found by the 2026-10-22 prioritized persistent review, group `032-link-opening` (finding f1).

Suggested bucket: highest only if the reachability premise holds (it would bypass the local-network link permission);
otherwise other. Recorded in highest because the premise is open.

Confidence: likely for the classification (read from code, and recording confirmed it); unverified for practical
reachability.

Mechanism: `permission_for_url` (`crates/farhelm-helm/src/browser.rs:57-80`) treats an IPv6 host as local only for
`to_ipv4_mapped()` (`::ffff:0:0/96`) with a local IPv4 inside, loopback, unspecified, multicast, unique-local and
link-local. Every other IPv6 literal is `Public`, including forms that embed an IPv4 address:

- the NAT64 well-known prefix `64:ff9b::/96` and local-use `64:ff9b:1::/48`, for example `http://[64:ff9b::c0a8:101]/`
  for 192.168.1.1 through a translator;
- deprecated IPv4-compatible `::/96`, for example `http://[::127.0.0.1]/`;
- 6to4 `2002::/16`, for example `2002:c0a8:0101::`;
- site-local `fec0::/10`.

With the host's public-link permission on and its local-network permission off, `approvals::ask_with_answer`
(`crates/farhelm-helm/src/approvals.rs:356-363`) returns `Allowed::Raw`, and `open_url` hands the URL to the native
opener with no card. SPEC.md "Agent-spawned sessions" (around lines 2542-2547) says "A local-network link always asks
unless its own permission is on", and SPEC.md "Local authority and trust between hosts" bounds which permission lets a
remote host make the browser load the user's own addresses.

Unverified premise: that any of these literals reaches the user's machine or LAN from a macOS browser. IPv4-compatible
addresses are generally unrouted on modern kernels. RFC 6052 says the NAT64 well-known prefix must not represent
non-global IPv4 addresses, and many translators refuse them. Whether a CLAT/464XLAT or home NAT64 setup forwards
`64:ff9b::<private>` to the LAN is unconfirmed. Verify on a NAT64 network by checking whether
`http://[64:ff9b::<router IPv4>]/` loads the router's page.

Fix sketch: also classify as local any IPv6 literal in `64:ff9b::/96`, `64:ff9b:1::/48`, `::/96` and `2002::/16` whose
embedded IPv4 is local per `local_ipv4` (or those whole ranges, conservatively), plus `fec0::/10`, and update the
SPEC.md list to match.

Possible cover, for triage to judge (the reviewer's `proposed_drop`): SPEC.md "Agent-spawned sessions" lists the
local-network categories, with "mapped local IPv4" as the only IPv4-in-IPv6 form, and says classification is lexical.
The code matches that list, and the extra forms are probably unroutable to the user's network in practice. The same
paragraph accepts that "a public-looking name may still resolve to a private address", but that acceptance is about DNS
names, not IP literals. SPEC_impl.md's link-request paragraph (around line 5566) also covers classification.
