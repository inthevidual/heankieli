import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Lang, Quality, AppInfo, MediaInfo, ModelStatus, Transcript } from "./types";

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  log: (msg: string) => invoke<void>("log", { msg }).catch(() => {}),
  mediaUrl: (path: string) => invoke<string>("media_url", { path }),
  startup: () => invoke<{ files: string[]; lang: Lang | null; quality: Quality | null; start: boolean; at: number | null; ui: "sv" | "en" | null }>("startup"),
  listModels: () => invoke<ModelStatus[]>("list_models"),
  downloadModel: (id: string) => invoke<boolean>("download_model", { id }),
  cancelDownload: (id: string) => invoke<void>("cancel_download", { id }),
  deleteModel: (id: string) => invoke<void>("delete_model", { id }),
  probe: (path: string) => invoke<MediaInfo>("probe_media", { path }),
  transcribe: (job: { jobId: string; path: string; modelId: string; prompt?: string }) =>
    invoke<Transcript>("transcribe", { job }),
  cancelTranscription: () => invoke<void>("cancel_transcription"),
  makePreview: (path: string) => invoke<string>("make_preview", { path }),
  cancelPreview: () => invoke<void>("cancel_preview"),
  waveform: async (path: string) => new Uint8Array(await invoke<ArrayBuffer>("waveform", { path })),
  saveProject: (mediaPath: string, data: string) => invoke<void>("save_project", { mediaPath, data }),
  loadProject: (mediaPath: string) => invoke<string | null>("load_project", { mediaPath }),
  deleteProject: (mediaPath: string) => invoke<void>("delete_project", { mediaPath }),
};

export interface DownloadEvent {
  id: string;
  received: number;
  total: number;
  state: "downloading" | "verifying" | "done" | "error" | "cancelled";
  bytesPerSec: number;
  message: string | null;
}
export interface ProgressEvent {
  jobId: string;
  stage: "decoding" | "detecting" | "loading" | "transcribing";
  progress: number;
}
export interface SegmentEvent {
  jobId: string;
  start: number;
  end: number;
  text: string;
}

export function on<T>(event: string, fn: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => fn(e.payload));
}

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}
