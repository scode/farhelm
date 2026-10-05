---
kind: added
---

The Mac app now keeps itself up to date. Shortly after it starts, and about once a day after that, it checks for a new
stable release and, when there is one, installs it in the background with the same installer you would run by hand.
When a newer version is installed, the version number at the top of the sidebar turns red with an up-arrow, and its
hover says which version is waiting. Select it and choose **restart to update** to restart Farhelm on the new version,
or **what's new** to see the releases on GitHub. Your sessions keep running through the restart, and quitting and
reopening Farhelm also finishes the update. To check right away, choose **check for updates** in the `?` menu, or
**update** in the menu of the local (this machine) host. To stop the automatic checks, untick **install updates
automatically** in the settings dialog. Updates come from GitHub and, like the installer, are not checked against a
release signature.
