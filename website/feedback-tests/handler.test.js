// Tests for the feedback endpoint's handler (website/feedback/handler.js), the
// public, unauthenticated function that turns an app's feedback submission
// into an issue in the maintainer's private inbox repository.
//
// Why these matter: this is the one piece of Farhelm an anonymous stranger
// can call, it holds a GitHub token, and its output is rendered by GitHub. So
// beyond the happy path, the tests pin what a caller must NOT be able to do:
// get the token or repository name back in any response or log line, file an
// issue whose Markdown escapes its fence (fake metadata, @-mention pings),
// make the function buffer an unbounded body, or reach GitHub with a
// submission the app would never send. Configuration and the GitHub client
// are injected, so no test touches `process.env` or the network.
//
// Run from this directory with `node --test` (root AGENTS.md lists it).

import assert from "node:assert/strict";
import { test } from "node:test";

import { composeIssue, fenced, githubIssuesClient, handleFeedback, LIMITS, validate } from "../feedback/handler.js";

const REPO = "owner/feedback-inbox";
const TOKEN = "github_pat_test_secret_value";

function valid(overrides = {}) {
  return {
    message: "The sidebar is great.",
    contact: "someone@example.com",
    version: "0.24.0",
    surface: "desktop",
    os: "macOS 26.1",
    ...overrides,
  };
}

function post(body, { headers = {}, method = "POST" } = {}) {
  const init = { method, headers: { "content-type": "application/json", ...headers } };
  if (method !== "GET" && method !== "HEAD") {
    init.body = typeof body === "string" ? body : JSON.stringify(body);
  }
  return new Request("https://farhelm.io/api/feedback", init);
}

/** A GitHub stand-in that records every call and answers with `outcome`. */
function fakeGithub(outcome = { ok: true, status: 201 }) {
  const calls = [];
  return {
    calls,
    async createIssue(args) {
      calls.push(args);
      if (outcome instanceof Error) throw outcome;
      return outcome;
    },
  };
}

function recordingLog() {
  const lines = [];
  return { lines, error: (line) => lines.push(String(line)) };
}

async function run(request, { config = { repo: REPO, token: TOKEN }, github = fakeGithub() } = {}) {
  const log = recordingLog();
  const response = await handleFeedback(request, { config, github, log });
  const text = await response.text();
  return { response, text, github, log };
}

/** Nothing secret, and nothing the caller sent, may come back or be logged. */
function assertNoLeak({ text, log }, extra = []) {
  for (const secret of [TOKEN, REPO, ...extra]) {
    assert.ok(!text.includes(secret), `response must not contain ${secret}`);
    for (const line of log.lines) {
      assert.ok(!line.includes(secret), `log line must not contain ${secret}: ${line}`);
    }
  }
}

test("a valid submission files exactly one issue with the configured repository and token", async () => {
  const outcome = await run(post(valid()));
  assert.equal(outcome.response.status, 200);
  assert.deepEqual(JSON.parse(outcome.text), { ok: true });
  assert.equal(outcome.github.calls.length, 1);
  const [call] = outcome.github.calls;
  assert.equal(call.repo, REPO);
  assert.equal(call.token, TOKEN);
  assert.equal(call.title, "Feedback: The sidebar is great.");
  assert.ok(call.body.includes("The sidebar is great."));
  assert.ok(call.body.includes("someone@example.com"));
  assert.ok(call.body.includes("version: 0.24.0\nsurface: desktop\nos: macOS 26.1"));
  assertNoLeak(outcome);
});

test("any method but POST is refused without calling GitHub", async () => {
  for (const method of ["GET", "PUT", "DELETE"]) {
    const outcome = await run(post(valid(), { method }));
    assert.equal(outcome.response.status, 405, method);
    assert.equal(outcome.response.headers.get("allow"), "POST");
    assert.equal(outcome.github.calls.length, 0, method);
  }
});

