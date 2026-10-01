# A font that never loads makes every terminal reconnect leak memory

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If the terminal font fails to load in a browser, a terminal left trying to reconnect (for example overnight) keeps every
failed attempt's terminal in memory, so the page's memory grows without bound.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F56 / COR-FONT-PROMISE-LEAK`, tagged **possible**. Anchor and title:
`crates/farhelm-ui/assets/terminal.js:3264` — a terminal font that never loads makes every terminal mount leak.

The browser terminal uses the JetBrains Mono font, and `crates/farhelm-ui/assets/terminal.js` tracks whether its regular
and bold weights have loaded with two page-wide tracker objects, each exposing a `loaded` promise. When a terminal is
mounted before the font is ready, the mount attaches a `.then` callback to that page-wide promise
(`terminal.js:3264-3278` for regular, `:3299-3303` for bold) so it can switch to the right font and refit once it
arrives.

If the font never loads, that promise never settles. The code says so itself: the comment on the callback explains that
`loaded` never rejects and "a weight that fails just leaves it pending forever". This also happens when the browser has
no Font Loading API, where the trackers are a dummy whose promises never resolve (`dummyFontWeight`, line 431), and when
the polling gives up at its deadline. A callback attached to a pending promise is never released, and this one closes
over the whole mount: the xterm terminal object, its DOM, and its socket. The mount's `alive` flag stops the callback
from acting on a disposed terminal, but it does not stop the promise from holding on to it.

That would be a small fixed cost per mount, except that the automatic reconnect logic makes a fresh mount on every
attempt (`runReconnect` calls `mountWhenReady` each time). A terminal left in recovery overnight therefore accumulates a
retained terminal instance for every reconnect attempt, potentially thousands, so a long-lived page grows without bound
in exactly the recovery situation the reconnect logic exists for. The open premise is that the font load actually fails
on that page; with a working font the promise resolves and everything is released.

The suggested fix is to subscribe once per page instead of once per mount: keep a set of live terminals awaiting each
font, add to it at mount, remove from it in unmount and in the mount's rollback path, and have a single `.then` walk the
set when the font arrives.
