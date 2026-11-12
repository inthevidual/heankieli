<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { store } from "./lib/store.svelte";
  import { t } from "./lib/i18n.svelte";
  import Logo from "./components/Logo.svelte";
  import Queue from "./components/Queue.svelte";
  import Player from "./components/Player.svelte";
  import Timeline from "./components/Timeline.svelte";
  import CueList from "./components/CueList.svelte";
  import StylePanel from "./components/StylePanel.svelte";
  import ExportBar from "./components/ExportBar.svelte";
  import ModelsDialog from "./components/ModelsDialog.svelte";
  import Icon from "./components/Icon.svelte";

  let ready = $state(false);
  let dragging = $state(false);
  let tab = $state<"subs" | "style">("subs");
  let player = $state<ReturnType<typeof Player> | null>(null);

  onMount(() => {
    store.init().then(() => (ready = true));
    const un = getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type === "enter" || e.payload.type === "over") dragging = true;
      else if (e.payload.type === "leave") dragging = false;
      else if (e.payload.type === "drop") {
        dragging = false;
        store.addFiles(e.payload.paths);
      }
    });
    return () => {
      un.then((f) => f());
    };
  });

  function typing(e: KeyboardEvent) {
    const el = e.target as HTMLElement;
    return el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable;
  }

  function keydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === "z" && !typing(e)) {
      e.preventDefault();
      if (e.shiftKey) store.redo();
      else store.undo();
    } else if (mod && e.key.toLowerCase() === "y" && !typing(e)) {
      e.preventDefault();
      store.redo();
    } else if (e.key === " " && !typing(e) && (e.target as HTMLElement).tagName !== "BUTTON") {
      e.preventDefault();
      player?.toggle();
    } else if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      player?.jumpCue(e.key === "ArrowLeft" ? -1 : 1);
    } else if (!typing(e) && !mod && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      store.seek(store.currentTime + (e.key === "ArrowLeft" ? -2 : 2) * (e.shiftKey ? 5 : 1));
    }
  }
</script>

<svelte:window onkeydown={keydown} />

<div class="app">
  <header class="top">
    <Logo />
    <div class="top-right">
      {#if store.info}
        <span class="engine" title={store.info.backend}><Icon name="cpu" size={15} />{t("gpu", { b: store.info.backend })}</span>
      {/if}
      <button class="btn top-btn" onclick={() => (store.showModels = true)}><Icon name="gear" />{t("models")}</button>
    </div>
    <div class="flag"><i></i><i></i><i></i></div>
  </header>

  {#if ready && store.info && !store.info.ffmpeg}
    <div class="banner"><Icon name="alert" />{t("noFfmpeg")}</div>
  {/if}

  <main>
    <aside class="left"><Queue {dragging} /></aside>

    <section class="center">
      <div class="viewer">
        <Player bind:this={player} />
        <Timeline />
      </div>
      <ExportBar />
      <p class="keys">{t("shortcuts")}</p>
    </section>

    <aside class="right">
      <div class="tabs" role="tablist">
        <button role="tab" aria-selected={tab === "subs"} class:on={tab === "subs"} onclick={() => (tab = "subs")}>{t("subtitles")}</button>
        <button role="tab" aria-selected={tab === "style"} class:on={tab === "style"} onclick={() => (tab = "style")}>{t("style")}</button>
      </div>
      <div class="tab-body">
        {#if tab === "subs"}<CueList />{:else}<StylePanel />{/if}
      </div>
    </aside>
  </main>

  {#if store.toast}
    <div class="toast">
      <span>{store.toast.text}</span>
      {#if store.toast.action}
        <button class="btn small accent" onclick={() => { store.toast?.action?.run(); store.toast = null; }}>{store.toast.action.label}</button>
      {/if}
      <button class="icon-btn" aria-label={t("close")} onclick={() => (store.toast = null)}><Icon name="x" size={15} /></button>
    </div>
  {/if}

  <ModelsDialog />
</div>

<style>
  .app { display: grid; grid-template-rows: auto auto 1fr; height: 100vh; }
  .top { position: relative; display: flex; align-items: center; justify-content: space-between; padding: 12px 20px 15px; background: var(--blue); color: #fff; }
  .flag { position: absolute; left: 0; right: 0; bottom: 0; height: 5px; display: grid; grid-template-columns: 1fr 1fr 1fr; }
  .flag i:nth-child(1) { background: var(--yellow); }
  .flag i:nth-child(2) { background: var(--paper); }
  .flag i:nth-child(3) { background: var(--blue-2); }
  .top-right { display: flex; align-items: center; gap: 12px; }
  .engine { display: inline-flex; align-items: center; gap: 6px; font-family: var(--font-label); font-weight: 700; font-size: 10.5px; letter-spacing: 0.14em; text-transform: uppercase; color: rgba(255, 255, 255, 0.8); }
  .top-btn { background: rgba(255, 255, 255, 0.1); border-color: rgba(255, 255, 255, 0.25); color: #fff; }
  .top-btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.18); border-color: rgba(255, 255, 255, 0.5); }

  .banner { display: flex; gap: 8px; align-items: center; padding: 10px 20px; background: var(--danger-soft); color: var(--danger); font-weight: 600; }

  main { display: grid; grid-template-columns: 290px minmax(420px, 1fr) 440px; min-height: 0; }
  .left { border-right: 1px solid var(--line); min-height: 0; background: #efece2; }
  .center { display: flex; flex-direction: column; gap: 12px; padding: 16px; min-height: 0; min-width: 0; }
  .viewer { flex: 1; min-height: 0; display: flex; flex-direction: column; box-shadow: var(--shadow); border-radius: var(--radius); }
  .keys { margin: -4px 2px 0; font-size: 11.5px; color: var(--muted); }
  .right { border-left: 1px solid var(--line); background: var(--card); display: flex; flex-direction: column; min-height: 0; }
  .tabs { display: flex; gap: 2px; padding: 10px 10px 0; border-bottom: 1px solid var(--line); background: #fbfaf6; }
  .tabs button { border: 0; background: transparent; padding: 9px 14px 10px; font-weight: 700; font-size: 14px; color: var(--muted); cursor: pointer; border-bottom: 3px solid transparent; margin-bottom: -1px; }
  .tabs button.on { color: var(--ink); border-bottom-color: var(--yellow); }
  .tab-body { flex: 1; min-height: 0; }

  .toast { position: fixed; left: 50%; bottom: 22px; transform: translateX(-50%); display: flex; align-items: center; gap: 12px; background: var(--ink); color: #fff; padding: 10px 10px 10px 16px; border-radius: 10px; box-shadow: var(--shadow-lg); font-weight: 600; z-index: 10; }
  .toast .icon-btn { color: rgba(255, 255, 255, 0.7); }
</style>
