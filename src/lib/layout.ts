// Turns timed words into subtitles.
//
// The transcript is kept as a flat list of timed words. Subtitles (cues) and
// their line breaks are always computed from the words and the layout rules,
// so changing the rules (characters per line, lines, durations...) re-flows
// everything. Manual edits are stored on the words themselves as flags
// (split here, keep together, new line here, exact start/end), so they
// survive a re-flow.

import type { Cue, LayoutRules, Word } from "./types";

const FUNCTION_WORDS = new Set(
  [
    // Swedish: articles, prepositions, conjunctions, pronouns that lean on the next word
    "och", "att", "en", "ett", "i", "på", "av", "för", "med", "till", "om", "som", "den", "det", "de",
    "är", "var", "har", "hade", "inte", "men", "så", "från", "vid", "under", "över", "efter", "utan",
    "mot", "hos", "eller", "när", "där", "vi", "jag", "du", "han", "hon", "man", "ni", "sig", "sin",
    "sitt", "sina", "min", "mitt", "mina", "din", "ditt", "dina", "vår", "vårt", "våra", "er", "ert",
    "era", "hans", "hennes", "deras", "dess", "denna", "detta", "dessa", "vilken", "vilket", "vilka",
    "kan", "ska", "skulle", "vill", "måste", "får", "fick", "blir", "blev", "bli", "kommer", "mycket",
    "lite", "mer", "mest", "inom", "genom", "mellan", "bland", "kring", "än", "både", "varken", "ju",
    // English
    "the", "a", "an", "of", "to", "in", "on", "at", "for", "with", "and", "or", "but", "nor", "by",
    "from", "as", "is", "are", "was", "were", "be", "been", "i", "you", "he", "she", "we", "they", "it",
    "my", "your", "his", "her", "our", "their", "its", "this", "that", "these", "those", "not", "no",
    "very", "so", "if", "than", "into", "about", "can", "could", "will", "would", "should", "must",
    "has", "have", "had", "do", "does", "did", "what", "which", "who", "whose", "whom", "where", "when",
    "some", "any", "every", "each", "all", "more", "most", "such", "own", "just", "also", "through",
    "between", "over", "under", "after", "before", "without", "within", "upon", "onto", "per", "via",
    "i'm", "you're", "we're", "they're", "it's", "that's", "there's", "don't", "doesn't", "didn't",
  ],
);

export const len = (s: string) => [...s].length;