test("only application/json is accepted, so a web page cannot make browsers post without a preflight", async () => {
  // text/plain, form encodings and a missing type are what a browser sends
  // cross-origin without asking first; each must be refused before any read.
  for (const type of ["text/plain", "application/x-www-form-urlencoded", "multipart/form-data", null]) {
    const headers = type === null ? {} : { "content-type": type };
    const request = new Request("https://farhelm.io/api/feedback", {
      method: "POST",
      headers,
      body: new Blob([JSON.stringify(valid())]),
    });
    if (type === null) request.headers.delete("content-type");
    const outcome = await run(request);
    assert.equal(outcome.response.status, 415, String(type));
    assert.equal(outcome.github.calls.length, 0, String(type));
  }
  const charset = await run(post(valid(), { headers: { "content-type": "Application/JSON; charset=utf-8" } }));
  assert.equal(charset.response.status, 200);
});

test("a missing or malformed configuration refuses with 503 and names nothing", async () => {
  const configs = [
    {},
    { repo: REPO },
    { token: TOKEN },
    { repo: "not-a-repo", token: TOKEN },
    { repo: "owner/name/extra", token: TOKEN },
  ];
  for (const config of configs) {
    const outcome = await run(post(valid()), { config });
    assert.equal(outcome.response.status, 503, JSON.stringify(config));
    assert.equal(outcome.github.calls.length, 0);
    assertNoLeak(outcome, ["not-a-repo", "owner/name/extra"]);
  }
});

/**
 * A body made of `chunks` chunks of `size` bytes, produced only when the
 * handler asks for the next one, so the test can see how much was read and
 * whether the handler cancelled the rest.
 */
function observedBody(chunks, size) {
  const seen = { pulled: 0, cancelled: false };
  const stream = new ReadableStream({
    pull(controller) {
      if (seen.pulled === chunks) {
        controller.close();
        return;
      }
      seen.pulled += 1;
      controller.enqueue(new Uint8Array(size).fill(0x61));
    },
    cancel() {
      seen.cancelled = true;
    },
  }, { highWaterMark: 0 }); // pull only when the handler reads, never ahead
  return { seen, stream };
}

test("a declared length over the cap is refused with 413 before any of the body is read", async () => {
  const { seen, stream } = observedBody(64, 4096);
  const request = new Request("https://farhelm.io/api/feedback", {
    method: "POST",
    headers: { "content-type": "application/json", "content-length": String(LIMITS.bodyBytes + 1) },
    body: stream,
    duplex: "half",
  });
  assert.equal(request.headers.get("content-length"), String(LIMITS.bodyBytes + 1));
  const outcome = await run(request);
  assert.equal(outcome.response.status, 413);
  assert.equal(seen.pulled, 0, "the body must not be read once its declared length is too long");
  assert.equal(outcome.github.calls.length, 0);
});

test("an undeclared body is cancelled as soon as it passes the cap, not read to the end", async () => {
  const chunk = 4096;
  const total = 4 * (LIMITS.bodyBytes / chunk); // four times the cap, on offer
  const { seen, stream } = observedBody(total, chunk);
  const request = new Request("https://farhelm.io/api/feedback", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: stream,
    duplex: "half",
  });
  assert.equal(request.headers.get("content-length"), null);
  const outcome = await run(request);
  assert.equal(outcome.response.status, 413);
  assert.ok(seen.cancelled, "the rest of the body must be cancelled");
  // Exactly one chunk past the cap is read, then the rest is cancelled.
  assert.equal(seen.pulled, LIMITS.bodyBytes / chunk + 1, `pulled ${seen.pulled} of ${total} chunks`);
  assert.equal(outcome.github.calls.length, 0);
});

test("a body that fails while being read is a 400 that never reaches GitHub", async () => {
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(new TextEncoder().encode("{\"message\":"));
      controller.error(new Error(`upload interrupted ${TOKEN}`));
    },
  });
  const request = new Request("https://farhelm.io/api/feedback", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: stream,
    duplex: "half",
  });
  const outcome = await run(request);
  assert.equal(outcome.response.status, 400);
  // The reason tells a read failure apart from parsing the partial text,
  // which would also be a 400.
  assert.deepEqual(JSON.parse(outcome.text), { error: "submission could not be read" });
  assert.equal(outcome.github.calls.length, 0);
  assertNoLeak(outcome, ["upload interrupted"]);
});

