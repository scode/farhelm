# Saving a template shortcut can erase another saved binding

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

A number-key change in Templates can undo an agent's already saved binding on a different key if the helm is still
sending the window an older set of bindings.

## Details

Haiku pass H-P8-5, checked by gpt-6.2-sol. Suggested queue bucket **highest**. Confirmed source route to losing a saved
setting. A host command can hold the helm’s ordered GUI channel while the agent writes independently.

- Verdict: CRITICAL
- Route checked: `templates/shortcuts.rs:54–89,104–119`, `templates/dialog.rs:993–1016`, `templates/editor.rs:901–936`,
  `farhelm-helm/src/approvals.rs:876–891`, `farhelm-helm/src/agent_requests.rs:3272–3297`,
  `farhelm-helm/src/ui_channel.rs:960–968,982–1038`, `farhelm-helm/src/store.rs:5799–5800,5864`, and
  `SPEC.md:1270–1273,5119–5128`.
- Reason: A template approval releases the agent handler without waiting for its write. While a later channel command
  waits on a host, feed updates are deferred. The agent can finish saving an unrelated key in that interval. A Templates
  key click can then read the earlier pushed map and queue a complete replacement, which remains fixed even when the
  channel delivers the newer map before executing the queued write. The previously committed binding is lost. This can
  span the host wait. SPEC accepts races after a fresh read. The old browser issued that fresh helm read before
  calculating its write.

A Trash listing can hold the feed without taking the preferences key. The listing reads connected hosts, and closing
Trash leaves the reads in flight (`crates/gui/farhelm-gui/src/trash.rs:260–306,456–502`,
`crates/farhelm-helm/src/sessions.rs:624–631`). Answering the template approval releases the agent's handler to write
independently. When the Trash read gets ahead of the feed, the Templates key click builds and queues an already stale
payload (`crates/gui/farhelm-gui/src/templates/dialog.rs:1027–1045,1121–1134`).

Read current helm preferences immediately before creating each whole-map binding save, including the rename update. Hold
the feed with a shipped host wait, commit an agent binding, then click another key in Templates before the feed resumes.
Assert that both assignments survive.

SPEC.md:1270–1273 and :5119–5128 (“One GUI at a time”) accept an agent change between a fresh read and the write. The
pushed store widens the window back before the GUI click. Triage can choose a fresh read or an explicit specification
change. `quick-launch-clear-erases-new-agent-mark.md` and its ledger decision concern a ⚡ owner change after a fresh
read. The orchestrator deferred an immediate fix pending maintainer input.
