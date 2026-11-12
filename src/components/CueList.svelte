<script lang="ts">
  import { tick } from "svelte";
  import { store } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import { deleteRange, insertAt, mergeRange, replaceText, setTiming, splitAt, unlockRange } from "../lib/layout";
  import { parseClock, shortClock } from "../lib/formats";
  import type { Cue, Word } from "../lib/types";
  import Icon from "./Icon.svelte";

  const UNSURE = 0.3;
  let editing = $state<number | null>(null);
  let editText = $state("");
  let textarea = $state<HTMLTextAreaElement | null>(null);
  let list = $state<HTMLDivElement | null>(null);
  let finding = $state(false);
  let query = $state("");
  let replacement = $state("");
  let timeEdit = $state<{ index: number; edge: "start" | "end"; value: string } | null>(null);

  let item = $derived(store.selected);
  let words = $derived(item?.words ?? []);
  let cues = $derived(store.cues);
  let activeIndex = $derived.by(() => {
    const now = store.currentTime;
    const c = cues.find((c) => now >= c.start && now < c.end);
    return c ? c.index : -1;
  });

  // follow playback
  $effect(() => {
    const i = activeIndex;
    if (i < 0 || editing !== null || !store.settings.follow || !list) return;
    const el = list.querySelector<HTMLElement>(`[data-cue="${i}"]`);
    if (!el) return;
    const r = el.getBoundingClientRect(), lr = list.getBoundingClientRect();
    if (r.top < lr.top + 40 || r.bottom > lr.bottom - 40) el.scrollIntoView({ block: "center", behavior: "smooth" });
  });

  /** text as the user edits it: spaces, and new lines only where forced */
  function plainText(c: Cue): string {
    let s = "";
    for (let k = c.from; k < c.to; k++) s += (k > c.from ? (words[k].nl ? "\n" : " ") : "") + words[k].text;
    return s;
  }

  function linesWithWords(c: Cue): { w: Word; k: number }[][] {
    const out: { w: Word; k: number }[][] = [];
    let k = c.from;
    for (const line of c.lines) {
      const n = line.split(" ").length;
      out.push(words.slice(k, k + n).map((w, j) => ({ w, k: k + j })));
      k += n;
    }
    return out;
  }

  function manual(c: Cue): boolean {
    for (let k = c.from; k < c.to; k++) {
      const w = words[k];
      if ((k > c.from && (w.join || w.nl)) || (k === c.from && w.br) || w.cueStart !== undefined || w.cueEnd !== undefined) return true;
    }
    return words[c.to]?.br === true;
  }

  async function startEdit(c: Cue) {
    editing = c.index;
    editText = plainText(c);
    store.seek(c.start);
    await tick();
    textarea?.focus();
    textarea?.setSelectionRange(editText.length, editText.length);
  }

  function commit(): Cue | null {
    if (editing === null) return null;
    const c = cues[editing];
    editing = null;
    if (!c) return null;
    if (editText !== plainText(c)) store.edit((w) => replaceText(w, c.from, c.to, editText));
    return c;
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      editing = null;
      e.preventDefault();
    } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      splitAtCaret();
    } else if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      const c = commit();
      if (c && e.altKey) {
        const next = store.cues.find((q) => q.from >= c.to - 0) ?? null;
        if (next) startEdit(next);
      }
    } else if (e.key === "Tab") {
      e.preventDefault();
      const idx = editing!;
      commit();
      const next = store.cues[idx + (e.shiftKey ? -1 : 1)];
      if (next) startEdit(next);
    }
  }

  function splitAtCaret() {
    if (editing === null || !textarea) return;
    const c = cues[editing];
    const caret = textarea.selectionStart;
    const before = editText.slice(0, caret).trim();
    const nBefore = before ? before.split(/\s+/).length : 0;
    const total = editText.trim().split(/\s+/).filter(Boolean).length;
    editing = null;
    if (nBefore <= 0 || nBefore >= total) {
      if (editText !== plainText(c)) store.edit((w) => replaceText(w, c.from, c.to, editText));
      return;
    }
    const text = editText;
    store.edit((w) => splitAt(replaceText(w, c.from, c.to, text), c.from + nBefore));
  }

  function merge(c: Cue) {
    const next = cues[c.index + 1];
    if (next) store.edit((w) => mergeRange(w, c.from, next.to));
  }

  function remove(c: Cue) {
    store.edit((w) => deleteRange(w, c.from, c.to));
  }

  function unlock(c: Cue) {
    store.edit((w) => unlockRange(w, c.from, c.to + 1 <= w.length ? c.to + 1 : c.to));
  }

  function insert() {
    let idx = -1;
    store.edit((w) => {
      const [out, i] = insertAt(w, store.currentTime, t("newText"));
      idx = i;
      return out;
    });
    tick().then(() => {
      const c = store.cues.find((q) => q.from === idx);
      if (c) startEdit(c);
    });
  }

  function setEdgeToPlayhead(c: Cue, edge: "start" | "end") {
    const now = store.currentTime;
    if (edge === "start" && now >= c.end) return;
    if (edge === "end" && now <= c.start) return;
    store.edit((w) => setTiming(w, c, edge === "start" ? now : undefined, edge === "end" ? now : undefined));
  }

  function commitTime() {
    if (!timeEdit) return;
    const v = parseClock(timeEdit.value);
    const c = cues[timeEdit.index];
    const edge = timeEdit.edge;
    timeEdit = null;
    if (v === null || !c) return;
    if (edge === "start" && v < c.end) store.edit((w) => setTiming(w, c, v, undefined));
    if (edge === "end" && v > c.start) store.edit((w) => setTiming(w, c, undefined, v));
  }

  // find & replace
  let needle = $derived(query.trim().toLowerCase());
  let matchSet = $derived.by(() => {
    const s = new Set<number>();
    if (!needle) return s;
    for (const c of cues) if (c.lines.join(" ").toLowerCase().includes(needle)) s.add(c.index);
    return s;
  });

  function escapeRe(s: string) {
    return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  }

  function replaceAll() {
    if (!needle) return;
    const re = new RegExp(escapeRe(query.trim()), "gi");
    const targets = cues.filter((c) => matchSet.has(c.index)).reverse();
    store.edit((w) => {
      let out = w;
      for (const c of targets) {
        let s = "";
        for (let k = c.from; k < c.to; k++) s += (k > c.from ? (out[k].nl ? "\n" : " ") : "") + out[k].text;
        const r = s.replace(re, replacement);
        if (r !== s) out = replaceText(out, c.from, c.to, r);
      }
      return out;
    });
  }

  function jumpToMatch() {
    const now = store.currentTime;
    const list = cues.filter((c) => matchSet.has(c.index));
    const next = list.find((c) => c.start > now + 0.01) ?? list[0];
    if (next) store.seek(next.start);
  }

  function highlight(text: string): string {
    const esc = text.replace(/&/g, "&amp;").replace(/</g, "&lt;");
    if (!needle) return esc;
    return esc.replace(new RegExp(escapeRe(needle.replace(/&/g, "&amp;").replace(/</g, "&lt;")), "gi"), (m) => `<mark>${m}</mark>`);
  }
