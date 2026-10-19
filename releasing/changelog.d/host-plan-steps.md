---
kind: changed
---

When adding, updating or removing Farhelm on a remote host, the confirmation shows a numbered list of every step it will run, including uploads and service reloads. Paths appear under each step, with their full spelling on hover, and Copy plan copies the complete text. While the run proceeds, the host row shows those same steps and names. Setup and uninstall expand the row once for each run; you can fold it again without stopping the work.

After uninstall, a short notice confirms which host was removed. You can hover to see where its data remains, copy that path, or dismiss the notice.

If planning setup, Update or uninstall fails, a dialog shows the reason. When Farhelm cannot uninstall because its install directory is a symbolic link, that dialog can take you to Remove's ordinary confirmation; Remove forgets the host and leaves its installation and sessions in place. A definite refusal after confirming setup or uninstall also opens a dialog again. A failure after the run starts remains in the host row with the failed step.

Retry and adopt refusals appear in a dialog too. Remove keeps its confirmation open until Farhelm answers and shows any refusal there; when confirmation is skipped, its refusal opens a short dialog. A refused setup after adding a host reopens the add dialog with the destination still filled in.
