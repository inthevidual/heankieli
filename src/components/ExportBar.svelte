<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { store, FORMATS } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import { toText, type Format } from "../lib/formats";
  import Icon from "./Icon.svelte";

  let item = $derived(store.selected);
  let done = $derived(store.items.filter((i) => i.status === "done"));
  let copied = $state(false);

  function toggle(f: Format) {
    const s = new Set(store.settings.formats);
    if (s.has(f)) s.delete(f);
    else s.add(f);
    store.settings.formats = FORMATS.map((x) => x.id).filter((x) => s.has(x));
  }

  async function chooseDir() {
    const d = await open({ directory: true });
    if (typeof d === "string") store.settings.outDir = d;
  }

  async function copy() {
    if (!item) return;
    await navigator.clipboard.writeText(toText(item.words));
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  let dirLabel = $derived(store.settings.outDir ? store.settings.outDir.split(/[\\/]/).filter(Boolean).pop() : null);
</script>

<div class="export">
  <div class="formats">
    <span class="label">{t("formats")}</span>
    {#each FORMATS as f (f.id)}
      <button class="fmt" class:on={store.settings.formats.includes(f.id)} onclick={() => toggle(f.id)}>
        <span class="box">{#if store.settings.formats.includes(f.id)}<Icon name="check" size={12} />{/if}</span>{t(`fmt_${f.id}`)}
      </button>
    {/each}
  </div>
  <div class="dest">
    <span class="label">{t("destination")}</span>
    <select
      value={store.settings.outDir ? "dir" : "next"}
      onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v === "next") store.settings.outDir = null; else if (v === "choose") { chooseDir(); (e.target as HTMLSelectElement).value = store.settings.outDir ? "dir" : "next"; } }}
    >
      <option value="next">{t("nextToFile")}</option>
      {#if store.settings.outDir}<option value="dir">{dirLabel}</option>{/if}
      <option value="choose">{t("chooseFolder")}</option>
    </select>
    <label class="check auto"><input type="checkbox" bind:checked={store.settings.autoSave} />{t("autoSave")}</label>
  </div>
  <div class="go">
    <button class="btn ghost" disabled={!item || item.status !== "done"} onclick={copy}><Icon name={copied ? "check" : "copy"} />{copied ? t("copied") : t("copyText")}</button>
    {#if done.length > 1}
      <button class="btn" disabled={!store.settings.formats.length} onclick={() => store.exportMany(done)}>{t("exportAll", { n: done.length })}</button>
    {/if}
    <button class="btn primary" disabled={!item || item.status !== "done" || !store.settings.formats.length} onclick={() => item && store.exportMany([item])}>
      <Icon name="download" />{t("export")}
    </button>
  </div>
</div>

<style>
  .export { display: flex; align-items: center; gap: 18px; flex-wrap: wrap; padding: 12px 14px; background: var(--card); border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow); }
  .formats, .dest { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .formats .label, .dest .label { margin-right: 4px; }
  .fmt { display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 10px 0 7px; border-radius: 7px; border: 1px solid var(--line-2); background: #fff; cursor: pointer; font-weight: 600; font-size: 13px; color: var(--ink-2); }
  .fmt.on { border-color: var(--blue); color: var(--blue); background: var(--blue-soft); }
  .box { width: 16px; height: 16px; border-radius: 4px; border: 1.5px solid currentColor; display: grid; place-items: center; }
  .fmt.on .box { background: var(--blue); border-color: var(--blue); color: #fff; }
  .dest select { width: auto; height: 30px; padding: 3px 28px 3px 9px; font-size: 13px; max-width: 220px; }
  .auto { font-size: 13px; color: var(--ink-2); margin-left: 6px; }
  .go { display: flex; gap: 6px; margin-left: auto; }
</style>
