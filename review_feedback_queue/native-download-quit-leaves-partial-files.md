# Desktop Quit leaves partial downloads behind

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

Quitting Farhelm during a file download can leave hidden partial files in Downloads indefinitely. Reopening the app does
not reclaim them.

## Details

Suggested bucket: `other`. Confidence: likely; the cleanup ownership and normal Quit boundary are established by code,
but Quit during a transfer was not reproduced. Source: adversarial file-security reviewer. Proposed drop: none. Possible
cover: none found; SPEC.md's terminal-download contract (`1420–1422`) promises no partial file after failure or
interruption, and its Desktop Quit section (`2259–2266`) retains ordinary cleanup while requiring prompt exit.

Introduced by `550e7e592a6489cfc475b71c678131de4d9f67e7`. `NativeSave::create` puts staging files in a private
`.farhelm-download-*` directory beneath Downloads (`crates/farhelm-helm/src/downloads.rs:163–176`). Cleanup belongs to
explicit handler cleanup or NativeSave/TempDir destruction (`190–216`). The embedded helm runs in the desktop process
(`crates/farhelm-ui/src/desktop.rs:447–467`), whose normal Quit terminates without running destructors, as documented at
`desktop.rs:637–643`. No startup recovery or separate cleanup owner for this prefix was found under `crates/`.

Start a slow native download, establish that its staging directory contains partial bytes, then use ordinary Quit.
Reopen and inspect Downloads. The existing cancellation test (`downloads.rs:461–546`) aborts a handler while its runtime
stays alive and does not exercise process termination. Interrupted native-download staging needs recovery that survives
the app's exit, without delaying Quit or deleting unrelated Downloads files.
