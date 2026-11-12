export interface Word {
  text: string;
  start: number;
  end: number;
  /** lowest token probability, 0..1 (1 for typed words) */
  p: number;
  /** first word of a Whisper segment (a sentence-like unit) */
  seg?: boolean;
  /** a new subtitle must start at this word (manual split) */
  br?: boolean;
  /** this word must stay in the same subtitle as the previous one (manual merge) */
  join?: boolean;
  /** a new line must start at this word, inside its subtitle */
  nl?: boolean;
  /** the subtitle starting with this word starts exactly here (manual timing) */
  cueStart?: number;
  /** the subtitle ending with this word ends exactly here (manual timing) */
  cueEnd?: number;
}

export interface Cue {
  index: number;
  /** words[from..to) */
  from: number;
  to: number;
  start: number;
  end: number;
  lines: string[];
  /** text breaks the layout rules even after layout (e.g. a manual merge) */
  overflow: boolean;
  /** characters per second, for the reading-speed indicator */
  cps: number;
}

export interface LayoutRules {
  maxChars: number;
  maxLines: number;
  minDuration: number;
  maxDuration: number;
  maxCps: number;
  minGap: number;
  /** how long a subtitle may stay up after the speech ends */
  linger: number;
  /** "bottom" = lower line the longer one (Nordic TV convention), "balanced", "top" */
  balance: "bottom" | "balanced" | "top";
  /** snap cue times to video frames */
  snapFrames: boolean;
}

export interface Look {
  font: string;
  weight: number;
  /** font width in percent (font-stretch), for fonts with a width axis */
  width: number;
  /** font size as a percentage of picture height */
  size: number;
  color: string;
  /** "box": a plate behind each line; "outline"; "shadow" */
  background: "box" | "outline" | "shadow";
  boxColor: string;
  boxOpacity: number;
  /** distance of the bottom line from the bottom, % of picture height */
  bottom: number;
  lineHeight: number;
  /** lines left-aligned inside a centred block (SVT), or each line centred */
  align: "left" | "center";
  /** white fading to grey towards the baseline, with a soft shadow (SVT) */
  shade: boolean;
}

export interface Template {
  id: string;
  name: { en: string; sv: string };
  rules: LayoutRules;
  look: Look;
}

export type Lang = "en" | "sv";
export type Quality = "accurate" | "fast";

export interface MediaInfo {
  path: string;
  name: string;
  duration: number;
  hasVideo: boolean;
  hasAudio: boolean;
  width: number;
  height: number;
  fps: number;
  videoCodec: string;
  audioCodec: string;
  container: string;
}

export interface TranscriptSegment {
  start: number;
  end: number;
  words: { text: string; start: number; end: number; p: number }[];
}

export interface Transcript {
  language: string;
  modelId: string;
  modelName: string;
  duration: number;
  segments: TranscriptSegment[];
  elapsed: number;
}

export interface ModelStatus {
  id: string;
  language: string;
  quality: string;
  name: string;
  repo: string;
  file: string;
  localName: string;
  size: number;
  sha256: string;
  installed: boolean;
  partial: number;
}

export interface AppInfo {
  version: string;
  backend: string;
  locale: string;
  platform: string;
  ffmpeg: string | null;
  modelsDir: string;
}
