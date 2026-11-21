---
kind: changed
pr: 4433, 4436, 4438, 4442, 4460
---

The session launcher is redesigned around its command field, which now sits at the top with the keyboard focus in it,
with examples of what you can type; Enter on the empty field starts the setup in the form. Recent setups and launch
templates sit beside the form, and typing shows matching choices there. Host and folder share one address field, the
Host menu also offers new containers and sandboxes, and conversation IDs and wrapper prefixes fold under **More
options**.

Context for curation: the form puts labels above its fields, groups the destination in a card, shows the session
list's agent marks on the harness picker, and keeps Model, Effort and Permissions on one row when there is room. In a
narrow window, search results open below the field instead of beside the form. **More options** stays folded unless a
conversation ID or wrapper prefix is set, required or needs correcting. The footer keeps the launch actions together on
the right.

Caveats a returning user will notice: the footer's summary line of what Launch will start is gone, and the **Runs on**
switch between an existing host and a new container or sandbox is now part of the Host menu.
