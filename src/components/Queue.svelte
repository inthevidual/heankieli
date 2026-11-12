<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { store, type Item } from "../lib/store.svelte";
  import { t, type Key } from "../lib/i18n.svelte";
  import { shortClock } from "../lib/formats";
  import Icon from "./Icon.svelte";
  import type { Lang, Quality } from "../lib/types";

  let { dragging = false }: { dragging?: boolean } = $props();

  const MEDIA = ["mp4", "mov", "m4v", "mkv", "webm", "avi", "mxf", "mts", "m2ts", "ts", "wmv", "flv", "mpg", "mpeg", "3gp",
    "wav", "mp3", "m4a", "aac", "flac", "ogg", "opus", "aif", "aiff", "wma", "caf"];

  async function choose() {
    const picked = await open({ multiple: true, filters: [{ name: "Media", extensions: [...MEDIA, ...MEDIA.map((e) => e.toUpperCase())] }] });
    if (picked) store.addFiles(Array.isArray(picked) ? picked : [picked]);
  }

  function setLang(l: Lang) {
    store.settings.lang = l;
    for (const it of store.items) if (it.status === "waiting" || it.status === "error" || it.status === "cancelled") it.lang = l;
  }
  function setQuality(q: Quality) {
    store.settings.quality = q;
    for (const it of store.items) if (it.status === "waiting" || it.status === "error" || it.status === "cancelled") it.quality = q;
  }

  function statusText(it: Item): string {
    if (it.status === "transcribing" || it.status === "downloading") {
      if (it.status === "downloading") {
        const d = Object.values(store.downloads).find((x) => x.state === "downloading" || x.state === "verifying");
        if (d) return `${t("status_downloading")} ${Math.floor((d.received / d.total) * 100)} %`;
        return t("status_downloading");
      }
      const k = ("status_" + it.stage) as Key;
      const pct = it.stage === "transcribing" || it.stage === "decoding" ? ` ${Math.floor(it.progress * 100)} %` : "";
      return t(k) + pct;
    }
    if (it.status === "done") return it.restored ? t("restored") : t("status_done");
    return t(("status_" + it.status) as Key);
  }

  function progressOf(it: Item): number {
    if (it.status === "downloading") {
      const d = Object.values(store.downloads).find((x) => x.state === "downloading");
      return d ? d.received / d.total : 0;
    }
    if (it.stage === "decoding") return it.progress * 0.05;
    if (it.stage === "detecting") return 0.05;
    if (it.stage === "loading") return 0.07;
    return 0.08 + it.progress * 0.92;
  }

  let waiting = $derived(store.items.filter((i) => i.status === "waiting" && i.info).length);
  let busy = $derived(store.running);
</script>

