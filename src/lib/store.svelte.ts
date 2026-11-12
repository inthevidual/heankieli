import { api, errorText, on, type DownloadEvent, type ProgressEvent, type SegmentEvent } from "./api";
import { buildCues, wordsFromSegments } from "./layout";
import { cloneTemplate, TEMPLATES } from "./presets";
import { FORMATS, outputPath, render, type Format } from "./formats";
import { langFromLocale, setUiLang, t, type UiLang } from "./i18n.svelte";
import type { AppInfo, Cue, Lang, Look, LayoutRules, MediaInfo, ModelStatus, Quality, Word } from "./types";
import { exists, writeTextFile } from "@tauri-apps/plugin-fs";
import { ask } from "@tauri-apps/plugin-dialog";

export type Status = "waiting" | "downloading" | "transcribing" | "done" | "error" | "cancelled";

const MAX_UNDO = 200;

export class Item {
  id = crypto.randomUUID();
  path: string;
  name: string;
  info = $state<MediaInfo | null>(null);
  lang = $state<Lang>("sv");
  quality = $state<Quality>("accurate");
  status = $state<Status>("waiting");
  stage = $state<string>("");
  progress = $state(0);
  error = $state<string | null>(null);
  live = $state<string[]>([]);
  words = $state.raw<Word[]>([]);
  modelName = $state("");
  elapsed = $state(0);
  restored = $state(false);
  peaks = $state.raw<Uint8Array | null>(null);
  mediaUrl = $state<string | null>(null);
  previewSrc = $state<string | null>(null);
  past: Word[][] = [];
  future: Word[][] = [];
  savedAt = $state(0);

  constructor(path: string, lang: Lang, quality: Quality) {
    this.path = path;
    this.name = path.split(/[\\/]/).pop() ?? path;
    this.lang = lang;
    this.quality = quality;
  }

  get modelId() {
    return `${this.lang}-${this.quality}`;
  }
}

interface Settings {
  lang: Lang;
  quality: Quality;
  prompt: string;
  templateId: string;
  rules: LayoutRules;
  look: Look;
  formats: Format[];
  outDir: string | null;
  autoSave: boolean;
  follow: boolean;
  uiLang: "auto" | UiLang;
}

const SETTINGS_KEY = "heankieli.settings.v5";

function loadSettings(): Settings {
  const base = cloneTemplate("svt");
  const defaults: Settings = {
    lang: "sv",
    quality: "accurate",
    prompt: "",
    templateId: "svt",
    rules: base.rules,
    look: base.look,
    formats: ["srt"],
    outDir: null,
    autoSave: false,
    follow: true,
    uiLang: "auto",
  };
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (raw) {
      const s = JSON.parse(raw);
      return { ...defaults, ...s, rules: { ...defaults.rules, ...s.rules }, look: { ...defaults.look, ...s.look } };
    }
  } catch {
    /* fresh start */
  }
  return defaults;
}

class Store {
  info = $state<AppInfo | null>(null);
  items = $state<Item[]>([]);
  selectedId = $state<string | null>(null);
  models = $state<ModelStatus[]>([]);
  downloads = $state<Record<string, DownloadEvent>>({});
  settings = $state<Settings>(loadSettings());
  running = $state(false);
  stopRequested = false;
  currentTime = $state(0);
  seekRequest = $state<{ t: number; n: number } | null>(null);
  showModels = $state(false);
  toast = $state<{ text: string; action?: { label: string; run: () => void } } | null>(null);
  private toastTimer: ReturnType<typeof setTimeout> | undefined;

  selected = $derived(this.items.find((i) => i.id === this.selectedId) ?? null);
  fps = $derived(this.selected?.info?.hasVideo ? this.selected.info.fps : 25);
  cues: Cue[] = $derived(this.selected ? buildCues(this.selected.words, this.settings.rules, this.fps) : []);

