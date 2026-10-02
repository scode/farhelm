# The desktop page can still request arbitrary local files through the framework's asset fallback

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

The desktop page can request files outside the embedded assets through the framework protocol. No script-injection
exploit was found; this is a native hardening concern.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F21 / SEC-ASSET-FALLBACK`, reviewer `security_general`, pass 1. Confidence: **possible**. Review disposition: **would
surface**. Queue priority at recording: **highest**.

Anchor against the reviewed commit: `crates/farhelm-ui/src/desktop/assets.rs:46`. Recorded from the completed review
without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording:
SPEC_impl.md's desktop asset discussion describes the registered assets prefix and removal of the bundle-directory
fallback. The finding concerns unclaimed paths outside that prefix. No injection exploit was established; do not restate
this as demonstrated remote credential disclosure.

Farhelm's embedded-asset handler protects only desktop URLs whose first path segment is literally `assets`. Other paths
retain the default behavior of the pinned Dioxus 0.7.10 desktop framework. That framework passes unregistered paths to
its asset resolver, which percent-decodes the path, accepts an existing absolute filesystem path, and reads its
contents. A page request such as `dioxus://index.html/etc/passwd` therefore reaches the local filesystem instead of
Farhelm's fixed embedded asset set. Neither the handler registration nor the desktop configuration installs a catch-all
refusal.

No script-injection path was established, so this is a native-client hardening finding rather than a demonstrated remote
file-disclosure exploit. If a flaw in the page or terminal dependency allows injected script to run, however, this
capability exposes files readable by the desktop process, potentially including unrelated credentials. SPEC.md
explicitly calls for proportionate native-app hardening against hypothetical flaws. Restrict the custom protocol to
required Dioxus internal endpoints and embedded assets, rejecting other paths before the filesystem fallback.
Investigate the framework's extension points or a narrow dependency patch; enumerating filesystem top-level directories
as routes would not establish the boundary. Test that an embedded asset loads while a known external fixture cannot be
read, including percent-encoded variants.
