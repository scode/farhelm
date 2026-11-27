# SPEC_impl.md's CLI restart paragraph and `farhelm agent restart`'s `--mode` refusal still state that every restart resumes the session's own conversation

Reviewed commit: 4f0692f0387d6d35039f2e2125d2924569953d14

## TLDR

An agent that passes the removed `--mode` flag is told "every restart resumes the session's own conversation", and the
implementation spec says "Restart always resumes", but since this change a Claude session whose reported transcript does
not exist is restarted fresh. The text is now slightly wrong for that case; no behavior is affected.

## Details

4f0692f0 amended SPEC.md (Restart), SPEC_impl.md "Restart only resumes" and the capture-provenance paragraph with the
Claude no-transcript exception, but left SPEC_impl.md:7850 ("Restart always resumes and forwards the consent unchanged
... refused with a message saying every restart resumes"), the CLI message at crates/farhelm/src/main.rs:887-889
("farhelm agent restart no longer takes --mode: every restart resumes the session's own conversation, so drop --mode")
and the hidden flag's doc at crates/farhelm/src/main.rs:349. Fix: qualify the SPEC_impl sentence with a pointer to
"Restart when Claude saved nothing", and reword the CLI refusal (for example "Restart has one kind; drop --mode") so it
does not promise a resume.

Reviewer revision notes: Revision (turn 2): extended to the user-facing CLI text found on re-check.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (wording inconsistency; the CLI message is shown only to an agent using a removed
  flag).
- Confidence: confirmed (text present at tip).
- possible_cover: none known.
- proposed_drop: none.

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g16/f2 (finding), group g16 "Claude restart fresh when no conversation was saved: spec and supervisor", findings log
  line 5.
