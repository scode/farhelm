# A continued unit line bypasses the service ownership guard

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

Setup or uninstall can mistake a modified service that runs more than one installation for a supported Farhelm service,
then overwrite, stop or remove it instead of refusing it.

## Details

Suggested bucket: `highest`, conservatively because the action can affect another installation's service and its
configuration. Confidence: confirmed parser mismatch by inspection; an ordinary occurrence and actual process loss were
not reproduced. Source: adversarial helm/CLI reviewer, corroborated by an independent reviewer. Proposed drop: the
independent reviewer rejected dropping this as fully covered by the earlier multiple-ExecStart decision.

The type guard added in `3502b69062a39c25dae11e52a1747e9137b50067` (#1817) reads physical lines separately
(`crates/farhelm-helm/src/units.rs:280–297`). Consider this edit to a real setup-generated, marked service:

```ini
# managed-by: farhelm helm setup
[Service]
Type=oneshot
Environment=LABEL=foo \
Type=simple
ExecStart=/other/farhelm supervisor run
ExecStart=/selected/farhelm supervisor run
```

Systemd joins the continued line to `Environment=`, so its effective type remains `oneshot`. Farhelm reads the physical
`Type=simple` line as a directive and accepts it. Its command reader then selects the last executable
(`units.rs:239–267`). Standalone uninstall can classify the service as belonging to the selected installation
(`crates/farhelm/src/setup.rs:484–517`), and removal disables/stops the service and removes its file (`904–925`). The
setup/legacy-uninstall guard also admits it (`1113–1142`). See
[systemd's version-255 syntax documentation](https://github.com/systemd/systemd/blob/v255/man/systemd.syntax.xml) for
the continuation rule.

Verify the parser with continued environment text and a non-simple effective type; any service-level reproduction must
use owned fixtures. Conservatively refusing continuation-bearing unit text is sufficient at this ownership boundary;
supporting arbitrary unit syntax is unnecessary.

Possible cover: `TRIAGE_OUTCOMES.md:8393–8408`,
`multiple-execstart-commands-incorrectly-treated-last-assignment-wins.md`, records a fix requiring refusal of non-simple
units. The continuation trigger bypasses its delivered guard; that decision does not accept the bypass. The underlying
last-command ownership bug predates v0.25.0, so this is an incomplete fix in the reviewed range, not a newly introduced
user-visible failure. SPEC.md:2622–2625 explicitly requires refusing a Farhelm unit edited to another type.

SPEC.md's deliberate same-account interference exception could cover a manufactured imitation planted to fool Farhelm.
This finding instead depends on an edit to a real managed unit. Its ordinary-use plausibility remains a caveat; do not
claim an external attacker can write the service or that a fabricated marker proves ownership.

## Additional affected scope

Independent drop decision: a fresh `gpt-6.1-sol` high reviewer rejected complete duplicate coverage. The additional case
fabricates executable evidence in a genuinely simple service, rather than hiding a non-simple effective type. Preserve
that trigger below as an extension of the existing parser finding. The independent review did not reproduce service
removal or process loss.

Reviewed commit: b226160d78884053136a35120b508231821e7cfe

## TLDR

Setup or uninstall can misclassify an edited service and act on another installation's service.

## Details

Suggested bucket: `highest`. Confidence: confirmed parser mismatch; ordinary occurrence and actual process loss not
reproduced. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: Covered by
review_feedback_queue/continued-unit-line-bypasses-service-type-guard.md:1-50. Preserve for independent confirmation of
coverage; the finding is not dismissed as acceptable behavior..

crates/farhelm-helm/src/units.rs:232-252 interprets physical lines independently. An ExecStart-looking line following an
Environment line ending in backslash is environment text to systemd, but ownership evidence to Farhelm. With a real
command for A followed by that continued text naming selected B, setup.rs:501-517 can select the service for B and
:898-914 stop/remove it. This parser limitation predates the commit. The current queue demonstrates the related
continuation bypass of the later Type guard.

Possible cover: That queue item has the same continuation trigger, wrong-installation ownership consequence and
setup/uninstall scope. It already records independent rejection of treating the earlier multi-command decision as
sufficient coverage for the bypass.

Status at frozen endpoint: Persists at pinned main units.rs:245-265 and :283-294. No fixing commit found; the existing
queue item remains open.

Review coverage: full commit diff read: true. No commit-diff content unread. Broad search output and one combined
contextual read were truncated; relevant code and specification portions were reopened in bounded reads. The entire
13,089-line current triage ledger and entire specifications were not read; relevant sections and heading searches were
read. Unrelated later procs.rs changes beyond the first 190 diff lines were not reviewed. A guessed historical server.rs
path did not exist; the actual provisioning caller was located and read. No new actionable regression attributable to
this commit was established; findings retain covered pre-existing highest candidates for independent drop review.

Runtime evidence: None, as requested. No macOS compile, live kernel read, systemd execution or destructive reproduction
performed. Checkout files were not created, modified or deleted. HEAD confirmed as
4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703.
