# Desktop sign-in declares success when the terminal credential was never installed

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

Desktop sign-in can appear successful while terminals, uploads and the event feed remain unusable because the window
kept a revoked or missing credential.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F8 / COR-DESKTOP-CREDENTIAL`, reviewer `correctness_data_flow`, pass 2. Confidence: **definite**. Review disposition:
**would fix**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-ui/assets/desktop-auth.js:100`. Recorded from the completed review
without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: No
matching existing queue item or prior decision was identified.

Desktop sign-in validates a credential and saves it in native storage, then tries to install it in the embedded page's
localStorage. If that write fails, the script swallows the error and still reports `ready: true`. Terminal connections,
attachment requests, and the event feed read their credentials from localStorage; they do not fall back to native
storage. When the key is empty or still contains a revoked credential, the application opens while those features
continue sending missing or invalid credentials. Reconnecting cannot repair the failed write, and native REST requests
may still work, making the partially broken window look authenticated.

The review reports a focused reproduction against the shipped authentication function in which a throwing storage writer
leaves the revoked value in place while readiness succeeds; the source confirms that failure path. Treat failure to
install the webview credential as authentication failure and use the existing error/Retry surface, or provide a current
in-memory credential that every affected consumer actually reads before declaring readiness. Cover failed `setItem`,
including a stale prior credential.
