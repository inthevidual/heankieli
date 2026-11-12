<script lang="ts">
  import { store } from "../lib/store.svelte";
  import { t, uiLang } from "../lib/i18n.svelte";
  import { TEMPLATES } from "../lib/presets";

  let r = $derived(store.settings.rules);
  let L = $derived(store.settings.look);
  let fps = $derived(store.fps || 25);
  let gapFrames = $derived(Math.round(r.minGap * fps));
</script>

<div class="panel">
  <div class="field">
    <span class="label">{t("template")}</span>
    <div class="tpl">
      <select value={store.settings.templateId} onchange={(e) => store.applyTemplate((e.target as HTMLSelectElement).value)}>
        {#each TEMPLATES as tpl (tpl.id)}
          <option value={tpl.id}>{tpl.name[uiLang()]}</option>
        {/each}
      </select>
      {#if store.templateChanged}
        <button class="btn small ghost" onclick={() => store.applyTemplate(store.settings.templateId)}>{t("resetTemplate")}</button>
      {/if}
    </div>
  </div>

  <h3>{t("layout")}</h3>

  <div class="grid">
    <label class="row" for="maxChars">
      <span>{t("maxChars")}</span>
      <input id="maxChars" type="number" min="16" max="80" bind:value={store.settings.rules.maxChars} />
    </label>
    <input type="range" min="16" max="60" bind:value={store.settings.rules.maxChars} aria-label={t("maxChars")} />

    <div class="row">
      <span>{t("maxLines")}</span>
      <div class="seg mini">
        {#each [1, 2, 3] as n}
          <button class:on={r.maxLines === n} onclick={() => (store.settings.rules.maxLines = n)}>{n}</button>
        {/each}
      </div>
    </div>

    <label class="row" for="minDur">
      <span>{t("minDuration")}</span>
      <span class="unit"><input id="minDur" type="number" min="0.3" max="5" step="0.1" bind:value={store.settings.rules.minDuration} />s</span>
    </label>
    <label class="row" for="maxDur">
      <span>{t("maxDuration")}</span>
      <span class="unit"><input id="maxDur" type="number" min="2" max="15" step="0.5" bind:value={store.settings.rules.maxDuration} />s</span>
    </label>
    <label class="row" for="cps">
      <span>{t("maxCps")}</span>
      <span class="unit"><input id="cps" type="number" min="8" max="30" step="1" bind:value={store.settings.rules.maxCps} />{t("cpsUnit")}</span>
    </label>
    <label class="row" for="gap">
      <span>{t("minGap")}</span>
      <span class="unit"><input id="gap" type="number" min="0" max="25" step="1" value={gapFrames}
        oninput={(e) => (store.settings.rules.minGap = Math.max(0, +(e.target as HTMLInputElement).value) / fps)} />{t("frames")}</span>
    </label>
    <div class="field tight">
      <span>{t("balance")}</span>
      <div class="seg">
        {#each ["bottom", "balanced", "top"] as const as b}
          <button class:on={r.balance === b} onclick={() => (store.settings.rules.balance = b)}>{t(`balance_${b}`)}</button>
        {/each}
      </div>
    </div>
    <label class="check"><input type="checkbox" bind:checked={store.settings.rules.snapFrames} />{t("snapFrames")}</label>
  </div>

  <h3>{t("look")}</h3>
  <div class="grid">
    <label class="row" for="font">
      <span>{t("font")}</span>
      <select id="font" class="font-sel" bind:value={store.settings.look.font}>
        <option value="Roboto Condensed">{t("fontSvt")}</option>
        <option value="Archivo">Archivo</option>
        <option value="Noto Sans">Noto Sans</option>
      </select>
    </label>
    <label class="row" for="size">
      <span>{t("fontSize")}</span>
      <span class="mono val">{L.size.toFixed(1)} %</span>
    </label>
    <input id="size" type="range" min="2.5" max="9" step="0.1" bind:value={store.settings.look.size} />

    <label class="row" for="weight">
      <span>{t("weight")}</span>
      <span class="mono val" style:font-family="'{L.font}'" style:font-weight={L.weight} style:font-stretch="{L.width}%" style:font-size="16px">Aa {L.weight}</span>
    </label>
    <input id="weight" type="range" min="300" max="800" step="10" bind:value={store.settings.look.weight} />

    {#if L.font === "Archivo"}
      <label class="row" for="fw"><span>{t("fontWidth")}</span><span class="mono val">{L.width} %</span></label>
      <input id="fw" type="range" min="62" max="100" step="1" bind:value={store.settings.look.width} />
    {/if}

    <div class="field tight">
      <span>{t("align")}</span>
      <div class="seg">
        {#each ["left", "center"] as const as a}
          <button class:on={L.align === a} onclick={() => (store.settings.look.align = a)}>{t(`align_${a}`)}</button>
        {/each}
      </div>
    </div>
    <label class="check"><input type="checkbox" bind:checked={store.settings.look.shade} />{t("shade")}</label>

    <div class="field tight">
      <span>{t("background")}</span>
      <div class="seg">
        {#each ["box", "outline", "shadow"] as const as b}
          <button class:on={L.background === b} onclick={() => (store.settings.look.background = b)}>{t(`bg_${b}`)}</button>
        {/each}
      </div>
    </div>
    {#if L.background === "box"}
      <label class="row" for="op"><span>{t("opacity")}</span><span class="mono val">{Math.round(L.boxOpacity * 100)} %</span></label>
      <input id="op" type="range" min="0.2" max="1" step="0.05" bind:value={store.settings.look.boxOpacity} />
    {/if}
    <label class="row" for="col">
      <span>{t("textColor")}</span>
      <span class="swatches">
        {#each ["#ffffff", "#fecc02", "#f2f2f2"] as c}
          <button class="sw" class:on={L.color === c} style:background={c} aria-label={c} onclick={() => (store.settings.look.color = c)}></button>
        {/each}
        <input id="col" type="color" bind:value={store.settings.look.color} />
      </span>
    </label>
    <label class="row" for="pos"><span>{t("position")}</span><span class="mono val">{L.bottom.toFixed(1)} %</span></label>
    <input id="pos" type="range" min="0" max="40" step="0.2" bind:value={store.settings.look.bottom} />
  </div>
  <p class="note">{t("lookNote")}</p>
</div>

<style>
  .panel { padding: 16px 18px 30px; overflow-y: auto; height: 100%; }
  .tpl { display: flex; gap: 8px; align-items: center; }
  h3 { font-family: var(--font-label); font-weight: 700; font-size: 10.5px; letter-spacing: 0.16em; text-transform: uppercase; color: var(--blue); margin: 24px 0 10px; padding-bottom: 6px; border-bottom: 1px solid var(--line); }
  .grid { display: grid; gap: 10px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 10px; font-size: 14px; }
  .row input[type="number"] { width: 70px; text-align: right; padding: 4px 8px; }
  .unit { display: flex; align-items: center; gap: 6px; color: var(--muted); font-size: 13px; }
  .val { font-size: 12.5px; color: var(--ink-2); }
  .field.tight { gap: 6px; font-size: 14px; }
  .seg.mini { min-width: 140px; }
  .seg.mini button { padding: 4px 8px; }
  .font-sel { width: auto; max-width: 230px; }
  .swatches { display: flex; gap: 6px; align-items: center; }
  .sw { width: 22px; height: 22px; border-radius: 50%; border: 1px solid var(--line-2); cursor: pointer; padding: 0; }
  .sw.on { outline: 2px solid var(--blue-2); outline-offset: 2px; }
  input[type="color"] { width: 28px; height: 24px; border: 0; padding: 0; background: none; cursor: pointer; }
  .note { font-size: 12.5px; color: var(--muted); margin-top: 18px; line-height: 1.45; }
</style>
