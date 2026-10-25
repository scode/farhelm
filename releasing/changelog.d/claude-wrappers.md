---
kind: fixed
---

Claude sessions started through multiple wrappers that stay running can now resume their conversation with Restart.
If Farhelm follows another Claude started inside the session, it switches back when the original Claude finishes a
turn; sessions already running need a relaunch to gain that correction.

Farhelm follows the first accepted report and corrects a nested child's selection when a recorded ancestor reports,
with a notification explaining what Restart would have resumed before the correction. Sessions already running across
the upgrade start unpinned; a nested child may report first until its parent next reports. Claude's Stop is an ordinary
report of the current conversation under the same pin, kept in the existing single latest-report slot. Rival Stop
reports are acknowledged without warnings or mutation. Existing launches retain their old argv until relaunch.
Claude's existing Restart offers remain available.
