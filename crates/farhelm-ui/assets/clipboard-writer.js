// A single-flight, latest-value-wins clipboard writer. The terminal parser
// must remain synchronous, while both the native bridge and the browser API
// may take an unbounded time to settle.
//
// The queue deliberately retains only one pending value. A remote program may
// emit many OSC 52 writes before the desktop can apply one, but older values
// have no useful meaning once a newer value exists. Keeping this policy in a
// tiny plain-script asset lets node --test exercise the exact function the
// page uses without introducing a second implementation for tests.
(function () {
  /**
   * Wrap a best-effort clipboard operation with one in-flight request and one
   * replaceable pending value.
   *
   * The underlying operation may return a promise, a thenable, no value, or
   * throw synchronously. Every outcome releases the in-flight slot; failures
   * stay silent because clipboard writes are deliberately best effort.
   *
   * @param {(text: string) => unknown} write the underlying clipboard route
   * @returns {(text: string) => void} a synchronous enqueue operation
   */
  function createClipboardWriter(write) {
    let inFlight = false;
    let hasPending = false;
    let pending;

    const settle = () => {
      inFlight = false;
      if (!hasPending) return;
      const next = pending;
      hasPending = false;
      pending = undefined;
      start(next);
    };

    const start = (text) => {
      inFlight = true;
      let result;
      try {
        result = write(text);
      } catch (_) {
        settle();
        return;
      }
      if (result && typeof result.then === "function") {
        Promise.resolve(result).then(settle, settle);
      } else {
        settle();
      }
    };

    return (text) => {
      if (inFlight) {
        pending = text;
        hasPending = true;
        return;
      }
      start(text);
    };
  }

  const api = { createClipboardWriter };
  if (typeof window !== "undefined") window.farhelmClipboardWriter = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})();
