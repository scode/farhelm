---
kind: fixed
---

The New session dialog (and Clone, Replace with, and recent setups) no longer refuses a saved choice that names an
effort when the list of available models has not loaded yet or failed to load. It used to call the choice "no longer
supported" and disable Launch, and after a failed load the only way out was to close the dialog and lose what you had
entered. A failed load now says so and offers a retry. "Restart with" no longer refuses while that list is still
loading either.
