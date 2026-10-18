// The backend's commands and events, typed. Every edit or step goes through
// one of these, which call mudcrab's library exactly as the CLI does.

import { invoke } from "@tauri-apps/api/core";

export type Markdown = string;

export interface Docs {
  title?: string;
  links?: { url: string; label?: string }[];
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
  raw?: { download_page?: string };
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
  /** Present, and false, for a mod installed unticked in MO2. */
  enabled?: boolean;
  pages: { mod_url: string }[];
  condition?: ConditionDoc;
  docs: Docs;
  archives: ArchiveDoc[];
  plugins: string[];
  steps: StepDoc[];
  merge?: { output: string; sources: { mod: string; plugin: string }[] };
}

export interface SectionDoc {
  name: string;
  path: string[];
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

export interface Tools {
  path: string | null;
  from: string;
  exists: boolean;
  problems: string[];
}

export interface Locations {
  tools: Tools;
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

export interface FindingItem {
  text: Span[];
  mod_id: string | null;
}

export interface FindingGroup {
  heading: string;
  tone: Tone;
  items: FindingItem[];
}

export interface Checked {
  done: Done;
  problems: number;
  warnings: number;
  groups: FindingGroup[];
}

// Author mode, reading.

export interface PluginRow {
  name: string;
  mods: string[];
  inactive: boolean;
  /** Read by check; null before it has run, or for a plugin made at install. */
  masters: string[] | null;
}

export interface LoadOrder {
  plugins: PluginRow[];
  game: string[];
  masters_read: boolean;
  merged: Record<string, string>;
}

export interface ModView {
  id: string;
  left_out: boolean;
  archives: { number: number; label: string; layout: string | null; condition: string | null }[];
}

export interface FileRow {
  path: string;
  name: string;
  depth: number;
  folder: boolean;
  state: "all" | "some" | "none";
  offered: boolean;
  package: boolean;
  destination: string | null;
  filtered_as: string | null;
  /** The exclude that leaves exactly this out; null where the layout does not offer it. */
  exclude_glob: string | null;
  files: number;
  taken: number;
}

export interface InstalledFile {
  path: string;
  laid_out: string;
  hidden: boolean;
}

export interface Tree {
  label: string;
  layout: string | null;
  include: string[];
  exclude: string[];
  bain_subpackages: string[];
  bain_conditional: string[];
  rows: FileRow[];
  installed: InstalledFile[];
  entries: number;
  taken: number;
  command: string;
}

export interface Partner {
  mod_id: string;
  files: number;
}

export interface Conflicts {
  sweep: {
    mods: { id: string; section: string[]; wins: Partner[]; losses: Partner[] }[];
    installed: number;
    missing: number;
  };
  command: string;
}

export interface Between {
  shared: { files: number; others: Partner[]; overlap: string[]; already_hidden: number };
  command: string;
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

// Author mode, editing: what an edit did, and the session's history.

export interface Change {
  command: string;
  diff: string;
}

export interface Edited {
  change: Change;
  can_undo: boolean;
  can_redo: boolean;
}

export interface Changes {
  edits: Change[];
  diff: string;
  can_undo: boolean;
  can_redo: boolean;
}

// Adding a mod from an archive.

export interface ArchiveRow {
  path: string;
  file_name: string;
  size: number;
  modified: number | null;
  source: string | null;
  mod_name: string | null;
  name: string | null;
  version: string | null;
  folder: string;
  used_by: string[];
}

export interface Started {
  path: string;
  file_name: string;
  layout: { kind: string; [detail: string]: unknown };
  layout_label: string;
  plugins: string[];
  source: string;
  suggested_id: string;
  has_installer: boolean;
  identified: boolean;
}

export type Availability = "free" | "recommended" | "required" | "unavailable";

export interface Question {
  title: string;
  rule: "exactly_one" | "at_most_one" | "at_least_one" | "any" | "all";
  choices: { name: string; description: string; availability: Availability }[];
  default: number[];
}

export type Answer = { pick: number[] } | { yes: boolean };

export type Walk =
  | { kind: "ask"; asked: { kind: "choose"; question: Question } | { kind: "yes_no"; path: string } }
  | { kind: "done"; installed: { notes: string[]; actions: string[] } };

export interface AddRequest {
  path: string;
  id: string;
  section: string[];
  before_mod: string | null;
  condition: string | null;
  source: string | null;
  answers: Answer[];
  plugins: string[];
  load_after: string | null;
}

export interface AddPlan {
  block: string;
  diff: string;
  command: string;
  installs_plugins: string[];
  files: number;
  suggested_after: string | null;
  notes: string[];
}

export interface NexusFileRow {
  file_id: number;
  name: string | null;
  version: string | null;
  file_name: string | null;
  category_name: string | null;
  size_kb: number | null;
  here: string | null;
}

export interface NexusMod {
  mod_id: number;
  file_id: number | null;
  files: NexusFileRow[];
}

export interface Fetched {
  path: string | null;
  page: string | null;
  why: string | null;
}

// Tauri passes arguments by their camelCase names.
export const api = {
  opening: () => invoke<string | null>("opening"),
  recent: () => invoke<string[]>("recent"),
  openList: (path: string) => invoke<ListView>("open_list", { path }),
  locations: () => invoke<Locations>("locations"),
  setLocation: (kind: string, path: string | null) =>
    invoke<Locations>("set_location", { kind, path }),
  setToolsConfig: (path: string | null) => invoke<Locations>("set_tools_config", { path }),
  addSearchFolder: (path: string) => invoke<Locations>("add_search_folder", { path }),
  removeSearchFolder: (path: string) => invoke<Locations>("remove_search_folder", { path }),
  plan: (answers: Record<string, string>) => invoke<Done>("step_plan", { answers }),
  download: () => invoke<Downloaded>("step_download"),
  lookForDownloads: () => invoke<ByHand[]>("look_for_downloads"),
  openPage: (url: string) => invoke<void>("open_page", { url }),
  check: () => invoke<Checked>("step_check"),
  install: () => invoke<Done>("step_install"),
  manualSteps: () => invoke<StepByHand[]>("manual_steps"),
  loadOrder: () => invoke<LoadOrder>("load_order"),
  modView: (id: string) => invoke<ModView>("mod_view", { id }),
  modFiles: (id: string, number: number) => invoke<Tree>("mod_files", { id, number }),
  conflicts: () => invoke<Conflicts>("conflicts"),
  conflictsBetween: (id: string, other: string) =>
    invoke<Between>("conflicts_between", { id, other }),
  reloadList: () => invoke<ListView>("reload_list"),
  undo: () => invoke<Edited>("undo"),
  redo: () => invoke<Edited>("redo"),
  changes: () => invoke<Changes>("changes"),
  movePlugin: (plugin: string, before: string | null) =>
    invoke<Edited>("move_plugin", { plugin, before }),
  moveMod: (id: string, section: string[], beforeMod: string | null) =>
    invoke<Edited>("move_mod", { id, section, beforeMod }),
  editFilter: (id: string, number: number, key: string, add: string[], drop: string[]) =>
    invoke<Edited>("edit_filter", { id, number, key, add, drop }),
  setFields: (id: string, archive: number | null, sets: string[], unsets: string[]) =>
    invoke<Edited>("set_fields", { id, archive, sets, unsets }),
  addAction: (id: string, action: string, fields: string[], condition: string | null) =>
    invoke<Edited>("add_action", { id, action, fields, condition }),
  removeAction: (id: string, number: number) => invoke<Edited>("remove_action", { id, number }),
  actions: (id: string) => invoke<string[]>("actions", { id }),
  addInput: (name: string, kind: string, query: string, choices: string[], fallback: string | null) =>
    invoke<Edited>("add_input", { name, kind, query, choices, default: fallback }),
  setInput: (name: string, sets: string[], unsets: string[]) =>
    invoke<Edited>("set_input", { name, sets, unsets }),
  removeInput: (name: string) => invoke<Edited>("remove_input", { name }),
  setDocs: (id: string, fields: [string, string | null][]) => invoke<Edited>("set_docs", { id, fields }),
  fields: (id: string, archive: number | null) =>
    invoke<[string, string | number | boolean | (string | number | boolean)[]][]>("fields", { id, archive }),
  removeMod: (id: string) => invoke<Edited>("remove_mod", { id }),
  setEnabled: (id: string, enabled: boolean) => invoke<Edited>("set_enabled", { id, enabled }),
  archives: () => invoke<ArchiveRow[]>("archives"),
  addStart: (path: string) => invoke<Started>("add_start", { path }),
  addWalk: (path: string, answers: Answer[]) => invoke<Walk>("add_walk", { path, answers }),
  addPlan: (request: AddRequest) => invoke<AddPlan>("add_plan", { request }),
  addMod: (request: AddRequest) => invoke<Edited>("add_mod", { request }),
  chooseWalk: (id: string, number: number, answers: Answer[]) =>
    invoke<Walk>("choose_walk", { id, number, answers }),
  chooseWrite: (id: string, number: number, answers: Answer[]) =>
    invoke<Edited>("choose_write", { id, number, answers }),
  nexusLink: (link: string) => invoke<NexusMod>("nexus_link", { link }),
  nexusFetch: (modId: number, fileId: number, fileName: string | null) =>
    invoke<Fetched>("nexus_fetch", { modId, fileId, fileName }),
  nexusLook: () => invoke<string | null>("nexus_look"),
};

/** A value for a `key=value` edit, as TOML: a string quoted, so any text is safe. */
export function tomlString(text: string): string {
  return JSON.stringify(text);
}
