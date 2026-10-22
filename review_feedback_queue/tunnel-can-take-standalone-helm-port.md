# A localhost tunnel can take the standalone helm's port and expose the browser's helm credential

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

If you have ever signed in to Farhelm's browser UI on your Mac, the browser keeps a long-lived credential for
`http://127.0.0.1:7433`, the standalone helm's default address. When the desktop app later opens a localhost tunnel to a
service on a remote host that listens on port 7433, the tunnel takes that same address on the Mac, because no standalone
helm is holding it while the desktop app runs. The remote host's page then loads in your browser at exactly the address
that holds the helm credential, and its script can read it. If that tab is still open when a standalone helm is started
again on 7433, the page can drive the helm with full authority: type into any session's terminal on any host and create
or stop sessions. A remote host would gain execution on other hosts without your approval.

## Details

Found by the 2026-10-22 prioritized persistent review, group `001-helm-http-edge-1of3` (finding f1).

Suggested bucket: highest (security: a remote host gains a helm credential and, through it, execution across hosts).

Confidence: likely. Verified premises: the browser UI stores the device secret in origin-scoped localStorage, keyed by
the full origin including port (`crates/farhelm-ui/src/auth.rs:351-415`); a tunnel opens through the desktop link
opener, which is the user's system browser (`crates/farhelm-helm/src/tunnels.rs:274-305`). Unverified premise: how often
a Mac running the desktop app still holds a browser device secret for 7433 from an earlier standalone helm. The reviewer
read the tunnel port choice only as context (it belongs to another review group). Recording confirmed the bind code and
the origin guard against the reviewed commit; nothing was run.

The credential is origin-scoped:

- The browser stores its device secret in origin-scoped localStorage (`crates/farhelm-helm/src/auth.rs:322-324`,
  `crates/farhelm-ui/src/auth.rs:351-415`).
- The origin guard admits requests whose Host and Origin are exactly `127.0.0.1:<port>`
  (`crates/farhelm-helm/src/middleware.rs:143-186`).
- The authentication middleware admits any request that carries a stored device secret (`auth.rs:360-397`).

The design comment at `middleware.rs:127-142` and SPEC.md "Security", the "Client to helm" bullet, rest on the rule that
no credential is ever stored under an origin another account can serve. The residual SPEC.md accepts there is a port
squatter that is another local user binding the port while the helm is down.

The gap:

- `bind_listener` (`crates/farhelm-helm/src/tunnels.rs:230-235`) binds the remote service's port on `127.0.0.1` whenever
  it is free and falls back to an ephemeral port only when that bind fails. Nothing excludes the standalone helm's port,
  which defaults to 7433 (`crates/farhelm-helm/src/lib.rs:285-286`).
- The desktop app's embedded helm uses a fresh random port at each launch, so the standalone port is free whenever only
  the desktop app runs. The two cannot serve the same state directory at once, and a standalone helm refuses tunnel
  requests, so tunnels only exist while 7433 is likely free.
- A remote host decides which port its service listens on, and an agent there can request the tunnel. Tunnels open after
  the user's approval or under the blanket tunnel consent.

Scenario:

1. The user signs in once to the browser UI of a standalone helm on 7433, then switches to the desktop app, the
   recommended client.
2. A compromised or malicious remote host (or an agent on it) serves a page on its port 7433 and gets a tunnel opened.
   The page loads in the user's browser at `http://127.0.0.1:7433` and reads `localStorage`, which holds the device
   secret.
3. Later the desktop app quits and a standalone helm starts on 7433 while that tab is still open. The tab's script now
   reaches the real helm same-origin: Host and Origin match, `Sec-Fetch-Site` is `same-origin`, and it carries the
   stolen secret. The helm admits it, giving terminal input, session creation and prompts on every host.

The secret itself stays valid until the user rotates the token. Step 3 needs only a same-origin page, and the tunnel
page is one. The reviewer also confirmed that a tunneled page on a different loopback port is refused by the
Sec-Fetch-Site rule (`middleware.rs:219-225`); the gap is specific to a tunnel landing on the helm's own port.

A related, weaker effect of the same bind (noted at recording, not by the reviewer): a bookmark or reopened tab for the
standalone helm's address loads the remote host's page while the tunnel holds 7433, so a remote host can show a
lookalike token prompt there. SPEC.md accepts the lookalike prompt only for another local account squatting the port.

How to verify: read `tunnels.rs:230-235` and confirm that no port is excluded from the preferred-port bind. In a browser
profile holding a device secret for `127.0.0.1:7433`, open a tunnel to a remote service listening on 7433 and run
`localStorage` in the devtools console of the tunneled page.

