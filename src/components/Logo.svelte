<script lang="ts">
  import { t } from "../lib/i18n.svelte";
</script>

<!-- The PlumppuVox logo as on the plugin panels: "Plumppu" and a painted
     "Vox" badge on the first line, the product name in Barlow Black below. -->
<div class="logo" aria-label="PlumppuVox Heänkieli">
  <div class="brand">
    <span class="plumppu">Plumppu</span><span class="vox">Vox</span>
  </div>
  <div class="model">HEÄNKIELI</div>
  <div class="tag">{t("tagline")}</div>
</div>

<style>
  .logo { display: grid; justify-items: start; line-height: 1; }
  .brand { display: flex; align-items: center; gap: 4px; }
  .plumppu { font-family: "B612", sans-serif; font-weight: 700; font-size: 17px; letter-spacing: -0.01em; color: #fff; }
  .vox {
    font-family: "B612", sans-serif; font-weight: 700; font-size: 17px; letter-spacing: -0.01em;
    background: var(--yellow); color: var(--blue); border-radius: 3px; padding: 3px 5px 2px;
  }
  .model {
    font-family: "Barlow", sans-serif; font-weight: 900; font-size: 31px; letter-spacing: 0.04em;
    color: var(--yellow); margin-top: 4px;
  }
  .tag {
    font-family: "B612", sans-serif; font-weight: 700; font-size: 8.5px; letter-spacing: 0.3em;
    text-transform: uppercase; color: rgba(255, 255, 255, 0.82); margin-top: 4px;
  }
</style>
