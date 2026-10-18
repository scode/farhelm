# Tab can skip an empty initial prompt

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

With some agents selected, Tab jumps from the conversation selector back to Launch without reaching the empty initial
prompt. Shift-Tab from Launch skips it too.

## Details

The launcher's focus-trap selector omits `textarea` (`crates/farhelm-ui/src/list/create_form.rs:1594–1613`). The prompt
is placed after the conversation selector (`launch_controls.rs:516–592`). For Claude, Cursor, Grok, OMP and OpenCode,
non-Custom permissions and no browse results can leave no later control when the prompt is empty, causing early wrap.
Typing nonempty text adds Clear, which incidentally lets traversal reach the textarea; deleting all text restores the
defect. Include enabled textareas in the focusable selector.

Suggested bucket: **other**. Confidence: **confirmed statically**; no browser reproduction. Disabled textareas while
busy are correctly excluded. Later Custom, trust or browse controls avoid this particular early wrap. No security or
loss-of-work consequence is demonstrated.

Possible cover: none found. SPEC.md's Initial prompt and keyboard-access requirements do not accept skipping an enabled
editor; SPEC_impl's separate prompt-draft contract does not change its reachability. No matching Planned item, BUGS
entry, queue item, ledger decision or filter was found.

Proposed drop: none. Provenance: persistent Astra/high review, `ui-new-session-1/f1`.