  async init() {
    this.info = await api.appInfo();
    this.applyUiLang();
    if (!localStorage.getItem(SETTINGS_KEY)) {
      // first run: transcribe in the language the interface speaks
      this.settings.lang = langFromLocale(this.info.locale) === "sv" ? "sv" : "en";
    }
    await this.refreshModels();
    on<DownloadEvent>("model-download", (e) => {
      this.downloads[e.id] = e;
      if (e.state === "done" || e.state === "cancelled" || e.state === "error") this.refreshModels();
    });
    on<ProgressEvent>("transcribe-progress", (e) => {
      const it = this.items.find((i) => i.id === e.jobId);
      if (it) {
        it.stage = e.stage;
        it.progress = e.progress;
      }
    });
    on<SegmentEvent>("transcribe-segment", (e) => {
      const it = this.items.find((i) => i.id === e.jobId);
      if (it && e.text) it.live = [...it.live.slice(-200), e.text];
    });
    const st = await api.startup();
    if (st.ui) setUiLang(st.ui); // for this run only
    if (st.lang) this.settings.lang = st.lang;
    if (st.quality) this.settings.quality = st.quality;
    if (st.files.length) {
      await this.addFiles(st.files);
      if (st.at !== null) setTimeout(() => this.seek(st.at!), 1500);
      if (st.start) {
        // wait until the files have been read
        const t0 = Date.now();
        while (this.items.some((i) => !i.info && i.status !== "error") && Date.now() - t0 < 10000) await new Promise((r) => setTimeout(r, 100));
        this.start();
      }
    }
    $effect.root(() => {
      $effect(() => {
        try {
          localStorage.setItem(SETTINGS_KEY, JSON.stringify(this.settings));
        } catch {
          /* storage unavailable */
        }
      });
    });
  }

  applyUiLang() {
    const pref = this.settings.uiLang;
    setUiLang(pref === "auto" ? langFromLocale(this.info?.locale ?? navigator.language) : pref);
  }

  async refreshModels() {
    this.models = await api.listModels();
  }

