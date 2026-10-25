import { Ghostty, type Terminal } from 'ghostty-web';
import wasmUrl from 'ghostty-web/ghostty-vt.wasm?url';

export function loadGhostty(): Promise<Ghostty> {
  // Each mounted view owns its WASM memory, including replay and screen state.
  return Ghostty.load(wasmUrl);
}

/** Bridge committed browser text that ghostty-web 0.4.0's keydown handler skips. */
export function attachTerminalInput(terminal: Terminal): () => void {
  const element = terminal.element!;
  const textarea = terminal.textarea!;
  // Keep native editing and IME composition inside the input, away from canvas.
  element.removeAttribute('contenteditable');
  element.tabIndex = -1;
  const focus = (event: FocusEvent) => {
    if (event.target === element) textarea.focus({ preventScroll: true });
  };
  const beforeInput = (rawEvent: Event) => {
    const event = rawEvent as InputEvent;
    if (event.inputType !== 'insertText' || event.isComposing || !event.data) return;
    // Ordinary keydown input is already preventDefault'ed by the library, so
    // it cannot also take this path. Composition is sent by compositionend.
    event.preventDefault();
    event.stopImmediatePropagation();
    terminal.input(event.data, true);
  };
  element.addEventListener('focus', focus, true);
  element.addEventListener('beforeinput', beforeInput, true);
  textarea.focus({ preventScroll: true });
  return () => {
    element.removeEventListener('focus', focus, true);
    element.removeEventListener('beforeinput', beforeInput, true);
  };
}
