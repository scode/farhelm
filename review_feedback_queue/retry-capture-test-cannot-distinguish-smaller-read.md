# Timeout retry test does not establish that its capture takes fewer rows

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The migration regression can pass if a timed-out history capture is retried at its original size, leaving the promised
smaller retry untested.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by static
source.

Source locations: fba73468a9cb09c217a39da2be7f2c7cea064cb8:crates/farhelm/tests/e2e/host_migrations.rs:692-735.

host_migrations.rs:666-684 names and explains retrying fewer rows so a capture that is too expensive can finish. Its
fault at :692-701 counts calls and returns TimesOut for the first three, then Ok. Assertions at :721-735 establish two
attempts on refusal and four total attempts on success, followed by the ordinary seeded-history helper. The fixture at
:84-105 prints sixty history lines, at 80x24 (:237-238); the helper at :271-291 checks the same early history lines,
blank old screen and default modes. None observes which row allowance the retry uses. At the tip, host_restart.rs:297
sets TMUX_SEED_RETRY_SHARE=4, :788-796 computes the capacity and :809-829 passes capacity/share to capture_pane_history.
DEFAULT_SCROLLBACK_LINES is 12000 (hosts/terminal.rs:318 and tmux.rs:59), and snapshot/mod.rs:604-607,703-711 leaves
that full capacity at 80x24, giving a 3000-row retry. Both a full-size retry and the required quarter keep all of this
tiny fixture's history. tmux.rs:4152-4173 confirms the passed row allowance controls capture and the trim flag; the test
asserts neither. The production reduction is currently correct; this is a regression-oracle defect, not a shipped
timeout failure or observed flake. Fix: provide a bounded history fixture exceeding the quarter allowance, inject the
first timeout using the existing seam, and assert the retained newest range and trim state; alternatively observe the
allowance directly without adding a broad new seam.

the timeout fault closure counts calls and returns TimesOut for the first three, then Ok; assertions check two refused
reads and four total reads plus the ordinary small seeded history. The fixture prints only sixty history lines, so the
successful retry retains the same lines whether its row allowance is reduced or not.

Possible coverage and caveats: none found. Searched SPEC.md, SPEC_impl.md, TODO Planned, unlanded plans/queue, BUGS.md,
queue INDEX and queue text, TRIAGE_OUTCOMES.md. SPEC_impl.md:2195-2265, especially :2218-2219, requires the smaller
retry; it does not accept this missing oracle. TODO Planned is empty at :23-25; its Near-term migration item :181-213
concerns hidden terminals and is unrelated. BUGS.md:1-100 has no matching test exception. Queue INDEX:1-1170 has no
matching item; source-name and timeout/quarter searches found no duplicate queue or ledger entry.
TRIAGE_OUTCOMES.md:11976-11986 discards a different test when a deterministic fix needs a new production hook; this test
already has a timeout seam and can instead use discriminating history, so that is not established coverage. No FILTER.md
filter covers this ordinary false-pass test gap.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g191/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is fba73468a9cb09c217a39da2be7f2c7cea064cb8. Tip
persistence: git diff against ca9b0a19856dc136f63ad2d14f1076027edce1eb is empty for both this test file and
service/core/host_restart.rs.. This was a source-only review; runtime occurrence and frequency were not measured.
