# Input before the restored tab is presented is unverified after investigation

Reviewed commit: 9864b874b3ac36670cb52a566ecbc08610a71430

## TLDR

Input might reach the restored shell while the old terminal is still on screen. The required timing remains unverified.

## Details

**other**. Unverified after investigation.

Unverified after investigation. The pane can change its dispatch target when the remembered tab row arrives, before the
user sees the switch. A key reaching the replacement shell while the Agent frame is still visible remains unverified.
Source checks: TerminalSet commits on row arrival (set.rs:524–545,590–613). SessionPane updates home on the notification
(session_pane.rs:445–465). Focus containment reads the current dispatch tree (focus.rs:724–731,740–767,806–812). GPUI
can draw at key dispatch before its interceptor, and only the frame callback presents
(vendor/gpui-pre/src/window.rs:2043–2095,3705–3708,3886–3901,6503–6509,6568–6577). The first key causing that dirty draw
cannot already send ordinary shell input: the newly built tab opens only after the deferred fit and becomes live only
after replay (attached.rs:989–1018,1065–1083,1178–1215,1379–1439,2410–2427. Vendor/gpui-pre/src/window.rs:2867–2874).
Additional synthesis checks: Wayland queues a dirty frame or waits for an already armed compositor callback
(vendor/gpui-pre-linux/src/linux/wayland/window.rs:1012–1057,2065–2077). Its input callback separately dispatches a key
without checking presentation (1577–1599, client.rs:2175–2208,2249–2254). This establishes separate ordering paths
without settling the exact required premise that first replay and a subsequent input can occur before a successful
presentation while the old Agent frame is still visible. Captured source-derived event/presentation evidence or an
existing deterministic test could settle it. No such test was in the assigned changes, and the charter allows no runtime
experiments. Estimate other for the established possibility of dispatch ahead of presentation with the first new-shell
press unable to send. SPEC.md:5395–5400 permits ignoring input for an unpresented frame. Could be highest if
wrong-program input is established. Possible_cover: SPEC.md:2250–2253 permits the late selection when the row arrives
before input, without granting consent to retarget input. No drop proposed for the retained uncertainty. F5 and
shell-presentation/f3 involve a different mechanism: an existing Close closure reads later consent and has no replay
prerequisite. Docs follow-up and the later Shell label do not change this input path at pinned tip
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `tabs/f7`.
