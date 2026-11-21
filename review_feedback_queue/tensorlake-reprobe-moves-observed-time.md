# The same suspension can look new after a failed status check

Reviewed commit: 84bf819d21f41e8a8b808b8cf90aa1ccc44981a1

## TLDR

An interruption in Tensorlake status checks can make the same suspension appear to have started later.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed by source, without runtime observation. An unanswered status permits a fallback dial, and the dial of the
still-paused sandbox can fail, replacing Suspended with Unreachable (405b3a3dcec6216ed9252d7a5128de649eab8530
manager.rs:4859–4868,4358–4363). When status answers Suspended again, the state no longer has observed_at to reuse and
the new time is used, even with no intervening Running evidence (4365–4388). The session card draws the new value as
“Suspended outside Farhelm, noticed” (environment_ui.rs:728–734 through session_pane/unavailable.rs:320–334).
SPEC.md:342–344 asks for suspension timing, and SPEC_impl.md:6723–6727 distinguishes observation time from Running-based
retirement. The allowed fallback at SPEC_impl.md:6716–6719 explains the temporary dial, without specifying a reset. The
runtime reset predates 405b3a3dcec6216ed9252d7a5128de649eab8530 (its parent, manager.rs:4342–4362). The assigned
follow-up's unqualified comment at manager.rs:610–612 is what overstates the behavior. No later manager change through
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. Estimated bucket other, wording only. Possible_cover: the fallback and
observation contract above.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `tensorlake/f8`.
