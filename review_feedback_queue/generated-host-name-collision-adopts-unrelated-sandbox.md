# Generated host name collision adopts unrelated sandbox

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313

## TLDR

A generated host-name collision can make first-time Add adopt an unrelated sandbox and replace its endpoint credentials
before later copied-state checks can refuse it.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Source location:
`crates/farhelm-helm/src/provisioning/tensorlake_add.rs:40-58,516-520,447-477 at ca28b604b876770b1ab757783416f83ad1709313`.

Source assessment (read with the later corrections below) (review record g071/l5, event 13):

Reviewed commit ca28b604b876770b1ab757783416f83ad1709313; unrepaired through pinned
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: A request to create a new sandbox can silently claim an unrelated
existing sandbox on a generated-name collision, change its agent configuration, then leave that existing sandbox
represented as the failed/new host. Exact source route: sandbox_name :40-58 uses four-base36 per-helm tag, display slug
and four hex UUID characters. Local check_new_entry/store registration check only this registry's destination/alias rows
(store.rs:6378-6396,6409-6453); neither checks account-wide provenance. New row starts with no sandbox_id or
pending_create_at. create_host_sandbox :516-520 accepts ANY successful get_sandbox(name) as existing, even with neither
marker; the marker is written only in NotFound/create branch :549-557. :571-574 stores adopted ID, then :443-449
prepares the sandbox and applies configured endpoints before :474-477 checks copied supervisor state. Thus copied-state
refusal gates later identity admission/dialing, not earlier wrong-resource configuration or deletion. Concrete highest
consequence independent of later Delete: if the existing sandbox has ~/.claude/settings.json with a previously saved
endpoint token and the new recipe has a Claude endpoint, configure_agents :619-636 reads it, agent_endpoints.rs:24-30
replaces ANTHROPIC_BASE_URL and ANTHROPIC_AUTH_TOKEN, and install_bytes replaces that persisted file. This changes/loses
the existing sandbox's saved endpoint credential/configuration after confirmation only of NEW creation; it needs neither
concurrent edits nor an already connected supervisor. No backup/preservation of replaced owned endpoint keys is made by
that merge. This is distinct from g053-f1's concurrent-save lost update. Confirmation scope checked directly: initial UI
add_host.rs:218-235 calls it 'A Tensorlake sandbox created from a recipe', asks 'Create Tensorlake sandbox?', shows the
generated name, CPU/memory and setup steps, and billing/idle notices; :280 says 'Create sandbox'. Server steps say
Create sandbox with requested recipe resources. It neither probes the account nor discloses that the generated name
already denotes an existing resource; no prior resource identity/content or choice to adopt/reuse it is shown. Therefore
wrong-resource adoption/configuration is not a user misreading a correct but stale destination: creation and its effect
on existing work are different operations despite the same name string. At later tip the standard confirmation still
authorizes creation, not existing-resource adoption. Deletion qualification: host removal confirmation is NOT harmless
Forget. settings_dialog.rs:889-898 explicitly says it will terminate named sandbox, delete snapshots, remove host, and
files/sessions will be gone; it always asks, with no don't-ask choice. hosts.rs:3175-3193 binds removal answers to the
shown Host token. This warning names the same sandbox string deletion targets. Consequently I do not claim stale-name
deletion or damage outside that warning's stated destruction solely from its text. Complete downstream risk: after
adoption, tensorlake/hosts.rs:124-212 terminates saved adopted ID and deletes all snapshots with it; without saved ID
its name lookup also adopts with no pending-create prerequisite. A user deleting the failed/new entry is offered
destruction of the sandbox misclassified by the earlier add; confirmation does not repair initial ownership mistake, but
deletion itself is fully warned. Earlier unqualified wording is superseded by this exact scope. Named g050-f2 inspected
only for naming/ownership assumptions: independently generated registry namespaces, same-account support, four-base36
collision domain and no provider owner metadata support the cross-installation premise. Its deterministic builder
cleanup can automatically terminate another helm's builder; l5 concerns generated HOST name plus four-hex suffix
collision and new-host adoption/configuration, followed by separately confirmed Delete. Keep them distinct. Requires
account resource outside local registry; no same-registry duplicate or simultaneous helms needed. Chance/observed
frequency is not claimed, and no collision pair/cloud experiment produced. Suggested bucket highest: existing persisted
endpoint credential/configuration can be replaced on wrongly adopted resource before copied-state refusal and before any
deletion warning. Confidence confirmed source conditional on collision/existing endpoint; later deletion has warning
qualification above. proposed_drop false. possible_cover: SPEC_impl.md Adding Tensorlake hosts literally documents
recovery-by-name, but does not accept unrelated-work loss; exact phrase is 'Host creation looks up the pre-chosen name
first and adopts an existing sandbox, including after a lost create response.' Any highest exclusion needs independent
adjudication. Fix: first-use name preexistence must refuse/reselect; recovery needs evidence of this row's prior create,
and deleting a never-sent row must not adopt unrelated resource by name. Preserve legitimate ambiguous-create
recovery/resource tracking. Record append initially failed because event JSON contained a raw newline in detail;
immediately repaired and retried before further analysis/read.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g071/l5`; supporting records
`g032/g071:l5`, `g033/f5`, `g047/interface-g071-l5`, `g059/g071:l5`, `g068/s5`, `g072/i071-l5`, `g048/i3`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
