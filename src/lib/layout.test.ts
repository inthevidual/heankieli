import { describe, expect, it } from "vitest";
import { buildCues, insertAt, layoutLines, len, mergeRange, replaceText, splitAt, wordsFromSegments } from "./layout";
import { parseClock, toSrt, toText, toTimestamped, toVtt, outputPath } from "./formats";
import { DEFAULT_TEMPLATE } from "./presets";
import type { Word } from "./types";

const rules = { ...DEFAULT_TEMPLATE.rules, snapFrames: false };

/** Words spoken evenly at `rate` words/second from t0. */
function speak(text: string, t0 = 0, rate = 2.6): Word[] {
  return text.split(" ").map((t, k) => ({ text: t, start: t0 + k / rate, end: t0 + (k + 0.85) / rate, p: 0.9 }));
}

describe("layoutLines", () => {
  it("keeps a short cue on one line", () => {
    const w = speak("Hej och välkommen hit.");
    expect(layoutLines(w, 0, w.length, rules).lines).toEqual(["Hej och välkommen hit."]);
  });

  it("breaks two lines within the limit, not after a function word", () => {
    const w = speak("Det här är en mening som är lite för lång för en enda rad.");
    const lay = layoutLines(w, 0, w.length, rules);
    expect(lay.fits).toBe(true);
    expect(lay.lines).toHaveLength(2);
    for (const l of lay.lines) expect(len(l)).toBeLessThanOrEqual(37);
    const lastOfTop = lay.lines[0].split(" ").pop()!.toLowerCase();
    expect(["en", "som", "är", "för", "det"]).not.toContain(lastOfTop);
  });

  it("prefers the line break at a comma", () => {
    const w = speak("Vi åkte till stugan i fredags, och sedan regnade det hela helgen.");
    const lay = layoutLines(w, 0, w.length, rules);
    expect(lay.lines[0].endsWith(",")).toBe(true);
  });

  it("honours forced new lines", () => {
    const w = speak("– Kommer du? – Ja, strax.");
    w[3].nl = true;
    expect(layoutLines(w, 0, w.length, rules).lines).toEqual(["– Kommer du?", "– Ja, strax."]);
  });
});

describe("buildCues", () => {
  const text =
    "Det här är ett test av undertextningen. Vi vill se att meningarna delas på ett vettigt sätt, " +
    "att raderna inte blir för långa och att varje undertext står kvar tillräckligt länge för att hinna läsas. " +
    "Sedan kommer en paus.";
  const words = speak(text);
  words.push(...speak("Och här fortsätter det efter pausen.", words[words.length - 1].end + 3));

  const cues = buildCues(words, rules);

  it("covers every word exactly once, in order", () => {
    let k = 0;
    for (const c of cues) {
      expect(c.from).toBe(k);
      k = c.to;
    }
    expect(k).toBe(words.length);
  });

  it("respects line length and line count", () => {
    for (const c of cues) {
      expect(c.lines.length).toBeLessThanOrEqual(2);
      for (const l of c.lines) expect(len(l)).toBeLessThanOrEqual(37);
    }
  });

  it("never overlaps and keeps a gap", () => {
    for (let i = 1; i < cues.length; i++) expect(cues[i].start - cues[i - 1].end).toBeGreaterThanOrEqual(rules.minGap - 1e-9);
  });

  it("ends a subtitle at a long pause", () => {
    const pauseWord = words.findIndex((w) => w.text === "Och");
    expect(cues.some((c) => c.from === pauseWord)).toBe(true);
  });

  it("gives subtitles at least the minimum duration when there is room", () => {
    const last = cues[cues.length - 1];
    expect(last.end - last.start).toBeGreaterThanOrEqual(rules.minDuration - 1e-9);
  });

  it("snaps to frames", () => {
    const c = buildCues(words, { ...rules, snapFrames: true }, 25);
    for (const q of c) {
      expect(Math.abs(q.start * 25 - Math.round(q.start * 25))).toBeLessThan(1e-6);
      expect(Math.abs(q.end * 25 - Math.round(q.end * 25))).toBeLessThan(1e-6);
    }
  });

  it("re-flows with other rules", () => {
    const one = buildCues(words, { ...rules, maxLines: 1, maxChars: 42 });
    for (const c of one) expect(c.lines).toHaveLength(1);
    expect(one.length).toBeGreaterThan(cues.length);
  });
});

