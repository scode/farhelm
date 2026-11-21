# Retirement still has an old comment naming Agent as the notice recipient

Reviewed commit: 9864b874b3ac36670cb52a566ecbc08610a71430

## TLDR

The terminal retirement comment still sends maintainers looking for notices on Agent even when they follow the selected
shell.

## Details

**other**. Confirmed by source and contract inspection.

The retirement comment at attached.rs:1800 still says `say so in the session's next agent terminal`. The actual call at
:1801–1802 hands notices to session-keyed Carried. Attachments.rs:665–690 and set.rs:618–628 explicitly give them to the
currently shown terminal, as SPEC_impl.md:486–493 requires. The follow-up corrects the separate constructor comment at
attached.rs:633–638 and does not change retire. Proposed_drop: the stale inline sentence has no user-facing or
executable effect, and source refutes interpreting it as the notice-routing behavior. Source only. Other, could be
highest: no. Possible_cover: SPEC_impl.md:488–489 directly documents the actual restored-tab recipient. No behavior
change through pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Independent check confirmed that the routing works as designed and that the misleading comment remains.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `tabs/f16`.
