# Shell template approval cards misstate a stored permission field

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Approval cards can say a shell template has no permissions while saving a concrete incompatible setting.

## Details

**other**. Confirmed by source and contract inspection.

An approval for creating, editing, or deleting a Bash or Zsh template can claim `None (shell)` even though the template
sets a permission. Source-confirmed through the ordinary CLI: main.rs:620-634 and :1078-1100 carry harness and
permissions as independent template edit fields; :870-883 and :945-957 parse them independently. The helm merges and
checks launch kind and template shape (agent_requests.rs:3208-3240, :3659-3697), supplies the resulting fields on every
write card and retains them on save (:3255-3295). Delete sends the stored definition (:3320-3343).
Farhelm-proto/src/launcher.rs:520-549 checks the permission/flags pairing, and leaves harness compatibility to template
application. Approval model.rs:568-590, :801-815 calls the shared template summary helper, which returns `None (shell)`
for every shell permission (templates/summary.rs:171-183). Related Custom words, when retained from an existing
template, still appear in a separate fact. SPEC.md:4331-4335 requires the whole resulting template, :4353-4354 requires
stored permission labels, and :4499-4502 says approval covers exactly what the card showed. SPEC.md:1186-1203 allows
refusing an inapplicable template at use. Concrete permissions cannot launch as a shell:
farhelm-proto/src/launcher.rs:921-931 and launch.rs:506-539 refuse that template during application. The issue is the
false approval fact, with no established hidden execution route. Bucket other. Possible_cover: shell-launcher/f7
contains the same suppressed value in the Templates summary and separate editor behavior. This is the approval surface,
and a shared formatter correction may cover both displays. Shell-launcher/f6 corroborates the distinction between a
concrete incompatible choice and an accepted null reset. No later change to approval, formatter or
template-compatibility paths through pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-permissions/f9`.
