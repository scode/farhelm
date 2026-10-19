# A setup failure can leave a menu beside a different row

Reviewed commit: 6a780e2ec502a9711eaa55cb16ab66fd5f1e46a4

## TLDR

If a host setup fails while its progress is already expanded, an open menu below it can remain at its old position,
beside a different row. Its actions still affect the original host or session. Mistaking which session those controls
belong to could stop or delete work; that harmful sequence has not been reproduced.

## Details

The host-list layout signature watches automatic-disclosure membership and collapsed trace shape
(`crates/farhelm-ui/src/list/view.rs:1026–1054`). Initial expansion is observed. When an already-expanded setup fails,
the disclosure remains present (`crates/farhelm-ui/src/provisioning.rs:2695–2715`), and the expanded row publishes no
collapsed trace (`:3032–3053`). A new step or run diagnostic can nevertheless increase its height (`:3128–3141`). A
fixed host or session menu below that row can retain coordinates measured before the growth. The existing comment at
`list/view.rs:1092–1106` names host optional-line growth as a remaining gap. The changed menu-position test folds the
run before failing it, so it does not exercise the already-expanded case; the landing report identifies that omission
(`plans/reports/host-uninstall-dialogs.report.md:187–191`).

The menu does not switch its stored target. The session menu names its original session at
`crates/farhelm-ui/src/list/row.rs:2099–2111`, and Delete/Replace confirmations name and bind that same session
(`:2001–2007,2031–2037,2070–2087`). The original row retains its tint. Those are real mitigations, but they do not prove
the whole consequence is visual: Stop does not require confirmation, and the preference to skip session confirmations
can bypass the second question. The nearby Add-form guard explicitly treats the same stale fixed-menu geometry as a
destructive-target confusion risk (`crates/farhelm-ui/src/hosts.rs:1301–1310`).

Suggested bucket: conservatively **highest**, because mistaken destructive use remains unresolved, not because a
wrong-target dispatch or loss of work was demonstrated. The primary reviewer suggested **other** for the visual
consequence. Confidence: **likely** for menu displacement, from static layout and notification tracing; the browser
sequence and consequential reliance are unverified. No runtime reproduction was run.

Possible cover: `TRIAGE_OUTCOMES.md`, `row-menu-drifts-on-row-height-change.md`, permits leaving the host-row residual
if the session-row fix cannot cheaply cover it. That decision called the original coordinate/header/tint case mostly
visual but explicitly did not exclude it under the filters; the user chose a fix (`5136–5157`). Its completion entry
does not establish which host residual was accepted. The specification, Planned TODO, BUGS and queue searches found no
clearer coverage for this setup-failure case. The primary reviewer proposed the rare, self-correcting glitch filter for
temporary placement alone. An independent reviewer could not establish that no consequence falls outside that filter, so
the finding is retained for triage. This does not reopen the completed session-detail-line fix.

Proposed drop: none after independent audit. Provenance: trailing review of
`c5fcfee8681e46b1d47718771b321488037d3f83..6a780e2ec502a9711eaa55cb16ab66fd5f1e46a4`, host UI reviewer, independent
exclusion auditor and parent source inspection. A useful check would leave setup progress expanded, open a menu below
it, then deliver a failure diagnostic tall enough to move the original row and compare the menu's position with its
named target.
