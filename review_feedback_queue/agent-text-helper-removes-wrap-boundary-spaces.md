# Grid logical-line reconstruction removes spaces at soft wraps

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Browser agent checks can fail to recognize text that is present when a space lands at a soft-wrap boundary.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source-contract
defect. No actual failing suite run, vendor banner geometry, or measured flake is claimed; runtime reproduction is
forbidden, and the source finding does not depend on claiming a particular existing default viewport fails today.

Source locations: e2e/tests/helpers/real-agent.ts:154-172.

real-agent.ts:154,158 removes each row's trailing spaces before :170-172 joins continued rows, so it changes logical
text rather than just its display line breaks. The declared contract at :110-125 reconstructs logical lines; marker
waits use literal includes at :347-362 and are specified to find a substring at :434-471. The supplied ready/trust
markers at :105-107 contain spaces. In contrast, production grid-buffer.js:19-34,123-145 keeps blanks on continued rows
and removes only non-text wide-character padding (:91-105,140). Existing wrap coverage real-agent.spec.ts:374-405 uses
only x padding and a space-free marker. Suggested repair: preserve spaces across continued rows, retaining the
wide-padding exception, and trim at logical-line ends.

e2e/tests/helpers/real-agent.ts:154,158 strips trailing spaces from each grid row before :170-172 concatenates
soft-wrapped rows. A marker containing a space, such as 'Claude Code v' (:105-107), becomes 'ClaudeCode v' if its first
space ends the wrapped row, violating the helper's logical-line contract (:110-125). The grid's actual text semantics
preserve spaces on continued rows: crates/farhelm-ui/assets/grid-buffer.js:21-34,66-88,135-143. The existing wrap
regression (e2e/tests/real-agent.spec.ts:374-405) uses only x padding and a space-free marker, so does not detect this
case. Suggested repair: retain spaces through soft-wrap joins, trim only logical-line endings, and preserve the distinct
wide-character padding exception in grid-buffer.js:91-105,140.

browser agent checks can fail to recognize text that is present when a space lands at a soft-wrap boundary.

Possible coverage and caveats: none found. Coverage sources searched at ca9b0a19856dc136f63ad2d14f1076027edce1eb:
SPEC.md, SPEC_impl.md, TODO.md Planned (empty :23-25), plans/queue and INDEX, BUGS.md, review_feedback_queue INDEX and
files, and TRIAGE_OUTCOMES.md. The full matching plan acceptance section terminal-font-required.md:31-83 changes font
fallback commentary, not wrap-text reconstruction. SPEC_impl.md Testing:6742-6808 does not accept lossy test text; its
OSC-8 rules :1671-1688 address a different disclosure surface. The three complete BUGS.md entries do not cover this
cause or consequence. FILTER.md's rare/self-correcting diagnostic filter does not fully match a deterministic transform
under retained output alignment. Synthesis: retained separately from g173/f1's styled wide spacer and product selection
findings, which concern other paths and consequences. No security, user-work loss, or GUI-answer issue is asserted.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g198/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 9fcfe774c11b4bb2c57137f859f0e5545dbdc1a4.. This
was a source-only review; runtime occurrence and frequency were not measured.
