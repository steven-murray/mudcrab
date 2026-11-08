import type { ModDoc } from "./api";

/** One mod in the list scan (`scan_list`): what it installs, and what the
 *  scan found. */
export interface ModScan {
  mod_id: string;
  inis: string[];
  bsas: string[];
  /** Earlier mods whose files it replaces, and later ones replacing its own. */
  wins: [string, number][];
  losses: [string, number][];
  /** Its files as its actions leave them; else as its archives lay them out. */
  finished: boolean;
  /** Every archive read. */
  read: boolean;
  installers: ("fomod" | "bain")[];
  missing: boolean;
  problems: number;
  warnings: number;
}

export interface ListScan {
  mods: ModScan[];
  problems: number;
  warnings: number;
}

/** What a filter can ask about besides the mod itself. */
export interface Context {
  /** The scan, by mod id; null until it has run. */
  scan: Map<string, ModScan> | null;
  /** How many mods that are not optional need it. */
  neededBy: (id: string) => number;
  /** Mods a merge takes plugins from. */
  mergeSources: Set<string>;
}

export interface Filter {
  key: string;
  group: string;
  label: string;
  /** Needs the scan: until it has run, nothing passes this filter. */
  scan?: boolean;
  /** Whether the mod has it; null when that is not known. */
  test: (mod: ModDoc, cx: Context) => boolean | null;
}

const scanned = (test: (s: ModScan) => boolean) => (mod: ModDoc, cx: Context) => {
  const s = cx.scan?.get(mod.id);
  return s ? test(s) : null;
};
const acts = (test: (action: string) => boolean) => (mod: ModDoc) => mod.steps.some((s) => test(s.action));

export const FILTERS: Filter[] = [
  { key: "needed", group: "Needed", label: "Needed by a mod that isn't optional", test: (m, cx) => cx.neededBy(m.id) > 0 },
  { key: "optional", group: "Needed", label: "Optional, or has an if", test: (m) => !!m.optional || !!m.condition },
  { key: "unticked", group: "Needed", label: "Unticked in MO2", test: (m) => m.enabled === false },

  { key: "plugins", group: "What it installs", label: "Has plugins (no: resources only)", test: (m) => m.plugins.length > 0 },
  { key: "ini", group: "What it installs", label: "Has an INI", scan: true, test: scanned((s) => s.inis.length > 0) },
  { key: "bsa", group: "What it installs", label: "Has a BSA", scan: true, test: scanned((s) => s.bsas.length > 0) },
  {
    key: "installer",
    group: "What it installs",
    label: "Installer choices (FOMOD or BAIN)",
    scan: true,
    test: scanned((s) => s.installers.length > 0),
  },

  {
    key: "conflicts",
    group: "Conflicts",
    label: "Shares files with another mod",
    scan: true,
    test: scanned((s) => s.wins.length > 0 || s.losses.length > 0),
  },
  { key: "wins", group: "Conflicts", label: "Replaces another mod's files", scan: true, test: scanned((s) => s.wins.length > 0) },
  { key: "losses", group: "Conflicts", label: "Has files another mod replaces", scan: true, test: scanned((s) => s.losses.length > 0) },

  { key: "offsite", group: "Archives", label: "Has archives not from Nexus", test: (m) => m.archives.some((a) => !a.nexus) },
  { key: "missing", group: "Archives", label: "An archive isn't downloaded", scan: true, test: scanned((s) => s.missing) },
  {
    key: "problems",
    group: "Archives",
    label: "The scan found problems or warnings",
    scan: true,
    test: scanned((s) => s.problems + s.warnings > 0),
  },

  { key: "actions", group: "Actions", label: "Has actions", test: (m) => m.steps.length > 0 },
  { key: "ini_actions", group: "Actions", label: "Changes INI settings", test: acts((a) => a.startsWith("ini_")) },
  { key: "xedit", group: "Actions", label: "Cleans or edits plugins (QAC, xEdit)", test: acts((a) => a === "qac" || a === "delete_records") },
  { key: "merge", group: "Actions", label: "In a merge (a source or the output)", test: (m, cx) => !!m.merge || cx.mergeSources.has(m.id) },

  { key: "notes", group: "Docs", label: "Has notes for the guide", test: (m) => !!(m.docs.summary || m.docs.body) },
];

export type Want = "yes" | "no";

/** Whether `mod` passes every filter in `wants`. */
export function passes(mod: ModDoc, wants: Record<string, Want>, cx: Context): boolean {
  for (const [key, want] of Object.entries(wants)) {
    const filter = FILTERS.find((f) => f.key === key);
    if (!filter) continue;
    const has = filter.test(mod, cx);
    if (has === null || has !== (want === "yes")) return false;
  }
  return true;
}
