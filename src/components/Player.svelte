<script lang="ts">
  import { api, errorText, on } from "../lib/api";
  import { store } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import { shortClock } from "../lib/formats";
  import type { Cue } from "../lib/types";
  import Icon from "./Icon.svelte";

  let video = $state<HTMLVideoElement | null>(null);
  let stage = $state<HTMLDivElement | null>(null);
  let stageW = $state(0);
  let stageH = $state(0);
  let playing = $state(false);
  let rate = $state(1);
  let proxying = $state<number | null>(null);
  let previewError = $state<string | null>(null);
  let triedProxy = new Set<string>();

  let item = $derived(store.selected);
  let info = $derived(item?.info ?? null);
  let aspect = $derived(info?.hasVideo && info.width && info.height ? info.width / info.height : 16 / 9);
  let frame = $derived.by(() => {
    if (!stageW || !stageH) return { w: 0, h: 0 };
    let w = stageW, h = stageW / aspect;
    if (h > stageH) { h = stageH; w = stageH * aspect; }
    return { w: Math.floor(w), h: Math.floor(h) };
  });
  let src = $derived(item ? (item.previewSrc ?? item.mediaUrl) : null);

  // the active cue
  function cueAt(cues: Cue[], time: number): Cue | null {
    let lo = 0, hi = cues.length - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (cues[mid].end <= time) lo = mid + 1;
      else if (cues[mid].start > time) hi = mid - 1;
      else return cues[mid];
    }
    return null;
  }
  let active = $derived(cueAt(store.cues, store.currentTime));

  // keep time smooth while playing
  $effect(() => {
    if (!playing || !video) return;
    let raf = 0;
    const tick = () => {
      if (video) store.currentTime = video.currentTime;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });

  $effect(() => {
    const req = store.seekRequest;
    if (req && video) {
      video.currentTime = req.t;
      store.currentTime = req.t;
    }
  });

  $effect(() => {
    // a new file
    void src;
    previewError = null;
    playing = false;
    store.currentTime = 0;
  });

  $effect(() => {
    if (video) video.playbackRate = rate;
  });

  $effect(() => {
    if (!stage) return;
    const ro = new ResizeObserver(([e]) => {
      stageW = e.contentRect.width;
      stageH = e.contentRect.height;
    });
    ro.observe(stage);
    return () => ro.disconnect();
  });

  function onError() {
    const err = video?.error;
    api.log(`video error ${err?.code} ${err?.message ?? ""} src=${video?.currentSrc}`);
    needProxy();
  }

  async function needProxy() {
    const it = item;
    if (!it || triedProxy.has(it.path)) {
      if (it) previewError = t("previewFailed");
      return;
    }
    triedProxy.add(it.path);
    proxying = 0;
    const un = await on<{ path: string; progress: number }>("preview-progress", (e) => {
      if (e.path === it.path) proxying = e.progress;
    });
    try {
      const p = await api.makePreview(it.path);
      it.previewSrc = await api.mediaUrl(p);
    } catch (e) {
      if (errorText(e) !== "cancelled") previewError = t("previewFailed");
    } finally {
      un();
      proxying = null;
    }
  }

  function onMeta() {
    // audio decoded but the picture cannot be: make a copy the web view can show
    if (info?.hasVideo && video && video.videoWidth === 0) needProxy();
  }

  export function toggle() {
    if (!video) return;
    if (video.paused) video.play().catch(() => needProxy());
    else video.pause();
  }

  function jumpCue(dir: -1 | 1) {
    const cues = store.cues;
    if (!cues.length) return;
    const now = store.currentTime + 0.001;
    let target: Cue | undefined;
    if (dir > 0) target = cues.find((c) => c.start > now);
    else {
      const cur = cueAt(cues, now);
      const before = cues.filter((c) => c.start < (cur ? cur.start - 0.01 : now - 0.3));
      target = before[before.length - 1];
    }
    if (target) store.seek(target.start);
  }

  export { jumpCue };

  let fontPx = $derived((store.settings.look.size / 100) * frame.h);
  let textShadow = $derived.by(() => {
    const L = store.settings.look;
    const s = Math.max(1, fontPx * 0.06);
    if (L.background === "outline") {
      const c = L.boxColor;
      const d: string[] = [];
      for (let a = 0; a < 16; a++) {
        const r = (a / 16) * Math.PI * 2;
        d.push(`${(Math.cos(r) * s).toFixed(2)}px ${(Math.sin(r) * s).toFixed(2)}px 0 ${c}`);
      }
      return d.join(",");
    }
    if (L.background === "shadow") return `0 ${s}px ${s * 2.5}px rgba(0,0,0,.9), 0 0 ${s * 1.5}px rgba(0,0,0,.8)`;
    return "none";
  });
  function hexA(hex: string, a: number) {
    const n = parseInt(hex.slice(1), 16);
    return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
  }
</script>

<div class="player">
  <div class="stage" bind:this={stage}>
    {#if item}
      <div class="frame" style:width="{frame.w}px" style:height="{frame.h}px">
        <!-- svelte-ignore a11y_media_has_caption -->
        <video
          bind:this={video}
          {src}
          preload="auto"
          onplay={() => (playing = true)}
          onpause={() => { playing = false; if (video) store.currentTime = video.currentTime; }}
          onseeked={() => { if (video) store.currentTime = video.currentTime; }}
          ontimeupdate={() => { if (!playing && video) store.currentTime = video.currentTime; }}
          onloadedmetadata={onMeta}
          onerror={onError}
          onclick={toggle}
          class:hidden={info && !info.hasVideo}
        ></video>
        {#if info && !info.hasVideo}
          <div class="audio-only" role="button" tabindex="-1" onclick={toggle} onkeydown={() => {}}>
            <Icon name="wave" size={34} />
            <span class="label">{t("audioOnly")}</span>
            <span class="fname">{item.name}</span>
          </div>
        {/if}
        {#if active && frame.h}
          {@const L = store.settings.look}
          <div class="subs" style:bottom="{(L.bottom / 100) * frame.h}px">
            <div
              class="block"
              class:left={L.align === "left"}
              style:font-family="'{L.font}', 'Noto Sans', sans-serif"
              style:font-weight={L.weight}
              style:font-stretch="{L.width}%"
              style:font-size="{fontPx}px"
              style:line-height={L.lineHeight}
            >
              {#each active.lines as line, i (i)}
                <span
                  class="plate"
                  style:background={L.background === "box" ? hexA(L.boxColor, L.boxOpacity) : "transparent"}
                  style:padding={L.background === "box" ? `0 ${fontPx * 0.3}px` : "0"}
                ><span
                    class="txt"
                    class:shade={L.shade}
                    style:--c={L.color}
                    style:color={L.shade ? "transparent" : L.color}
                    style:text-shadow={L.shade ? "none" : textShadow}
                  >{line}</span></span>
              {/each}
            </div>
          </div>
        {/if}
        {#if proxying !== null}
          <div class="notice">
            <span>{t("makingPreview")}</span>
            <span class="pbar"><span style:width="{proxying * 100}%"></span></span>
          </div>
        {:else if previewError}
          <div class="notice"><span>{previewError}</span></div>
        {/if}
      </div>
    {:else}
      <div class="empty">
        <div class="stripes"><i></i><i></i><i></i></div>
        <p>{t("noSelection")}</p>
      </div>
    {/if}
  </div>

  <div class="transport">
    <button class="icon-btn" title={t("prevCue")} onclick={() => jumpCue(-1)} disabled={!item}><Icon name="prev" /></button>
    <button class="play" title={playing ? t("pause") : t("play")} onclick={toggle} disabled={!item}>
      <Icon name={playing ? "pause" : "play"} size={18} />
    </button>
    <button class="icon-btn" title={t("nextCue")} onclick={() => jumpCue(1)} disabled={!item}><Icon name="next" /></button>
    <span class="time mono">{shortClock(store.currentTime, 2)}<span class="muted"> / {shortClock(info?.duration ?? 0, 1)}</span></span>
    <span class="spacer"></span>
    {#if active}
      <span class="cps mono" class:fast={active.cps > store.settings.rules.maxCps + 0.5} title={t("maxCps")}>{active.cps.toFixed(1)} {t("cpsUnit")}</span>
    {/if}
    <select class="rate" bind:value={rate} title={t("speed")}>
      <option value={0.5}>0.5×</option>
      <option value={0.75}>0.75×</option>
      <option value={1}>1×</option>
      <option value={1.25}>1.25×</option>
      <option value={1.5}>1.5×</option>
      <option value={2}>2×</option>
    </select>
  </div>
</div>

<style>
  .player { display: flex; flex-direction: column; min-height: 0; height: 100%; }
  .stage { flex: 1; min-height: 0; background: var(--stage); display: grid; place-items: center; position: relative; overflow: hidden; border-radius: var(--radius) var(--radius) 0 0; }
  .frame { position: relative; background: #000; overflow: hidden; container-type: size; }
  video { width: 100%; height: 100%; display: block; object-fit: contain; background: #000; cursor: pointer; }
  video.hidden { position: absolute; opacity: 0; pointer-events: none; }
  .audio-only { position: absolute; inset: 0; display: grid; place-content: center; justify-items: center; gap: 8px; color: #7d8696; background: radial-gradient(ellipse at 50% 40%, #1b2433, #0b0f15); cursor: pointer; }
  .audio-only .label { color: #9aa3b2; }
  .fname { font-size: 13px; color: #6b7484; max-width: 70%; text-align: center; overflow-wrap: anywhere; }
  .subs { position: absolute; left: 3%; right: 3%; display: flex; justify-content: center; pointer-events: none; white-space: pre; }
  .block { display: inline-flex; flex-direction: column; align-items: center; gap: 0.1em; }
  .block.left { align-items: flex-start; }
  .plate { display: inline-block; }
  /* SVT: white shading to light grey towards the baseline, with a soft shadow */
  .txt.shade {
    background: linear-gradient(to bottom, var(--c) 28%, color-mix(in srgb, var(--c) 52%, #000) 86%);
    -webkit-background-clip: text;
    background-clip: text;
    filter: drop-shadow(0 0.03em 0.035em rgba(0, 0, 0, 0.8));
  }
  .notice { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); background: rgba(13, 17, 23, 0.86); color: #e8ebf0; padding: 14px 18px; border-radius: var(--radius); display: grid; gap: 10px; font-size: 14px; max-width: 80%; text-align: center; }
  .pbar { height: 4px; background: rgba(255, 255, 255, 0.15); border-radius: 4px; overflow: hidden; }
  .pbar span { display: block; height: 100%; background: var(--yellow); }
  .empty { display: grid; justify-items: center; gap: 22px; color: #8e97a6; max-width: 420px; text-align: center; font-size: 15px; }
  .stripes { display: grid; width: 120px; height: 72px; border-radius: 10px; overflow: hidden; opacity: 0.9; }
  .stripes i:nth-child(1) { background: var(--yellow); }
  .stripes i:nth-child(2) { background: var(--paper); }
  .stripes i:nth-child(3) { background: var(--blue-2); }

  .transport { display: flex; align-items: center; gap: 6px; padding: 8px 12px; background: var(--card); border: 1px solid var(--line); border-top: 0; }
  .play { width: 38px; height: 38px; border-radius: 50%; border: 0; background: var(--blue); color: #fff; display: grid; place-items: center; cursor: pointer; }
  .play:hover:not(:disabled) { background: var(--blue-2); }
  .play:disabled { opacity: 0.4; }
  .time { font-size: 14px; margin-left: 8px; }
  .spacer { flex: 1; }
  .cps { font-size: 12px; color: var(--muted); padding: 3px 7px; border-radius: 5px; background: #f1eee5; }
  .cps.fast { color: var(--warn); background: var(--warn-soft); }
  .rate { width: auto; height: 30px; padding: 2px 26px 2px 9px; font-size: 13px; }
</style>
