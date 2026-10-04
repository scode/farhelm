# The feedback endpoint: setup and operations

NOTE: This is for the maintainer. It is what has to happen once, outside the repository, before the app's **Send
feedback** dialog can deliver anything. Until it is done, every send fails with the dialog's ordinary failure message,
which is the agreed behavior.

The app's dialog sends through the helm to `https://farhelm.io/api/feedback`. That URL is a Vercel function in the docs
website's project (`website/api/feedback.js`, logic in `website/feedback/handler.js`). For every accepted submission it
creates one issue in a private GitHub repository that serves as the inbox. SPEC.md's Feedback section says what the user
sees; SPEC_impl.md's "Feedback forwarding" records why it is built this way.

## What the function assumes about the Vercel project

The repository cannot show these, so check them once:

- The Vercel project's root directory is `website/` (where `website/vercel.json` lives). The function is picked up from
  `api/` under that root.
- The site still builds as plain static Astro output, with no Vercel adapter in `website/astro.config.mjs`.

If either changes, the function's location has to be rethought before it can be deployed.

## One-time setup

- Create a private GitHub repository to be the inbox. Any name works; it never appears in the source.
- Create a fine-grained personal access token: repository access limited to that one repository, and the single
  repository permission **Issues: Read and write** (GitHub adds the read-only metadata permission on its own). Pick an
  expiry you will notice, because an expired token silently turns every send into a failure.
- In the Vercel project's settings, under environment variables, add both of these to the Production environment, marked
  sensitive:
  - `FEEDBACK_GITHUB_REPO`: the inbox as `owner/name`.
  - `FEEDBACK_GITHUB_TOKEN`: the token.
- Add a firewall rate-limit rule in the Vercel dashboard so floods never reach the function: match requests whose path
  is `/api/feedback`, key on the client IP, a fixed window of 10 minutes, and deny over the limit. About 5 requests per
  10 minutes per IP is a reasonable start; a person sending feedback rarely sends more than one. The function itself has
  no rate limiting. How many rate-limit rules your Vercel plan allows is shown in the dashboard; this needs one.
- The variables only exist in Production, so a preview deployment of the website answers every feedback send with 503;
  that is expected, not a bug.
- Deploy: ask for "deploy the website" (root `AGENTS.md`, "Vercel deployments"), which runs
  `website/scripts/vercel-deploy.sh main`. Environment variable changes only reach new deployments, so deploy after
  setting them.
- Verify: send feedback from the app and check that an issue appears in the inbox, with the message and the metadata in
  plain code blocks.

## Turning it off and on again

To stop all feedback at once, either revoke the token in GitHub or add a firewall rule that denies `/api/feedback`. Both
take effect immediately; changing the environment variables does not, because they only reach new deployments. With the
token revoked the function answers with a failure, and the app says sending failed and keeps the user's text.

To turn it back on, remove the deny rule, or create a new token, set it as `FEEDBACK_GITHUB_TOKEN`, and deploy again.

## What the function does with a request

It accepts only `POST` with a JSON content type, refuses a body over 64 KiB while reading it, and checks every field
against the same limits the app uses. Requiring JSON matters: a web page can make a visitor's browser post plain text to
any URL without asking first, which would let one page turn every visitor into a sender, each from a different IP
address. A JSON post makes the browser ask first, and the function refuses that. An accepted submission becomes an issue
titled `Feedback:` plus the start of the message's first line. Every user-written string in the issue body sits inside a
code fence, so nothing in it formats the issue or mentions anyone. No response and no log line contains the inbox's
name, the token, the message or the contact. When either environment variable is missing, or the repository value is not
`owner/name`, every JSON submission is refused with 503.

Its tests run with `cd website/feedback-tests && node --test`. They never touch the network or `process.env`. The
function has not run on Vercel itself until the first deployment, so the first verification send is the first real test
of the deployment assumptions above.
