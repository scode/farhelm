# Delete anyway can forget a second tag whose absence was never part of the warning

Reviewed commit: f2b3b5b9c4be17779dbdce517c1a7af04c9f4eaa

## TLDR

One Delete anyway warning can let Farhelm forget multiple unfinished saves, including a tag whose first absence check
occurs during confirmed Delete.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman/build.rs:769`.

Settings’ per-place Rebuild is enabled after work settles even when cleanup is pending
(settings/recipes/places.rs:132–149,229–249), dispatches through hosts.rs:4305–4322 and podman.rs:1050–1083. When two
successive ordinary rebuild attempts reach a capture with an ambiguous outcome, each records its own fresh tag before
sending (podman/build.rs:207–213. Podman.rs:562–605). On retry the worker reconciles helpers but has no retirement gate
before starting the next capture (podman/build.rs:240–280,298–315). The failure cleaner and ordinary Delete stop
same-place retirement at the first absent tag and preserve the rest (:688–704,769–780), so an earlier tag A can be the
sole observed absence even after a second retained tag B is recorded. The marker carries only the need for confirmation
(tensorlake/recipes.rs:685–686,709–720), the editor freezes the displayed recipe name and true/false mode
(leaving.rs:250–281,291–307), and the confirmed request’s guard grants Boolean consent for every place
(tensorlake/recipes.rs:670–689. Tensorlake/build.rs:2652–2670. Podman/build.rs:799–809). A fresh negative existence
result for B on that retry goes straight to removing its record, even though B was never included in the one-capture
warning. This proof needs no rebuild while the question is open, no other client and no recipe replacement. Existence is
still rechecked for each tag. A present image still goes through ordinary ownership and usage checks. SPEC.md:509–511
and SPEC_impl.md:973–978,6195–6198 (tip :6204–6207) bind consequences to what was shown and describe consent to forget
the reported tag. Bind the confirmed cleanup to observations covered by the warning. Bucket: other, source establishes
loss of tracking and possible manual cleanup of Farhelm-generated artifacts. There is no established route to user-owned
data loss. Confidence: confirmed by source, no runtime observation. Possible coverage: the new recipe handler and GUI
assertions exercise a single tag or marker only. The tip also makes the frame rule explicit in SPEC.md:5593–5598: the
action uses the consent and preconditions from the frame the user saw. This supports binding the capture exception to
its warning even when the editor and recipe name remain the same. The identical product wording is at SPEC.md:5581–5586
in the changed revision.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recipes/f1`.
