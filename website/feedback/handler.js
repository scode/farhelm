// The feedback endpoint's logic, kept free of Vercel and of `process.env` so
// `node --test` exercises exactly this file with injected configuration and an
// injected GitHub client (website/feedback-tests/). The thin Vercel entry,
// website/api/feedback.js, only reads the environment and passes the real
// client in. It lives outside `api/` on purpose: Vercel deploys every file in
// that directory as its own function.
//
// What an accepted submission becomes: one issue in a private GitHub
// repository, the maintainer's feedback inbox (SPEC.md "Feedback", SPEC_impl.md
// "Feedback forwarding"). The repository and the token are deployment
// configuration and never appear in a response or a log line, so a caller
// cannot learn where feedback goes or with what credential.
//
// The function is a public, unauthenticated URL. Everything an anonymous
// caller sends is untrusted: the body is size-capped while it is read, every
// field is type- and length-checked, and every user-controlled string in the
// issue body sits inside a fenced code block, so its Markdown, HTML and
// `@mentions` render as inert text instead of formatting the issue or pinging
// anyone. The title carries a shortened first line of the message; GitHub
// renders at most inline code there, which can neither format the body nor
// mention anyone.
//
// Only `application/json` is accepted. A browser sends a cross-origin POST
// with a "simple" content type such as text/plain without asking first, so
// without this check any web page could make every visitor's browser file
// feedback, one IP address per visitor, around the per-IP rate limit. A JSON
// content type makes the browser ask first (a CORS preflight), which this
// function refuses like any other method; the helm is not a browser and is
// unaffected. Rate limiting is deliberately not here: a Vercel firewall rule on
// the path refuses excess traffic before this code runs
// (docs/feedback-endpoint.md).

/**
 * Size caps. `message` and `contact` match the helm's and the UI's caps in
 * `farhelm-proto` (`FEEDBACK_MESSAGE_MAX_CHARS`, `FEEDBACK_CONTACT_MAX_CHARS`),
 * counted in Unicode code points like Rust's `chars().count()`, so a
 * submission the app accepts is never refused here for its length. `version`
 * and `os` are short machine-written strings; their caps only bound what an
 * anonymous caller can put in an issue. `bodyBytes` bounds the raw request
 * before any parsing, because this is the public boundary. It sits well above
 * the largest submission the field caps allow even when every character is
 * JSON-escaped (about 52 KiB: a character outside the Basic Multilingual
 * Plane escapes to two `\uXXXX` sequences, 12 bytes), so the cap never
 * refuses a submission the app accepted.
 */
export const LIMITS = Object.freeze({
  bodyBytes: 64 * 1024,
  message: 4000,
  contact: 200,
  version: 64,
  os: 128,
});

/** Issue titles carry about this many characters of the message's first line. */
export const TITLE_CHARS = 60;

const SURFACES = new Set(["desktop", "web"]);

/** `owner/name` as GitHub allows it, so a misconfigured value fails closed. */
const REPO_PATTERN = /^[A-Za-z0-9-]+\/[A-Za-z0-9._-]+$/;

/**
 * Handle one request to `/api/feedback` and return the response to send.
 *
 * `config` is `{ repo, token }` read from the deployment's environment; either
 * missing (or a malformed `repo`) refuses every submission with 503, which the
 * app shows as an ordinary send failure. `github.createIssue({ repo, token,
 * title, body })` files the issue and resolves to `{ ok, status }`; it may
 * also throw. `log` receives only fixed event names and HTTP status codes,
 * never the message, the contact, the repository or the token.
 *
 * Status codes: 200 on success; 400 for a malformed or invalid submission
 * (or a body that fails while being read); 405 for any method but POST; 413
 * for an oversized body; 415 for any content type but `application/json`;
 * 502 when GitHub refuses or cannot be reached; 503 when the deployment is
 * not configured.
 * Every response body is a small JSON object whose text is fixed, so nothing
 * a caller sends and nothing from the configuration is echoed back.
 */
