# Session titles are unescaped in delete/replace prompts

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A session renamed by an agent can look identical to another session in the list and in the delete prompt, so the user
may delete or replace the wrong one.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F28 / SEC-TITLES-RAW-IN-CONFIRMS`, tagged **possible**. Anchors and title: `crates/farhelm-ui/src/list/row.rs:1810`,
`crates/farhelm-ui/src/list/row.rs:1863`, `crates/farhelm-ui/src/list/row.rs:1522` — Session titles render unescaped in
the delete and replace confirmations and the visible row

This is the same peer-text rule as F27, applied to the sidebar. The delete prompt (line 1810) and the replace prompt
(line 1863) quote the session title as `"\"{session.title}\""` inside a `confirm-title` span. The row's visible title
(line 1522) is also `{session.title}` as-is. Only that span's `title` tooltip goes through `display_peer`. None of these
spans has direction isolation: the only `unicode-bidi: isolate` rule in `app.css` is on `.peer-value`.

Agents may rename any session, and the supervisor rejects only `is_control` characters, so bidi controls, zero-width
characters and U+2028/2029 get through. An agent can therefore make one session's title render identically to another's
(differing only by invisible characters), or place an override inside the quoted title so the prompt reads differently
from the session it acts on. `peer.rs` names spoofing of a confirmation as exactly the risk it exists to prevent, and
the delete prompt is the last guard before an irreversible action. The suggested fix is to render every visible title,
including both `confirm-title` spans, through `display_peer` inside a `.peer-value` element.
