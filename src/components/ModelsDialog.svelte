<script lang="ts">
  import { api, errorText } from "../lib/api";
  import { store } from "../lib/store.svelte";
  import { t, type Key } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";
  import type { ModelStatus } from "../lib/types";

  let dialog = $state<HTMLDialogElement | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    if (!dialog) return;
    if (store.showModels && !dialog.open) {
      store.refreshModels();
      dialog.showModal();
    } else if (!store.showModels && dialog.open) dialog.close();
  });

  const gb = (n: number) => (n >= 1e9 ? (n / 1e9).toFixed(1) + " GB" : Math.round(n / 1e6) + " MB");

  async function download(m: ModelStatus) {
    error = null;
    try {
      await api.downloadModel(m.id);
    } catch (e) {
      error = `${t("downloadFailed")}: ${errorText(e)}`;
    }
    store.refreshModels();
  }

  async function del(m: ModelStatus) {
    await api.deleteModel(m.id);
    store.refreshModels();
  }

  let groups = $derived([
    { key: "model_sv" as Key, list: store.models.filter((m) => m.language === "sv") },
    { key: "model_en" as Key, list: store.models.filter((m) => m.language === "en") },
    { key: "model_vad" as Key, list: store.models.filter((m) => m.id === "vad") },
  ]);
</script>

<dialog bind:this={dialog} onclose={() => (store.showModels = false)} onclick={(e) => e.target === dialog && (store.showModels = false)}>
  <div class="inner">
    <header>
      <h2>{t("modelsTitle")}</h2>
      <button class="icon-btn" onclick={() => (store.showModels = false)} aria-label={t("close")}><Icon name="x" /></button>
    </header>
    <p class="intro">{t("modelsIntro")}</p>

    {#each groups as g (g.key)}
      <h3 class="label">{t(g.key)}</h3>
      {#each g.list as m (m.id)}
        {@const d = store.downloads[m.id]}
        {@const active = d && (d.state === "downloading" || d.state === "verifying")}
        <div class="model">
          <div class="desc">
            <strong>{m.name}</strong>
            <span class="muted">{m.quality ? t(m.quality as Key) + " · " : ""}{gb(m.size)} · {m.repo}</span>
            {#if active}
              <span class="bar"><span style:width="{(d.received / d.total) * 100}%"></span></span>
              <span class="muted small mono">{d.state === "verifying" ? t("verifying") : `${gb(d.received)} / ${gb(d.total)} · ${(d.bytesPerSec / 1e6).toFixed(1)} MB/s`}</span>
            {/if}
          </div>
          <div class="act">
            {#if active}
              <button class="btn small" onclick={() => api.cancelDownload(m.id)}>{t("cancel")}</button>
            {:else if m.installed}
              <span class="ok"><Icon name="check" size={14} />{t("installed")}</span>
              {#if m.id !== "vad"}<button class="icon-btn" title={t("delete")} onclick={() => del(m)}><Icon name="trash" size={15} /></button>{/if}
            {:else}
              <button class="btn small" onclick={() => download(m)}><Icon name="download" />{m.partial ? t("resume") : t("download")}</button>
            {/if}
          </div>
        </div>
      {/each}
    {/each}
    {#if error}<p class="error">{error}</p>{/if}

    <div class="foot">
      <div class="field">
        <label class="label" for="uil">{t("uiLanguage")}</label>
        <select id="uil" bind:value={store.settings.uiLang} onchange={() => store.applyUiLang()}>
          <option value="auto">{t("auto")}</option>
          <option value="en">English</option>
          <option value="sv">Svenska</option>
        </select>
      </div>
      <div class="field">
        <span class="label">{t("modelsFolder")}</span>
        <span class="path mono">{store.info?.modelsDir}</span>
      </div>
    </div>
  </div>
</dialog>

<style>
  dialog { border: 0; border-radius: 14px; padding: 0; width: min(620px, 92vw); box-shadow: var(--shadow-lg); color: var(--ink); background: var(--paper); }
  dialog::backdrop { background: rgba(13, 17, 23, 0.45); }
  .inner { padding: 20px 22px 22px; max-height: 86vh; overflow-y: auto; }
  header { display: flex; justify-content: space-between; align-items: center; }
  h2 { font-family: var(--font-head); font-weight: 900; font-size: 24px; letter-spacing: 0.03em; text-transform: uppercase; color: var(--blue); margin: 0; }
  .intro { color: var(--ink-2); margin: 6px 0 4px; }
  h3 { margin: 18px 0 8px; }
  .model { display: flex; gap: 12px; align-items: center; justify-content: space-between; background: var(--card); border: 1px solid var(--line); border-radius: var(--radius-sm); padding: 10px 12px; margin-bottom: 6px; }
  .desc { display: grid; gap: 2px; min-width: 0; flex: 1; }
  .desc .muted { font-size: 12.5px; }
  .small { font-size: 11.5px; }
  .bar { height: 4px; border-radius: 4px; background: #e8e3d6; overflow: hidden; margin-top: 4px; }
  .bar span { display: block; height: 100%; background: var(--blue); }
  .act { display: flex; align-items: center; gap: 6px; }
  .ok { display: inline-flex; gap: 4px; align-items: center; color: var(--ok); font-size: 13px; font-weight: 600; }
  .error { color: var(--danger); }
  .foot { display: grid; grid-template-columns: 200px 1fr; gap: 16px; margin-top: 22px; padding-top: 16px; border-top: 1px solid var(--line); }
  .path { font-size: 11px; color: var(--ink-2); overflow-wrap: anywhere; user-select: text; -webkit-user-select: text; }
</style>
