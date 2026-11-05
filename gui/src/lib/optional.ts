import type { ModDoc } from "./api";

// Making a mod optional, wherever it is done from: its fields, its dialog,
// or its right-click menu.
//
// The player may untick an optional mod, so a mod that is not optional may
// not need one -- unless its `if` names it, and it comes and goes with it.
// `validate` refuses the edit; these say so before it is tried.

/** The mods that are not optional and need `id` without following it:
 *  while they do, it cannot be one of the optional mods. */
export function neededBy(id: string, mods: ModDoc[]): ModDoc[] {
  return mods.filter(
    (m) => m.id !== id && !m.optional && (m.dependencies ?? []).includes(id) && !(m.condition?.mods ?? []).includes(id),
  );
}

/** The optional mods `mod` needs: out of the optional mods, its `if` has to
 *  follow them. */
export function optionalNeeds(mod: ModDoc, mods: ModDoc[]): ModDoc[] {
  const needs = mod.dependencies ?? [];
  return mods.filter((m) => m.optional && needs.includes(m.id));
}

/** The list's groups of optional mods, by name: "" (the list's own) first. */
export function optionalGroups(mods: ModDoc[]): string[] {
  return [...new Set(["", ...mods.filter((m) => m.optional).map((m) => m.optional!.group ?? "")])];
}

/** A mod's `optional`, as TOML: in `group` ("" for the list's own), ticked to
 *  start or not. */
export function optionalToml(group: string, ticked: boolean): string {
  const name = group.trim();
  if (ticked) return name ? `{ group = ${JSON.stringify(name)}, default = true }` : "{ default = true }";
  return name ? JSON.stringify(name) : "true";
}

/** The mods a condition asks about, as `mod:id` or `mod:"id"`. */
export function namedMods(condition: string): string[] {
  return [...condition.matchAll(/mod:\s*("(?:[^"\\]|\\.)*"|[A-Za-z0-9_.-]+)/g)].map((m) =>
    m[1].startsWith('"') ? JSON.parse(m[1]) : m[1],
  );
}
