# The token prompt invites password managers to save the master token

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

When authenticating a browser, the browser offers to save the Farhelm master token as a password, and accepting stores
and may sync that root credential off the machine.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F34 / SEC-TOKEN-PROMPT-PASSWORD-MANAGER`, tagged **possible**. Anchors and title: `crates/farhelm-ui/src/auth.rs:455` —
The browser token prompt is a plain password field, so password managers offer to save and sync the master web token

In the browser build, a new device authenticates by pasting the web token (the root credential; see F33) into
`TokenPrompt`. The field is `<input type="password" autocomplete="off">` (line 455) inside a form that is submitted and
then removed from the page after a successful exchange. According to the reviewer, Chromium and Firefox ignore
`autocomplete="off"` on password fields, and Chromium treats a password form that disappears after a successful fetch as
a successful login. So the browser offers to save the token as the password for `http://localhost:<port>`, often into a
password store synced to the user's browser account. It may also later autofill it into any token prompt on that origin.
`docs/security.md` describes a lookalike page, served by another local user who takes the port while the helm is down,
and relies on the user having to paste the token deliberately. Autofill removes that deliberate step.

The design keeps the web token on the helm's machine, and a synced password store takes it off. Harm requires the user
to click "Save", hence "possible". The suggested fix is to stop presenting the field as a site password, for example
with `type="text"` plus `autocomplete="one-time-code"` and masking via `-webkit-text-security: disc`, or with
`autocomplete="new-password"`, verified in both Chromium and Firefox.

Restater note: the browser behaviour described (ignoring `autocomplete="off"`, the disappearing-form save heuristic) was
not tested here. Also, Chromium does not expose an autofilled password's value to page script until the user interacts
with the page, which somewhat limits the lookalike-autofill half. The save-and-sync half is unaffected by that.
