const test = require("node:test");
const assert = require("node:assert/strict");
const { capture, chooseName, extensionFor } = require("../assets/clipboard-name.js");

const policy = {
  placeholderStem: "image",
  placeholderMaxAgeMs: 5000,
  fallbackExtension: "bin",
  maxExtensionLength: 12,
  extensionAliases: { jpeg: "jpg" },
};

function facts(fileName, lastModified, fileType = "image/png") {
  return {
    itemCount: 1,
    items: [{ order: 0, kind: "file", type: fileType, fileName, fileType, lastModified }],
  };
}

// The debug affordance and filename decision must describe the paste event,
// not mutable browser objects inspected after asynchronous uploads begin.
test("capture freezes ordered file facts without retaining browser objects", () => {
  const file = { name: "diagram.png", type: "image/png", lastModified: 1234 };
  const captured = capture({
    items: [
      { kind: "string", type: "text/plain", getAsFile: () => null },
      { kind: "file", type: "image/png", getAsFile: () => file },
    ],
  });

  assert.deepEqual(captured, {
    itemCount: 2,
    items: [
      { order: 0, kind: "string", type: "text/plain", fileName: null, fileType: null, lastModified: null },
      { order: 1, kind: "file", type: "image/png", fileName: "diagram.png", fileType: "image/png", lastModified: 1234 },
    ],
    fileCount: 0,
    files: [],
  });
  assert.equal(JSON.stringify(captured).includes("getAsFile"), false);
});

// A failed projection is the case the diagnostic exists to explain. Losing
// the item record here would leave a real Mac failure with no facts to carry
// into a reproducible browser fixture.
test("capture keeps item facts when getAsFile throws", () => {
  const captured = capture({
    items: [{
      kind: "file",
      type: "image/tiff",
      getAsFile: () => { throw new Error("engine refused the File projection"); },
    }],
  });
  assert.deepEqual(captured.items, [{
    order: 0,
    kind: "file",
    type: "image/tiff",
    fileName: null,
    fileType: null,
    lastModified: null,
  }]);
});

test("a copied file keeps DataTransferItem File.name", () => {
  assert.equal(chooseName(facts("holiday.png", 10), 0, 20_000, policy), "holiday.png");
});

// Age distinguishes a real file named image.png from the engine's synthetic
// screenshot placeholder without keeping a growing platform-name list.
test("an old real file named like the engine placeholder keeps its name", () => {
  assert.equal(chooseName(facts("image.png", 10), 0, 20_000, policy), "image.png");
});

test("a freshly synthesized engine placeholder gets a generated name", () => {
  assert.equal(chooseName(facts("image.png", 19_500), 0, 20_000, policy), null);
});

// The bounded heuristic intentionally accepts this false positive: a real file
// literally named image.png and modified moments ago is indistinguishable from
// WebKit's synthetic screenshot placeholder with the available clipboard facts.
test("a fresh real file matching the placeholder is the documented false positive", () => {
  assert.equal(chooseName(facts("image.png", 19_500), 0, 20_000, policy), null);
});

test("a nameless clipboard image gets a generated name", () => {
  assert.equal(chooseName(facts("", 19_500), 0, 20_000, policy), null);
});

test("a non-image file always keeps its available name", () => {
  assert.equal(chooseName(facts("notes.txt", 19_500, "text/plain"), 0, 20_000, policy), "notes.txt");
});

test("a nameless non-image remains a file even though its upload name is generated", () => {
  assert.equal(chooseName(facts("", 19_500, "application/pdf"), 0, 20_000, policy), "");
});

// Some clipboard implementations expose files only through DataTransfer.files.
// The fallback must be frozen at event time and use the same naming policy.
test("the files-only fallback is captured and named through the pure policy", () => {
  const captured = capture({
    files: [{ name: "report.pdf", type: "application/pdf", lastModified: 42 }],
  });
  assert.equal(captured.fileCount, 1);
  assert.equal(chooseName(captured, 0, 20_000, policy, "files"), "report.pdf");
});

test("extension derivation covers normalized and hostile MIME spellings", () => {
  const cases = [
    ["image/png; charset=binary", "png"],
    ["IMAGE/JPEG", "jpg"],
    ["image/svg+xml", "svg"],
    ["image/vnd.example.webp", "webp"],
    ["image/x-icon", "icon"],
    ["image/!!!", "bin"],
    ["image/this-extension-is-far-too-long", "bin"],
    ["application/octet-stream", "bin"],
  ];
  for (const [mime, expected] of cases) {
    assert.equal(extensionFor(mime, policy), expected, mime);
  }
});
