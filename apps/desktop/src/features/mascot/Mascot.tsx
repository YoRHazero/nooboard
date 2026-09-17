import { useI18n } from '../../i18n/react';
import { useEffect, useRef, useState } from 'react';
import { useClient, useSnapshot } from '../../api/NooboardProvider';
import { Playback, type PlaybackState } from './playback';
import type { BirdScene } from './scene';
import type { TargetLayout } from './targets';
import { posterTargets } from './poster';

export function Mascot({
  onLayout,
  onPlayback,
}: {
  onLayout: (targets: TargetLayout) => void;
  onPlayback: (state: PlaybackState) => void;
}) {
  const { t } = useI18n();
  const client = useClient();
  const { settings } = useSnapshot();
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<BirdScene | null>(null);
  const [loaded, setLoaded] = useState(false);
  useEffect(() => {
    const abort = new AbortController();
    const subscriptions: (() => void)[] = [];
    const fallback = new ResizeObserver(() => {
      const element = host.current;
      if (!scene.current && element?.clientWidth && element.clientHeight)
        onLayout(posterTargets(element.clientWidth, element.clientHeight));
    });
    fallback.observe(host.current!);
    const preference = matchMedia('(prefers-reduced-motion: reduce)');
    void import('./scene')
      .then((module) =>
        module.createScene(
          host.current!,
          (serial) => control.finish(serial),
          abort.signal,
          onLayout,
        ),
      )
      .then((stage) => {
        if (abort.signal.aborted) {
          stage.dispose();
          return;
        }
        scene.current = stage;
        fallback.disconnect();
        setLoaded(true);
        let sleeping = false;
        const hidden = () => document.hidden || client.getSnapshot().desktop?.visible === false;
        const visibility = () => {
          const next = hidden();
          if (next === sleeping) return;
          sleeping = next;
          if (next) {
            control.reset();
            stage.sleep();
          } else stage.wake();
        };
        const update = () => {
          const state = client.getSnapshot();
          control.setMode(
            state.settings.paused
              ? 'paused'
              : !state.peers.some((peer) => peer.online)
                ? 'offline'
                : 'idle',
          );
          stage.setReduced(state.settings.reducedMotion || preference.matches);
          visibility();
        };
        subscriptions.push(
          client.onEvent((event) => {
            if (!hidden()) control.event(event);
          }),
          client.subscribe(update),
        );
        preference.addEventListener('change', update);
        document.addEventListener('visibilitychange', visibility);
        subscriptions.push(
          () => preference.removeEventListener('change', update),
          () => document.removeEventListener('visibilitychange', visibility),
        );
        update();
      })
      .catch(() => {
        /* The static scene keeps the workspace usable without WebGL. */
      });
    const control = new Playback((clip, serial) => scene.current?.play(clip, serial), onPlayback);
    return () => {
      abort.abort();
      fallback.disconnect();
      subscriptions.forEach((unsubscribe) => unsubscribe());
      onPlayback({ active: null, activityId: null });
      scene.current?.dispose();
      scene.current = null;
    };
  }, [client, onLayout, onPlayback]);
  useEffect(() => {
    const media = matchMedia('(prefers-color-scheme: dark)');
    const repaint = () => requestAnimationFrame(() => scene.current?.theme());
    repaint();
    media.addEventListener('change', repaint);
    return () => media.removeEventListener('change', repaint);
  }, [settings.theme]);
  return (
    <div className="mascot">
      <span className="sr-only">{t('home:stageDescription')}</span>
      {!loaded && <img className="mascot__poster" src="/mascot/poster.png" alt="" />}
      <div className="mascot__canvas" ref={host} />
    </div>
  );
}