<section class="queue">
  <button class="drop" class:dragging onclick={choose}>
    <Icon name="plus" size={22} />
    <strong>{t("dropHere")}</strong>
    <span>{t("dropHint")}</span>
  </button>

  {#if store.items.length}
    <div class="head"><span class="label">{t("queue")}</span><span class="label muted">{store.items.length}</span></div>
    <ul class="list">
      {#each store.items as it (it.id)}
        <li class:sel={store.selectedId === it.id} class:err={it.status === "error"}>
          <button class="row" onclick={() => store.select(it)} title={it.path}>
            <span class="kind"><Icon name={it.info?.hasVideo === false ? "wave" : "film"} size={16} /></span>
            <span class="name">{it.name}</span>
            <span class="meta">
              <span class="chip">{it.lang.toUpperCase()}</span>
              {#if it.info}<span class="mono dur">{shortClock(it.info.duration, 0)}</span>{/if}
            </span>
            <span class="status" class:done={it.status === "done"}>
              {#if it.status === "done"}<Icon name="check" size={13} />{/if}
              {#if it.status === "error"}<Icon name="alert" size={13} />{/if}
              {statusText(it)}
            </span>
            {#if it.status === "transcribing" || it.status === "downloading"}
              <span class="bar"><span style:width="{Math.round(progressOf(it) * 1000) / 10}%"></span></span>
              {#if it.live.length}<span class="live">{it.live[it.live.length - 1]}</span>{/if}
            {/if}
            {#if it.error}<span class="error">{it.error}</span>{/if}
          </button>
          <div class="actions">
            {#if it.status === "error" || it.status === "cancelled"}
              <button class="icon-btn" title={t("retry")} onclick={() => store.start([it])}><Icon name="refresh" size={15} /></button>
            {:else if it.status === "done" && !busy}
              <button class="icon-btn" title={t("redo")} onclick={() => { it.lang = store.settings.lang; it.quality = store.settings.quality; store.start([it]); }}><Icon name="refresh" size={15} /></button>
            {/if}
            {#if it.status !== "transcribing" && it.status !== "downloading"}
              <button class="icon-btn" title={t("remove")} onclick={() => store.remove(it)}><Icon name="x" size={15} /></button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="settings">
    <div class="field">
      <span class="label">{t("language")}</span>
      <div class="seg">
        <button class:on={store.settings.lang === "sv"} onclick={() => setLang("sv")}>{t("swedish")}<small>KB-Whisper</small></button>
        <button class:on={store.settings.lang === "en"} onclick={() => setLang("en")}>{t("english")}<small>Whisper large-v3</small></button>
      </div>
    </div>
    <div class="field">
      <span class="label">{t("quality")}</span>
      <div class="seg">
        <button class:on={store.settings.quality === "accurate"} onclick={() => setQuality("accurate")}>{t("accurate")}<small>{t("accurateHint")}</small></button>
        <button class:on={store.settings.quality === "fast"} onclick={() => setQuality("fast")}>{t("fast")}<small>{t("fastHint")}</small></button>
      </div>
    </div>
    <div class="field">
      <label class="label" for="prompt">{t("prompt")}</label>
      <input id="prompt" type="text" bind:value={store.settings.prompt} placeholder={t("promptHint")} spellcheck="false" />
    </div>

    <div class="go">
      {#if busy}
        <button class="btn" onclick={() => store.stop()}><Icon name="x" />{t("stop")}</button>
      {:else}
        <button class="btn accent big" disabled={!waiting} onclick={() => store.start()}>
          {waiting > 1 ? t("transcribeAll", { n: waiting }) : t("transcribe")}
        </button>
      {/if}
    </div>
  </div>
</section>

<style>
  .queue { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .drop {
    display: grid; justify-items: center; gap: 4px; text-align: center;
    margin: 16px 16px 0; padding: 18px 14px; border-radius: var(--radius);
    border: 1.5px dashed var(--line-2); background: rgba(255, 255, 255, 0.55); cursor: pointer;
    color: var(--ink-2); transition: border-color 0.15s, background 0.15s;
  }
  .drop:hover, .drop.dragging { border-color: var(--blue-2); background: var(--blue-soft); color: var(--blue); }
  .drop strong { font-weight: 700; font-size: 14.5px; color: var(--ink); }
  .drop span { font-size: 12.5px; }

  .head { display: flex; justify-content: space-between; padding: 16px 18px 6px; }
  .list { list-style: none; margin: 0; padding: 0 10px; overflow-y: auto; flex: 1; min-height: 60px; }
  .list li { position: relative; border-radius: var(--radius-sm); margin-bottom: 4px; }
  .list li.sel { background: var(--card); box-shadow: var(--shadow); }
  .list li.sel::before { content: ""; position: absolute; left: 0; top: 8px; bottom: 8px; width: 3px; border-radius: 3px; background: var(--yellow); }
  .row {
    display: grid; grid-template-columns: 22px 1fr auto; gap: 2px 8px; align-items: center;
    width: 100%; text-align: left; border: 0; background: transparent; padding: 9px 64px 9px 10px; cursor: pointer; border-radius: var(--radius-sm);
  }
  .list li:not(.sel) .row:hover { background: rgba(255, 255, 255, 0.6); }
  .kind { color: var(--muted); display: grid; }
  .name { font-weight: 600; font-size: 14px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { display: flex; gap: 6px; align-items: center; grid-column: 3; grid-row: 1; position: absolute; right: 10px; top: 10px; }
  .chip { font-family: var(--font-label); font-weight: 700; font-size: 9.5px; letter-spacing: 0.1em; padding: 2px 5px; border-radius: 4px; background: var(--blue-soft); color: var(--blue); }
  .dur { font-size: 11px; color: var(--muted); }
  .status { grid-column: 2 / 4; font-size: 12.5px; color: var(--muted); display: flex; align-items: center; gap: 4px; }
  .status.done { color: var(--ok); }
  li.err .status { color: var(--danger); }
  .bar { grid-column: 2 / 4; height: 4px; border-radius: 4px; background: #e8e3d6; overflow: hidden; margin-top: 3px; }
  .bar span { display: block; height: 100%; background: var(--blue); border-radius: 4px; transition: width 0.3s; }
  .live { grid-column: 2 / 4; font-size: 12px; color: var(--ink-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-style: italic; }
  .error { grid-column: 2 / 4; font-size: 12px; color: var(--danger); user-select: text; -webkit-user-select: text; }
  .actions { position: absolute; right: 6px; bottom: 6px; display: flex; opacity: 0; transition: opacity 0.1s; }
  .list li:hover .actions, .list li.sel .actions { opacity: 1; }
  .actions .icon-btn { width: 26px; height: 26px; }

  .settings { padding: 16px 18px 18px; border-top: 1px solid var(--line); background: rgba(255, 255, 255, 0.35); }
  .go { margin-top: 16px; display: grid; }
  .go .btn { height: 42px; font-size: 15.5px; font-weight: 700; }
</style>
