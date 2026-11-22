# A new launch reservation can make confirmed Delete refuse after changing artifacts

Reviewed commit: f2b3b5b9c4be17779dbdce517c1a7af04c9f4eaa

## TLDR

When an approved new container launch overlaps Delete anyway for the same recipe, Delete can report that a host uses the
recipe after already removing its prepared images.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/tensorlake/recipes.rs:665`.

tensorlake/recipes.rs:665–689 checks host use once before deletion admission and then runs cleanup. Independent agent
approval remains usable during modal Settings (SPEC.md:3695–3698). Its shipped path calls the owned provisioning task
after approval (agent_requests.rs:4498–4520,4540–4554). provisioning/tensorlake_add.rs:351–375,390–420 reads recipe
existence, then registers a host row before provider work. environment/mod.rs:116–138 dispatches to
store/podman.rs:355–394, whose transaction inserts the recipe-bearing row without participating in the deletion
registry. The launch task can still be waiting for a run slot (:479–509) or simply pending. The source/build gate that
rejects the deleting recipe happens later (environment/podman.rs:1133–1176. Tensorlake/build.rs:716–721,762–777). An
allowed order is the preflight seeing zero, the approved launch registering while confirmed Delete is still cleaning,
cleanup retiring/removing owned artifacts, and the final transaction counting the pending host row and returning InUse
(tensorlake/recipes.rs:150–155,399–418,689–705. Environment/podman/build.rs:799–812). The guard prevents an admitted new
source use but not the recorded launch reservation. Rows already present at preflight are handled correctly. The release
can therefore leave the recipe listed without its prior built/prepared state, and the existing error tells the user to
delete a host that was only being registered. SPEC.md:481–483 and SPEC_impl.md:294–299,6135–6142 require early in-use
refusal and source exclusion. Coordinate the host-use check and launch registration at deletion admission. The window
already existed in the parent but also applies to the new confirmed route. Bucket: other for the source-established
removal of rebuildable artifacts and misleading refusal. No live session or user-owned work loss shown. Confidence:
confirmed by source ordering, no runtime observation. Possible coverage:
a_delete_local_state_refuses_answers_at_once_and_cancels_nothing asserts a host row already present before the request,
not post-preflight registration. At tip df6199142d6495a95b2d008400ba2686efb775c6 the cloud/container launch entry has an
unrelated validation edit, but its registration and provider-ordering code, the store INSERT, and both Delete checks are
unchanged. The tip keeps independent approval cards usable at SPEC.md:3705–3708. An approval can already have been given
before confirmed Delete. Its owned work need not register the row before Delete’s preflight. Thus the route does not
require a second desktop client or another ordinary HTTP command to start ahead of the first command’s reply.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recipes/f13`.
