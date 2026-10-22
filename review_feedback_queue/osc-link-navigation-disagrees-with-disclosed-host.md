# OSC link navigation can disagree with its disclosed host

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

A program-emitted terminal link can show an external destination on hover but open a path on the current Farhelm browser
origin instead. Browser behavior and consequential effects have not been reproduced.

## Details

The OSC link classifier accepts a parsed HTTP(S) scheme using `new URL(uri)` without a base, but retains the original
spelling (`crates/farhelm-ui/assets/terminal-files.js:98–100`). Hover likewise derives the displayed host and target
from a no-base parse (`terminal-links.js:211–248,578–605`). Activation forwards the original spelling with
`validated=true` (`terminal.js:3755–3757`), bypassing the stricter plain-URL predicate (`terminal-links.js:135–139`),
and the browser opener passes it unchanged to `window.open` (`terminal-links.js:51–56`). The visibility guard checks
whether the disclosure fits and is uncovered, not whether both parse contexts name the same destination
(`terminal.js:3659–3660`). Fresh hover therefore does not repair the mismatch.

For example, an authority-less spelling such as `http:example.test` is parsed without a base for disclosure but is
resolved relative to the document when opened. With a matching HTTP base scheme, the URL parser can retain the base host
and interpret the remainder as a path. This follows from the
[URL Standard's scheme and relative states](https://url.spec.whatwg.org/#special-relative-or-authority-state) and the
[HTML window-open steps](https://html.spec.whatwg.org/multipage/nav-history-apis.html#window-open-steps), inspected
during review. It is a standards-based inference, not observed behavior of the shipped browsers. Fully qualified
ordinary URLs do not have this specific ambiguity. The bundled terminal link provider retains the original URI.

No direct authenticated mutation, file read, or credential export is established. File operations require authenticated
POST requests (`crates/farhelm-helm/src/lib.rs:1009–1030`), and REST authentication requires an explicit bearer header
(`auth.rs:360–395,430–437`). The opener uses `noopener` and appends no credential. A failed navigation is not the only
possible result: an extensionless route can serve the UI (`lib.rs:1356–1369`), which can load its same-origin stored
credential and select a session (`crates/farhelm-ui/src/auth.rs:351–372`, `list/view.rs:2924–3007`). A fresh ordinary
terminal attachment can displace an existing one (`crates/farhelm-helm/src/terminal.rs:334–360`). Successful page and
asset loading, authentication, selection and displacement at the resulting path remain unverified premises.

The native `dioxus:` opener has a different base scheme; this same-scheme browser argument does not establish a native
effect. Native interception and OS dispatch were not exercised. File links use a separate canonical-path transfer flow.

Use the same resolved URL for disclosure and opening, or refuse authority-less web spellings before advertising them.
Keep the existing no-extra-confirmation contract. No test, probe, or reproduction was run for this finding.

Suggested bucket: `other` from the originating reviewer, with a potentially `highest` disclosure-boundary interpretation
from the interface reviewer. The index provisionally preserves the higher interpretation for triage; harmful security
effects are unverified. Confidence: source-confirmed parse-context mismatch; likely browser consequence with the limits
above. Provenance: persistent reviewers B01-file-recognition B01-f1 and B02-file-hover-authority synthesis.

Proposed drop: none. Possible cover: `SPEC_impl.md:2227–2228` describes unchanged forwarding, but does not expressly
accept incorrect destination disclosure. `SPEC.md:1921–1938` requires disclosure of the real target. The completed
`osc8-link-target-never-shown.md` ledger decision (`TRIAGE_OUTCOMES.md:2356–2378`) added that disclosure; it does not
settle this mismatch. The same-site-navigation decision at `3156–3168` addresses different hardening. No matching
Planned item, BUGS acceptance, queue item or exact ledger disposition was found. The rare safe-failure filter cannot be
assumed to cover the whole consequence while successful same-origin UI navigation remains unresolved.
