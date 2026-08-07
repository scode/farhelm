// Pure clipboard naming decisions shared by the live terminal and node tests.
// Capture is separate from policy: a real WKWebView payload can be pasted into
// the debug dump and then kept as a fixture without changing the decision.
(function () {
  function normalizedMime(value) {
    return String(value || "").split(";")[0].trim().toLowerCase();
  }

  function extensionFor(mime, policy) {
    const normalized = normalizedMime(mime);
    const fallback = policy.fallbackExtension || "bin";
    if (normalized.indexOf("image/") !== 0) return fallback;
    let token = normalized.slice("image/".length).split("+")[0];
    token = token.slice(token.lastIndexOf(".") + 1);
    if (token.indexOf("x-") === 0) token = token.slice(2);
    token = token.replace(/[^a-z0-9]/g, "");
    if (!token || token.length > (policy.maxExtensionLength || 12)) return fallback;
    return (policy.extensionAliases || {})[token] || token;
  }

  /** Capture only serializable clipboard facts from both collection shapes. */
  function capture(data) {
    const items = data && data.items ? Array.prototype.slice.call(data.items) : [];
    const files = data && data.files ? Array.prototype.slice.call(data.files) : [];
    const fileFacts = function (file, order) {
      return {
        order,
        kind: "file",
        type: String((file && file.type) || ""),
        fileName: file ? String(file.name || "") : null,
        fileType: file ? String(file.type || "") : null,
        lastModified: file && typeof file.lastModified === "number" ? file.lastModified : null,
      };
    };
    return {
      itemCount: items.length,
      items: items.map(function (item, order) {
        let file = null;
        try {
          file = item.kind === "file" && item.getAsFile ? item.getAsFile() : null;
        } catch (error) {
          // A failed File projection is itself diagnostic. Keep the stable
          // item facts and omit only the fields the engine refused to expose.
          file = null;
        }
        return {
          order,
          kind: String(item.kind || ""),
          type: String(item.type || ""),
          fileName: file ? String(file.name || "") : null,
          fileType: file ? String(file.type || "") : null,
          lastModified: file && typeof file.lastModified === "number" ? file.lastModified : null,
        };
      }),
      fileCount: files.length,
      files: files.map(fileFacts),
    };
  }

  /**
   * Return the user-chosen file name for one captured file, or null when the
   * engine appears to have supplied a synthetic image placeholder and Farhelm
   * should generate `pasted-N.ext` instead. The freshness rule is a heuristic:
   * a real image genuinely named `image.<ext>` and modified just before paste
   * is indistinguishable from the engine placeholder and gets a generated
   * name. An empty string means a nameless non-image file; it still belongs in
   * the file bucket even though its upload name must be generated.
   */
  function chooseName(facts, order, now, policy, collection) {
    const entries = facts && facts[collection || "items"];
    const item = entries ? entries[order] : null;
    if (!item || item.kind !== "file") return undefined;
    const mime = normalizedMime(item.fileType || item.type);
    if (!item.fileName) return mime.indexOf("image/") === 0 ? null : "";
    if (mime.indexOf("image/") !== 0) return item.fileName;
    const placeholder = (policy.placeholderStem || "image") + "." + extensionFor(mime, policy);
    const fresh = typeof item.lastModified === "number"
      && Math.abs(now - item.lastModified) <= (policy.placeholderMaxAgeMs || 0);
    return item.fileName.toLowerCase() === placeholder && fresh ? null : item.fileName;
  }

  const api = { capture, chooseName, extensionFor };
  if (typeof window !== "undefined") window.farhelmClipboardNames = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})();
