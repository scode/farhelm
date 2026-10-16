# Checkout setup remains stale before a repository is selected

Reviewed commit: f2eca4357f00ae9802aa3db6b0c865ad8110198a

## TLDR

Changing the checkout folder elsewhere can leave an open launcher showing unnecessary setup, or hiding setup that is now
needed, until its repository input changes.

## Details

Suggested bucket: `other`. Confidence: confirmed static trace; no browser reproduction. Source: separate adversarial
trailing reviewer. This is a surviving pre-existing gap, also acknowledged in the landing report's follow-ups. Proposed
drop: none.

`crates/farhelm-ui/src/list/create_form.rs:3855–3868` invalidates the observation when the configuration revision
advances. While the destination remains `GithubInput`, preview authority remains `None` (`3138–3151`), so this revision
change does not advance `preview_generation` (`3158–3169`). Discovery stays enabled (`3873–3877`), but its authority is
unchanged (`3880–3886`). Its completed reply remains accepted (`3922–3940`) and is relabeled with the new revision
(`3941–3950`) without another request.

With stable empty repository input, obtain a missing-root answer, set the root externally, and deliver the history
invalidation. The earlier answer persists. Starting configured and clearing the root gives the reverse result. Include
the observed revision in discovery authority, or explicitly restart discovery without relabeling an older result.

Possible cover: the completed `inline-checkout-setup-retains-stale-config-eligibility.md` ledger outcome covers a
selected repository, not this unselected-input case. A report follow-up is not an acceptance. No matching Planned item,
BUGS entry, queue item or filter was found.
