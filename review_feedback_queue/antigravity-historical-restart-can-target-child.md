# The update can retain a subagent ID as the existing session's Restart target

Reviewed commit: c0835826d469bf115dff16c68da784ab8750cf85

## TLDR

After an update, Restart can stop Antigravity’s main session and try to resume a child conversation recorded by the
previous version.

## Details

**highest**. Confirmed by source and contract inspection.

The source confirms selection of the wrong ID and stopping the live main tree on that route. An actual successful resume
of the child's history depends on the vendor's acceptance of that ID and was not observed. The established consequence
is the stop and attempted relaunch outside the conversation promise, so retain estimated highest for independent drop
adjudication. Proposed_drop and possible_cover: SPEC.md, “Durability and resume,” 3837–3839 says “Historical stored
identities remain usable under the same per-type Resume rules, regardless of their source,” and SPEC_impl.md:4501–4502
says “Stored captures remain plain IDs, and Restart uses them without rereading the transcript.” SPEC.md:3769–3772 also
accepts continued use of a recorded target after tracking stops. These allowances cover stale identity behavior but
should be weighed against the exact resume and confirmation contracts (SPEC.md:1996–2008,2119–2133). The same-account
trust allowance at SPEC.md:5174–5196 does not dismiss an ordinary past vendor callback. The adjacent docs commits
022c0a7d727ee8059c582112d77a5c546b74c244 and 71f0d6bc6034882ea92843e856ff23f4ac327452 do not change this route, and the
pinned tip still does not gate Antigravity's pre-change captures.

The independent check found that historical-ID compatibility leaves this route open despite the promise to resume the
main conversation. Whether Antigravity accepts the child ID remains unverified.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `antigravity/f1`.
