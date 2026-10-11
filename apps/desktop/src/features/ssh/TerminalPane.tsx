import { memo, useEffect, useRef } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import { useDesktop } from '../../desktop/api';
import { useI18n } from '../../i18n/react';
import { TerminalStream } from './terminal-stream';

export const TerminalPane = memo(function TerminalPane({
  workspace,
  session,
  onSelection,
}: {
  workspace: string;
  session: string;
  onSelection: (text: string) => void;
}) {
  const desktop = useDesktop();
  const { t } = useI18n();
  const element = useRef<HTMLDivElement>(null);
  const selection = useRef(onSelection);
  selection.current = onSelection;
  useEffect(() => {
    const terminal = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      lineHeight: 1.25,
      fontFamily: '"SFMono-Regular", Consolas, "Liberation Mono", monospace',
      scrollback: 5000,
      allowProposedApi: false,
      theme: {
        background: '#20251f',
        foreground: '#e4e5da',
        cursor: '#dc9b71',
        selectionBackground: '#56634b',
        black: '#20251f',
        red: '#edaaa0',
        green: '#a1bc94',
        yellow: '#d9be88',
        blue: '#93b7c8',
        magenta: '#c2a5ca',
        cyan: '#98c8b9',
        white: '#e4e5da',
      },
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(element.current!);
    let disposed = false;
    let recovering = false;
    let again = false;
    let waiting: { offset: string; bytes: number[] }[] = [];
    const stream = new TerminalStream(
      (bytes) => terminal.write(bytes),
      () => {
        terminal.reset();
        terminal.writeln(`\r\n[${t('ssh:outputGap')}]\r\n`);
      },
    );
    const recover = async () => {
      if (recovering) {
        again = true;
        return;
      }
      recovering = true;
      try {
        const reply = await desktop.remote({
          type: 'replay',
          data: { workspace, session, after: String(stream.offset) },
        });
        if (disposed) return;
        if (reply.type === 'replay') stream.replay(reply.data);
        const chunks = waiting;
        waiting = [];
        for (const chunk of chunks) if (!stream.accept(chunk.offset, chunk.bytes)) again = true;
      } catch {
        /* Connection/status UI explains failures; next reset resumes. */
      } finally {
        recovering = false;
        if (again && !disposed) {
          again = false;
          void recover();
        }
      }
    };
    // Subscribe first so output emitted while taking the replay cannot be lost.
    const unsubscribe = desktop.onTerminal((frame) => {
      if (!frame) {
        void recover();
        return;
      }
      if (frame.sessionId !== session) return;
      if (recovering) {
        if (waiting.length < 128) waiting.push(frame);
        else {
          waiting = [];
          again = true;
        }
      } else if (!stream.accept(frame.offset, frame.bytes)) void recover();
    });
    let inputQueue = Promise.resolve();
    const sendInput = (bytes: Uint8Array) => {
      // Preserve input order across IPC and split large pastes into bounded requests.
      inputQueue = inputQueue
        .then(async () => {
          for (let start = 0; start < bytes.length && !disposed; start += 16384) {
            await desktop.remote({
              type: 'input',
              data: { workspace, session, bytes: [...bytes.subarray(start, start + 16384)] },
            });
          }
        })
        .catch(() => {});
    };
    const input = terminal.onData((data) => sendInput(new TextEncoder().encode(data)));
    const binary = terminal.onBinary((data) =>
      sendInput(Uint8Array.from(data, (c) => c.charCodeAt(0) & 255)),
    );
    const resize = terminal.onResize(({ cols, rows }) => {
      void desktop
        .remote({ type: 'resize', data: { workspace, session, columns: cols, rows } })
        .catch(() => {});
    });
    const selected = terminal.onSelectionChange(() => selection.current(terminal.getSelection()));
    let scheduled = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(scheduled);
      scheduled = requestAnimationFrame(() => {
        if (!disposed && element.current?.clientWidth) fit.fit();
      });
    });
    observer.observe(element.current!);
    fit.fit();
    void recover();
    terminal.focus();
    return () => {
      disposed = true;
      cancelAnimationFrame(scheduled);
      observer.disconnect();
      unsubscribe();
      input.dispose();
      binary.dispose();
      resize.dispose();
      selected.dispose();
      terminal.dispose();
    };
  }, [desktop, workspace, session, t]);
  return (
    <div className="ssh-terminal" aria-label={t('ssh:terminal')}>
      <div ref={element} className="ssh-terminal-host" />
    </div>
  );
});
