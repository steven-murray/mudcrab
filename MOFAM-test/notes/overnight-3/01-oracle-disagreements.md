# 1. Disagreements with the Oracle

Two new ones. Both are cases where we follow the guide and the Oracle does
something else, which is the rule we agreed on — flagged rather than silently
matched.

## 1.1 Urasek's voice replacement did not take in the Oracle

Part 25's eight villages each pack a BSA from a base archive plus a
voice-acting archive. Three rows — Feldscar (1), Molapi (4) and Urasek (7) —
say **"select Replace All"**, and those are exactly the three rows where the
two archives overlap. The instruction exists because BSArch Pro prompts.

Feldscar and Molapi hold the VA bytes for every overlapping file. **Urasek holds
the base mod's, for all 608 of them.**

Established by arithmetic, not by eyeballing: for each village I summed the file
sizes of both candidate compositions and compared them to the Oracle BSA's
payload total.

| | a-wins | b-wins | Oracle |
|---|---|---|---|
| Feldscar | 129881894 | **123583707** | 123583707 |
| Molapi | 30591895 | **26314383** | 26314383 |
| Urasek | **63083533** | 58223187 | 63083533 |

An exact match on a 63-million-byte total is not a coincidence.

**Ours follows the guide**, so our Urasek.bsa is 58223187 bytes of payload
against yours at 63083533. Everything else about it — 1161 files, same paths —
is identical. If you want the Oracle to match, re-pack Urasek with the VA
archive's sound dragged in last and "Replace All" chosen.

## 1.2 Nothing else

Part 26a produced **no** Oracle disagreements across 57 rows. Its 24 differences
are 18 merge-source hides and 6 QAC rows, both known classes.

## Still open from before

- **Thorn Addon (Part 24 row 7c)**: the guide's "Once installed delete
  tbskGuardsFeaturesThornAddon.esp" has not been applied in the Oracle, though
  rows 7b and 7d got the same instruction and were followed. Ours prunes it.
- **JaySuS Blades (Part 24)**: the guide says *hide* `JBlades!.esp`; the Oracle
  deleted it. Ours hides, so we have a `.mohidden` you do not have. Functionally
  identical.
