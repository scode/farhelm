// Rust supplies the physical keys and modifier masks. This file owns the
// synchronous keyboard boundary; Rust owns template resolution and launch.
(function (root) {
  /** Select the mask for this client's build or operating system.
   * The source decides whether macOS means this build or the runtime platform;
   * the returned mask supplies every required modifier explicitly.
   */
  function selectedMask(modifiers, definition) {
    const mac = modifiers.source === "platform" ? definition.mac_platform : definition.mac_desktop;
    return mac ? modifiers.mac : modifiers.other;
  }

  /** Match a chord against the exact modifier mask supplied by Rust. */
  function chord(event, modifiers, definition) {
    const mask = selectedMask(modifiers, definition);
    return event.metaKey === mask.meta && event.ctrlKey === mask.ctrl &&
      event.shiftKey === mask.shift && event.altKey === mask.alt;
  }

  /** Keep a sequence armed while a modifier chord is still being assembled.
   * The existing Meta, Control and Shift keys preserve it on their own. Alt
   * does so only when the pressed modifiers are a subset of a configured Alt
   * chord; an unrelated Alt press still cancels and passes to the terminal.
   */
  function partialModifierChord(event, definition) {
    if (!["Meta", "Control", "Shift", "Alt"].includes(event.key)) return false;
    if (!event.altKey) return event.key !== "Alt";
    // Only these two chords can add a reference after a sequence is armed.
    // Prefix and Cmd-J modifiers do not make an armed sequence's Alt relevant.
    return [definition.bolt.modifiers, definition.number.modifiers]
      .some(modifiers => {
        const mask = selectedMask(modifiers, definition);
        return mask.alt && (!event.metaKey || mask.meta) && (!event.ctrlKey || mask.ctrl) &&
          (!event.shiftKey || mask.shift);
      });
  }

  /** Classify an input without consulting the DOM or changing sequence state.
   * `ordinaryTextEditor` bars only an idle prefix. A bare number is meaningful
   * only while armed; keypad keys are never accepted. `none` consumes a held
   * recognized key without admitting it twice. When idle, `source` retains
   * the relay identity so a missing trigger cannot claim a repeated chord.
   * Cmd-J's key identity and modifier rule come from the same definition as
   * the template chords, even though it opens a list instead of a launch.
   */
  function classifyTemplateShortcutKey(event, armed, definition, ordinaryTextEditor) {
    const pass = { action: "pass", consume: false };
    const cancelPass = { action: "cancel", consume: false };
    if (event.isComposing) return armed ? cancelPass : pass;
    const bare = !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
    const number = definition.number.keys.find(key => key.code === event.code);
    const numberChord = chord(event, definition.number.modifiers, definition);
    const prefix = event.code === definition.prefix.code && chord(event, definition.prefix.modifiers, definition);
    const bolt = event.code === definition.bolt.code && chord(event, definition.bolt.modifiers, definition);
    const list = event.code === definition.template_list.code &&
      chord(event, definition.template_list.modifiers, definition);

    // Opening the template list supersedes an unfinished sequence. Recognize
    // it before the armed branch so its chord never falls through to xterm.
    if (list) return { action: "template-list", consume: true };

    if (!armed) {
      if (prefix && !ordinaryTextEditor) {
        return event.repeat ? { action: "none", consume: true, source: "begin" } :
          { action: "begin", consume: true };
      }
      if (bolt) {
        return event.repeat ? { action: "none", consume: true, source: "quick-bolt" } :
          { action: "quick-bolt", consume: true };
      }
      if (number && numberChord) {
        return event.repeat ? { action: "none", consume: true, source: "quick-digit", digit: number.value } :
          { action: "quick-digit", consume: true, digit: number.value };
      }
      return pass;
    }

    let action;
    let digit;
    if (number && (bare || numberChord)) {
      action = "digit";
      digit = number.value;
    } else if (bolt) action = "bolt";
    else if (event.key === "Enter" && bare) action = "launch";
    else if (event.key === "Escape") action = "cancel";
    // Holding the prefix must not clear the queue that its first event began.
    else if (prefix && event.repeat) action = "none";
    else if (partialModifierChord(event, definition)) return pass;
    else return cancelPass;

    if (event.repeat) return { action: "none", consume: true };
    return digit === undefined ? { action, consume: true } : { action, consume: true, digit };
  }

  /** Name the context shared by keyboard admission and Rust dispatch.
   * A hidden create form marks quick-launch preparation. Any other visible
   * modal owns the keyboard, including a dialog shown over the launcher.
   */
  function templateShortcutContext(doc) {
    const form = doc.querySelector(".create-session-form");
    const visible = [...doc.querySelectorAll('[role="dialog"][aria-modal="true"]')]
      .filter(node => node.getClientRects().length);
    if (visible.length) {
      if (visible.length !== 1 || visible[0] !== form) return "yield";
      // Saving a template owns this draft. Reserve the chord now, before an
      // awaited preference read could outlive a quick cancel of the panel.
      return form.querySelector(".save-template-panel") ? "preparing" : "launcher";
    }
    return form ? "preparing" : "page";
  }

  /** Admit template actions in the page, hidden preparation and open launcher. */
  function templateShortcutAdmitted(context) {
    return context === "page" || context === "preparing" || context === "launcher";
  }

  /** Decide when an editor keeps the sequence prefix's normal meaning.
   * Outside the launcher, text editing always wins. Its own fields yield the
   * prefix only when they hold a selection, so Cmd-X still cuts selected text.
   * xterm's implementation textarea is not an ordinary editor.
   */
  function prefixReservedForTextEditing(target, context, doc) {
    if (!(target instanceof Element)) return false;
    if (target.matches(".xterm-helper-textarea")) return false;
    const input = target.closest("input, textarea");
    if (!input && !target.isContentEditable) return false;
    if (context !== "launcher") return true;
    const form = doc.querySelector(".create-session-form");
    if (!form?.contains(target)) return true;
    if (input) return typeof input.selectionStart === "number" &&
      typeof input.selectionEnd === "number" && input.selectionStart !== input.selectionEnd;
    return root.getSelection?.()?.isCollapsed === false;
  }

  /** Install one capture listener before command listeners and terminal input.
   * Relay lookups remain live: a completed sequence can arrive before Rust's
   * next render. A disabled direct relay reserves its chord; absent relays do
   * not claim an unmounted application's keyboard.
   */
  function installTemplateShortcuts(definition) {
    if (root.__farhelmTemplateShortcuts) return;
    root.__farhelmTemplateShortcuts = true;
    const keys = { ...definition, mac_platform: /Mac/.test(navigator.platform || "") };
    const sequenceSelector = ".launch-sequence-trigger";
    const sequenceRelay = (action, digit) => document.querySelector(
      `${sequenceSelector}[data-sequence-action="${action}"]` +
      (digit === undefined ? "" : `[data-number-key="${digit}"]`),
    );
    const directRelay = (action, digit) => action === "quick-bolt"
      ? document.querySelector(".quick-launch-trigger")
      : document.querySelector(`.template-number-trigger[data-number-key="${digit}"]`);

    /** Retire both state owners before forwarding input or opening a dialog.
     * Focus, pointer and modal-opening hooks share this idempotent path. The
     * cancel relay must remain usable when new launch admission is disabled.
     */
    let armed = false;
    let armedHasEntry = false;
    const presses = [];
    // One snapshot per keyboard relay click: a direct shortcut, or the launch
    // relay at a sequence's Enter. Rust enqueues exactly one worker entry per
    // such click, refused or empty ones included, and its worker takes these
    // in order; that one-to-one pairing is the only thing tying a snapshot to
    // its press. Keep the physical press's modal epoch even while an earlier
    // read is pending.
    root.farhelmTakeTemplateShortcutPress = () => presses.shift();
    // This listener outlives the sidebar list that owes these snapshots: the
    // list, its worker queue and its relays unmount at the token prompt while
    // this window keeps its listener. Rust calls this as the list unmounts, so
    // a snapshot whose worker entry died with it cannot pair with a press made
    // after the list mounts again.
    root.farhelmClearTemplateShortcutPresses = () => { presses.length = 0; };
    const capturePress = (context, sequence = false) => presses.push({
      context, modal_opening: root.__farhelmModalOpening || 0,
      // This projection is published in the same render as the pill's names.
      // Copy at physical Enter; a newer Rust signal may not be on screen yet.
      sequence_bindings: sequence
        ? document.querySelector(".launch-sequence-bindings")?.getAttribute("data-bindings") ?? null
        : null,
    });
    function cancel() {
      if (!armed) return;
      armed = false;
      armedHasEntry = false;
      sequenceRelay("cancel")?.click();
    }
    root.cancelFarhelmLaunchSequence = cancel;
    root.farhelmTemplateShortcutContext = () => templateShortcutContext(document);

    root.addEventListener("keydown", event => {
      const context = root.farhelmTemplateShortcutContext();
      if (!templateShortcutAdmitted(context)) {
        cancel();
        return;
      }
      const result = classifyTemplateShortcutKey(
        event, armed, keys, prefixReservedForTextEditing(event.target, context, document),
      );
      if (result.action === "pass") return;
      if (result.action === "template-list") {
        cancel();
        // Cmd-J has the same modal boundary as Cmd-K. A live relay is the
        // sign that authentication and preference loading have completed.
        if (context !== "page") return;
        const trigger = document.querySelector(".quick-switcher-template-trigger");
        if (!trigger) return;
        event.preventDefault();
        event.stopImmediatePropagation();
        if (!event.repeat && !trigger.disabled) root.farhelmOpenQuickSwitcher?.(trigger);
        return;
      }
      if (result.action === "cancel" && !result.consume) {
        cancel();
        return;
      }

      // A hidden quick launch or open save panel reserves recognized keys
      // without queueing work that could apply after either state ends.
      if (context === "preparing") {
        cancel();
        if (result.consume) {
          event.preventDefault();
          event.stopImmediatePropagation();
        }
        return;
      }

      // A sequence takes precedence over direct shortcuts until Enter retires
      // it. Missing Begin yields; disabled Begin still consumes the prefix.
      const source = result.action === "none" ? result.source : result.action;
      const begin = !armed && source === "begin" ? sequenceRelay("begin") : null;
      const direct = !armed && ["quick-bolt", "quick-digit"].includes(source)
        ? directRelay(source, result.digit) : null;
      if (!armed && source === "begin" && !begin) return;
      if (!armed && ["quick-bolt", "quick-digit"].includes(source) && !direct) return;
      if (result.consume) {
        event.preventDefault();
        event.stopImmediatePropagation();
      }
      if (result.action === "none") return;
      if (result.action === "begin") {
        if (begin.disabled) return;
        armed = true;
        armedHasEntry = false;
        begin.click();
      } else if (direct) {
        if (!direct.disabled) {
          capturePress(context);
          direct.click();
        }
      } else if (result.action === "cancel") {
        cancel();
      } else if (result.action === "launch" && !armedHasEntry) {
        // Nothing queued means nothing to launch: retire the sequence like
        // Escape, so every launch relay click carries a snapshot.
        cancel();
      } else {
        const action = result.action;
        if (action === "launch") {
          armed = false;
          armedHasEntry = false;
        } else if (action === "digit" || action === "bolt") {
          armedHasEntry = true;
        }
        const relay = sequenceRelay(action, result.digit);
        // Capture only for a click that happens: a snapshot with no relay
        // click would be taken by the next press's worker entry instead. A
        // disabled button's .click() dispatches nothing, as on the direct path.
        const launches = action === "launch" && relay && !relay.disabled;
        if (launches) capturePress(context, true);
        relay?.click();
      }
    }, true);

    const cancelOnPointer = event => {
      // Programmatic relay clicks enter window's capture phase too; they are
      // part of the active sequence rather than a user leaving it.
      if (event.target instanceof Element && event.target.closest(sequenceSelector)) return;
      cancel();
    };
    root.addEventListener("pointerdown", cancelOnPointer, true);
    root.addEventListener("click", cancelOnPointer, true);
    root.addEventListener("blur", cancel);
    document.addEventListener("visibilitychange", () => {
      if (document.hidden) cancel();
    });
    root.addEventListener("focusin", () => {
      if (!templateShortcutAdmitted(root.farhelmTemplateShortcutContext())) cancel();
    }, true);
  }

  root.installTemplateShortcuts = installTemplateShortcuts;
  if (typeof module === "object") module.exports = {
    classifyTemplateShortcutKey, templateShortcutContext, prefixReservedForTextEditing,
  };
})(typeof window === "object" ? window : globalThis);
