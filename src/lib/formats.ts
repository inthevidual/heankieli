import type { Cue, Word } from "./types";

function pad(n: number, w = 2) {
  return String(n).padStart(w, "0");
}

export function clock(t: number, sep: "," | "." = ",", withMs = true): string {
  const ms = Math.max(0, Math.round(t * 1000));
  const h = Math.floor(ms / 3600000);
  const m = Math.floor((ms % 3600000) / 60000);
  const s = Math.floor((ms % 60000) / 1000);
  const base = `${pad(h)}:${pad(m)}:${pad(s)}`;
  return withMs ? `${base}${sep}${pad(ms % 1000, 3)}` : base;
}

/** Short display clock: m:ss.d or h:mm:ss.d */
export function shortClock(t: number, decimals = 1): string {
  const neg = t < 0;
  t = Math.abs(t);
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const s = t % 60;
  const ss = s.toFixed(decimals).padStart(decimals ? 3 + decimals : 2, "0");
  return (neg ? "-" : "") + (h ? `${h}:${pad(m)}:${ss}` : `${m}:${ss}`);
}

export function parseClock(s: string): number | null {
  const m = s.trim().replace(",", ".").match(/^(?:(\d+):)?(?:(\d+):)?(\d+(?:\.\d+)?)$/);
  if (!m) return null;
  const parts = [m[1], m[2], m[3]].filter((x) => x !== undefined).map(Number);
  let t = 0;
  for (const p of parts) t = t * 60 + p;
  return Number.isFinite(t) ? t : null;
}

export function toSrt(cues: Cue[]): string {
  return (
    cues
      .map((c, i) => `${i + 1}\n${clock(c.start)} --> ${clock(c.end)}\n${c.lines.join("\n")}\n`)
      .join("\n")
  );
}

export function toVtt(cues: Cue[]): string {
  return (
    "WEBVTT\n\n" +
    cues.map((c, i) => `${i + 1}\n${clock(c.start, ".")} --> ${clock(c.end, ".")}\n${c.lines.join("\n")}\n`).join("\n")
  );
}

interface Paragraph {
  start: number;
  text: string;
}

const SENTENCE_END = /[.!?…]["'»”’)\]]*$/;

/** Running text in paragraphs: a new paragraph after a long pause, or when one gets long. */
export function paragraphs(words: Word[], pause = 1.6, maxChars = 600): Paragraph[] {
  const out: Paragraph[] = [];
  let cur: Paragraph | null = null;
  let curLen = 0;
  words.forEach((w, k) => {
    const prev = words[k - 1];
    const gap = prev ? w.start - prev.end : 0;
    const sentenceStart = !prev || SENTENCE_END.test(prev.text);
    const newPara = !cur || (sentenceStart && (gap >= pause || curLen > maxChars));
    if (newPara) {
      cur = { start: w.start, text: w.text };
      curLen = w.text.length;
      out.push(cur);
    } else {
      cur!.text += " " + w.text;
      curLen += w.text.length + 1;
    }
  });
  return out;
}

export function toText(words: Word[]): string {
  return paragraphs(words)
    .map((p) => p.text)
    .join("\n\n") + "\n";
}

/** One line per sentence-like segment: [hh:mm:ss] text */
export function toTimestamped(words: Word[]): string {
  const lines: string[] = [];
  let cur: { start: number; text: string } | null = null;
  words.forEach((w, k) => {
    const prev = words[k - 1];
    const sentenceStart = !prev || SENTENCE_END.test(prev.text);
    const gap = prev ? w.start - prev.end : 0;
    if (!cur || (sentenceStart && (w.seg || gap > 1.0 || cur.text.length > 160))) {
      if (cur) lines.push(`[${clock(cur.start, ",", false)}] ${cur.text}`);
      cur = { start: w.start, text: w.text };
    } else cur.text += " " + w.text;
  });
  if (cur) lines.push(`[${clock((cur as { start: number }).start, ",", false)}] ${(cur as { text: string }).text}`);
  return lines.join("\n") + "\n";
}

export type Format = "srt" | "vtt" | "txt" | "tstxt";

export const FORMATS: { id: Format; ext: string; suffix: string }[] = [
  { id: "srt", ext: "srt", suffix: "" },
  { id: "vtt", ext: "vtt", suffix: "" },
  { id: "txt", ext: "txt", suffix: "" },
  { id: "tstxt", ext: "txt", suffix: ".timestamps" },
];

export function render(format: Format, cues: Cue[], words: Word[]): string {
  switch (format) {
    case "srt":
      return toSrt(cues);
    case "vtt":
      return toVtt(cues);
    case "txt":
      return toText(words);
    case "tstxt":
      return toTimestamped(words);
  }
}

/** Output path next to the media file: film.mp4 -> film.sv.srt */
export function outputPath(mediaPath: string, format: Format, lang: string, dir?: string): string {
  const f = FORMATS.find((x) => x.id === format)!;
  const sep = mediaPath.includes("\\") && !mediaPath.includes("/") ? "\\" : "/";
  const slash = mediaPath.lastIndexOf(sep);
  const folder = dir ?? (slash >= 0 ? mediaPath.slice(0, slash) : ".");
  const name = slash >= 0 ? mediaPath.slice(slash + 1) : mediaPath;
  const dot = name.lastIndexOf(".");
  const stem = dot > 0 ? name.slice(0, dot) : name;
  return `${folder}${sep}${stem}.${lang}${f.suffix}.${f.ext}`;
}
