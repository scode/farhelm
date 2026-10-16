# Vercel deployments

The root `AGENTS.md` section "Vercel deployments", moved here with its references rewritten, plus a paragraph from the
stable release procedure. Read it before deploying the docs website or triggering a Vercel preview, and before telling
anyone how the site gets deployed.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## Vercel deployments

The docs website deploys to Vercel only when asked. `website/vercel.json` turns off every Git-triggered deployment,
because Vercel bills even a skipped push for at least a minute of build time and nearly every push leaves the site
unchanged. Merging to main does not update the live site, and a PR gets no preview, until someone triggers one.

"Trigger vercel on main", "deploy the website", and similar wording mean `website/scripts/vercel-deploy.sh main`: a
production deployment of GitHub's current main. "Trigger vercel on this PR" (or on PR N, or on a named branch or
bookmark) means `website/scripts/vercel-deploy.sh <pr-number>`: a preview deployment of that PR's pushed head. Resolve
"this PR" from the current bookmark or branch with `gh pr view`; if there is no PR, ask rather than guessing one. The
script builds from GitHub, not from the working copy, so unpushed changes are not deployed: if the working copy is ahead
of what GitHub has, say so and ask whether to push. It waits for the build and prints the deployment URL; report that
URL and the outcome. It needs a logged-in Vercel CLI; if `vercel api` reports being logged out, ask the user to run
`! vercel login` rather than handling a token. A request to trigger Vercel is not a request to run the website build or
any other check first.

While the newest `CHANGELOG.md` section still opens with the draft notice (from the stable release's changelog PR until
its notes are finalized), main holds draft release notes, so a docs website deployed in that window shows the draft on
the Release notes page (`releasing/.agents/stable.md`, "Cutting a stable release"). When asked to deploy the website
while the notes are still a draft, say so before deploying.
