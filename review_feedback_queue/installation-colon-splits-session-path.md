# A colon in the installation directory changes command lookup

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

If the Mac installation's path contains a colon, new sessions can search unintended directories for commands. One of
those directories can be relative to the session's checkout, allowing a command there to take precedence unexpectedly.

## Details

`crates/farhelm-supervisor/src/launch.rs:1066–1073` prepends the executable's parent by concatenating it with the
inherited PATH. A colon inside that parent becomes another PATH separator. For example, a hypothetical home
`/Users/example:work` splits the app-forwarder directory into `/Users/example` and a relative
`work/Applications/Farhelm.app/Contents/MacOS` entry.

The installer checks that HOME is absolute but does not reject colons. Inspection of
`crates/farhelm-ui/src/desktop/bundle.rs:1–134`, `desktop.rs:374–463`, and the supervisor's construction path found no
upstream refusal. Linux unit generation does reject colon-bearing paths (`crates/farhelm-helm/src/units.rs:130–153`), so
that path is not the demonstrated omission.

Reject an unrepresentable directory before admitting installation/startup, or otherwise avoid introducing its text as a
PATH entry. Shell quoting cannot escape a colon within PATH. An isolated test should verify effective lookup from an
unprivileged session, including a similarly named relative directory in the checkout.

Suggested bucket: provisionally **highest**, for unintended command selection; at least high if only Farhelm lookup
breaks. Confidence: **confirmed delimiter splitting and absent guards in the inspected paths**. Practical macOS
execution, a supported ordinary source of the colon-bearing home, and malicious command selection are unverified. No
cross-host exploit was demonstrated.

Possible cover: SPEC.md:1892–1903 and the ledger's `launch-path-prepends-binary-directory.md` accept precedence for the
actual Farhelm directory. They do not expressly accept extra directories created by splitting that path. SPEC.md's Local
authority and trust between hosts rule treats deliberate external configuration as intent; whether this unusual path
qualifies needs triage. No exact Planned, BUGS or existing queue coverage was found. This is a conditional
correctness/security candidate, not an observed compromise.

Proposed drop: none. Provenance: persistent Astra/high review, `supervisor-process-lifecycle-1/f2`, corroborated by
`cli-install-and-removal-1/f6`.
