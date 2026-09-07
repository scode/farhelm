"use strict";

const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const LIMIT = 64 * 1024;

/**
 * Retain resolved runner policy without serializing credentials from `use`.
 *
 * The recorder supplies a private parent directory. Initial publication refuses
 * every existing destination; terminal publication replaces only the regular
 * file this instance published. Identity checks detect accidental replacement,
 * but do not claim protection against concurrent tampering by the same user.
 */
class RecordedPolicyReporter {
  constructor(options = {}) {
    this.outputFile = options.outputFile || process.env.FARHELM_PLAYWRIGHT_POLICY_FILE;
    this.published = null;
    this.document = null;
  }

  /** Publish a complete private file, preserving the previous record on failure. */
  _publish(value, replace) {
    if (!this.outputFile) throw new Error("FARHELM_PLAYWRIGHT_POLICY_FILE is required");
    const output = path.resolve(this.outputFile);
    const encoded = Buffer.from(JSON.stringify(value));
    if (encoded.length > LIMIT) throw new Error("policy report exceeds its size limit");
    const temporary = path.join(
      path.dirname(output),
      `.${path.basename(output)}.${crypto.randomBytes(12).toString("hex")}.tmp`,
    );
    let fd;
    let created = false;
    try {
      fd = fs.openSync(temporary, "wx", 0o600);
      created = true;
      fs.writeFileSync(fd, encoded);
      fs.fsyncSync(fd);
      const identity = fs.fstatSync(fd);
      fs.closeSync(fd);
      fd = undefined;

      if (replace) {
        const current = fs.lstatSync(output);
        if (!this.published || !current.isFile() ||
            current.dev !== this.published.dev || current.ino !== this.published.ino) {
          throw new Error("policy output was replaced");
        }
        fs.renameSync(temporary, output);
      } else {
        // Unlike an existence check followed by rename, link cannot overwrite
        // a stale file or dangling symlink created before publication.
        fs.linkSync(temporary, output);
        fs.unlinkSync(temporary);
      }
      created = false;
      this.published = identity;
    } finally {
      if (fd !== undefined) fs.closeSync(fd);
      if (created) fs.unlinkSync(temporary);
    }
  }

  /** Record the resolved policy before tests run, explicitly without completion. */
  onBegin(config) {
    if (!Array.isArray(config.projects) || config.projects.length > 512) {
      throw new Error("invalid project count");
    }
    const projects = config.projects.map((project) => ({
      name: project.name,
      engine: project.use.browserName ?? project.use.defaultBrowserType ?? "chromium",
      retries: project.retries,
      repeatEach: project.repeatEach,
      outputDir: project.outputDir,
    }));
    const document = {
      schema_version: 1,
      completed: false,
      workers: config.workers,
      forbidOnly: config.forbidOnly,
      failOnFlakyTests: config.failOnFlakyTests,
      projects,
    };
    this._publish(document, false);
    this.document = document;
  }

  /** Completion requires onEnd; a killed runner leaves the initial record intact. */
  onEnd(result) {
    if (!this.document) throw new Error("onEnd before onBegin");
    const document = { ...this.document, completed: true, status: result.status };
    this._publish(document, true);
    this.document = document;
  }
}
module.exports = RecordedPolicyReporter;
