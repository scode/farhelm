// Vercel function for POST https://farhelm.io/api/feedback, the inbox the
// app's "Send feedback" dialog reaches through the helm. Vercel deploys every
// file in this `api/` directory as a function beside the static docs site.
//
// This entry only wires the deployment in: it reads the private inbox
// repository and its token from the project's environment (never from
// source; docs/feedback-endpoint.md lists the variables) and hands the
// request to the tested handler with the platform's `fetch`.
import { githubIssuesClient, handleFeedback } from "../feedback/handler.js";

export default {
  fetch(request) {
    return handleFeedback(request, {
      config: {
        repo: process.env.FEEDBACK_GITHUB_REPO,
        token: process.env.FEEDBACK_GITHUB_TOKEN,
      },
      github: githubIssuesClient(fetch),
      log: console,
    });
  },
};
