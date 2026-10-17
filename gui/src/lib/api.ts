// The backend's commands and events, typed. Every edit or step goes through
// one of these, which call mudcrab's library exactly as the CLI does.

import { invoke } from "@tauri-apps/api/core";

export type Markdown = string;

export interface Docs {
  title?: string;
  summary?: Markdown;
  body?: Markdown;
  footer?: Markdown;
}

export type InputValue = boolean | string;

export interface InputDoc {
  id: string;
  type: "bool" | "choice" | "text";
  query: string;
  choices?: string[];
  default?: InputValue;
}

export interface ConditionDoc {
  expr: string;
  text: string;
  question?: string;
}

export interface StepDoc {
  action: string;
  kind: string;
  text: string;
  code?: string;
  condition?: ConditionDoc;
}

export interface ArchiveDoc {
  source: string;
  file_name?: string;
  url?: string;
  nexus?: { mod_url: string; file_url: string };
  condition?: ConditionDoc;
}

export interface ModDoc {
  id: string;
  title: string;
  anchor: string;
  mod_type?: string;
  condition?: ConditionDoc;
  docs: Docs;
  archives: ArchiveDoc[];
  plugins: string[];
  steps: StepDoc[];
  merge?: { output: string; sources: { mod: string; plugin: string }[] };
}

export interface SectionDoc {
  name: string;
  title: string;
  depth: number;
  anchor: string;
  docs: Docs;
  entries: Entry[];
}

export type Entry =
  | ({ kind: "section" } & SectionDoc)
  | ({ kind: "mod" } & ModDoc)
  | { kind: "revisit"; id: string; title: string; anchor: string; steps: StepDoc[] };

export interface Document {
  name: string;
  docs: Docs;
  inputs: InputDoc[];
  tools: ModDoc[];
  entries: Entry[];
  load_order: { name: string; active: boolean; mod?: string }[];
  mod_count: number;
}

export interface ListView {
  path: string;
  about: string | null;
  document: Document;
  answers: Record<string, string>;
}

export interface Location {
  path: string | null;
  from: string | null;
  exists: boolean;
}

export interface Locations {
  instance: Location;
  downloads: Location;
  game: Location;
  profile: string;
  search: Location[];
}

export interface Done {
  summary: string;
  writes: string[];
  command: string;
}

export interface ByHand {
  mod_id: string;
  label: string;
  page: string | null;
  note: string | null;
  why: string;
}

export interface Downloaded {
  done: Done;
  by_hand: ByHand[];
  browser_folder: string | null;
}

export interface Checked {
  done: Done;
  problems: number;
  warnings: number;
}

export interface StepByHand {
  title: string;
  steps: string[];
  done: boolean | null;
}

// Events from mudcrab's library (mudcrab::events), as the backend emits them.
export type Tone = "good" | "warning" | "problem" | "note";
export type Span = { kind: "plain" | "name" | "dim"; text: string };
export type LibEvent =
  | { event: "status"; tone: Tone; text: Span[] }
  | { event: "section"; tone: Tone; heading: string; items: { text: Span[]; mod_id: string | null }[] }
  | { event: "manual_step"; title: string; steps: string[] };

export interface Bar {
  id: number;
  label: string;
  len: number;
  pos: number;
  message: string;
  done: boolean;
}

export const api = {
  opening: () => invoke<string | null>("opening"),
  recent: () => invoke<string[]>("recent"),
  openList: (path: string) => invoke<ListView>("open_list", { path }),
  locations: () => invoke<Locations>("locations"),
  setLocation: (kind: string, path: string | null) =>
    invoke<Locations>("set_location", { kind, path }),
  addSearchFolder: (path: string) => invoke<Locations>("add_search_folder", { path }),
  removeSearchFolder: (path: string) => invoke<Locations>("remove_search_folder", { path }),
  plan: (answers: Record<string, string>) => invoke<Done>("step_plan", { answers }),
  download: () => invoke<Downloaded>("step_download"),
  lookForDownloads: () => invoke<ByHand[]>("look_for_downloads"),
  openPage: (url: string) => invoke<void>("open_page", { url }),
  check: () => invoke<Checked>("step_check"),
  install: () => invoke<Done>("step_install"),
  manualSteps: () => invoke<StepByHand[]>("manual_steps"),
};
