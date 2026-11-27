# The supervisor refuses a notify reason by raw byte count, before cleaning, with a message that names a 2000 limit

Reviewed commit: 721dd96fe7315659b661faa0ede8656d5f4192a1

## TLDR

An agent running `farhelm agent notify` can be told two different limits for the same reason. A reason over 2000 bytes
is refused with "the reason is N bytes, more than the 2000 a notification may carry". If the agent shortens it to just
under 2000 bytes, the helm then refuses it again with "more than the 500 a notification may carry". Separately, a reason
whose cleaned form would fit (one with lots of padding whitespace or invisible characters the helm strips) is refused
outright, although SPEC.md says only an empty reason or one over 500 characters is refused.

## Details

`validate_agent_verb` (crates/farhelm-supervisor/src/service/handlers.rs:3685-3696 at the tip) checks
`reason.len() > AGENT_NOTIFY_REASON_CAP * 4` on the raw string and words the refusal as "more than the {cap} a
notification may carry", with cap = 2000. The helm's `notify_reason` (crates/farhelm-helm/src/agent_requests.rs,
`fn notify_reason`) first drops presentation-unsafe characters and collapses whitespace, then refuses above 500
characters with the same "a notification may carry" wording. The supervisor comment justifies the factor of four as
"four bytes for each character the helm would keep", but the raw string also holds characters the helm does not keep
(whitespace runs, zero-width and bidi characters), so the bound is not a superset of what the helm accepts. SPEC.md
(`farhelm agent notify`) says the helm "refuses an empty reason or one longer than 500 characters"; SPEC_impl.md says
only that "the supervisor only bounds the reason's bytes". Verify by reading the two functions side by side: an ASCII
reason of 2001 bytes gets the 2000 wording, and one of 1999 bytes then gets the 500 wording. The fix is to word the
supervisor refusal so it does not state a notification limit (for example, "the reason is N bytes, far more than a
notification may carry (at most 500 characters)"), and optionally to document in SPEC.md that a raw reason over 2000
bytes is refused before cleaning.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other. The reader is an agent, the refusal is recoverable, and no data or authority is
  involved.
- Confidence: confirmed (from source).
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g20/f1 (finding), group g20 "An agent telling the user it is blocked: helm, protocol, CLI", findings log line 1.