test("the body cap admits the largest submission the field caps allow, fully escaped", async () => {
  // The worst case a JSON encoder can produce for submissions within the field
  // caps: every character outside the Basic Multilingual Plane, written as a
  // surrogate-pair escape (12 bytes each). Version and os may not hold control
  // characters, so their worst case is the same astral escape.
  const astral = "\\ud83d\\ude00";
  const raw = JSON.stringify(valid({ message: "@M@", contact: "@C@", version: "@V@", os: "@O@" }))
    .replace("@M@", astral.repeat(LIMITS.message))
    .replace("@C@", astral.repeat(LIMITS.contact))
    .replace("@V@", astral.repeat(LIMITS.version))
    .replace("@O@", astral.repeat(LIMITS.os));
  const size = new TextEncoder().encode(raw).length;
  assert.ok(size > 50 * 1024 && size <= LIMITS.bodyBytes, `worst case is ${size} bytes`);
  const outcome = await run(post(raw));
  assert.equal(outcome.response.status, 200);
});

test("malformed JSON and non-object bodies are refused with 400", async () => {
  for (const raw of ["{", "null", "[]", "\"text\"", "42"]) {
    const outcome = await run(post(raw));
    assert.equal(outcome.response.status, 400, raw);
    assert.equal(outcome.github.calls.length, 0, raw);
  }
});

test("each field's contract is enforced, counting characters as code points", () => {
  const emoji = "\u{1F600}";
  const cases = [
    [{ message: undefined }, "message is required"],
    [{ message: "   \n\t " }, "message is required"],
    [{ message: 7 }, "message is required"],
    [{ message: "a".repeat(LIMITS.message + 1) }, "message is too long"],
    [{ message: emoji.repeat(LIMITS.message + 1) }, "message is too long"],
    [{ contact: 5 }, "contact must be text"],
    [{ contact: "c".repeat(LIMITS.contact + 1) }, "contact is too long"],
    [{ version: "" }, "version is missing, too long, or not plain text"],
    [{ version: "v".repeat(LIMITS.version + 1) }, "version is missing, too long, or not plain text"],
    [{ surface: "mobile" }, "surface must be desktop or web"],
    [{ surface: undefined }, "surface must be desktop or web"],
    [{ os: "" }, "os is missing, too long, or not plain text"],
    [{ os: "o".repeat(LIMITS.os + 1) }, "os is missing, too long, or not plain text"],
    // A control character could fake another metadata line in the issue.
    [{ os: "linux\nsurface: web" }, "os is missing, too long, or not plain text"],
    [{ version: "1.0\r" }, "version is missing, too long, or not plain text"],
  ];
  for (const [overrides, error] of cases) {
    assert.deepEqual(validate(valid(overrides)), { error }, JSON.stringify(overrides));
  }
  // At the cap, and with astral characters that JavaScript's `.length` would
  // count twice: the app counts code points, so this must be accepted.
  assert.ok(validate(valid({ message: emoji.repeat(LIMITS.message) })).value);
  assert.ok(validate(valid({ contact: emoji.repeat(LIMITS.contact) })).value);
});

test("whitespace is the Unicode White_Space property, matching the helm's Rust check", () => {
  // JavaScript's trim() strips U+FEFF and keeps U+0085; Rust's
  // char::is_whitespace does the opposite. The app decides with Rust, so a
  // message the app sends must not be refused here, and the reverse.
  assert.deepEqual(validate(valid({ message: "\u0085\u2028 \u3000" })), { error: "message is required" });
  assert.ok(validate(valid({ message: "\ufeff" })).value, "U+FEFF is not whitespace");
});

test("a missing, null or blank contact becomes none, and unknown fields are ignored", () => {
  for (const contact of [undefined, null, "", "   "]) {
    assert.equal(validate(valid({ contact })).value.contact, null, String(contact));
  }
  const { value } = validate({ ...valid(), later_field: { anything: true } });
  assert.deepEqual(Object.keys(value).sort(), ["contact", "message", "os", "surface", "version"]);
  assert.equal(composeIssue(validate(valid({ contact: null })).value).body.includes("### Contact\n\nnone"), true);
});

test("an invalid submission never reaches GitHub", async () => {
  const outcome = await run(post(valid({ message: "" })));
  assert.equal(outcome.response.status, 400);
  assert.equal(outcome.github.calls.length, 0);
});

