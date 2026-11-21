# A failed host session refresh can suppress the Suspend question

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Suspend can skip its activity question when the host stays connected but Farhelm could only read an older idle or empty
session list.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed by source. The manager distinguishes a healthy connection from a healthy list read: `RefreshHealth::Failed`
stays under Connected and retains the previous list (manager.rs:346-370, :5686-5712); `is_connected` remains true
(:688-699). The existing manager test at :11964-11995 pins those expected states. No execution was performed. GET
/api/sessions reads what the helm already recorded, with no live supervisor read (sessions.rs:879-914). Rows get `stale`
only for a disconnected host (aggregate.rs:207-235), and the cap flag describes the retained list rather than the
refresh's success (aggregate.rs:514-553. Manager.rs:5928-5945). The host view already carries
`refresh: RefreshView::Failed` separately (hosts.rs:58-69, :96-109. Farhelm-proto/src/ui_channel.rs:1000-1015,
:1135-1152). The UI channel pushes changes before replying (ui_channel.rs:1049-1057), but activity_asks matches
`HostStateView::Connected { .. }`, discards refresh health, and treats any non-truncated list with no stale or active
row as conclusive (environment.rs:252-265, :484-517). SPEC.md:350-355 requires asking when activity cannot be checked
completely; :1620-1625 and :5137-5141 apply the same rule to environment sessions. This can skip the warning after
explicit Suspend and after the dedicated Stop and suspend choice when ordinary Stop does not independently require a
question. The inline confirmation is unaffected. Tests at environment_tests.rs:492-570 cover a running row, truncation
and a disconnected phase, without exercising Connected/Failed with a retained idle listing. Bucket other for a missed
warning before an explicit action that otherwise preserves other sessions, consistent with shell-runtime/f11.
Independent of shell-runtime/f10 and shell-contract/f9: those cases use a fresh but misclassified shell row.
Possible_cover: none. No later changes to affected paths through 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-permissions/f13`.
