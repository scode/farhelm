# An older listing can replace the newly opened session with another session

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An older listing can permanently switch away from a newly launched session.

## Details

F104 — **definite** — `crates/farhelm-ui/src/list/view.rs:1438` — An older listing can replace the newly opened session
with another session

Create and Replace select their successful result without invalidating a listing request started before the mutation.
That older response can omit the new session, clear its selection, and choose a fallback session instead. A later fresh
listing does not restore the user's intended selection, so this is more than a brief stale display. Fence earlier
listing reads when accepting a create or replacement result, then fetch a fresh listing.

## Evidence and triage context

- crates/farhelm-ui/src/list/view.rs:2203–2211 and 3429–3498 accept and open newly created sessions without calling
  listing_reads.fence().
- crates/farhelm-ui/src/list/view.rs:1252–1259 gates incoming listings; crates/farhelm-ui/src/ops.rs:274–279 accepts a
  success newer than the last applied generation, even if a mutation occurred meanwhile.
- crates/farhelm-ui/src/list/view.rs:1399–1443 treats absence from a complete authoritative listing as grounds to call
  on_removed.
- crates/farhelm-ui/src/lib.rs:1619–1622 clears the matching current selection.
- crates/farhelm-ui/src/list/view.rs:2675–2699,2733–2746 chooses an older listed fallback for an empty selection; line
  2676 prevents later automatic restoration once that fallback is selected.
- crates/farhelm-ui/src/reader.rs:644–659 queues reads, so requesting a fresh listing does not itself invalidate the
  currently outstanding one.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- TRIAGE_OUTCOMES.md:5219–5241, restart-can-still-deselect-session.md, concerns a restarting session temporarily omitted
  by the supervisor. Its fix keeps that existing session listed during relaunch. It does not fence a pre-create snapshot
  missing a genuinely new ID.

Caveats:

- Requires the stale listing and fallback effect to land before the fresh listing.
- No wrong-target mutation or process loss was demonstrated.
- No runtime reproduction was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_12_cor:p1:F1`.

- `ui_desktop_12_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
