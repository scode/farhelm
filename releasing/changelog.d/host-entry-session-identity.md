---
kind: breaking
---

Session commands and pending confirmations now stay bound to the host entry they named. Agent commands use full session
IDs from `farhelm agent sessions`, and discovery JSON moves to schema version 7 with UUID host IDs.

Old bare targets are refused except for the caller's exact own `$FARHELM_SESSION_ID`. Existing sessions and older
supervisors keep working. Removing and re-adding a host starts its pins, seen state and notification marks fresh;
the upgrade also clears viewer state and the last selection where Farhelm cannot attribute the old ID to one host.
An approval card already waiting during the update may get no answer.

Matching local session IDs on different hosts remain separately listed and addressable.

Approval cards show distinct spellings for session IDs containing spaces, formatting characters or literal escape
notation, and their Copy buttons copy the complete original ID.