</script>

<div class="editor">
  <div class="tools">
    <button class="icon-btn" title="{t('undo')} (Ctrl+Z)" disabled={!item?.past.length} onclick={() => store.undo()}><Icon name="undo" /></button>
    <button class="icon-btn" title="{t('redoEdit')} (Ctrl+Shift+Z)" disabled={!item?.future.length} onclick={() => store.redo()}><Icon name="redo" /></button>
    <button class="icon-btn" title={t("insertCue")} disabled={!item || item.status !== "done"} onclick={insert}><Icon name="insert" /></button>
    <button class="icon-btn" class:on={finding} title={t("find")} onclick={() => (finding = !finding)}><Icon name="search" /></button>
    <span class="count label">{t("cues", { n: cues.length })}</span>
    <label class="check follow"><input type="checkbox" bind:checked={store.settings.follow} />{t("follow")}</label>
  </div>

  {#if finding}
    <div class="find">
      <input type="text" placeholder={t("find")} bind:value={query} onkeydown={(e) => e.key === "Enter" && jumpToMatch()} spellcheck="false" />
      <input type="text" placeholder={t("replaceWith")} bind:value={replacement} spellcheck="false" />
      <button class="btn small" disabled={!matchSet.size} onclick={replaceAll}>{t("replaceAll")}</button>
      <span class="muted small">{needle ? t("matches", { n: matchSet.size }) : ""}</span>
    </div>
  {/if}

  <div class="list" bind:this={list}>
    {#if item && item.status === "done" && cues.length === 0}
      <p class="empty">{t("emptyTranscript")}</p>
    {/if}
    {#each cues as c (c.from + ":" + c.to + ":" + c.index)}
      {@const on = c.index === activeIndex}
      {@const fast = c.cps > store.settings.rules.maxCps + 0.5}
      <div class="cue" class:on class:editing={editing === c.index} class:match={matchSet.has(c.index)} data-cue={c.index}>
        <div class="gutter">
          <span class="num mono">{c.index + 1}</span>
        </div>
        <div class="times mono">
          {#each ["start", "end"] as const as edge}
            {#if timeEdit && timeEdit.index === c.index && timeEdit.edge === edge}
              <!-- svelte-ignore a11y_autofocus -->
              <input class="time-input" type="text" bind:value={timeEdit.value} autofocus
                onblur={commitTime} onkeydown={(e) => { if (e.key === "Enter") commitTime(); if (e.key === "Escape") timeEdit = null; }} />
            {:else}
              <button class="time" class:locked={edge === "start" ? words[c.from]?.cueStart !== undefined : words[c.to - 1]?.cueEnd !== undefined}
                onclick={() => store.seek(edge === "start" ? c.start : Math.max(c.start, c.end - 0.5))}
                ondblclick={() => (timeEdit = { index: c.index, edge, value: shortClock(edge === "start" ? c.start : c.end, 2) })}
                title={edge === "start" ? t("startHere") : t("endHere")}>{shortClock(edge === "start" ? c.start : c.end, 2)}</button>
            {/if}
          {/each}
          <span class="dur" class:fast title={fast ? t("tooFast", { cps: c.cps.toFixed(1) }) : ""}>{(c.end - c.start).toFixed(1)}s</span>
        </div>

        <div class="body">
          {#if editing === c.index}
            <textarea bind:this={textarea} bind:value={editText} onkeydown={keydown} onblur={() => commit()} rows={Math.max(2, c.lines.length)} spellcheck="true"></textarea>
            <div class="hint">{t("editHint")}</div>
          {:else}
            <div class="text" role="button" tabindex="0"
              onclick={() => store.seek(c.start)}
              ondblclick={() => startEdit(c)}
              onkeydown={(e) => e.key === "Enter" && startEdit(c)}>
              {#each linesWithWords(c) as line, li (li)}
                <div class="line">
                  {#each line as { w, k }, wi (k)}{#if wi > 0}{" "}{/if}<span class:unsure={w.p < UNSURE}>{@html highlight(w.text)}</span>{/each}
                </div>
              {/each}
            </div>
          {/if}
          <div class="flags">
            {#if c.overflow}<span class="flag bad" title={t("overflow")}><Icon name="alert" size={12} /></span>{/if}
            {#if fast}<span class="flag warn mono">{c.cps.toFixed(0)}</span>{/if}
          </div>
        </div>

        <div class="row-actions">
          <button class="icon-btn" title={t("startHere")} onclick={() => setEdgeToPlayhead(c, "start")}><Icon name="start" size={15} /></button>
          <button class="icon-btn" title={t("endHere")} onclick={() => setEdgeToPlayhead(c, "end")}><Icon name="end" size={15} /></button>
          <button class="icon-btn" title={t("split")} onclick={() => startEdit(c)}><Icon name="split" size={15} /></button>
          <button class="icon-btn" title={t("mergeNext")} disabled={c.index >= cues.length - 1} onclick={() => merge(c)}><Icon name="merge" size={15} /></button>
          {#if manual(c)}
            <button class="icon-btn" title={t("autoLayout")} onclick={() => unlock(c)}><Icon name="unlock" size={15} /></button>
          {/if}
          <button class="icon-btn" title={t("deleteCue")} onclick={() => remove(c)}><Icon name="trash" size={15} /></button>
        </div>
      </div>
    {/each}
  </div>
  {#if cues.some((c) => linesWithWords(c).some((l) => l.some(({ w }) => w.p < UNSURE)))}
    <div class="legend"><span class="unsure">abc</span> {t("lowConfidence")}</div>
  {/if}
</div>

<style>
  .editor { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .tools { display: flex; align-items: center; gap: 2px; padding: 8px 10px; border-bottom: 1px solid var(--line); }
  .tools .icon-btn.on { background: var(--blue-soft); color: var(--blue); }
  .count { margin-left: 10px; }
  .follow { margin-left: auto; font-size: 13px; color: var(--ink-2); }
  .find { display: grid; grid-template-columns: 1fr 1fr auto; gap: 6px; padding: 8px 10px; border-bottom: 1px solid var(--line); background: #fbfaf6; align-items: center; }
  .find .small { grid-column: 1 / -1; font-size: 12px; min-height: 0; }
  .find input { padding: 5px 8px; font-size: 13.5px; }

  .list { flex: 1; overflow-y: auto; padding: 6px 0 40px; }
  .empty { padding: 20px; color: var(--muted); }
  .cue { position: relative; display: grid; grid-template-columns: 26px 70px 1fr; gap: 0 8px; padding: 9px 10px 9px 4px; border-bottom: 1px solid #f0ece2; }
  .cue:hover { background: #fbfaf6; }
  .cue.on { background: var(--yellow-soft); }
  .cue.on .gutter::before { content: ""; position: absolute; left: 0; top: 0; bottom: 0; width: 3px; background: var(--yellow); }
  .cue.match { box-shadow: inset 3px 0 0 var(--blue-2); }
  .cue.editing { background: #fff; box-shadow: inset 0 0 0 2px var(--blue-2); }
  .gutter { padding-top: 2px; text-align: right; }
  .num { font-size: 11px; color: var(--muted); }
  .times { display: grid; align-content: start; gap: 1px; font-size: 11.5px; }
  .time { border: 0; background: transparent; text-align: left; padding: 1px 4px; border-radius: 4px; color: var(--ink-2); cursor: pointer; font: inherit; }
  .time:hover { background: var(--blue-soft); color: var(--blue); }
  .time.locked { color: var(--blue); }
  .time.locked::after { content: " •"; color: var(--yellow); }
  .time-input { padding: 1px 4px; font-size: 11.5px; font-family: var(--font-mono); width: 84px; }
  .dur { padding: 1px 4px; color: var(--muted); font-size: 10.5px; }
  .dur.fast { color: var(--warn); }
  .body { position: relative; min-width: 0; }
  .text { font-family: "Noto Sans", sans-serif; font-weight: 500; font-size: 14px; line-height: 1.38; cursor: text; padding: 0 30px 0 2px; user-select: text; -webkit-user-select: text; outline: none; }
  .text :global(mark) { background: #cfe0ff; color: inherit; border-radius: 2px; }
  .line { white-space: pre; }
  .unsure { text-decoration: underline dotted #c27c00; text-decoration-thickness: 2px; text-underline-offset: 3px; }
  textarea { font-family: "Noto Sans", sans-serif; font-size: 14.5px; line-height: 1.38; resize: vertical; padding: 4px 6px; }
  .hint { font-size: 11px; color: var(--muted); margin-top: 4px; }
  .flags { position: absolute; right: 0; top: 0; display: flex; gap: 4px; }
  .flag { display: inline-grid; place-items: center; min-width: 20px; height: 18px; padding: 0 4px; border-radius: 4px; font-size: 10.5px; }
  .flag.bad { background: var(--danger-soft); color: var(--danger); }
  .flag.warn { background: var(--warn-soft); color: var(--warn); }
  .row-actions { position: absolute; right: 8px; bottom: 4px; display: none; background: rgba(255, 255, 255, 0.95); border-radius: 7px; box-shadow: var(--shadow); padding: 1px; }
  .cue:hover .row-actions { display: flex; }
  .cue.editing .row-actions { display: none; }
  .row-actions .icon-btn { width: 26px; height: 26px; }
  .legend { padding: 7px 12px; border-top: 1px solid var(--line); font-size: 12px; color: var(--muted); }
</style>
