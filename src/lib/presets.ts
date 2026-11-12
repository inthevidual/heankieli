import type { Template } from "./types";

// The default follows how SVT Play shows subtitles: a condensed grotesque
// (Roboto Condensed has the closest letterforms) in white shading to light grey,
// with a soft shadow, on a dark see-through plate behind each line. The lines
// are left-aligned inside a block centred near the bottom. At most two lines of
// about 37 characters (the classic Swedish TV line), one line up for 2–4 s and
// two lines for 4–6 s.
export const TEMPLATES: Template[] = [
  {
    id: "svt",
    name: { en: "SVT Play style", sv: "SVT Play-stil" },
    rules: {
      maxChars: 37,
      maxLines: 2,
      minDuration: 1.5,
      maxDuration: 6,
      maxCps: 14,
      minGap: 0.12,
      linger: 0.5,
      balance: "bottom",
      snapFrames: true,
    },
    look: {
      font: "Roboto Condensed",
      weight: 470,
      width: 100,
      size: 5.6,
      color: "#ffffff",
      background: "box",
      boxColor: "#000000",
      boxOpacity: 0.72,
      // SVT Play puts the lower plate almost on the bottom edge of the picture
      bottom: 0.6,
      lineHeight: 1.24,
      align: "left",
      shade: true,
    },
  },
  {
    id: "streaming",
    name: { en: "Streaming (42 characters)", sv: "Streaming (42 tecken)" },
    rules: {
      maxChars: 42,
      maxLines: 2,
      minDuration: 1.2,
      maxDuration: 7,
      maxCps: 17,
      minGap: 0.083,
      linger: 0.5,
      balance: "balanced",
      snapFrames: true,
    },
    look: {
      font: "Noto Sans",
      width: 100,
      weight: 500,
      size: 4.6,
      color: "#ffffff",
      background: "shadow",
      boxColor: "#000000",
      boxOpacity: 0.8,
      bottom: 8,
      lineHeight: 1.25,
      align: "center",
      shade: false,
    },
  },
  {
    id: "single",
    name: { en: "One line (web clips)", sv: "En rad (webbklipp)" },
    rules: {
      maxChars: 42,
      maxLines: 1,
      minDuration: 1.0,
      maxDuration: 4,
      maxCps: 17,
      minGap: 0.083,
      linger: 0.3,
      balance: "balanced",
      snapFrames: true,
    },
    look: {
      font: "Noto Sans",
      width: 100,
      weight: 600,
      size: 5.0,
      color: "#ffffff",
      background: "box",
      boxColor: "#000000",
      boxOpacity: 0.7,
      bottom: 8,
      lineHeight: 1.3,
      align: "center",
      shade: false,
    },
  },
  {
    id: "social",
    name: { en: "Vertical video", sv: "Vertikal video" },
    rules: {
      maxChars: 22,
      maxLines: 2,
      minDuration: 0.8,
      maxDuration: 3.5,
      maxCps: 18,
      minGap: 0.04,
      linger: 0.2,
      balance: "balanced",
      snapFrames: true,
    },
    look: {
      font: "Noto Sans",
      width: 100,
      weight: 700,
      size: 3.6,
      color: "#ffffff",
      background: "outline",
      boxColor: "#000000",
      boxOpacity: 1,
      bottom: 24,
      lineHeight: 1.2,
      align: "center",
      shade: false,
    },
  },
];

export const DEFAULT_TEMPLATE = TEMPLATES[0];

export function cloneTemplate(id: string) {
  const t = TEMPLATES.find((x) => x.id === id) ?? DEFAULT_TEMPLATE;
  return { rules: { ...t.rules }, look: { ...t.look } };
}
