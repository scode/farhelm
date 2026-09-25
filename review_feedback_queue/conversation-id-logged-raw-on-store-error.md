# A reported conversation id is logged raw on the store-error path

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In a rare failure path, the supervisor log can show forged entries.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F17 / COR-CONVO-LOG-RAW`, tagged **definite**. Anchors and title:
`farhelm-supervisor/src/service/core.rs:13036-13041` — An unchecked reported conversation id is written raw into a
supervisor log line on the store-error path

Agent hooks inside a session report the agent's conversation id to the supervisor (`ReportConversation`, authenticated
by the session credential), so that a later restart can resume that conversation. In `Supervisor::report_conversation`,
one path writes a warn line containing the reported id: the session's in-memory entry is absent, and reading its row
from the store fails. That line formats the id with tracing's `%` (Display) (`conversation = %report.conversation`,
`core.rs:13036-13041`), which writes the string's bytes raw. At that point the id has been checked only for length
(`MAX_CONVERSATION_BYTES`), not for content. An id containing a newline would split the log line, and the part after the
newline could be crafted to look like separate, forged supervisor log entries.

SPEC keeps formatting and redaction promises even against same-account senders. Reaching this line is hard, though.
There must be no published in-memory entry for the session (normally only in the short gap between a create reserving
the row and publishing its entry), and the store read must fail. An attacker cannot easily force either. One of the
original reviewers considered the case and judged the trigger unreachable for an attacker; the merged finding keeps it
as definite but rare. The fix is small: log only the id's length on this path, or sanitize it the way the neighboring
`source` field is sanitized.
