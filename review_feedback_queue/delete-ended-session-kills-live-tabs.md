# Deleting an ended session kills live tabs unconfirmed

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After stopping an agent, a single click on Delete kills the shells, dev servers or builds still running in that
session's terminal tabs, with no warning.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F1 / COR-DELETE-IGNORES-LIVE-TABS`, tagged **definite**. Anchors and title: `crates/farhelm-ui/src/list/view.rs:1605`,
`crates/farhelm-ui/src/list/shared.rs:23`, `crates/farhelm-ui/src/list/row.rs:977`,
`crates/farhelm-ui/src/status.rs:349` — Deleting a session whose agent has ended kills its still-running terminal tabs
without any confirmation

When the user clicks Delete in a sidebar row, `ListView::on_delete` decides between deleting at once and opening an
inline "confirm delete" prompt. The decision is made only from the agent's status: `if target.status.has_ended()`
deletes immediately, and `has_ended()` is true for Exited, Interrupted and Error. The value it inspects, `DeleteTarget`
(`list/shared.rs:23`), carries just the session id and its status. It is built in `row.rs:977` from a `Session` that
does carry the session's terminal tabs (`Session::tabs`, populated on both the listing and detail routes), but the tab
list is deliberately left out of it.

"Agent ended, tabs still running" is an ordinary state, not an edge case. SPEC's Stop operation says it "terminates the
agent… Terminal tabs keep running", and an agent that simply exits leaves its tabs alone too. A user who stopped an
agent and still has a dev server or a build running in one of its tabs will lose all of it to a single Delete click:
delete tears down the whole session, tabs included, with no prompt at all. The long comment above the immediate-delete
branch explains why no confirmation is needed, but it only reasons about the agent's own leftover descendants, which the
UI cannot see. Tabs are different: the UI knows about them, because they are in the listing. Even when a prompt does
open (for a live or Unknown status), its wording from `status::confirm_consequence` ("still running — deleting kills the
agent:") mentions only the agent, never the tabs.

This breaks SPEC "Lifecycle operations", which says Delete terminates "the agent and tabs if running — with confirmation
that says so when anything is still alive". It is also inconsistent within the UI: closing a single tab has its own
confirmation, while Delete kills every tab silently.

The suggested fix: add a tab count (or a `has_tabs` flag) to `DeleteTarget` in `row.rs`; confirm whenever the status is
not ended or any tab is listed; make `confirm_consequence` (and `replace_consequence`, since Replace also deletes) say
that the tabs will be killed; and add a row-level test for an Exited session that still has tabs.
