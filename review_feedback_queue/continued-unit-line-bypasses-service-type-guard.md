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