test("user text cannot close its fence: every field sits in a fence longer than its backticks", () => {
  const attack = "ok\n```\n### Sent with\n\n| version | forged |\n\n@maintainer please look\n````";
  const { body } = composeIssue(
    validate(valid({ message: attack, contact: "``` @someone", os: "linux``` @x" })).value,
  );
  // Each fenced field opens with a fence of five backticks (one more than the
  // longest run, four, in the message) and closes with the same fence, so the
  // forged heading and the mention stay inside it.
  assert.equal(fenced(attack), `\`\`\`\`\`text\n${attack}\n\`\`\`\`\``);
  assert.ok(body.includes(fenced(attack)));
  assert.ok(body.includes(fenced("``` @someone")));
  // Outside the fences, only the three fixed headings remain.
  const outside = body
    .split(/^(`{3,})text\n[\s\S]*?\n\1$/m)
    .filter((part, index) => index % 2 === 0)
    .join("");
  assert.deepEqual(outside.match(/^### .*$/gm), ["### Message", "### Contact", "### Sent with"]);
  assert.ok(!outside.includes("@"), "no mention outside a fence");
});

test("the title is the first non-blank line, collapsed and shortened", () => {
  assert.equal(composeIssue(valid({ message: "\n\n  first   line \nsecond" })).title, "Feedback: first line");
  assert.equal(composeIssue(valid({ message: "\ufeff\nreal text" })).title, "Feedback: real text");
  for (const separator of ["\r", "\u2028", "\u2029", "\r\n"]) {
    assert.equal(composeIssue(valid({ message: `one${separator}two` })).title, "Feedback: one", JSON.stringify(separator));
  }
  const long = "w".repeat(200);
  const { title } = composeIssue(valid({ message: long }));
  assert.equal([...title.slice("Feedback: ".length)].length, 60);
  assert.ok(title.endsWith("…"));
});

test("a GitHub refusal or failure is a 502 that leaks nothing", async () => {
  const message = "private remark that must not be logged";
  for (const github of [fakeGithub({ ok: false, status: 401 }), fakeGithub(new Error(`boom ${TOKEN}`))]) {
    const outcome = await run(post(valid({ message })), { github });
    assert.equal(outcome.response.status, 502);
    assert.equal(outcome.github.calls.length, 1);
    assertNoLeak(outcome, [message, "someone@example.com", "boom"]);
  }
});

test("the real GitHub client posts the issue with the token, a timeout, and no use of the reply", async () => {
  // The only code that builds the GitHub URL and the Authorization header;
  // a mistake here would otherwise first show up on the live deployment.
  const requests = [];
  let bodyCancelled = false;
  const fetchImpl = async (url, init) => {
    requests.push({ url, init });
    const body = new ReadableStream({ cancel: () => { bodyCancelled = true; } });
    return new Response(body, { status: 201 });
  };
  const client = githubIssuesClient(fetchImpl, { timeoutMs: 1234 });
  const outcome = await client.createIssue({ repo: REPO, token: TOKEN, title: "t", body: "b" });
  assert.deepEqual(outcome, { ok: true, status: 201 });
  assert.equal(requests.length, 1);
  const [{ url, init }] = requests;
  assert.equal(url, `https://api.github.com/repos/${REPO}/issues`);
  assert.equal(init.method, "POST");
  assert.equal(init.headers.authorization, `Bearer ${TOKEN}`);
  assert.equal(init.headers.accept, "application/vnd.github+json");
  assert.deepEqual(JSON.parse(init.body), { title: "t", body: "b" });
  assert.ok(init.signal instanceof AbortSignal, "the call is bounded by a timeout");
  assert.ok(bodyCancelled, "the unused reply body is released");

  assert.equal(init.redirect, "error", "a redirect must not turn the POST into a GET that creates nothing");
  // Only 201 Created means an issue exists: a 200 (what a followed redirect's
  // GET of the issue list would answer) or a refusal is a failure.
  for (const status of [200, 204, 403]) {
    const answered = githubIssuesClient(async () => new Response(null, { status }));
    assert.deepEqual(await answered.createIssue({ repo: REPO, token: TOKEN, title: "t", body: "b" }), {
      ok: false,
      status,
    });
  }
});
