# Failed feedback checks bypass evidence retention before guest deletion

Reviewed commit: 2a0922238385455c345a3dd13a7f751c8a77d076

## TLDR

After a failed feedback check, cleanup can delete the test guest even when its exported screenshots have disappeared,
destroying the remaining source of the failure evidence.

## Details

Suggested bucket: `highest`, conservatively because deletion can destroy the remaining retained work. Confidence:
confirmed cleanup mechanism, with the live-guest evidence-loss premise unverified. Source: separate adversarial
trailing-review agent, brick-feedback scope. Proposed drop: none.

`releasing/mac-vm-test/brick_test.py:783–790` considers P1 retained whenever every recorded result is nonpassing.
`verdict()` at `673–679` returns before checking artifact hashes for failed or unavailable observations. Ordinary
cleanup consequently reaches guest deletion at `799–807` even when the recorded P1 export is missing or changed.
`RECIPE.md:137–140` requires failure evidence to be retained before cleanup; the reviewed `BRICK-TEST.md:287–291` also
requires its preservation. Successful P1 observations retain the integrity gate.

Complete P0, record P1 as `fail` or `could-not-run` with an exported screenshot, then remove or overwrite that export.
Cleanup still succeeds. The real coordinator with the existing synthetic Tart substrate reproduced both statuses in
recorder run `ac7dd320-5682-45c9-b9f2-1494376b05a9`: cleanup recorded `phase: deleted` and both modeled children exited.
No live guest was touched. Actual loss depends on the guest still holding evidence absent from the export; the
reproduction proves the missing gate rather than destruction of a real screenshot.

Validate recorded P1 artifact integrity independently of whether P1 passed before deleting its guest. A failed P1 should
remain nonblocking once its evidence is retained. Extend the missing-success-evidence regression to failed and
unavailable observations. No exact specification acceptance, Planned item, accepted bug, queue or ledger coverage was
found. A rare lost-diagnostic exclusion is uncertain because this operation deletes the remaining recoverable source;
that uncertainty must not silently discard a potential work-loss finding.

A fresh independent reviewer found the diagnostic-filter match ambiguous and agreed to retaining the candidate
conservatively as `highest`. That review confirmed the gate by inspection without rerunning the synthetic reproduction.
