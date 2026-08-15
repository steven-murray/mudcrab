# MOFAM Condensed Instructions (Compression Round 1)

Source transcription: `MOFAM-test/input/mofam-source.md`

Goal of this file:
- Keep only behaviorally-relevant structure for mudcrab translation.
- Separate normal installs from feature-driving edge cases.

## 1. Install Topology

- Setup and tooling: Parts 1, 36-40.
- Core gameplay stack: Parts 2-35.
- Finalization: plugin sorting, bashed patch, conflict resolution, slowLODGen.

Observed workflow graph:
1. Tool setup (MO2, xEdit, Wrye Bash, zMerge, BSArch, BAE, slowLODGen).
2. Bulk mod install in separator order.
3. Repeated local actions during install:
	 - set data directory
	 - hide/delete files
	 - choose FOMOD/BAIN options
	 - edit INI values
	 - repack BSAs
4. Build multiple merges in dedicated profiles.
5. Import load order and build bashed patch.
6. Run final CR and LOD.

## 2. High-Signal Behavior Classes

### A. Direct archive install

- Baseline supported behavior.
- Typical TOML mapping: `path + handler + layout + include/exclude`.

### B. Manual install normalization (`[MI]`)

- Common cases:
	- nested archive with real data under subfolder
	- missing expected data root
	- hand-created folders before install
- Needed model: `layout = custom-data-folder` + optional path rewrite actions.

### C. Plugin cleaning (`[QAC]`)

- Requires post-install tool invocation against specific plugins.
- Needed model: action step bound to installed plugin artifacts.

### D. Dummy plugin + archive packing (`[DP]`)

- Repack loose files into BSA and attach/create plugin name.
- Needed model: archive-pack action with naming contract.

### E. FOMOD/BAIN choice trees

- Many mods require deterministic option selections.
- Needed model: installer-choice DSL or archive selectors.

### F. File hiding/deleting after install

- Extremely frequent.
- Needed model: declarative file operations in action pipeline.

### G. INI editing

- Repeated key/value updates in mod or game INIs.
- Needed model: patch-ini action with section/key semantics.

### H. Multi-mod merge procedures

- zMerge profile-specific merge definitions with fixed plugin lists.
- Needed model: merge action or external orchestrator integration.

### I. External/non-Nexus sources

- AFKMods/Mediafire/Mega/ModDB/Tesall links.
- Needed model: alternate download handlers + user-provided local archives.

## 3. Minimal TOML Inclusion Rules

Include in minimal only if an item demonstrates at least one of:
1. `MI` path normalization.
2. `QAC` cleaning action.
3. `DP` repack + dummy plugin.
4. FOMOD/BAIN option matrix.
5. File hide/delete pruning.
6. INI mutation.
7. Non-Nexus download source.
8. Merge participation.

Exclude from minimal:
- plain one-step downloads with no post-processing.
- repeated texture replacements with identical behavior.

## 4. Candidate Feature Backlog (From This Round)

1. `action.set_data_directory`
2. `action.clean_plugin_qac`
3. `action.pack_archive_bsa`
4. `action.create_dummy_plugin`
5. `action.file_prune` (hide/delete)
6. `action.ini_patch`
7. `installer.selection` (fomod/bain)
8. `download_handler.http_any` + local file ingest for off-Nexus URLs
9. `action.merge_plugins` (or bridge command contract)

## 5. First Minimal Candidate Set

Use representative entries instead of full list volume:
1. One `MI` mod from Part 2 or Part 7.
2. One `QAC` plugin from Parts 3/4/12.
3. One `DP` packaging example from Part 5 or Part 11.
4. One FOMOD-heavy mod from Part 6, 19, or 32.
5. One INI-heavy mod from Part 8, 14, or 30.
6. One non-Nexus source from Parts 16, 18, or 25.
7. One merge input sample from Part 36.

## 6. Translation Tracking Format

For each translated mod row, annotate:
- `classification`: direct | conditional | custom
- `features_used`: list of behavior classes above
- `minimal_candidate`: yes/no
- `requires_feature`: none | specific backlog item
