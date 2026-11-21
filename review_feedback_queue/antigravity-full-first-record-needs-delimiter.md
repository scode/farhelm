# The byte ceiling can refuse a final line with no visible delimiter

Reviewed commit: c0835826d469bf115dff16c68da784ab8750cf85

## TLDR

A supported first transcript record at the read limit could be refused if it has no newline. Whether Antigravity writes
such a record remains unverified.

## Details

**other**. Unverified after investigation.

If the first transcript line fills the byte budget before its delimiter is visible, Farhelm leaves the capture as it was
until a later verifiable report. Proposed_drop and possible_cover: SPEC.md:3958–3962 explicitly treats “an unrecognized
first record, or a first record exceeding the 64 KiB read limit” as insufficient and permits a later Stop to recover.
Agent_kind/records.rs:92–101,110–130 describes and implements a reader where “incomplete final lines are left for the
vendor parser to judge”. The verifier at antigravity.rs:30–34 requires the visible boundary when the prefix is full.
Without that boundary it has no evidence the complete record fits, and no checked-in vendor format evidence establishes
that a complete supported 64 KiB first line omits the delimiter. The confirmed behavior is the intended fail-closed
limit, with no separate demonstrated vendor regression. Could be highest: no for the established refusal. Estimated
other if queued. The pinned tip retains the check. The callback itself returns independently of admission.

Independent check confirmed the verifier’s full-limit behavior. The missing premise is whether a supported Antigravity
version produces a complete record at that exact boundary without a newline.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `antigravity/f7`.
