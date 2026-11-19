import { useEffect, useRef } from "react";

const GAP = 26;
const REACH = 110;

function mix(a: number[], b: number[], t: number): string {
  return a.map((v, i) => Math.round(v + (b[i] - v) * t)).join(",");
}

/**
 * Background dot grid, fixed behind the whole page. Dots near the cursor grow and shift colour.
 * Touch screens and reduced motion get a still grid only.
 */
export function PointerLayer() {
  const gridRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = gridRef.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    const fine = window.matchMedia?.("(pointer: fine)").matches ?? false;
    const live = fine && !reduce;

    let w = 0;
    let h = 0;
    let dpr = 1;
    const target = { x: -999, y: -999 };
    const smooth = { x: -999, y: -999 };
    let raf = 0;
    let running = false;
    let last = 0;

    const draw = () => {
      const dark = document.documentElement.classList.contains("dark");
      const base = dark ? "148,156,200" : "80,86,121";
      const a = dark ? [79, 216, 200] : [8, 115, 108];
      const b = dark ? [180, 157, 245] : [94, 63, 192];
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);
      for (let gx = GAP / 2; gx < w; gx += GAP) {
        for (let gy = GAP / 2; gy < h; gy += GAP) {
          const d = Math.hypot(gx - smooth.x, gy - smooth.y);
          const k = d < REACH ? (1 - d / REACH) ** 2 : 0;
          const rgb = k > 0 ? mix(a, b, Math.min(1, d / REACH)) : base;
          ctx.fillStyle = `rgba(${rgb},${0.14 + k * 0.38})`;
          ctx.beginPath();
          ctx.arc(gx, gy, 1 + k * 1.4, 0, Math.PI * 2);
          ctx.fill();
        }
      }
    };

    const tick = (now: number) => {
      const dt = Math.min((now - last) / 1000, 0.1);
      last = now;
      const e1 = 1 - Math.exp(-dt * 9);
      smooth.x += (target.x - smooth.x) * e1;
      smooth.y += (target.y - smooth.y) * e1;
      draw();
      const settled = Math.abs(target.x - smooth.x) < 0.3 && Math.abs(target.y - smooth.y) < 0.3;
      if (settled) {
        running = false;
        return;
      }
      raf = requestAnimationFrame(tick);
    };
    const wake = () => {
      if (running) return;
      running = true;
      last = performance.now();
      raf = requestAnimationFrame(tick);
    };

    const resize = () => {
      dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = window.innerWidth;
      h = window.innerHeight;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      draw();
    };
    const onMove = (event: PointerEvent) => {
      if (event.pointerType === "touch") return;
      if (target.x < 0) {
        smooth.x = event.clientX;
        smooth.y = event.clientY;
      }
      target.x = event.clientX;
      target.y = event.clientY;
      wake();
    };

    resize();
    window.addEventListener("resize", resize);
    if (live) window.addEventListener("pointermove", onMove, { passive: true });
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
      window.removeEventListener("pointermove", onMove);
    };
  }, []);

  return <canvas ref={gridRef} aria-hidden="true" className="pointer-events-none fixed inset-0 -z-10 h-full w-full" />;
}
