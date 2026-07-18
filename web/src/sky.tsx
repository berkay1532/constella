import { useEffect, useRef } from 'react';

const reduceMotion = () =>
  typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion:reduce)').matches;

/** Ambient drifting starfield, painted behind the hero. */
export function Starfield({ className }: { className?: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const x = c.getContext('2d');
    if (!x) return;
    const reduce = reduceMotion();
    let w = 0, h = 0, raf = 0;
    let stars: { x: number; y: number; z: number; r: number; tw: number; sp: number }[] = [];
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const size = () => {
      const rect = c.getBoundingClientRect();
      w = c.width = rect.width * dpr;
      h = c.height = rect.height * dpr;
      const n = Math.min(150, Math.floor((w * h) / 16000));
      stars = Array.from({ length: n }, () => ({
        x: Math.random() * w, y: Math.random() * h, z: Math.random(),
        r: Math.random() * 1.4 + 0.3, tw: Math.random() * Math.PI * 2, sp: Math.random() * 0.02 + 0.004,
      }));
    };
    size();
    window.addEventListener('resize', size);
    const draw = () => {
      x.clearRect(0, 0, w, h);
      for (const s of stars) {
        s.tw += s.sp;
        const a = Math.max(0, 0.35 + Math.sin(s.tw) * 0.3 + s.z * 0.3);
        x.beginPath();
        x.arc(s.x, s.y, s.r * dpr, 0, 7);
        x.fillStyle = `rgba(${170 + s.z * 40},${160 + s.z * 40},255,${a})`;
        x.fill();
        if (!reduce) { s.y += s.sp * 4; if (s.y > h) s.y = 0; }
      }
      raf = requestAnimationFrame(draw);
    };
    if (reduce) {
      for (const s of stars) {
        x.beginPath();
        x.arc(s.x, s.y, s.r * dpr, 0, 7);
        x.fillStyle = `rgba(180,170,255,${0.4 + s.z * 0.4})`;
        x.fill();
      }
    } else {
      draw();
    }
    return () => { cancelAnimationFrame(raf); window.removeEventListener('resize', size); };
  }, []);
  return <canvas ref={ref} className={className} aria-hidden="true" />;
}

/** The token as a constellation: one glowing star per active module, wired in a ring. */
export function Constellation({ mods, labels }: { mods: string[]; labels: Record<string, string> }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const modsRef = useRef(mods);
  modsRef.current = mods;
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const cx = c.getContext('2d');
    if (!cx) return;
    const reduce = reduceMotion();
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    let w = 0, h = 0, raf = 0, t = 0;
    const size = () => { const r = c.getBoundingClientRect(); w = c.width = r.width * dpr; h = c.height = r.height * dpr; };
    size();
    window.addEventListener('resize', size);
    const draw = () => {
      t += 0.01;
      cx.clearRect(0, 0, w, h);
      const active = modsRef.current;
      const cxp = w / 2, cyp = h / 2, R = Math.min(w, h) * 0.32;
      const pts = active.map((m, i) => {
        const ang = (i / active.length) * Math.PI * 2 - Math.PI / 2 + (reduce ? 0 : Math.sin(t + i) * 0.03);
        return { x: cxp + Math.cos(ang) * R, y: cyp + Math.sin(ang) * R, m };
      });
      cx.strokeStyle = 'rgba(139,125,255,.4)';
      cx.lineWidth = 1 * dpr;
      for (let i = 0; i < pts.length; i++) {
        const a = pts[i], b = pts[(i + 1) % pts.length];
        cx.beginPath(); cx.moveTo(a.x, a.y); cx.lineTo(b.x, b.y); cx.stroke();
      }
      if (pts.length > 2) {
        cx.strokeStyle = 'rgba(79,224,212,.14)';
        for (let i = 0; i < pts.length; i++) {
          const a = pts[i], b = pts[(i + 2) % pts.length];
          cx.beginPath(); cx.moveTo(a.x, a.y); cx.lineTo(b.x, b.y); cx.stroke();
        }
      }
      for (const p of pts) {
        const pulse = reduce ? 4 : 4 + Math.sin(t * 2) * 1.2;
        const g = cx.createRadialGradient(p.x, p.y, 0, p.x, p.y, 18 * dpr);
        g.addColorStop(0, 'rgba(171,157,255,.9)');
        g.addColorStop(1, 'rgba(139,125,255,0)');
        cx.fillStyle = g;
        cx.beginPath(); cx.arc(p.x, p.y, 18 * dpr, 0, 7); cx.fill();
        cx.fillStyle = '#cdc6ff';
        cx.beginPath(); cx.arc(p.x, p.y, pulse * dpr * 0.5, 0, 7); cx.fill();
        cx.fillStyle = 'rgba(236,238,251,.72)';
        cx.font = `${10 * dpr}px ui-monospace,monospace`;
        cx.textAlign = 'center';
        cx.fillText(labels[p.m] || p.m, p.x, p.y + 26 * dpr);
      }
      if (active.length === 0) {
        cx.fillStyle = 'rgba(143,151,189,.5)';
        cx.font = `${12 * dpr}px ui-monospace,monospace`;
        cx.textAlign = 'center';
        cx.fillText('select modules to form your token', cxp, cyp);
      }
      raf = requestAnimationFrame(draw);
    };
    draw();
    return () => { cancelAnimationFrame(raf); window.removeEventListener('resize', size); };
  }, [labels]);
  return <canvas ref={ref} className="constel-canvas" aria-hidden="true" />;
}