What a fix looks like (one or more of):

- Make the tunnel's preferred-port bind refuse the standalone helm's port (the configured or default 7433, and any port
  a standalone helm on this state directory has served) and fall back to an ephemeral port, as it already does for busy
  ports.
- Spell out the residual explicitly in SPEC.md if the maintainer accepts it.

Possible cover, for triage to judge: SPEC.md "Local authority and trust between hosts", the localhost-tunnels paragraph,
says the user accepted that a tunneled page on `127.0.0.1` can read other services' cookies and that "a Mac program
expecting that port may reach the remote host instead" (added by the `tunnel-port-can-divert-local-traffic.md` outcome
in `TRIAGE_OUTCOMES.md`, decided 2026-10-18 as an aid, not a safeguard). Neither mentions Farhelm's own helm credential
or its origin, and the same section says its acceptances never relax the remote-host boundary. SPEC.md "Security",
"Client to helm", accepts the port-squatter gap only for other local users. The tunnel queue items
`removed-host-strands-tunnel-listeners.md` and `tunnel-consent-conflicts-with-restart-dialog.md` do not cover it.

The reviewer filled in `proposed_drop` on the possible-cover reading above, adding that the trigger is uncommon: it
needs earlier browser-UI use on the same Mac, a remote service on the helm's port, and a later standalone helm start
while the tab stays open. It was recorded rather than dropped because the cover does not match the consequence
(credential theft and cross-host execution rather than a diverted connection).

## Independent report from the tunnels review

The same review's group `030-tunnels-1of2` (finding f2) reported the same defect independently, with the same location
(`tunnels.rs:229-235`) and the same suggested bucket (highest). Its confidence is likely: the port preference with no
exclusion, the 7433 default and the localStorage key and origin are confirmed from code. It adds the following.

Persistence without an open tab. Step 3 above needs the tunneled tab to stay open until a standalone helm starts. The
tunnels reviewer describes a route that does not: while it holds `http://127.0.0.1:7433`, the remote server answers `/`
and the UI's known asset paths (public in the open-source build) with a long-lived `Cache-Control` (for example
`max-age=31536000, immutable`), or the page registers a service worker (`127.0.0.1` is a secure context, so registration
is allowed). When the user later opens the standalone helm at that address, the browser may serve the cached document or
let the worker answer the navigation. That code is same-origin with the real helm, so the Origin and `Sec-Fetch-Site`
checks in `crates/farhelm-helm/src/middleware.rs` pass, and it can read the stored device secret and call every
authenticated API, which to the helm are ordinary GUI requests with no approval card. Recording found no `Cache-Control`
header set anywhere in `crates/farhelm-helm/src` at the reviewed commit, so the real helm's responses do not mark the
document uncacheable. Unverified premises for this route: that a browser serves a long-max-age cached document on a
later typed or bookmarked navigation, and that a registered service worker survives the real helm's 404 for its script
on the next update check. Reading the secret needs neither.

Another way 7433 is free. Besides a standalone helm that is not running, the reviewer names a helm elsewhere reached
through an `ssh -L 7433:...` forward that is not up at the moment. The browser's localStorage, HTTP cache and
service-worker registrations for that origin persist either way.

Nothing marks the port as special. The tunnel's hover shows only host and port, so nothing tells the user that
`localhost:7433` is Farhelm's own address. A remote agent can also offer it through its `farhelm open` card.

The auth docstring states the property the tunnel defeats. `crates/farhelm-ui/src/auth.rs:353-357` says localStorage was
chosen because it is scoped to the complete origin including the port, unlike a cookie, so unrelated loopback services
cannot see it. The browser UI reads the secret under key `farhelm.device-secret` (`crates/farhelm-ui/assets/terminal.js`
around line 373, `events.js` around line 73).

Further fix options from that report: exclude any port recorded as having served a Farhelm helm UI on this machine, or
always use a kernel-chosen port for well-known Farhelm ports. As defense in depth on the helm side, serve the UI
document with `Cache-Control: no-store` and have the UI unregister foreign service workers on load; that limits cache
and worker persistence but not reading the secret. The spec's tunnel warning and SPEC_impl.md should state whatever
exclusion is chosen.

How to verify the persistence route: serve `/` from the remote host with `Cache-Control: max-age=31536000`, tunnel
`http://localhost:7433/`, then with the real helm running navigate to `http://127.0.0.1:7433/` and check in DevTools
whether the page came "from disk cache".
