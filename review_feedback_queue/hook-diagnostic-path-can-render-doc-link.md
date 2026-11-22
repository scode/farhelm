# Hook setup path text can be rendered as a Farhelm documentation link

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A path inside a hook setup error can be shown as a Farhelm documentation link even though Farhelm supplied only
diagnostic text.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d crates/farhelm-supervisor/src/agent_kind/persistent_hooks.rs:44-56;
crates/farhelm-supervisor/src/service/notifications.rs:284-293,397-408,575-585;
crates/gui/farhelm-gui/src/notices_bell.rs:217-269,754-806; SPEC_impl.md:1157-1180; SPEC.md:3175-3183

A filesystem path printed in a hook setup failure can acquire a clickable documentation action even though Farhelm did
not add a docs reference. Reviewed commit 1a53d9d35889d7dde96e31320cc1681dd013737d. The installer formats its
host-supplied configuration path in its error, and HookSkip::PersistentSetup flattens the raw reason into
SessionNotification.text. The bell scans that entire string for a known docs URL prefix before sending remaining text to
HostText. The scan searches for a substring rather than requiring a left token boundary, and has no structured origin to
distinguish a path from a Farhelm-authored link. Matching text becomes a link button invoking a browser open. The prefix
and accepted tail restrict the immediate address to Farhelm's docs, and the remaining text is escaped and
direction-isolated as a whole. No external destination or automatic browser action is established. The reviewer
suggested other for the unintended link and misleading attribution, confirmed by source conditional on a failing
configured path containing matching text and a user clicking the link. Separate plain diagnostic text from explicit docs
actions in the wire view, or disallow link inference for this notice. This is distinct from the hook-session HS-05
Restart and recovery formatting issue. ; in supplied inventory. SPEC.md:3175-3183 allows actual documentation links in a
notification while keeping other addresses plain. The source-supported mismatch is promotion of a path substring to an
action. The domain restriction itself works as documented.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-supervisor/HS-13`.
