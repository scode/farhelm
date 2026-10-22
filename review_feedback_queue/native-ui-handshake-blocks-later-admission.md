# Native UI handshake blocks later admission

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

An unauthenticated connection to the desktop framework's local listener can hold up later UI connections. Another local
account would have to find its random port and arrive before the window; that practical startup race is unverified. An
already-admitted window is unaffected by this mechanism.

## Details

Farhelm launches the framework at `crates/farhelm-ui/src/desktop.rs:205–241`. `Cargo.lock:1234–1287` pins
`dioxus-desktop` 0.7.10 with `tungstenite` 0.28.0. Cached dependency source for those versions shows the following path;
this is source evidence, not inspection of a release binary.

Dioxus binds `127.0.0.1:0` (`src/edits.rs:145–156`). Its sole accept loop calls the connection handler synchronously
(`203–206`). The handler performs blocking `tungstenite::accept_hdr` (`278–285`) before spawning its connection worker
(`300–304`). No socket read timeout or nonblocking mode is set in that path. The key check is a header callback
(`235–267`), so it runs only after the peer sends enough input for the handshake. Tungstenite's synchronous handshake
reads from the stream (`src/server.rs:50–68`, `src/handshake/mod.rs:39–54`, `src/handshake/machine.rs:44–45`). An idle
accepted connection needs no key to occupy that read and prevent later admission.

The listener is constructed before the page's onload connection (`dioxus-desktop/src/app.rs:83`,
`src/protocol.rs:165–172`). Retrying the webview does not close a different idle peer. This establishes an ordering
opportunity, not its duration or another account's ability to exploit it. Possible remediation belongs at the framework
handshake boundary; the review does not establish that a downstream fork is proportionate.

Suggested bucket: **high** for conditional startup unavailability, held in **provisional highest** for triage because
the proposed actor crosses the Unix-account boundary. Confidence: **confirmed static pre-authentication blocking path;
practical cross-account trigger unverified**. No credentials, execution, durable corruption, lost processes or user work
were demonstrated. No port-discovery method, release-platform timing, or OS access rule was verified. Source inspection
only; no reproduction was performed or requested after the user's stop.

Possible cover: `SPEC.md:2901–2903` accepts the framework listener's existence because removing it needs a fork. It does
not explicitly accept this handshake stall. Same-account interference and malicious remote-host denial allowances
(`3168–3196,3366–3400`) concern different actors. The browser-only local-account exception does not clearly cover this
native path. Filesystem-fallback deferral (`TRIAGE_OUTCOMES.md:6019–6046`) and log-only broken-window handling
(`12376–12386`) have different triggers. No exact Planned, BUGS, queue, ledger or filter coverage was found. Proposed
drop: none; independent assessment retained the conditional finding.

Provenance: persistent reviewer `A04-native-bootstrap/f3`, independent `native-static-10`, parent dependency/source and
coverage verification.
