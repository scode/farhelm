# UI channel command backlog can buffer ~4 GiB of request bodies

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

A client holding the helm's device credential can make the helm buffer about 4 GiB of command bodies and run out of
memory, which for the embedded helm exits the app. The window itself never does this; it takes a broken or compromised
client, and a credential holder can already do worse through the API, so this is hardening rather than a new authority.
Sessions survive under their supervisors; an app restart recovers.

## Details

**other.** Source inspection; commissioner confirmed the constants and the count-only admission check. Hardening: the
trigger requires the per-launch device credential (other local users cannot obtain it; same-account processes are inside
SPEC.md's local trust boundary), and no established security or user-data-loss consequence follows (lost GUI drafts are
spec-accepted as not user-owned work, `review_feedback_queue/AGENTS.md` "Priority buckets").

`crates/farhelm-helm/src/ui_channel.rs:91` sets `MAX_CLIENT_MESSAGE = 4 MiB`; `:130` sets
`MAX_QUEUED_COMMANDS = 2 * LIST_SESSIONS_CAP = 1000` (`crates/farhelm-proto/src/lib.rs:334`, cap 500). The admission
check in `run` (line 1037) bounds only the count, not bytes: ordered commands queue with their full JSON bodies
(`ordered_queue.push_back(command)`) and run one at a time; unordered ones hold bodies in flight
(`unordered.push(channel.start(command))`). 1000 near-4-MiB bodies ≈ 4 GiB; the incoming queue (`INCOMING_QUEUE = 64`,
line 124) adds up to ~256 MiB more. The comment at lines 122-129 sizes the allowance for "one pending edit per session"
(small bodies) — the product of the two bounds is enormous relative to that intent. The `MAX_CLIENT_MESSAGE` doc (lines
87-90) says the bound "only keeps a broken peer from making the helm buffer an unbounded message", but the backlog
multiplies it back out.

Verify: inspect the two constants and the count-only check; or in a test harness send 1000 ordered commands with
near-4-MiB bodies over an authenticated channel and observe helm memory growth. Fix shape: bound total queued body bytes
alongside the count, or lower `MAX_CLIENT_MESSAGE` to the largest legitimate command body (template/credential saves are
far smaller and their routes bound their own bodies).

Reviewer metadata (area 1 chunk B, helm UI channel): suggested bucket: other; confidence: confirmed arithmetic, OOM
consequence likely but machine-dependent; possible cover: none. Commissioner validation: constants and admission logic
confirmed at the cited lines; coverage searched (queue, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue) — no match.
FILTER.md "Rare, self-correcting glitches" considered and not matched: an OOM app exit is not a briefly-wrong display, a
safely-failing retryable operation, or an imprecise diagnostic.
