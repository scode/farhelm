# Outer recorder cleanup removes nested failed-run evidence

Reviewed commit: 0f6e63bca4b371d7f2dedcb554f1d677f3ea8918

## TLDR

Recording a script that starts another recorder can delete the inner failed run's evidence during outer
temporary-directory cleanup. The release invocation that explicitly forwards both output roots is outside this route.

## Details

Suggested bucket: **other**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

The existing worker-download and archive-omission items concern remote transfer, not deletion by a parent recorder's
local TMPDIR cleanup. The accepted passing-run slimming policy does not accept removing failed nested records; explicit
outer output-root alone does not protect an inner default root.

## Recorded source assessment

Reviewed commit 0f6e63bca4b371d7f2dedcb554f1d677f3ea8918 (tip line numbers below include
b131ef29161731b359338a537476eec9daa7cd56 retention additions). Synthesis with g046/c1 confirms that explicit output-root
gates only prune/slim, not child TMPDIR removal. Tip :2139 always calls release_child_tmp after command/trace
collection; :1717-1720 recursively removes any socket-free child directory regardless of final_exit or --output-root. An
inner default-root recorder follows the inherited TMPDIR at :273-293, so its failed UUID directory is ordinary nested
content and goes too. This remains true if the OUTER recorder has --output-root but the inner invocation does not.
Concrete local caller remains scripts/test-provision-centos.sh:49,500; release .github/dist-build-setup.yml:306-309
forwards --output-root to both layers and is safe on this route. TLDR: local nested failed-test reports can disappear
immediately even though the failure-retention test g046/c1 correctly preserves failed runs within the selected root. No
duplication with that candidate: inner retention helpers never remove the failed run; outer temporary-directory teardown
does. Suggested bucket other; confidence confirmed; proposed_drop false; possible_cover none checked. l2 stays withdrawn
because named Rust/browser harness state itself is explicitly rooted at /tmp; the recorder evidence root is a different
TMPDIR-sensitive caller/callee contract. No runtime checks.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g045/l3`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
