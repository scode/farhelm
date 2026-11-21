# A stale registry read can skip clearing an older Suspend timestamp

Reviewed commit: 405b3a3dcec6216ed9252d7a5128de649eab8530

## TLDR

A failed registry refresh can prevent a successful reconnect from clearing an old Suspend timestamp.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed conditional source path, with no runtime observation. Start with an admitted actor whose last successfully
read row has no stamp. Explicit Suspend writes the stamp then publishes the confirmed snapshot
(tensorlake/lifecycle.rs:148–178, environment/lifecycle.rs:109–111). If the actor's row refreshes after that write fail,
the actor retains the old row even after the hold and next probe (manager.rs:4137–4163,5866–5884. List_hosts is a
separate, fallible read at store.rs:6753–6780,6816–6869). The next Running reply still advances the in-memory evidence
(manager.rs:4854–4857,4890–4894). When a dial settles successfully, the clear is skipped solely because the stale row
has no stamp (4197–4198,4899–4905). The Connected branch serves until loss or interruption (4237–4272). If Farhelm quits
from there, shutdown cancels actors and cannot flush a clear that was never dispatched (3570–3604). The scenario
requires the post-write row refreshes to fail while the Suspend write and subsequent dial can succeed. It is not a claim
that a stale row survives healthy refreshes, or that the current app displays retired attribution despite preserving the
evidence. SPEC.md:340–347 and SPEC_impl.md:6723–6727 govern attribution and the two retirement paths. Possible_cover:
SPEC_impl.md:6777–6781 permits a failed clear and restart, but this guard suppresses a write even when no clear fails
and a successful dial ends the ladder. Distinct from f5's undispatched retry window. Estimated bucket other, wording
only. The comment-only follow-up 84bf819d21f41e8a8b808b8cf90aa1ccc44981a1 and pinned tip
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f do not change this guard.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `tensorlake/f11`.
