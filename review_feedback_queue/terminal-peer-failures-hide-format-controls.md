# Terminal notices hide formatting characters in host errors

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

Host-supplied terminal notices and attachment failures can hide or reorder characters in their explanations instead of
showing those characters visibly. These messages appear in Farhelm's controls, outside the terminal's own output.

## Details

Suggested bucket: `other`, for diagnostic presentation. Confidence: confirmed static production peer-to-render paths;
not visually reproduced. Provenance: retained `browser-terminal-1/f3`, second-turn synthesis. Proposed drop: none. This
is not HTML or script injection: `textContent` safely treats HTML as text. No independent execution, authorization or
data-loss consequence was established.

The banner assigns raw `textContent` (`crates/farhelm-ui/assets/terminal.js:2705`), as does attachment status (:2054).
The helm preserves `ControlMsg::Detached` reason/code (`crates/farhelm-helm/src/client.rs:1852–1864`) and forwards them
to the browser (`terminal.rs:328–330`). The browser displays decision codes `taken_over`, `tab_closed` and `stalled`
(`terminal.js:4839–4870`), except an established tab's silent reap. A fresh tab attachment refused with peer NotFound or
TakenOver becomes a decision detach carrying raw error text (`terminal.rs:530–544`). Generic `Other` detach reasons are
not a displayed-banner example and are excluded from this claim.

Upload `ControlMsg::Error.message` becomes an unchanged `SupervisorError` (`client.rs:2584–2600`), while
`UploadAborted.reason` becomes the upload ending reason (:1966–1969). Begin/commit errors and abort reasons pass through
`uploads.rs:92–98,179–185,343–349` and the raw HTTP error formatting in `lib.rs:2297–2312`. Browser upload reads that
body into an Error (`terminal.js:2391–2400`), inserts it into attachment status (:2445–2453), then renders it unescaped.
Ordinary supervisor staging failures can contain raw host paths
(`crates/farhelm-supervisor/src/service/uploads.rs:937–952`, forwarded as Error.message at :449–458); a hostile peer can
also supply the full reason and code. Fixed local network and stall strings alone do not establish this defect.

Apply the shared presentation-character escaping to peer values before rendering, preserving literal escape notation,
while keeping `textContent`. Check both decision-detach notices and upload failures with bidirectional and zero-width
characters. SPEC.md:2837–2841 requires visible escaping for host-supplied GUI text regardless of how it reached the
helm; terminal bytes are the exception, these controls are not.

Possible cover: no exact match. The ledger's “rename-refusal-text-bypasses-peer-text-rendering.md” and
“restart-refusal-text-bypasses-peer-text-rendering.md” concern Rust dialogs, and
“file-disclosure-misses-invisible-path-characters.md” concerns link disclosure. No matching Planned, BUGS or current
queue item was found. The rare-diagnostic filter is not established as full coverage of the explicit host-supplied
presentation rule; normal host paths can also reach these surfaces, with no timing race required.
