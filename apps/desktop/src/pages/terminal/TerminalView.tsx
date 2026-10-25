import { useEffect, useRef, useState } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import type { TerminalTarget } from '../../client/api';
import { useCommand, useDesktop } from '../../client/react';
import { useI18n } from '../../shared/i18n/react';
import '@xterm/xterm/css/xterm.css';

export function TerminalView({ target }: { target: TerminalTarget }) {
  const { terminal } = useDesktop();
  const { execute } = useCommand();
  const { t } = useI18n();
  const root = useRef<HTMLDivElement>(null);
  const [connecting, setConnecting] = useState(true);
  useEffect(() => {
    let disposed = false;
    let disposeView = () => {};
    setConnecting(true);
    void execute(async () => {
      await terminal.attach(target);
      if (disposed) return;
      const buffer = terminal.buffer(target);
      if (!buffer || !root.current) return;
      const xterm = new Terminal({
        cursorBlink: true,
        fontSize: 13,
        fontFamily: 'SFMono-Regular, Menlo, Consolas, monospace',
        theme: {
          background: '#1e231f',
          foreground: '#e3e7dc',
          cursor: '#dc9b71',
          selectionBackground: '#53664c',
        },
        allowProposedApi: false,
        screenReaderMode: true,
      });
      const fit = new FitAddon();
      xterm.loadAddon(fit);
      xterm.open(root.current);
      const writes = new Set<() => void>();
      const write = (bytes: Uint8Array | string) =>
        new Promise<void>((resolve) => {
          if (disposed) {
            resolve();
            return;
          }
          const done = () => {
            writes.delete(done);
            resolve();
          };
          writes.add(done);
          xterm.write(bytes, done);
        });
      const observed = buffer.observe((frame) => {
        if (frame.type === 'data') return write(new Uint8Array(frame.data.bytes));
        if (frame.type === 'gap') return write(`\r\n${t('terminal:outputGap')}\r\n`);
      });
      if (observed.offset > 0n) void write(`${t('terminal:outputGap')}\r\n`);
      void write(observed.bytes);
      const data = xterm.onData((text) => {
        void execute(() =>
          terminal.request({
            type: 'input',
            data: { target, bytes: [...new TextEncoder().encode(text)] },
          }),
        );
      });
      const binary = xterm.onBinary((text) => {
        void execute(() =>
          terminal.request({
            type: 'input',
            data: { target, bytes: [...text].map((char) => char.charCodeAt(0) & 255) },
          }),
        );
      });
      let previous = '';
      const resize = () => {
        if (disposed || !root.current?.clientWidth || !root.current.clientHeight) return;
        fit.fit();
        const size = `${xterm.cols}:${xterm.rows}`;
        if (previous === size) return;
        previous = size;
        void execute(() =>
          terminal.request({
            type: 'resize',
            data: { target, cols: xterm.cols, rows: xterm.rows },
          }),
        );
      };
      const observer = new ResizeObserver(resize);
      observer.observe(root.current);
      resize();
      setConnecting(false);
      xterm.focus();
      disposeView = () => {
        observed.unsubscribe();
        observer.disconnect();
        data.dispose();
        binary.dispose();
        writes.forEach((done) => done());
        xterm.dispose();
      };
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
