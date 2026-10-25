# A tunnel can reuse a custom browser helm origin

Reviewed commit: 93047acad011ff100d9dca286c6bd20d23e5ddda

## TLDR

A desktop tunnel can receive a previously used custom browser helm port and expose its retained device credential to
remote HTML in the same browser profile. The default helm port and embedded desktop credentials are outside this
conditional route.

## Details

Suggested bucket: **highest**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-helm/src/tunnels.rs:245-257,430-435`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed 93047acad011ff100d9dca286c6bd20d23e5ddda. Independently read g263 synthesis and its added source: UI
main.rs:37-57 derives API base from location.origin; api.rs:1929-1960 returns the exchanged device secret to the storing
token form. Helm client/tcp_relays.rs:100-128 and supervisor service/tcp_relays.rs:128-154 carry remote bytes unchanged,
allowing remote HTML to read the origin's farhelm.device-secret and return it over its own tunnel. UI
assets/desktop-auth.js:54-61,116-132 scrubs only the native webview's window.localStorage; it cannot clear the separate
external system browser origin. This resolves the cleanup defense without adding a separate finding. Existing scoped
lifetime, browser-profile, unrevoked-secret, available custom P and conditional allocation caveats remain. No
default-port or embedded-credential impact asserted. Old fallback allocation already allowed related collisions:
preserve pre-existing attribution. Exact allocator decision is implemented but does not explicitly authorize the
separate credential consequence (pinned SPEC.md:3280-3283 vs 3314-3321; SPEC_impl.md:1870-1879;
TRIAGE_OUTCOMES.md:18473-18518). DISAGREE with whole-coverage exclusion; proposed_drop none. Nine critical
binder/auth/opener/relay/UI files have empty review-to-pinned-tip diffs, independently checked. No new defect,
correction, withdrawal or runtime observation. f2 and GUI source refutations remain narrowly scoped.

The complete trigger uses a custom standalone helm port P (`crates/farhelm/src/lib.rs:281-286,2229-2238,2305-2334`),
followed by stopping that listener and opening a desktop tunnel to a different remote port Q. The kernel can choose the
now-free P, and the rewritten address opens in the external system browser (`tunnels.rs:393-435`; `desktop.rs:505-518`).
In the same browser profile, authentication stored the device secret under that exact origin
(`auth.rs:350-371,404-416,461-476`). The durable device-session check has no expiry or port binding
(`crates/farhelm-helm/src/auth.rs:228-254`; `store.rs:4849-4896`), and stopping the standalone listener does not revoke
it (`lib.rs:2345-2355`). A remote page served through the tunnel can therefore obtain a still-valid secret. Later
rotation or eviction can defeat this trigger. No collision frequency or actual credential theft was measured.

Possible cover: the kernel-allocation decision and accepted cookie/private-network limitations do not explicitly accept
this retained local-storage credential consequence. The default helm port and native embedded-helm credential path
remain excluded from this finding.

Provenance: `a042/f1`, `g263/f1`, `g138/f3`, `g204/f2`, `g205/f1`. These are reviewer-local identities, not additional
findings. Proposed drop: none for the retained remainder.
