# Ssh key tooltip test lacks loading witness

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246

## TLDR

The SSH-key tooltip regression can make its assertion without establishing the key-loading state it intends to test.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/tests/tooltip-coverage.spec.ts:330-339 at da24fb6822d289a144e18677cdd248cc35093246`.

Source assessment (read with the later corrections below) (review record g062/g062-l6, event 16):

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246, with the persistent consequence checked through inclusive tip
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: the browser tooltip audit can pass without ever checking the Tensorlake
SSH-key action, so removing that action's hover help need not fail the audit. This corrects the previous withdrawal: the
later recipe button fixes the intermediate minimum-count false failure, but does not establish that the SSH action is
present during measurement. Exact mechanism: helpers/settings.ts:32-47 waits only for panel visibility; the account
snapshot is asynchronously loaded at tensorlake_settings.rs:75-99, and None deliberately renders no SSH action at
:215-217. The sweep counts only painted controls (:76-94 in tooltip-coverage.spec.ts) and expectCovered (:123-127)
samples once. The assigned test visits Tensorlake once at :330-339, then switches to Terminal and other panes; :342-355
audits Notifications/list/Sessions at phone width and closes Settings, never returning to Tensorlake. Other later sweeps
cannot inspect the now-hidden panel. At da24 the pending account gives four controls and can fail minimum 5. In
68790603037bb51ccd8655e4a948c7ffb747760a, unconditional RecipeList (tensorlake_settings.rs:273-275) and unconditional
New recipe (tensorlake/recipes.rs:159-163) satisfy minimum 5 without the SSH action; that removes the false failure but
allows the missing-premise measurement to pass. The test and helper are unchanged through tip; tip RecipeList still
mounts unconditionally at tensorlake_settings.rs:299-301 and its New recipe remains unconditional at recipes.rs:396-400.
A legitimate delayed initial GET can remain pending through the Tensorlake visit and settle only after another pane is
selected, or after Settings closes; there is no response-consumer/RAF ordering assumption here, because the named
account response need not have arrived at all. Source checks of the full remainder of this test (:341-457) found no
later Tensorlake visit or loaded-account assertion. The test-authoring contract requires the fixture premise before
measurement; SPEC.md's Settings section requires Create SSH key before a key exists, or its registration Copy action
after it exists. Suggested fix: locally wait for the expected SSH-key action and account readiness before this sweep,
preserving selectSettingsSection's intentional visibility-only contract for other loading tests. Suggested verification,
not run: remove the SSH action's tooltip in a controlled variant and delay the initial account GET until after the
section is hidden; the current minimum is met by the recipe button and no subsequent visible sweep sees the omitted
tooltip. Suggested bucket other, confined to a test-oracle coverage gap; no highest route. Confidence confirmed by the
explicit pending DOM, count floor, one-shot measurement and absence of a revisit; no claim of an observed runtime false
pass or frequency. proposed_drop false. possible_cover not searched. Owner g061:g061-l1 remains a distinct unverified
response-retirement candidate; its scheduling refutation is not extended to this absent-initial-response path. Owner
g036:l6 is unrelated to this coverage defect.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g062/g062-l6`; supporting
records `g060/f13`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
