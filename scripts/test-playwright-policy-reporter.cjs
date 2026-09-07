"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const Reporter = require("../e2e/recorded-policy-reporter.cjs");

/** Each publication test owns its directory, including when an assertion fails. */
function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "policy-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const output = path.join(root, "policy.json");
  return { root, output, reporter: new Reporter({ outputFile: output }) };
}

/** Resolved projects may carry secrets in use; only the engine belongs in policy. */
function config() {
  return {
    workers: 1, forbidOnly: true, failOnFlakyTests: true,
    projects: [{
      name: "webkit-x", retries: 0, repeatEach: 1, outputDir: "out",
      use: { defaultBrowserType: "webkit", extraHTTPHeaders: { Authorization: "secret" } },
    }],
  };
}

test("initial policy is private and incomplete until terminal replacement", (t) => {
  const { root, output, reporter } = fixture(t);
  reporter.onBegin(config());
  const initial = JSON.parse(fs.readFileSync(output));
  assert.equal(initial.completed, false);
  assert.equal(initial.status, undefined);
  assert.equal(fs.statSync(output).mode & 0o777, 0o600);
  reporter.onEnd({ status: "passed" });
  const terminal = JSON.parse(fs.readFileSync(output));
  assert.equal(terminal.completed, true);
  assert.equal(terminal.status, "passed");
  assert.equal(terminal.projects[0].engine, "webkit");
  assert.equal(JSON.stringify(terminal).includes("secret"), false);
  assert.deepEqual(fs.readdirSync(root), ["policy.json"]);
});

test("stale files and dangling symlinks survive refused initial publication", (t) => {
  for (const linked of [false, true]) {
    const { root, output, reporter } = fixture(t);
    if (linked) fs.symlinkSync(path.join(root, "missing"), output);
    else fs.writeFileSync(output, "old");
    assert.throws(() => reporter.onBegin(config()), { code: "EEXIST" });
    assert.equal(fs.lstatSync(output).isSymbolicLink(), linked);
    if (!linked) assert.equal(fs.readFileSync(output, "utf8"), "old");
    assert.deepEqual(fs.readdirSync(root), ["policy.json"]);
  }
});

test("terminal publication refuses a different file or symlink", (t) => {
  for (const linked of [false, true]) {
    const { root, output, reporter } = fixture(t);
    reporter.onBegin(config());
    // Keep the old inode allocated so this test cannot depend on inode reuse.
    fs.renameSync(output, path.join(root, "initial"));
    if (linked) fs.symlinkSync(path.join(root, "initial"), output);
    else fs.writeFileSync(output, "replacement");
    assert.throws(() => reporter.onEnd({ status: "passed" }), /replaced/);
    assert.equal(JSON.parse(fs.readFileSync(path.join(root, "initial"))).completed, false);
    if (!linked) assert.equal(fs.readFileSync(output, "utf8"), "replacement");
    assert.deepEqual(fs.readdirSync(root).sort(), ["initial", "policy.json"]);
  }
});

test("terminal replacement requires both device and inode identity", (t) => {
  const { output, reporter } = fixture(t);
  reporter.onBegin(config());
  reporter.published = { ...reporter.published, dev: reporter.published.dev + 1 };
  assert.throws(() => reporter.onEnd({ status: "passed" }), /replaced/);
  assert.equal(JSON.parse(fs.readFileSync(output)).completed, false);
});

test("project and byte bounds fail before publication", (t) => {
  const { root, reporter } = fixture(t);
  const many = config();
  many.projects = Array.from({ length: 513 }, () => many.projects[0]);
  assert.throws(() => reporter.onBegin(many), /project count/);
  const large = config();
  large.projects[0].name = "x".repeat(64 * 1024);
  assert.throws(() => reporter.onBegin(large), /size limit/);
  assert.deepEqual(fs.readdirSync(root), []);
});

test("onEnd cannot invent an initial publication", (t) => {
  const { root, reporter } = fixture(t);
  assert.throws(() => reporter.onEnd({ status: "passed" }), /before onBegin/);
  assert.deepEqual(fs.readdirSync(root), []);
});

test("a temporary-name collision preserves the foreign file", (t) => {
  const { root, output } = fixture(t);
  const moduleFixture = { exports: {} };
  // Give this isolated module a deterministic nonce without replacing process
  // globals or adding test-only configuration to the production reporter.
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, "../e2e/recorded-policy-reporter.cjs"), "utf8"), {
    module: moduleFixture,
    Buffer,
    require: (name) => name === "node:crypto"
      ? { randomBytes: (size) => Buffer.alloc(size) }
      : require(name),
  });
  const temporary = path.join(root, ".policy.json." + "0".repeat(24) + ".tmp");
  fs.writeFileSync(temporary, "foreign");
  const reporter = new moduleFixture.exports({ outputFile: output });
  assert.throws(() => reporter.onBegin(config()), { code: "EEXIST" });
  assert.equal(fs.readFileSync(temporary, "utf8"), "foreign");
  assert.equal(fs.existsSync(output), false);
});
