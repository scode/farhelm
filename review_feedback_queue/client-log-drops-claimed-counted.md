# Client-log drop warning claims drops are counted

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

During a desktop webview error storm, the helm log understates how many console errors were thrown away and promises a
count that never comes.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F13 / COR-CLIENTLOG-COUNT`, tagged **definite**. Anchors and title: `client_log.rs:197-219`, `client_log.rs:292-298` —
The client-log drop warning says later drops "are counted", but nothing counts them

`POST /api/client-log` lets the desktop webview forward its JavaScript console errors into the helm's log, so failures
inside the webview can be diagnosed. The helm budgets this at 60 entries per minute, shared across requests.
`RateWindow::account` (lines 197–219) works out how many entries each request may log. It reports a drop count only for
the first request in a one-minute window that drops anything. That request logs (lines 292–298): "client-log entries
dropped by server-side caps; further drops this window are counted but not logged individually".

Nothing counts them. Later drops in the same window are computed and discarded. `RateWindow` has no field that
accumulates them, and no code ever reports a total, at window rollover or anywhere else. The unit test near line 425
says later drops are "counted implicitly", which repeats the same claim.

An operator reading the log during a burst of webview errors will look for a total that never comes and will
underestimate how much evidence was lost. This is a logging-accuracy issue with no functional effect.

Suggested fix: keep a per-window dropped total and log it once when the window rolls over (still at most one line per
window). The text-only alternative is to change the message to "further drops this window are not reported".
