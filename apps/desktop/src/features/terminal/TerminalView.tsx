import { getTerminalStreams } from './io/output-stream';
import { useActionFeedback } from '../../shared/action-feedback';
import { useEffect, useRef, useState } from 'react';
import type { TerminalTarget } from '../../shared/types';
import { useI18n } from '../../shared/i18n/react';
import { attachTerminalInput, loadGhostty } from './ghostty';

export function TerminalView({ target }: { target: TerminalTarget }) {
  const terminal = getTerminalStreams();
  const { execute } = useActionFeedback();
  const { t } = useI18n();
  const root = useRef<HTMLDivElement>(null);
  const [connecting, setConnecting] = useState(true);
  useEffect(() => {
    let disposed = false;
    let disposeView = () => {};
    setConnecting(true);
    void execute(async () => {
      const [, { ghostty, Terminal, FitAddon }] = await Promise.all([
        terminal.attach(target),
        loadGhostty(),
      ]);
      if (disposed) return;
      const buffer = terminal.buffer(target);
      if (!buffer || !root.current) return;
      const emulator = new Terminal({
        ghostty,
        cursorBlink: true,
        fontSize: 13,
        fontFamily: 'SFMono-Regular, Menlo, Consolas, monospace',
        theme: {
          background: '#1e231f',
          foreground: '#e3e7dc',
          cursor: '#dc9b71',
          selectionBackground: '#53664c',
        },
      });
      const fit = new FitAddon();
      emulator.loadAddon(fit);
      const element = document.createElement('div');
      element.className = 'terminal-emulator';
      root.current.appendChild(element);
      disposeView = () => {
        emulator.dispose();
        element.remove();
      };
      emulator.open(element);
      const detachInput = attachTerminalInput(emulator);
      const data = emulator.onData((text) => {
        void execute(() =>
          terminal.request({
            type: 'input',
            data: { target, bytes: [...new TextEncoder().encode(text)] },
          }),
        );
      });
      // Ghostty parses writes synchronously. Acknowledge parsed output without
      // waiting for animation frames, which pause when the app is backgrounded.
      const write = (bytes: Uint8Array | string) => {
        if (!disposed) emulator.write(bytes);
      };
      const observed = buffer.observe((frame) => {
        if (frame.type === 'data') write(new Uint8Array(frame.data.bytes));
        if (frame.type === 'gap') write(`\r\n${t('terminal:outputGap')}\r\n`);
      });
      let previous = '';
      const resize = () => {
        if (disposed || !root.current?.clientWidth || !root.current.clientHeight) return;
        const dimensions = fit.proposeDimensions();
        if (dimensions) emulator.resize(dimensions.cols, dimensions.rows);
        const size = `${emulator.cols}:${emulator.rows}`;
        if (previous === size) return;
        previous = size;
        void execute(() =>
          terminal.request({
            type: 'resize',
            data: { target, cols: emulator.cols, rows: emulator.rows },
          }),
        );
      };
      const observer = new ResizeObserver(resize);
      observer.observe(root.current);
      disposeView = () => {
        observed.unsubscribe();
        observer.disconnect();
        data.dispose();
        detachInput();
        emulator.dispose();
        element.remove();
      };
      resize();
      if (observed.offset > 0n) write(`${t('terminal:outputGap')}\r\n`);
      write(observed.bytes);
      setConnecting(false);
      emulator.focus();
    });
    return () => {
      disposed = true;
      disposeView();
    };
  }, [terminal, target.kind, target.id, execute, t]);
  return (
    <div className="terminal-output-wrap">
      {connecting && <span className="terminal-output-loading">{t('terminal:connecting')}</span>}
      <div ref={root} className="terminal-output" />
    </div>
  );
}
