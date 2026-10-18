# A reused checkout path can trust replacement content

Reviewed commit: 0f982643c59f553d153c29ac343798c903da3796

## TLDR

If a managed checkout is moved away and another folder takes its path, Farhelm can treat the replacement as managed and
automatically trust its agent settings or commands.

## Details

The managed-checkout membership read checks allocated registry path ancestry without comparing the folder's recorded
filesystem identity (`crates/farhelm-supervisor/src/working_copies.rs:1094–1120`). Directory inspection canonicalizes an
existing replacement and uses that path predicate (`crates/farhelm-supervisor/src/service/core.rs:5002–5036`). With a
trust switch enabled, the helm grants the agent trust override from that answer
(`crates/farhelm-helm/src/sessions.rs:2299–2347`). The registry carries identity information, but this new read does not
consult it. Replacing an allocated checkout at the same path before inspection is a focused way to verify the issue. A
later replacement after inspection may have a separate timing boundary; this finding concerns a replacement already
present when membership is checked.

Suggested bucket: highest, for automatic trust of ordinary folder content. Confidence: likely; the path-only read is
confirmed, but the exact replacement sequence was not reproduced and another create-stage check has not been ruled out.
Provenance: trailing review of the stated commit. `SPEC.md`, “Managed checkouts,” says ordinary directories receive no
trust override. The existing queue item about archive recovery clearing trust at a reused path concerns a different
consequence and does not cover this grant.
