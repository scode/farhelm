# A queued template can change the launch before its new settings are shown

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

A delayed template can change the program or wrapper before the launcher shows the change, and Launch can submit it in
that interval.

## Details

**high**. Confirmed by source and contract inspection.

Confirmed conditional ordering by source, no runtime observation. A `tl:` pick arriving during the catalog load enters
waiting_batches via view.rs:978–994. A later hand choice uses choices.rs:59–94 and view.rs:934–946 without retiring the
batch. A shell with a settled ordinary folder is launchable without catalog data (request.rs:328–364). On reply,
view.rs:484–506 applies the waiting edit; :1011–1032 mutates and synchronizes the draft then calls cx.notify, without
checking that a frame presented it. An applicable template can explicitly select a different valid agent or shell and
wrapper (templates/apply.rs:458–479 and :528–536). View.rs:2583–2595 attaches only Launcher::press, independent of the
target rendered at :2611. View.rs:1258–1277 and :1283–1306 sample live draft state while the shown-frame guard covers
source liveness only. The plain create serializes that selection at request.rs:609–624. Enter on an empty search also
calls press (view/search.rs:176–184). Prompt.rs:1149–1162 establishes that handling a key can precede actual
presentation even when the window drew early. This differs from f5 (wrong search row, with a further review opportunity)
and f11 (fresh-by-name direct launch explicitly accepted). SPEC.md:1390–1396 says the editable footer shows the chosen
program, host and destination beside Launch, and :1110–1113 sends the chosen, valid selection. SPEC_impl.md:8687–8699
permits waiting and says the user still owns Launch, not that a replay can overtake the displayed action. Estimated high
for launching a different program or configuration on an ordinary action. No established data-loss or security
consequence. `possible_cover`: existing waiting-batch rule permits deferred edits but does not bind submission to an
unpresented edit. The sequencing predates the landing, which introduces the shell selection/replay path. No later
launcher edit through pinned 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-launcher/f12`.