describe("edits survive a re-flow", () => {
  const words = speak("Ett två tre fyra fem sex sju åtta nio tio elva tolv tretton fjorton femton sexton sjutton arton nitton tjugo.");

  it("split", () => {
    const w = splitAt(words, 3);
    for (const r of [rules, { ...rules, maxChars: 60 }]) expect(buildCues(w, r).some((c) => c.from === 3)).toBe(true);
  });

  it("merge", () => {
    const cues = buildCues(words, { ...rules, maxChars: 20 });
    expect(cues.length).toBeGreaterThan(1);
    const w = mergeRange(words, cues[0].from, cues[1].to);
    const after = buildCues(w, { ...rules, maxChars: 20 });
    expect(after[0].to).toBe(cues[1].to);
  });

  it("text edit keeps timing when the word count is the same", () => {
    const w = replaceText(words, 0, 3, "En två tre");
    expect(w[0].text).toBe("En");
    expect(w[0].start).toBe(words[0].start);
    expect(w[0].p).toBe(1);
    expect(w[1].p).toBe(words[1].p);
  });

  it("text edit spreads new words over the old span", () => {
    const w = replaceText(words, 0, 2, "Ett, två och\ntre");
    expect(w.slice(0, 4).map((x) => x.text)).toEqual(["Ett,", "två", "och", "tre"]);
    expect(w[0].start).toBe(words[0].start);
    expect(w[3].end).toBeCloseTo(words[1].end);
    expect(w[3].nl).toBe(true);
    expect(w.length).toBe(words.length + 2);
  });

  it("empty text deletes", () => {
    expect(replaceText(words, 0, 2, "  ").length).toBe(words.length - 2);
  });

  it("inserted subtitle stands alone at its time", () => {
    const [w, i] = insertAt(words, 100, "Ny text här");
    const cues = buildCues(w, rules);
    const c = cues.find((q) => q.from === i)!;
    expect(c.lines.join(" ")).toBe("Ny text här");
    expect(c.start).toBe(100);
  });
});

describe("formats", () => {
  const words = wordsFromSegments([
    { words: [{ text: " Hej", start: 1, end: 1.4, p: 1 }, { text: "där.", start: 1.5, end: 2, p: 1 }] },
    { words: [{ text: "Hur", start: 5, end: 5.3, p: 1 }, { text: "mår", start: 5.4, end: 5.7, p: 1 }, { text: "du?", start: 5.8, end: 6.1, p: 1 }] },
  ]);
  const cues = buildCues(words, rules);

  it("srt", () => {
    expect(toSrt(cues)).toBe("1\n00:00:01,000 --> 00:00:02,500\nHej där.\n\n2\n00:00:05,000 --> 00:00:06,500\nHur mår du?\n");
  });
  it("vtt", () => {
    expect(toVtt(cues).startsWith("WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.500\n")).toBe(true);
  });
  it("text and timestamps", () => {
    expect(toText(words)).toBe("Hej där.\n\nHur mår du?\n");
    expect(toTimestamped(words)).toBe("[00:00:01] Hej där.\n[00:00:05] Hur mår du?\n");
  });
  it("clock parsing", () => {
    expect(parseClock("1:02.5")).toBe(62.5);
    expect(parseClock("01:00:01,250")).toBe(3601.25);
    expect(parseClock("x")).toBeNull();
  });
  it("output path", () => {
    expect(outputPath("/a/b/film.mp4", "srt", "sv")).toBe("/a/b/film.sv.srt");
    expect(outputPath("/a/b/film.mp4", "tstxt", "en")).toBe("/a/b/film.en.timestamps.txt");
  });
});
