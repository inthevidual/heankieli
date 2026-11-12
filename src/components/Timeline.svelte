<script lang="ts">
  import { store } from "../lib/store.svelte";
  import { setTiming } from "../lib/layout";
  import { snap } from "../lib/layout";
  import type { Cue } from "../lib/types";

  let canvas = $state<HTMLCanvasElement | null>(null);
  let wrap = $state<HTMLDivElement | null>(null);
  let width = $state(600);
  const height = 104;
  let span = $state(24); // seconds visible
  let viewStart = $state(0);
  let drag: { kind: "seek" } | { kind: "edge"; cue: Cue; edge: "start" | "end"; t: number } | null = null;
  let dragPreview = $state<{ index: number; start: number; end: number } | null>(null);
  let hoverEdge = $state(false);

  let duration = $derived(store.selected?.info?.duration ?? 0);
  const pps = $derived(width / span);
  const xOf = (t: number) => (t - viewStart) * pps;
  const tOf = (x: number) => viewStart + x / pps;

  // follow the playhead
  $effect(() => {
    const t = store.currentTime;
    if (drag) return;
    if (t < viewStart || t > viewStart + span * 0.92) viewStart = Math.max(0, t - span * 0.15);
  });
  $effect(() => {
    void store.selectedId;
    viewStart = 0;
  });

  $effect(() => {
    if (!wrap) return;
    const ro = new ResizeObserver(([e]) => (width = Math.max(100, e.contentRect.width)));
    ro.observe(wrap);
    return () => ro.disconnect();
  });

  function roundRect(g: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
    r = Math.min(r, w / 2, h / 2);
    g.beginPath();
    g.moveTo(x + r, y);
    g.arcTo(x + w, y, x + w, y + h, r);
    g.arcTo(x + w, y + h, x, y + h, r);
    g.arcTo(x, y + h, x, y, r);
    g.arcTo(x, y, x + w, y, r);
    g.closePath();
  }

  $effect(() => {
    if (!canvas) return;
    const g = canvas.getContext("2d")!;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, width, height);

    // ruler
    const rulerH = 18;
    g.fillStyle = "#f7f5ef";
    g.fillRect(0, 0, width, rulerH);
    g.fillStyle = "#fbfaf6";
    g.fillRect(0, rulerH, width, height - rulerH);
    const step = [0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300].find((s) => s * pps >= 60) ?? 600;
    g.font = "700 9.5px 'B612 Mono', monospace";
    g.textBaseline = "middle";
    for (let tt = Math.floor(viewStart / step) * step; tt < viewStart + span; tt += step) {
      const x = Math.round(xOf(tt)) + 0.5;
      g.strokeStyle = "#ddd8ca";
      g.beginPath();
      g.moveTo(x, rulerH - 6);
      g.lineTo(x, height);
      g.stroke();
      g.fillStyle = "#8a8f99";
      const m = Math.floor(tt / 60), s = tt % 60;
      g.fillText(`${m}:${(s < 10 ? "0" : "") + (step < 1 ? s.toFixed(1) : Math.round(s))}`, x + 4, rulerH / 2 + 1);
    }

    // waveform
    const peaks = store.selected?.peaks;
    const midY = rulerH + (height - rulerH) / 2;
    if (peaks) {
      g.fillStyle = "rgba(20,35,61,0.20)";
      const amp = (height - rulerH) / 2 - 4;
      for (let x = 0; x < width; x++) {
        const a = Math.floor(tOf(x) * 50), b = Math.max(a + 1, Math.floor(tOf(x + 1) * 50));
        let m = 0;
        for (let k = a; k < b && k < peaks.length; k++) if (k >= 0 && peaks[k] > m) m = peaks[k];
        const h = (m / 255) * amp;
        if (h > 0.4) g.fillRect(x, midY - h, 1, h * 2);
      }
    }

    // cues
    const cues = store.cues;
    const cur = store.currentTime;
    const top = rulerH + 14, h = height - rulerH - 28;
    g.font = "600 12px 'Barlow', sans-serif";
    for (const c of cues) {
      let { start, end } = c;
      if (dragPreview && dragPreview.index === c.index) ({ start, end } = dragPreview);
      if (end < viewStart || start > viewStart + span) continue;
      const x0 = xOf(start), x1 = xOf(end);
      const on = cur >= start && cur < end;
      roundRect(g, x0, top, Math.max(2, x1 - x0), h, 5);
      g.fillStyle = on ? "rgba(254,204,2,0.88)" : c.overflow ? "rgba(180,35,24,0.16)" : "rgba(0,53,128,0.13)";
      g.fill();
      g.strokeStyle = on ? "#d9ad00" : c.overflow ? "rgba(180,35,24,0.6)" : "rgba(0,53,128,0.45)";
      g.lineWidth = 1;
      g.stroke();
      if (x1 - x0 > 24) {
        g.save();
        g.beginPath();
        g.rect(x0 + 6, top, x1 - x0 - 12, h);
        g.clip();
        g.fillStyle = "#14233d";
        g.fillText(c.lines.join(" / "), x0 + 7, top + h / 2 + 1);
        g.restore();
      }
    }

    // playhead
    const px = Math.round(xOf(cur)) + 0.5;
    g.strokeStyle = "#003580";
    g.lineWidth = 2;
    g.beginPath();
    g.moveTo(px, 0);
    g.lineTo(px, height);
    g.stroke();
    g.fillStyle = "#003580";
    g.beginPath();
    g.moveTo(px - 5, 0);
    g.lineTo(px + 5, 0);
    g.lineTo(px, 7);
    g.fill();
  });

  function edgeAt(x: number): { cue: Cue; edge: "start" | "end" } | null {
    for (const c of store.cues) {
      if (Math.abs(xOf(c.start) - x) < 6) return { cue: c, edge: "start" };
      if (Math.abs(xOf(c.end) - x) < 6) return { cue: c, edge: "end" };
    }
    return null;
  }

  function down(e: PointerEvent) {
    if (!store.selected) return;
    const x = e.offsetX;
    const edge = e.offsetY > 18 ? edgeAt(x) : null;
    (e.target as Element).setPointerCapture(e.pointerId);
    if (edge) {
      drag = { kind: "edge", cue: edge.cue, edge: edge.edge, t: edge.edge === "start" ? edge.cue.start : edge.cue.end };
      dragPreview = { index: edge.cue.index, start: edge.cue.start, end: edge.cue.end };
    } else {
      drag = { kind: "seek" };
      store.seek(Math.min(duration, Math.max(0, tOf(x))));
    }
  }

  function move(e: PointerEvent) {
    const x = e.offsetX;
    if (!drag) {
      hoverEdge = e.offsetY > 18 && !!edgeAt(x);
      return;
    }
    if (drag.kind === "seek") {
      store.seek(Math.min(duration, Math.max(0, tOf(x))));
    } else {
      const cues = store.cues;
      const c = drag.cue;
      const prev = cues[c.index - 1], next = cues[c.index + 1];
      const gap = store.settings.rules.minGap;
      let t = snap(tOf(x), store.settings.rules.snapFrames ? store.fps : undefined);
      if (drag.edge === "start") t = Math.min(Math.max(t, prev ? prev.end + gap : 0), c.end - 0.2);
      else t = Math.max(Math.min(t, next ? next.start - gap : duration), c.start + 0.2);
      drag.t = t;
      dragPreview = drag.edge === "start" ? { index: c.index, start: t, end: c.end } : { index: c.index, start: c.start, end: t };
    }
  }

  function up() {
    if (drag?.kind === "edge") {
      const d = drag;
      store.edit((w) => setTiming(w, d.cue, d.edge === "start" ? d.t : undefined, d.edge === "end" ? d.t : undefined));
    }
    drag = null;
    dragPreview = null;
  }

  function wheel(e: WheelEvent) {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) {
      const at = tOf(e.offsetX);
      span = Math.min(600, Math.max(4, span * (e.deltaY > 0 ? 1.2 : 1 / 1.2)));
      viewStart = Math.max(0, at - e.offsetX / (width / span));
    } else {
      const d = (Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY) / pps;
      viewStart = Math.max(0, Math.min(Math.max(0, duration - span * 0.5), viewStart + d));
    }
  }
</script>

<div class="timeline" bind:this={wrap}>
  <canvas
    bind:this={canvas}
    style:width="{width}px"
    style:height="{height}px"
    style:cursor={hoverEdge ? "ew-resize" : "pointer"}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    onwheel={wheel}
  ></canvas>
</div>

<style>
  .timeline { background: var(--card); border: 1px solid var(--line); border-top: 0; border-radius: 0 0 var(--radius) var(--radius); overflow: hidden; touch-action: none; }
  canvas { display: block; }
</style>