  notify(text: string, action?: { label: string; run: () => void }) {
    this.toast = { text, action };
    clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => (this.toast = null), action ? 8000 : 4000);
  }

  // ---------------------------------------------------------------- files

  async addFiles(paths: string[]) {
    let first: Item | null = null;
    for (const path of paths) {
      if (this.items.some((i) => i.path === path)) continue;
      const it = new Item(path, this.settings.lang, this.settings.quality);
      this.items.push(it);
      const added = this.items[this.items.length - 1];
      first ??= added;
      this.prepare(added);
    }
    if (first && !this.selected) this.selectedId = first.id;
  }

  private async prepare(it: Item) {
    try {
      it.info = await api.probe(it.path);
    } catch (e) {
      it.status = "error";
      it.error = errorText(e);
      return;
    }
    api.mediaUrl(it.path).then((u) => (it.mediaUrl = u));
    try {
      const saved = await api.loadProject(it.path);
      if (saved) {
        const p = JSON.parse(saved);
        if (Array.isArray(p.words)) {
          it.words = p.words;
          it.lang = p.lang ?? it.lang;
          it.quality = p.quality ?? it.quality;
          it.modelName = p.modelName ?? "";
          it.status = "done";
          it.restored = true;
        }
      }
    } catch {
      /* ignore broken project files */
    }
    api.waveform(it.path).then((p) => (it.peaks = p)).catch(() => {});
  }

  remove(it: Item) {
    if (it.status === "transcribing" || it.status === "downloading") return;
    const idx = this.items.indexOf(it);
    this.items.splice(idx, 1);
    if (this.selectedId === it.id) this.selectedId = this.items[Math.min(idx, this.items.length - 1)]?.id ?? null;
  }

  select(it: Item) {
    this.selectedId = it.id;
  }

  // ---------------------------------------------------------------- queue

  pending() {
    return this.items.filter((i) => i.status === "waiting" && i.info);
  }

  async start(items?: Item[]) {
    if (items) for (const it of items) if (it.status !== "transcribing") (it.status = "waiting"), (it.error = null);
    if (this.running) return;
    this.running = true;
    this.stopRequested = false;
    try {
      while (!this.stopRequested) {
        const it = this.items.find((i) => i.status === "waiting" && i.info);
        if (!it) break;
        await this.runOne(it);
      }
    } finally {
      this.running = false;
    }
  }

  async stop() {
    this.stopRequested = true;
    for (const id of Object.keys(this.downloads)) if (this.downloads[id].state === "downloading") api.cancelDownload(id);
    await api.cancelTranscription();
  }

  private async ensureModel(id: string): Promise<boolean> {
    const m = this.models.find((x) => x.id === id);
    if (m?.installed) return true;
    try {
      const ok = await api.downloadModel(id);
      await this.refreshModels();
      return ok;
    } catch (e) {
      // someone else (the models dialog) may be downloading it already
      if (errorText(e).includes("Already")) {
        while (this.downloads[id]?.state === "downloading" || this.downloads[id]?.state === "verifying") {
          await new Promise((r) => setTimeout(r, 500));
        }
        await this.refreshModels();
        return !!this.models.find((x) => x.id === id)?.installed;
      }
      throw e;
    }
  }

  private async runOne(it: Item) {
    it.error = null;
    it.live = [];
    it.progress = 0;
    try {
      it.status = "downloading";
      it.stage = "downloading";
      if (!(await this.ensureModel("vad")) || !(await this.ensureModel(it.modelId))) {
        it.status = "cancelled";
        return;
      }
      it.status = "transcribing";
      it.stage = "decoding";
      const tr = await api.transcribe({
        jobId: it.id,
        path: it.path,
        modelId: it.modelId,
        prompt: this.settings.prompt.trim() || undefined,
      });
      it.past = it.words.length ? [...it.past, it.words].slice(-MAX_UNDO) : it.past;
      it.future = [];
      it.words = wordsFromSegments(tr.segments);
      it.modelName = tr.modelName;
      it.elapsed = tr.elapsed;
      it.status = "done";
      it.restored = false;
      this.persist(it);
      if (!this.selected) this.selectedId = it.id;
      if (this.settings.autoSave) await this.exportItem(it, true);
    } catch (e) {
      const msg = errorText(e);
      if (msg === "cancelled" || this.stopRequested) {
        it.status = it.words.length ? "done" : "cancelled";
      } else {
        it.status = "error";
        it.error = msg;
      }
    }
  }

  // ---------------------------------------------------------------- editing

  private saveTimers = new Map<string, ReturnType<typeof setTimeout>>();

  persist(it: Item) {
    clearTimeout(this.saveTimers.get(it.id));
    this.saveTimers.set(
      it.id,
      setTimeout(() => {
        const data = JSON.stringify({
          version: 1,
          lang: it.lang,
          quality: it.quality,
          modelName: it.modelName,
          words: it.words,
        });
        api.saveProject(it.path, data).then(() => (it.savedAt = Date.now()));
      }, 400),
    );
  }

  edit(fn: (w: Word[]) => Word[]) {
    const it = this.selected;
    if (!it) return;
    const next = fn(it.words);
    if (next === it.words) return;
    it.past = [...it.past, it.words].slice(-MAX_UNDO);
    it.future = [];
    it.words = next;
    this.persist(it);
  }

  undo() {
    const it = this.selected;
    if (!it || !it.past.length) return;
    it.future = [it.words, ...it.future];
    it.words = it.past[it.past.length - 1];
    it.past = it.past.slice(0, -1);
    this.persist(it);
  }

  redo() {
    const it = this.selected;
    if (!it || !it.future.length) return;
    it.past = [...it.past, it.words];
    it.words = it.future[0];
    it.future = it.future.slice(1);
    this.persist(it);
  }

  seek(t: number) {
    this.seekRequest = { t: Math.max(0, t), n: (this.seekRequest?.n ?? 0) + 1 };
  }

  // ---------------------------------------------------------------- style

  applyTemplate(id: string) {
    const tpl = cloneTemplate(id);
    this.settings.templateId = id;
    this.settings.rules = tpl.rules;
    this.settings.look = tpl.look;
  }

  get templateChanged() {
    const tpl = TEMPLATES.find((x) => x.id === this.settings.templateId);
    if (!tpl) return true;
    return JSON.stringify(tpl.rules) !== JSON.stringify(this.settings.rules) || JSON.stringify(tpl.look) !== JSON.stringify(this.settings.look);
  }

  // ---------------------------------------------------------------- export

  async exportItem(it: Item, quiet = false): Promise<string[]> {
    if (it.status !== "done") return [];
    const fps = it.info?.hasVideo ? it.info.fps : 25;
    const cues = buildCues(it.words, this.settings.rules, fps);
    const files = this.settings.formats.map((f) => ({
      path: outputPath(it.path, f, it.lang, this.settings.outDir ?? undefined),
      text: render(f, cues, it.words),
    }));
    if (!quiet) {
      const existing: string[] = [];
      for (const f of files) if (await exists(f.path)) existing.push(f.path.split(/[\\/]/).pop()!);
      if (existing.length && !(await ask(t("overwrite", { files: existing.join(", ") }), { kind: "warning" }))) return [];
    }
    for (const f of files) await writeTextFile(f.path, f.text);
    return files.map((f) => f.path);
  }

  async exportMany(items: Item[]) {
    const written: string[] = [];
    try {
      for (const it of items) written.push(...(await this.exportItem(it)));
    } catch (e) {
      this.notify(errorText(e));
      return;
    }
    if (written.length) {
      const last = written[written.length - 1];
      this.notify(t("saved", { n: written.length }), {
        label: t("showInFolder"),
        run: async () => {
          const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
          revealItemInDir(last);
        },
      });
    }
  }
}

export const store = new Store();
export { FORMATS };