export async function handleFeedback(request, { config, github, log }) {
  if (request.method !== "POST") {
    return reply(405, { error: "method not allowed" }, { allow: "POST" });
  }
  const contentType = (request.headers.get("content-type") ?? "").split(";")[0].trim().toLowerCase();
  if (contentType !== "application/json") {
    return reply(415, { error: "submission must be JSON" });
  }
  const repo = config?.repo;
  const token = config?.token;
  if (!repo || !token || !REPO_PATTERN.test(repo)) {
    log.error("feedback: endpoint not configured");
    return reply(503, { error: "feedback is not available" });
  }

  let raw;
  try {
    raw = await readCapped(request, LIMITS.bodyBytes);
  } catch {
    // An interrupted or failing upload: answer it like any other bad
    // request instead of letting the rejection escape to the platform.
    return reply(400, { error: "submission could not be read" });
  }
  if (raw === null) {
    return reply(413, { error: "submission too large" });
  }
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return reply(400, { error: "submission is not valid JSON" });
  }
  const submission = validate(parsed);
  if (submission.error) {
    return reply(400, { error: submission.error });
  }

  const issue = composeIssue(submission.value);
  let outcome;
  try {
    outcome = await github.createIssue({ repo, token, ...issue });
  } catch {
    log.error("feedback: GitHub request failed");
    return reply(502, { error: "feedback could not be filed" });
  }
  if (!outcome?.ok) {
    log.error(`feedback: GitHub refused with status ${Number(outcome?.status) || 0}`);
    return reply(502, { error: "feedback could not be filed" });
  }
  return reply(200, { ok: true });
}

/**
 * Check a parsed body against the submission contract the helm forwards:
 * `{ message, contact, version, surface, os }`. Unknown fields are ignored,
 * which leaves room for later fields without a coordinated deploy; known ones
 * must have the right type and fit their caps. Returns `{ value }` with the
 * fields to use, or `{ error }` with a fixed, caller-safe reason.
 */
export function validate(parsed) {
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    return { error: "submission must be a JSON object" };
  }
  const { message, contact, version, surface, os } = parsed;
  if (typeof message !== "string" || isBlank(message)) {
    return { error: "message is required" };
  }
  if (codePoints(message) > LIMITS.message) {
    return { error: "message is too long" };
  }
  if (contact !== undefined && contact !== null && typeof contact !== "string") {
    return { error: "contact must be text" };
  }
  if (typeof contact === "string" && codePoints(contact) > LIMITS.contact) {
    return { error: "contact is too long" };
  }
  if (!isMachineField(version, LIMITS.version)) {
    return { error: "version is missing, too long, or not plain text" };
  }
  if (!SURFACES.has(surface)) {
    return { error: "surface must be desktop or web" };
  }
  if (!isMachineField(os, LIMITS.os)) {
    return { error: "os is missing, too long, or not plain text" };
  }
  const blankContact = typeof contact !== "string" || isBlank(contact);
  return { value: { message, contact: blankContact ? null : contact, version, surface, os } };
}

/**
 * The issue for one valid submission: a short title from the message's first
 * non-blank line, and a body whose every user-controlled string sits in its
 * own fenced block (see the module comment for why).
 */
export function composeIssue({ message, contact, version, surface, os }) {
  // Choose the line with the same rule that cleans it (JavaScript's trim), so
  // a line holding only characters the cleanup removes is skipped rather
  // than leaving an empty title. This is presentation only; whether the
  // message is blank at all is decided by `isBlank` in `validate`.
  const firstLine = message.split(/\r\n|[\n\r\u2028\u2029]/).find((line) => line.trim() !== "")?.trim() ?? "";
  const title = `Feedback: ${shorten(firstLine.replace(/\s+/g, " "), TITLE_CHARS)}`;
  const body = [
    "### Message",
    "",
    fenced(message),
    "",
    "### Contact",
    "",
    contact === null ? "none" : fenced(contact),
    "",
    "### Sent with",
    "",
    fenced(`version: ${version}\nsurface: ${surface}\nos: ${os}`),
    "",
  ].join("\n");
  return { title, body };
}

