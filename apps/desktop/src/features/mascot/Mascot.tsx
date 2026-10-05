import { useI18n } from '../../i18n/react';
import { useEffect, useRef, useState } from 'react';
import { useDesktop, useSnapshot } from '../../desktop/api';
import { Playback, type PlaybackState } from './playback';
import type { BirdScene } from './scene';
import type { TargetLayout } from './targets';
import { posterTargets } from './poster';
import type { Gesture } from './motions';
import { previewMotions, type PreviewMotion } from './motionPreview';

export function Mascot({
  onLayout,
  onPlayback,
  motionPreview = false,
  interaction,
}: {
  onLayout: (targets: TargetLayout) => void;
  onPlayback: (state: PlaybackState) => void;
  motionPreview?: boolean;
  interaction?: { id: number; motion: Gesture };
}) {
  const { t } = useI18n();
  const client = useDesktop();
  const { appearance } = useSnapshot();
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<BirdScene | null>(null);
  const controller = useRef<Playback | null>(null);
  const [performing, setPerforming] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);
  const [motion, setMotion] = useState<PreviewMotion>('idle');
  const [progress, setProgress] = useState(0);
  const [reduced, setReduced] = useState(false);
  useEffect(() => {
    setLoaded(false);
    setFailed(false);
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
          motionPreview
            ? {
                changed: (name) => {
                  setMotion(name);
                  setProgress(0);
                },
              }
            : undefined,
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
        const hidden = () => document.hidden || client.getSnapshot().host.visible === false;
        const visibility = () => {
          const next = hidden();
          if (next === sleeping) {
            if (next) stage.sleep();
            return;
          }
          sleeping = next;
          if (next) {
            if (motionPreview) stage.play('idle', null);
            else control.reset();
            stage.sleep();
          } else stage.wake();
        };
        const update = () => {
          const state = client.getSnapshot();
          if (!motionPreview)
            control.setMode(
              state.settings.paused
                ? 'paused'
                : !state.peers.some((peer) => peer.online)
                  ? 'offline'
                  : 'idle',
              !hidden(),
            );
          const reduced = state.appearance.reducedMotion || preference.matches;
          setReduced(reduced);
          stage.setReduced(reduced);
          visibility();
        };
        subscriptions.push(
          client.onEvent((event) => {
            if (!motionPreview && !hidden()) control.event(event);
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
        if (!abort.signal.aborted) setFailed(true);
      });
    const control = new Playback(
      (clip, serial) => {
        setMotion(clip);
        scene.current?.play(clip, serial);
      },
      (state) => {
        setPerforming(state.active !== null);
        onPlayback(state);
      },
    );
    controller.current = control;
    return () => {
      abort.abort();
      fallback.disconnect();
      subscriptions.forEach((unsubscribe) => unsubscribe());
      onPlayback({ active: null, activityId: null });
      scene.current?.dispose();
      scene.current = null;
      controller.current = null;
    };
  }, [client, onLayout, onPlayback, motionPreview]);
  useEffect(() => {
    if (
      interaction &&
      scene.current &&
      !motionPreview &&
      !document.hidden &&
      client.getSnapshot().host.visible !== false
    )
      controller.current?.gesture(interaction.motion);
  }, [interaction, motionPreview, client]);
  useEffect(() => {
    const media = matchMedia('(prefers-color-scheme: dark)');
    const repaint = () => requestAnimationFrame(() => scene.current?.theme());
    repaint();
    media.addEventListener('change', repaint);
    return () => media.removeEventListener('change', repaint);
  }, [appearance.theme]);
  return (
    <div
      className="mascot"
      data-motion={motion}
      data-performing={motionPreview ? motion !== 'idle' : performing}
    >
      <span className="sr-only">{t('home:stageDescription')}</span>
      {!loaded && !motionPreview && (
        <picture>
          {appearance.theme !== 'light' && (
            <source
              srcSet="/mascot/poster-dark.jpg"
              media={appearance.theme === 'system' ? '(prefers-color-scheme: dark)' : undefined}
            />
          )}
          <img className="mascot__poster" src="/mascot/poster.jpg" alt="" />
        </picture>
      )}
      <div className="mascot__canvas" ref={host} />
      {motionPreview && (
        <div className="mascot__motion-controls" role="group" aria-label={t('home:motionPreview')}>
          <span>{t('home:motionPreview')}</span>
          {previewMotions.map((name) => (
            <button
              key={name}
              type="button"
              disabled={!loaded}
              aria-pressed={loaded && motion === name}
              onClick={() => {
                setProgress(0);
                scene.current?.play(name, null);
              }}
            >
              {t(`home:motion_${name}`)}
            </button>
          ))}
          {loaded && (
            <label className="mascot__motion-scrubber">
              {t('home:motionInspect')}
              <input
                type="range"
                min="0"
                max="100"
                step="0.5"
                value={progress}
                onChange={(event) => {
                  const value = Number(event.target.value);
                  setProgress(value);
                  scene.current?.seekPreview(value / 100);
                }}
              />
              <output>{progress}%</output>
            </label>
          )}
          {!loaded && (
            <small role={failed ? 'alert' : 'status'}>
              {t(failed ? 'home:motionLoadFailed' : 'home:motionLoading')}
            </small>
          )}
          {loaded && reduced && <small>{t('home:motionStill')}</small>}
        </div>
      )}
    </div>
  );
}
