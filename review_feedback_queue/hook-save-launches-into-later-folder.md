# Set up and launch can start in a folder whose default settled after the answer

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Set up and launch can start a session in a default folder that resolves after the user answers.

## Details

**high.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/launcher/view/host_picker.rs:311-324;
launcher/draft.rs:388-405,987-1015,1025-1121,1124-1140; launcher/entry.rs:490-528,540-583;
launcher/view/history.rs:70-143,199-229; launcher/view/hook_question.rs:94-104,142-179,187-205,229-251;
launcher/request.rs:278-285,418-447,599-638; SPEC.md:1028-1045,1405-1412,5572-5581; SPEC_impl.md:319-341,984-988

Choose a different logical host before answering the open hook question. This keeps the same agent's question and arms a
pending history default for the untouched folder, using a selected session only if it matches the installation,
otherwise initially showing the fallback. The hook answer can send PUT while ordinary Launch would refuse that pending
destination. A history GET sent before the PUT can reply first, letting the launcher draw and seed the actual folder
without calling changed. The PUT callback then constructs a fresh request with that folder and the currently connected
incarnation, so the original click launches into a directory chosen later. A GET sent after the PUT cannot overtake it,
and an explicit host or folder edit after the answer retires the continuation. Explicit host picks store the logical
registry row ID and mark it user-chosen; a fresh New's open-session-derived host is installation-checked when seeded. A
remembered-folder pick separately binds to its origin installation and refuses if moved; a host re-pick clears that
binding for a touched folder, and typed or ordinary non-history picks retain their path spelling without that binding.
An untouched default remembers the origin and may be redecided after an identity change; a reconnect with the same
identity does not redecide it. The host incarnation in the request fences the connection used at dispatch, not the
answer-time installation. The former claim of a connected known-install swap during this window is unverified after
investigation, recorded separately in the prior event. Suggested high for the source-confirmed folder drift. Any
resulting disclosure or writes to user files are unverified. Bind a ready destination on answer or save the setting and
leave launch to a later explicit press. for the folder defect.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-launcher/HL-7`, with
`hook-protocol/hook-answer-live-draft`.
