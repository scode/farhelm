# A saved absence replaces a different place’s or provider’s cleanup error on ordinary retry

Reviewed commit: f2b3b5b9c4be17779dbdce517c1a7af04c9f4eaa

## TLDR

After a Delete has found a missing capture, ordinary retry shows Delete anyway while a separate cleanup blocker stays
hidden.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/tensorlake/recipes.rs:685`.

The ordinary combined-provider case is a managed Ubuntu recipe with a Tensorlake snapshot, an unresolved Podman capture,
and the Tensorlake API key cleared via Settings (SPEC.md:471–478. Settings/tensorlake_key.rs:64–90,453–480.
Tensorlake/config.rs:54–66). Tensorlake cleanup detects the resources and missing key (tensorlake/build.rs:1853–1868).
Its adapter creates the InvalidRequest error “this recipe has Tensorlake resources. Save a Tensorlake API key to clean
them up” (tensorlake/hosts.rs:170–185). environment/mod.rs:514–520 retains that first error and still runs Podman.
Likewise, a different Podman place that fails produces “recipe cleanup did not finish: container host <place>:
<reason>”. Within Podman that failure is preferred to the typed missing-capture error regardless of which place is
visited first (environment/podman/build.rs:799–833). The absence is saved before either combined outcome returns
(:769–775). For an in-bound attempt the handler passes the actual failure to http_error (tensorlake/recipes.rs:689–697.
Lib.rs:1714–1728,1782–1797), and the editor shows the refusal. If the first attempt times out, the user sees carries-on
and the completed separate error reaches only the log (sessions.rs:3827–3864). After completion an ordinary retry with
the observed tag still recorded does no provider work and answers the capture marker 409
(tensorlake/recipes.rs:670–697,709–720). The editor clears its old error at dispatch, turns the marker into the new
question, and clears the operation refusal (leaving.rs:319–364). Thus the durable note changes the next visible answer,
while the first attempt’s separate error retains precedence. A confirmed retry that settles within the bound can show
the current separate failure, even though confirming is unnecessary to learn it under the ordinary retry contract.
SPEC.md:586–589, SPEC_impl.md:294–299 and :6135–6142,6195–6198 (tip :6144–6151,6204–6207) call for the next Delete to
explain the failure and for a late absence to afford confirmation. Bucket: other, established diagnostic suppression
only. No evidence the marker causes the underlying cleanup failure. Confidence: confirmed by source. No runtime
observation. Possible coverage: recipe_delete_needs_explicit_consent_to_forget_an_absent_capture asserts cached warning
after a same-host failure, not the separate error case.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recipes/f2`.
