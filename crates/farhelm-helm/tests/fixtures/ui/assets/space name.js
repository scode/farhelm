// Step 1 embedded-UI test fixture: farhelm-helm/tests/fixtures/ui/assets.
// The space in this file's own name is the point: a real `dx` build can
// legitimately emit an asset whose name needs percent-encoding in a URL
// (this one is a stand-in for that), and the request path `serve_embedded`
// receives is percent-decoded before it is ever compared against
// `include_dir!`'s exact, unencoded entry names — this fixture is what
// proves that decoding step actually runs.
console.log("farhelm-embedded-ui-fixture-space-name-js");
