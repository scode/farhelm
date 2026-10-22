# Refused create retry can delete a concurrently restarted session

Reviewed commit: 7fe7ad3c36414cd31b84fd0970237602e72f3bdc

## TLDR

A stale retry of an interrupted create can remove the record of a session the user has just restarted. Depending on
ordering, the new agent can become unlisted or be killed by restart rollback. This conditional mechanism predates the
review range; the reviewed patch is not claimed to introduce it.

## Details

Suggested bucket: conditional **highest**, for losing or stranding the newly restarted agent/process. Confidence:
**confirmed conditional source mechanism**. Runtime ordering and frequency are unmeasured. This is an adjacent baseline
finding discovered while reviewing launch-file cleanup, not evidence that the later cleanup patch caused the race.

Under `crates/farhelm-supervisor/src/`, create admission retains the intent and directory guards
(`service/core.rs:7256–7286`). Restart takes lifecycle ownership but releases directory admission before slow launch
work (`11015–11071`). A retry that resolved an old Pending attempt before Restart advances can later validate against
the changed launch settings. The mismatch produces an ordinary refusal (`8494–8537`). Named Restart with can be
available without captured identity when an agent has a stored resume command
(`crates/farhelm-ui/src/session_view.rs:101–120`; `crates/farhelm-helm/src/sessions.rs:2806–2874`), so the considered
Restart route is not excluded merely by missing capture.

Ordinary refusal calls rollback without taking lifecycle ownership (`service/core.rs:7750–7788`), unlike retained
refusal. Its retry branch deletes by session ID and removes the in-memory entry (`9197–9211`), then clears
generation-zero files. `store.rs:5591–5625` has no generation or current-outcome guard on that deletion. Its
managed-checkout last-reference guard narrows the affected configurations; a non-managed session does not get that
protection. Thus generation-scoped artifact cleanup does not protect the session row or new live entry from the stale
rollback.

The exclusive base `5c6e49937b1e779be2a5445059637aadf3757356` already contains these mechanisms at
`service/core.rs:7570–7608,9014–9047,10813–10869`. No introduction or worsening by an assigned later patch was
established. The producer proposed excluding it on that attribution basis. The parent instead retains the adjacent
finding with this explicit limit; no highest finding is dropped on a unilateral scope judgment.

Possible cover: `SPEC.md:1331–1343` at the extended pinned tip `2ea2826caa0eec867c4e2bddc01badbac7bbe6c7` may cover
crash-dependent variants. It was not established as exact acceptance of a later stale rollback deleting an independently
restarted generation. No exact Planned, BUGS, current queue, ledger or FILTER coverage was found. Proposed drop: none
approved. Revalidate or fence ordinary retry rollback against lifecycle and current generation before deleting an
allocated session.

Provenance: trailing-review run `30f46c42-a021-475f-96a8-eac06930471b`, `g058/f-refused-retry-races-restart`. The parent
checked refusal, rollback and admission source. This is separate from the independently refuted old Error-observer
cleanup hypotheses; no reproduction was performed.
