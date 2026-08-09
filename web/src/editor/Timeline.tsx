/**
 * Timeline : une piste par élément animé, un bloc positionné selon délai et durée.
 * La tête de lecture est écrite directement dans le DOM à chaque frame — la faire passer par
 * un état React re-rendrait tout l'éditeur soixante fois par seconde pour déplacer un trait.
 */
import { useEffect, useRef, useState } from 'react';
import type { Dispatch } from 'react';
import type { Doc, Profile } from '../lib/types';
import { animatedElements, elementLabel } from './state';
import type { Action } from './state';
import s from './editor.module.css';

const DURATIONS = [4, 6, 8, 10];

interface TimelineProps {
  doc: Doc;
  dispatch: Dispatch<Action>;
  profile: Profile;
}

export function Timeline({ doc, dispatch, profile }: TimelineProps) {
  const duration = doc.timelineDuration;
  const tracks = animatedElements(doc);
  const [running, setRunning] = useState(false);
  const timeRef = useRef(0);
  const playheadRef = useRef<HTMLDivElement>(null);
  const readoutRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!running) return undefined;
    const origin = performance.now() - timeRef.current * 1000;
    let frame = requestAnimationFrame(function tick() {
      timeRef.current = ((performance.now() - origin) / 1000) % duration;
      if (playheadRef.current) playheadRef.current.style.left = `${(timeRef.current / duration) * 100}%`;
      if (readoutRef.current) readoutRef.current.textContent = `${timeRef.current.toFixed(2)} s`;
      frame = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(frame);
  }, [running, duration]);

  function stop() {
    setRunning(false);
    timeRef.current = 0;
    if (playheadRef.current) playheadRef.current.style.left = '0%';
    if (readoutRef.current) readoutRef.current.textContent = '0.00 s';
  }

  return (
    <section className={s.timeline} aria-label="Timeline des animations">
      <div className={s.timelineHead}>
        <div className={s.timelineSide}>
          <strong>Timeline</strong>
          <button
            type="button"
            className={s.tinyBtn}
            aria-pressed={running}
            onClick={() => setRunning((value) => !value)}
          >
            {running ? '❚❚ Pause' : '▶ Lecture'}
          </button>
          <button type="button" className={s.tinyBtn} onClick={stop}>
            ■ Arrêt
          </button>
          <span className={s.readout} ref={readoutRef}>
            0.00 s
          </span>
        </div>
        <label className={s.inlineLabel}>
          Durée totale
          <select
            name="timeline-duration"
            className={s.inlineSelect}
            value={duration}
            onChange={(event) => dispatch({ type: 'setDuration', seconds: Number(event.target.value) })}
          >
            {DURATIONS.map((value) => (
              <option key={value} value={value}>
                {value} s
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className={s.timelineBody}>
        <div className={s.trackLabels}>
          {tracks.length === 0 ? (
            <div className={s.trackLabel}>Aucune animation</div>
          ) : (
            tracks.map((element) => (
              <div key={element.id} className={s.trackLabel}>
                <span className={s.trackDot} aria-hidden="true" />
                {elementLabel(element, profile)}
              </div>
            ))
          )}
        </div>

        <div className={s.timelineScroll}>
          <div className={s.timelineScale} aria-hidden="true">
            {Array.from({ length: duration + 1 }, (_, second) => (
              <span key={second} className={s.tick} style={{ left: `${(second / duration) * 100}%` }}>
                {second}s
              </span>
            ))}
          </div>
          <div className={s.tracks}>
            {tracks.map((element) => {
              const left = Math.min(100, (element.anim.delay / duration) * 100);
              const width = Math.max(3, Math.min(100 - left, (element.anim.duration / duration) * 100));
              return (
                <div key={element.id} className={s.trackRow}>
                  <div className={s.animBlock} style={{ left: `${left}%`, width: `${width}%` }}>
                    {element.anim.preset} · {element.anim.duration}s
                  </div>
                </div>
              );
            })}
            <div className={s.playhead} ref={playheadRef} style={{ left: 0 }} aria-hidden="true" />
          </div>
        </div>
      </div>
    </section>
  );
}