const SENTENCE_END = /[.!?…]["'»”’)\]]*$/;
const CLAUSE_END = /[,;:–—]["'»”’)\]]*$/;
const DIALOGUE_START = /^[–—-]/;

function bare(text: string) {
  return text.toLowerCase().replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}']+$/gu, "");
}

/** How bad it is to break (a line or a subtitle) before word i. */
export function breakCost(words: Word[], i: number, forCue: boolean): number {
  if (i <= 0 || i >= words.length) return 0;
  const prev = words[i - 1].text;
  const cur = words[i];
  let c = 10;
  if (SENTENCE_END.test(prev)) c = 0;
  else if (CLAUSE_END.test(prev) || DIALOGUE_START.test(cur.text)) c = 3;
  else if (forCue && cur.seg) c = 5;
  if (FUNCTION_WORDS.has(bare(prev))) c += 12;
  if (forCue) {
    const gap = cur.start - words[i - 1].end;
    c -= Math.min(7, Math.max(0, gap) * 10);
  }
  return Math.max(0, c);
}

export function joinWords(words: Word[], from: number, to: number): string {
  let s = "";
  for (let k = from; k < to; k++) s += (k > from ? " " : "") + words[k].text;
  return s;
}

interface LineLayout {
  lines: string[];
  cost: number;
  fits: boolean;
}

/** Best way to break words[from..to) into at most rules.maxLines lines. */
export function layoutLines(words: Word[], from: number, to: number, rules: LayoutRules): LineLayout {
  const { maxChars, maxLines } = rules;
  // Forced new lines split the cue into blocks that are laid out on their own.
  const blocks: [number, number][] = [];
  let s = from;
  for (let k = from + 1; k < to; k++) {
    if (words[k].nl) {
      blocks.push([s, k]);
      s = k;
    }
  }
  blocks.push([s, to]);

  if (blocks.length > 1) {
    const lines: string[] = [];
    let cost = 0;
    let fits = true;
    const spare = Math.max(1, maxLines - blocks.length + 1);
    for (const [a, b] of blocks) {
      const sub = layoutLines(words.map((w, k) => (k === a ? { ...w, nl: false } : w)), a, b, { ...rules, maxLines: spare });
      lines.push(...sub.lines);
      cost += sub.cost;
      fits &&= sub.fits;
    }
    return { lines, cost, fits: fits && lines.length <= maxLines };
  }

  const whole = joinWords(words, from, to);
  if (len(whole) <= maxChars) return { lines: [whole], cost: 0, fits: true };

  // Enumerate break positions for 2..maxLines lines (cues are short).
  let best: LineLayout | null = null;
  const lens: number[] = [];
  for (let k = from; k < to; k++) lens.push(len(words[k].text));
  const lineLen = (a: number, b: number) => {
    let n = b - a - 1;
    for (let k = a; k < b; k++) n += lens[k - from];
    return n;
  };

  const tryBreaks = (breaks: number[]) => {
    const bounds = [from, ...breaks, to];
    const ls: number[] = [];
    for (let k = 0; k < bounds.length - 1; k++) {
      const l = lineLen(bounds[k], bounds[k + 1]);
      if (l > maxChars) return;
      ls.push(l);
    }
    let cost = 0;
    for (const b of breaks) cost += breakCost(words, b, false) * 0.8;
    if (ls.length === 2) {
      const diff = ls[0] - ls[1];
      if (rules.balance === "bottom") cost += diff > 0 ? diff * 0.6 : -diff * 0.12;
      else if (rules.balance === "top") cost += diff < 0 ? -diff * 0.6 : diff * 0.12;
      else cost += Math.abs(diff) * 0.35;
    } else {
      const avg = ls.reduce((a, b) => a + b, 0) / ls.length;
      for (const l of ls) cost += ((l - avg) * (l - avg)) / 20;
    }
    cost += (ls.length - 2) * 4;
    // a lonely word on a line of its own
    for (const l of ls) if (l < Math.max(8, maxChars * 0.25)) cost += 9;
    if (!best || cost < best.cost) {
      best = { lines: [], cost, fits: true };
      for (let k = 0; k < bounds.length - 1; k++) best.lines.push(joinWords(words, bounds[k], bounds[k + 1]));
    }
  };

  for (let a = from + 1; maxLines >= 2 && a < to; a++) {
    tryBreaks([a]);
    if (maxLines >= 3) for (let b = a + 1; b < to; b++) tryBreaks([a, b]);
  }
  if (best) return best;

  // Does not fit: greedy fill, flagged as overflow.
  const lines: string[] = [];
  let line = "";
  for (let k = from; k < to; k++) {
    const t = words[k].text;
    if (line && len(line) + 1 + len(t) > maxChars) {
      lines.push(line);
      line = t;
    } else line = line ? line + " " + t : t;
  }
  if (line) lines.push(line);
  return { lines, cost: 1000, fits: false };
}

/** Cost of a sentence ending inside a subtitle, other than at its line break. */
export let SENTENCE_INSIDE = 14;
export function setSentenceInside(v: number) { SENTENCE_INSIDE = v; }

/** Pauses longer than this always end a subtitle. */
const HARD_PAUSE = 2.0;

/** Split the words into cues. Returns [from, to) index pairs. */
export function segment(words: Word[], rules: LayoutRules): [number, number][] {
  const n = words.length;
  if (n === 0) return [];
  // allowed cue starts
  const canStart = (i: number) => i === 0 || !words[i].join;
  const mustStart = (i: number) => i === 0 || (!words[i].join && (!!words[i].br || words[i].start - words[i - 1].end > HARD_PAUSE));

  const best = new Array<number>(n + 1).fill(Infinity);
  const prev = new Array<number>(n + 1).fill(-1);
  best[0] = 0;
  const maxWords = Math.max(8, Math.ceil((rules.maxChars * rules.maxLines) / 2) + 4);

  for (let i = 0; i < n; i++) {
    if (best[i] === Infinity || !canStart(i)) continue;
    let atoms = 0;
    for (let j = i + 1; j <= n; j++) {
      if (j > i + 1 && mustStart(j - 1)) break; // a forced start inside the cue
      if (j < n && !canStart(j)) continue; // cannot end before a joined word
      atoms++;
      const single = atoms === 1;
      if (!single && j - i > maxWords) break;
      const dur = words[j - 1].end - words[i].start;
      if (!single && dur > rules.maxDuration) break;
      const lay = layoutLines(words, i, j, rules);
      if (!lay.fits && !single) break;

      let cost = 10 + lay.cost + (j < n ? breakCost(words, j, true) : 0);
      // reading speed: time available until the next word (plus a little linger)
      const chars = lay.lines.reduce((a, l) => a + len(l), 0);
      const until = j < n ? Math.min(words[j].start, words[j - 1].end + rules.linger) : words[j - 1].end + rules.linger;
      const avail = Math.max(0.2, until - words[i].start);
      const need = chars / rules.maxCps;
      if (need > avail) cost += (need - avail) * 8;
      // very short subtitles flash by
      if (j < n && dur < rules.minDuration * 0.5 && breakCost(words, j, true) > 0) cost += 6;
      if (chars < rules.maxChars * 0.3 && j < n && breakCost(words, j, true) > 2) cost += 4;
      // a lone short word as a whole subtitle, with speech right before or after
      if (chars < 10) {
        const gapBefore = i > 0 ? words[i].start - words[i - 1].end : 9;
        const gapAfter = j < n ? words[j].start - words[j - 1].end : 9;
        if (Math.min(gapBefore, gapAfter) < 0.6) cost += 10;
      }
      // a sentence ending inside the cue, but not at its line break
      for (let k = i + 1; k < j; k++) {
        if (SENTENCE_END.test(words[k - 1].text)) {
          const atLineBreak = lay.lines.length > 1 && joinWords(words, i, k) === lay.lines[0];
          if (!atLineBreak) cost += SENTENCE_INSIDE;
        }
      }
      const total = best[i] + cost;
      if (total < best[j]) {
        best[j] = total;
        prev[j] = i;
      }
    }
  }

  const out: [number, number][] = [];
  let j = n;
  while (j > 0) {
    let i = prev[j];
    if (i < 0) i = j - 1; // should not happen, fail safe
    out.push([i, j]);
    j = i;
  }
  return out.reverse();
}

export function snap(t: number, fps: number | undefined): number {
  if (!fps || fps <= 0) return Math.round(t * 1000) / 1000;
  return Math.round(t * fps) / fps;
}

/** Compute the cues: segmentation, line breaks and timing. */
export function buildCues(words: Word[], rules: LayoutRules, fps?: number): Cue[] {
  const spans = segment(words, rules);
  const cues: Cue[] = spans.map(([from, to], index) => {
    const lay = layoutLines(words, from, to, rules);
    const first = words[from];
    const last = words[to - 1];
    return {
      index,
      from,
      to,
      start: first.cueStart ?? first.start,
      end: last.cueEnd ?? last.end,
      lines: lay.lines,
      overflow: !lay.fits || last.end - first.start > rules.maxDuration + 0.5,
      cps: 0,
    };
  });

  const frame = fps && rules.snapFrames ? 1 / fps : 0.001;
  const gap = Math.max(rules.minGap, frame);

  for (let k = 0; k < cues.length; k++) {
    const c = cues[k];
    const next = cues[k + 1];
    const lockedEnd = words[c.to - 1].cueEnd !== undefined;
    const chars = c.lines.reduce((a, l) => a + len(l), 0);
    if (!lockedEnd) {
      const need = Math.max(rules.minDuration, chars / rules.maxCps);
      let target = Math.max(c.end + Math.min(rules.linger, 0.3), c.start + need);
      target = Math.min(target, Math.max(c.end, c.start + rules.maxDuration));
      if (next) {
        const nextStart = next.start;
        target = Math.min(target, nextStart - gap);
        // close short gaps so subtitles do not flicker off and on
        if (nextStart - target < 0.5 && nextStart - c.end < 1.0) target = nextStart - gap;
      }
      c.end = Math.max(target, c.start + frame);
    } else if (next && c.end > next.start - gap && words[next.from].cueStart === undefined) {
      next.start = c.end + gap;
    }
    if (next && c.end > next.start - gap) c.end = Math.max(c.start + frame, next.start - gap);
  }

  for (const c of cues) {
    if (fps && rules.snapFrames) {
      c.start = snap(c.start, fps);
      c.end = Math.max(c.start + 1 / fps, snap(c.end, fps));
    }
    const chars = c.lines.reduce((a, l) => a + len(l), 0);
    c.cps = chars / Math.max(0.1, c.end - c.start);
  }
  return cues;
}

// ---------------------------------------------------------------------------
// Edits. Each returns a new word list.

function tokens(text: string): { text: string; nl: boolean }[] {
  const out: { text: string; nl: boolean }[] = [];
  const lines = text.split(/\n/);
  lines.forEach((line, li) => {
    line
      .split(/\s+/)
      .filter(Boolean)
      .forEach((t, ti) => out.push({ text: t, nl: li > 0 && ti === 0 }));
  });
  return out;
}

/** Replace the text of words[from..to), keeping timing where it can. */
export function replaceText(words: Word[], from: number, to: number, text: string): Word[] {
  const toks = tokens(text);
  const old = words.slice(from, to);
  if (toks.length === 0) return [...words.slice(0, from), ...words.slice(to)];

  const first = old[0];
  const last = old[old.length - 1];
  let fresh: Word[];
  if (toks.length === old.length) {
    fresh = old.map((w, k) => ({
      ...w,
      text: toks[k].text,
      nl: toks[k].nl || undefined,
      p: w.text === toks[k].text ? w.p : 1,
    }));
  } else {
    // spread the words over the old time span by length
    const t0 = first.start;
    const t1 = Math.max(last.end, t0 + 0.1);
    const weights = toks.map((t) => len(t.text) + 1);
    const total = weights.reduce((a, b) => a + b, 0);
    let acc = 0;
    fresh = toks.map((t, k) => {
      const s = t0 + ((t1 - t0) * acc) / total;
      acc += weights[k];
      const e = t0 + ((t1 - t0) * acc) / total;
      const same = old.find((w) => w.text === t.text);
      return { text: t.text, start: s, end: e, p: same ? same.p : 1, nl: t.nl || undefined };
    });
    // keep the flags that belong to the ends
    fresh[0] = { ...fresh[0], seg: first.seg, br: first.br, join: first.join, cueStart: first.cueStart };
    fresh[fresh.length - 1] = { ...fresh[fresh.length - 1], cueEnd: last.cueEnd };
    // a manually merged cue stays merged
    if (old.slice(1).every((w) => w.join)) for (let k = 1; k < fresh.length; k++) fresh[k].join = true;
  }
  fresh[0].nl = undefined;
  return [...words.slice(0, from), ...fresh, ...words.slice(to)];
}

/** Start a new subtitle at word i. */
export function splitAt(words: Word[], i: number): Word[] {
  if (i <= 0 || i >= words.length) return words;
  return words.map((w, k) => (k === i ? { ...w, br: true, join: undefined, nl: undefined, cueStart: undefined } : w));
}

/** Merge two neighbouring cues into one, now and after re-flows. */
export function mergeRange(words: Word[], from: number, to: number): Word[] {
  return words.map((w, k) =>
    k > from && k < to ? { ...w, join: true, br: undefined, cueStart: undefined, cueEnd: k === to - 1 ? w.cueEnd : undefined } : w,
  );
}

/** Let the layout decide again: clear manual splits and merges in a range. */
export function unlockRange(words: Word[], from: number, to: number): Word[] {
  return words.map((w, k) =>
    k >= from && k < to ? { ...w, join: undefined, br: undefined, nl: undefined, cueStart: undefined, cueEnd: undefined } : w,
  );
}

export function setTiming(words: Word[], cue: Cue, start?: number, end?: number): Word[] {
  return words.map((w, k) => {
    if (k === cue.from && start !== undefined) w = { ...w, cueStart: Math.max(0, start) };
    if (k === cue.to - 1 && end !== undefined) w = { ...w, cueEnd: end };
    return w;
  });
}

export function deleteRange(words: Word[], from: number, to: number): Word[] {
  const out = [...words.slice(0, from), ...words.slice(to)];
  // the word after the gap starts a new sentence-like unit if the removed one did
  if (out[from] && words[from]?.seg) out[from] = { ...out[from], seg: true };
  return out;
}

/** Insert a new subtitle with `text` at time t. Returns the words and the index of its first word. */
export function insertAt(words: Word[], t: number, text: string, duration = 2): [Word[], number] {
  const toks = tokens(text);
  if (toks.length === 0) return [words, -1];
  let i = words.findIndex((w) => w.start >= t);
  if (i < 0) i = words.length;
  const nextStart = words[i]?.start ?? t + duration;
  const end = Math.max(t + 0.5, Math.min(t + duration, nextStart - 0.1));
  const step = (end - t) / toks.length;
  const fresh: Word[] = toks.map((tk, k) => ({
    text: tk.text,
    start: t + k * step,
    end: t + (k + 1) * step,
    p: 1,
    br: k === 0 ? true : undefined,
    join: k > 0 ? true : undefined,
    seg: k === 0 ? true : undefined,
    nl: tk.nl || undefined,
    cueStart: k === 0 ? t : undefined,
  }));
  fresh[fresh.length - 1].cueEnd = end;
  const out = [...words.slice(0, i), ...fresh, ...words.slice(i)];
  if (out[i + fresh.length]) out[i + fresh.length] = { ...out[i + fresh.length], br: true, join: undefined };
  return [out, i];
}

/** Words of a transcript, flagged at segment starts. */
export function wordsFromSegments(segments: { words: { text: string; start: number; end: number; p: number }[] }[]): Word[] {
  const out: Word[] = [];
  for (const s of segments) {
    s.words.forEach((w, k) => {
      if (!w.text.trim()) return;
      out.push({ text: w.text.trim(), start: w.start, end: w.end, p: w.p, seg: k === 0 ? true : undefined });
    });
  }
  // keep times monotonic
  for (let k = 1; k < out.length; k++) {
    if (out[k].start < out[k - 1].start) out[k].start = out[k - 1].start;
    if (out[k].end < out[k].start) out[k].end = out[k].start + 0.05;
  }
  return out;
}