/**
 * Wrap text in a code fence longer than any run of backticks inside it, so
 * the text cannot close the fence early and continue as live Markdown.
 */
export function fenced(text) {
  const longestRun = Math.max(0, ...(text.match(/`+/g) ?? []).map((run) => run.length));
  const fence = "`".repeat(Math.max(3, longestRun + 1));
  return `${fence}text\n${text}\n${fence}`;
}

/** Cut to `limit` code points, marking the cut with an ellipsis. */
function shorten(text, limit) {
  const points = [...text];
  return points.length <= limit ? text : `${points.slice(0, limit - 1).join("")}…`;
}

/**
 * Whether text is empty or only whitespace, where whitespace is the Unicode
 * `White_Space` property: what Rust's `char::is_whitespace` uses on the helm
 * and in the UI. JavaScript's own `trim()` disagrees with it on two characters
 * (it strips U+FEFF, which is not `White_Space`, and keeps U+0085, which is),
 * so using it here would refuse or admit messages the app treats otherwise.
 */
function isBlank(text) {
  return /^\p{White_Space}*$/u.test(text);
}

/**
 * A UI-written field (version, operating system): non-empty text within its
 * cap, with no control characters, so it can neither be missing from the
 * issue nor add a line that looks like another metadata field.
 */
function isMachineField(value, max) {
  return typeof value === "string" && value !== "" && codePoints(value) <= max && !/\p{Cc}/u.test(value);
}

function codePoints(text) {
  return [...text].length;
}

/**
 * Read the request body as text, or return null as soon as it exceeds
 * `limit` bytes. A declared Content-Length over the limit is refused before
 * reading; an undeclared or understated one is caught while streaming, so the
 * function never buffers more than the cap.
 */
async function readCapped(request, limit) {
  const declared = Number(request.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > limit) {
    return null;
  }
  if (!request.body) {
    return "";
  }
  const reader = request.body.getReader();
  const chunks = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (total > limit) {
      await reader.cancel().catch(() => {});
      return null;
    }
    chunks.push(value);
  }
  const bytes = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return new TextDecoder().decode(bytes);
}

function reply(status, payload, headers = {}) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json", "cache-control": "no-store", ...headers },
  });
}

/**
 * The real GitHub client: `POST /repos/{owner}/{name}/issues` with the
 * deployment's token, bounded by a timeout so a slow GitHub cannot hold the
 * function (and the helm waiting on it) open. `fetchImpl` is injected so the
 * entry passes the platform's `fetch`.
 */
export function githubIssuesClient(fetchImpl, { timeoutMs = 10_000 } = {}) {
  return {
    async createIssue({ repo, token, title, body }) {
      const response = await fetchImpl(`https://api.github.com/repos/${repo}/issues`, {
        method: "POST",
        headers: {
          accept: "application/vnd.github+json",
          authorization: `Bearer ${token}`,
          "content-type": "application/json",
          "user-agent": "farhelm-feedback",
          "x-github-api-version": "2022-11-28",
        },
        body: JSON.stringify({ title, body }),
        signal: AbortSignal.timeout(timeoutMs),
        // A renamed inbox repository answers with a redirect, and fetch would
        // follow it by turning this POST into a GET whose 200 creates
        // nothing. Refusing redirects makes a stale name fail visibly.
        redirect: "error",
      });
      // Nothing in the reply is used; release the connection rather than
      // leave the body unread.
      await response.body?.cancel().catch(() => {});
      // 201 Created is the only answer that means an issue now exists.
      return { ok: response.status === 201, status: response.status };
    },
  };
}
