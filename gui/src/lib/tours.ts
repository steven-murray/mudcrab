import type { Stop } from "./Tour.svelte";

// The guided tour's words, by part: `start` on the opening page, `list` once
// a list is open. Each stop names the `data-tour` element it points at.
export const tours: Record<"start" | "list", Stop[]> = {
  start: [
    {
      title: "Welcome to mudcrab",
      text: "mudcrab installs a modlist into a Mod Organizer 2 instance, and lets a list's author read and change it. This tour shows where things are; it takes a minute, and ? at the top brings it back.",
    },
    {
      target: "open",
      title: "Open a list",
      text: "A modlist is a .toml file. Open one to read its guide, answer its questions and install it.",
    },
    {
      target: "new",
      title: "Or start your own",
      text: "A new list starts empty and opens in author mode, where mods are added from your downloads or from Nexus links.",
    },
  ],
  list: [
    {
      mode: "install",
      title: "Installing a list",
      text: "From top to bottom on the right: choose a profile and answer the list's questions, then install. On the left is the list's guide.",
    },
    {
      target: "guide",
      mode: "install",
      title: "The list's guide",
      text: "Every mod the list installs, section by section, with its author's notes.",
    },
    {
      target: "guide-switch",
      title: "Guide, or load order",
      text: "Switch between the mods and the order their plugins load in.",
    },
    {
      target: "about",
      title: "About this list",
      text: "What the list's author wrote for whoever installs it: what it is, what it needs.",
    },
    {
      target: "settings",
      title: "Settings",
      text: "Where things go for this list: the Mod Organizer 2 instance, the downloads and game folders, other folders that may already hold archives. And mudcrab's own, for every list: your Nexus API key, and where xEdit, LOOT and Wine are.",
    },
    {
      target: "profile",
      mode: "install",
      title: "Profile",
      text: "A profile is a set of answers to the list's questions, installed into the MO2 profile of the same name. Several profiles can share one instance, with different answers.",
    },
    {
      target: "install",
      title: "Install",
      text: "Plans, downloads, checks and installs, one step after another. Cancel stops at the next archive or mod; installing again carries on from there. A plan that passed its check is not checked again unless you tick Check again.",
    },
    {
      target: "steps",
      title: "What each step does",
      text: "Each step shows what it is doing and the mudcrab command that does the same, to copy. Archives that need a click are listed here with their pages; when it is done, so are the steps left to do by hand.",
    },
    {
      target: "modes",
      title: "Install, or Author",
      text: "Author mode is for reading and changing the list itself. Next, a look at it.",
    },
    {
      target: "author-list",
      mode: "author",
      title: "The list's mods",
      text: "Each section and its mods. Choose one to see its files, notes, fields and actions; drag one by its handle to move it; right-click to remove or disable it.",
    },
    {
      target: "author-add",
      mode: "author",
      title: "Adding a mod",
      text: "From an archive in your downloads, or from a Nexus link. Installers (FOMOD, BAIN) are walked through as they would be at install.",
    },
    {
      target: "author-archives",
      mode: "author",
      title: "Archives",
      text: "Every archive in the downloads folders, which of them the list uses, and the rest, ready to add with a double-click.",
    },
    {
      target: "author-tabs",
      mode: "author",
      title: "Looking into the list",
      text: "The chosen mod, the load order with each plugin's masters, file conflicts between mods, a check of the whole list, its questions, and every change made this session.",
    },
    {
      target: "author-undo",
      mode: "author",
      title: "Every change is written at once",
      text: "Each edit is one of mudcrab's commands, written to the list straight away, and can be undone.",
    },
    {
      target: "help",
      mode: "install",
      title: "That's the tour",
      text: "Back in install mode. Press ? any time to see it again.",
    },
  ],
};
