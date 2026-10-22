# Cached directory UI can skip navigation admission

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

A previously cached standalone Farhelm page served with `--ui-dist` can be reused when a link from another loopback page
opens it, without asking the helm to enforce its navigation guard. If the real UI then starts with a saved valid
credential and selects a healthy terminal, its ordinary first attachment could displace another viewer. The cache
behavior was reproduced with inert matching responses; that complete Farhelm sequence remains unverified.

## Details

Suggested bucket: provisionally **highest**, for the conditional browser navigation trust-boundary bypass. Confidence:
**likely Farhelm defect**. Response-policy omission and the isolated cache trigger are confirmed; real application
loading, authenticated selection and viewer displacement were not observed together. No credential disclosure, arbitrary
execution, process termination, user-work loss or iframe bypass was demonstrated.

`crates/farhelm-helm/src/middleware.rs:87–125,184–224` bases navigation admission on Origin and Sec-Fetch-Site without
adding a corresponding cache restriction or response variation. The directory UI path at
`crates/farhelm-helm/src/lib.rs:1421–1428` uses tower-http 0.7.1 ServeDir/ServeFile, which supplies Last-Modified and
ETag. The only outer layer adds the build stamp. A browser reusing a fresh cached document makes no request through that
guard. Framing headers remain present and do not prevent a top-level navigation.

An isolated Linux fixture reproduced this in Chromium 156.0.8078.4 and WebKit 27.2 using Playwright 1.64.0. It served
the unchanged inert UI fixture with directory-style headers and the file's actual roughly one-hour-old modification
time. After directly visiting an exact `/index.html` URL, an ordinary link from another `127.0.0.1` port reused the
cached 200 response with zero target document requests. The same happened from `127.0.0.2`. Every unprimed control
reached the server and received 403, with the expected same-site/cross-site metadata. Bundled-style responses without
the file validators also reached the server and received 403. The corrected Chromium run removed Playwright's default
feature overrides; no cache, partitioning or web-security override was used. Recorder run:
`e3a561c4-c040-42d2-8577-47f6870e567d`. An earlier run with Playwright's default partitioning override was retained as
qualified evidence and is not the basis for the default-feature observation.

The source supports the conditional downstream effect. Browser authentication restores its origin-scoped credential
(`crates/farhelm-ui/src/auth.rs:351–375`; `api.rs:2009–2041`), and startup selects an eligible session
(`list/view.rs:2924–3008`). The session view constructs ordinary terminal paths and a fresh lease
(`session_view.rs:812–825,1749–1830,1878–1905`; `tabs.rs:182–217`). The first terminal synchronization counts as opening
(`assets/terminal.js:2968–2988`), leading to the ordinary path rather than the non-displacing reconnect path
(`3129–3150,3985–3990`). Helm `terminal.rs:334–342,184–207` and `client.rs:3548–3557,3617–3632` forward
`if_unowned=false`; supervisor `service/handlers.rs:1511–1629` removes other leases' attachments. These are static
connections, not a runtime witness of the combined sequence.

The fixture had no app scripts, credentials, WebSocket or supervisor. Verification still needs actual directory-served
responses and a compatible compiled UI completing asset loading and startup, restoring a valid credential, selecting a
healthy terminal, and displacing a separate fixture viewer. The exact primed URL and usable cache entry matter. Do not
generalize this result to `/`, expired or long-term disk caches, other file ages, bundled releases, public-site
navigation, macOS Safari or the native webview. The consented tunnel example is a macOS feature; the recorded browser
experiment ran on Linux. A response policy that prevents document reuse across admission contexts is a possible fix,
subject to validation against actual browser behavior.

Possible cover: `SPEC.md:2926–2928,3232–3235` explicitly refuses navigation from other loopback pages. The earlier
`TRIAGE_OUTCOMES.md:3156–3168`, "same-site-navigation-passes-origin-guard.md", discarded a hypothetical hardening
concern without this cache mechanism. `SPEC.md:3277–3284` still requires practical browser attacks; the missing
end-to-end premise matters to that judgment. No matching Planned item, BUGS entry or existing queue finding was found.
Do not dismiss the empirical limitation or treat the earlier decision as automatic acceptance of this mechanism.

Provenance: persistent-review A01-origin/A01-origin-2 and A01-routes/A01-routes-f1, consolidated as one underlying
finding. Both reviewers retained the full-product limitation. Runtime evidence is limited to the isolated cache fixture.
