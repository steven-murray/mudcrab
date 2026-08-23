//! The TES4 field table.
//!
//! One entry per `(record, field)` pair, sorted, binary-searched. Deliberately
//! a plain table rather than a macro DSL: it has to be greppable and reviewable
//! against xEdit's `wbDefinitionsTES4.pas`, where `wbFormIDCk`/`wbFormID` marks
//! a FormID field and `wbInteger`/`wbFloat` marks one that only looks like one.
//!
//! Scope is demand-driven. It covers the 50 record types the six MOFAM merges
//! need; anything else hard-errors with a message saying how to extend it.
//! `tests/fixtures/plugin/subrecord_matrix.txt` is the completeness target.
//!
//! Note how the same field signature means different things in different
//! records -- `DOOR/ANAM` is a sound FormID, `WEAP/ANAM` is a u16 enchantment
//! point count. That is why the key is the pair, never the field alone.
//!
//! # The OBME fields are missing on purpose
//!
//! `OBME`, `EFME`, `EFIX`, `EFXX`, `EDDX` and `ESCE` -- Oblivion Magic
//! Extender's additions to ALCH, ENCH, INGR, SPEL and MGEF -- are deliberately
//! absent, and **must not be added as `Opaque` to quieten the error**.
//!
//! Five of the six hold no FormIDs and would be safe. `EFIX` is not: xEdit
//! defines its Param #2 as a union whose type is decided by a byte in the
//! *`EFME` subrecord of the same record*, and it can be a FormID. `EFIT` has
//! the same shape -- already in this table as `Opaque`, which is correct for
//! vanilla TES4 and wrong once OBME is present.
//!
//! So a per-field table cannot answer this, and marking the harmless five
//! `Opaque` would let an OBME plugin through to the point where `EFIT` is read
//! as inert bytes and its FormID silently not renumbered. The refusal on the
//! other five is what keeps that from happening. Resolving it properly needs a
//! field kind with access to its sibling subrecords.

use super::{CustomKind, FieldKind};

use FieldKind::{FormId, FormIdArray, FormIdPrefix, Opaque, Struct, ZString};

const CONDITION: FieldKind = FieldKind::Custom(CustomKind::Condition);
const PACKAGE_LOCATION: FieldKind = FieldKind::Custom(CustomKind::PackageLocation);
const PACKAGE_TARGET: FieldKind = FieldKind::Custom(CustomKind::PackageTarget);

/// A leading FormID followed by a fixed number of trailing bytes.
const FORM_ID_THEN_8: FieldKind = Struct {
    sizes: &[8],
    form_id_offsets: &[0],
};
const FORM_ID_THEN_28: FieldKind = Struct {
    sizes: &[28],
    form_id_offsets: &[0],
};
/// Two FormIDs back to back: male then female, in every case that uses it.
const FORM_ID_PAIR_8: FieldKind = Struct {
    sizes: &[8],
    form_id_offsets: &[0, 4],
};
/// `SCIT` script effect: script FormID, magic school, a four-character visual
/// effect *signature* (not a FormID), then flags.
const SCRIPT_EFFECT: FieldKind = Struct {
    sizes: &[16],
    form_id_offsets: &[0],
};
/// `LVLO` levelled list entry: level, unused, the FormID, count, unused.
const LEVELLED_ENTRY: FieldKind = Struct {
    sizes: &[12],
    form_id_offsets: &[4],
};
/// `LSCR/LNAM`: direct target FormID, indirect worldspace FormID, grid y, grid x.
const LOADING_SCREEN_LOCATION: FieldKind = Struct {
    sizes: &[12],
    form_id_offsets: &[0, 4],
};
/// `MGEF/DATA`: eight of its sixteen words are FormIDs -- associated item,
/// light, effect shader, enchant effect, and four sounds.
const MAGIC_EFFECT_DATA: FieldKind = Struct {
    sizes: &[64],
    form_id_offsets: &[8, 24, 32, 36, 40, 44, 48, 52],
};
/// `REGN/RDWT` and `CLMT/WLST`: repeating {weather FormID, chance}.
const WEATHER_CHANCE: FieldKind = FieldKind::StructArray { stride: 8, form_id_offsets: &[0] };
/// `REGN/RDGS`: repeating {grass FormID, unused}.
const GRASS_ENTRY: FieldKind = FieldKind::StructArray { stride: 8, form_id_offsets: &[0] };
/// `REGN/RDSD`: repeating {sound FormID, flags, chance}.
const REGION_SOUND: FieldKind = FieldKind::StructArray { stride: 12, form_id_offsets: &[0] };
/// `REGN/RDOT`: repeating region object; the FormID leads each 52-byte entry.
const REGION_OBJECT: FieldKind = FieldKind::StructArray { stride: 52, form_id_offsets: &[0] };
/// `WATR/GNAM`: daytime, nighttime and underwater water FormIDs.
const RELATED_WATERS: FieldKind = FieldKind::Struct { sizes: &[12], form_id_offsets: &[0, 4, 8] };

/// Fixed-size structs containing no FormIDs.
const PLAIN_8: FieldKind = Struct {
    sizes: &[8],
    form_id_offsets: &[],
};
const PLAIN_24: FieldKind = Struct {
    sizes: &[24],
    form_id_offsets: &[],
};

/// MUST stay sorted by `(record, field)`; a test enforces it.
static FIELDS: &[([u8; 4], [u8; 4], FieldKind)] = &[
    // ---- ACHR ----
    (*b"ACHR", *b"DATA", PLAIN_24), // position + rotation floats
    (*b"ACHR", *b"EDID", ZString),
    (*b"ACHR", *b"NAME", FormId), // base NPC_
    (*b"ACHR", *b"XESP", FORM_ID_THEN_8), // enable parent + flags
    (*b"ACHR", *b"XHRS", FormId), // horse reference
    (*b"ACHR", *b"XMRC", FormId), // merchant container
    (*b"ACHR", *b"XRGD", Opaque), // ragdoll data
    (*b"ACHR", *b"XSCL", Opaque), // f32 scale -- NOT a FormID
    // ---- ACRE ----
    (*b"ACRE", *b"DATA", PLAIN_24),
    (*b"ACRE", *b"EDID", ZString),
    (*b"ACRE", *b"NAME", FormId), // base CREA
    (*b"ACRE", *b"XESP", FORM_ID_THEN_8),
    (*b"ACRE", *b"XOWN", FormId),
    (*b"ACRE", *b"XRGD", Opaque),
    (*b"ACRE", *b"XSCL", Opaque), // f32 scale -- NOT a FormID
    // ---- ACTI ----
    (*b"ACTI", *b"EDID", ZString),
    (*b"ACTI", *b"FULL", ZString),
    (*b"ACTI", *b"MODB", Opaque), // f32 bound radius
    (*b"ACTI", *b"MODL", ZString),
    (*b"ACTI", *b"MODT", Opaque), // model texture hashes
    (*b"ACTI", *b"SCRI", FormId),
    (*b"ACTI", *b"SNAM", FormId), // looping sound
    // ---- ALCH ----
    (*b"ALCH", *b"DATA", Opaque), // f32 weight
    (*b"ALCH", *b"EDID", ZString),
    (*b"ALCH", *b"EFID", Opaque), // 4-char effect signature -- NOT a FormID
    (*b"ALCH", *b"EFIT", Opaque), // magnitude/area/duration/type/actor value
    (*b"ALCH", *b"ENIT", Opaque), // value, flags
    (*b"ALCH", *b"FULL", ZString),
    (*b"ALCH", *b"ICON", ZString),
    (*b"ALCH", *b"MODB", Opaque), // f32 bound radius
    (*b"ALCH", *b"MODL", ZString),
    (*b"ALCH", *b"MODT", Opaque),
    (*b"ALCH", *b"SCIT", SCRIPT_EFFECT),
    (*b"ALCH", *b"SCRI", FormId),
    // ---- AMMO ----
    (*b"AMMO", *b"ANAM", Opaque), // u16 enchantment points -- NOT DOOR/ANAM
    (*b"AMMO", *b"DATA", Opaque), // speed, flags, value, weight, damage
    (*b"AMMO", *b"EDID", ZString),
    (*b"AMMO", *b"ENAM", FormId), // enchantment
    (*b"AMMO", *b"FULL", ZString),
    (*b"AMMO", *b"ICON", ZString),
    (*b"AMMO", *b"MODB", Opaque),
    (*b"AMMO", *b"MODL", ZString),
    (*b"AMMO", *b"MODT", Opaque),
    // ---- ANIO ----
    (*b"ANIO", *b"DATA", FormId), // IDLE animation
    (*b"ANIO", *b"EDID", ZString),
    (*b"ANIO", *b"MODB", Opaque),
    (*b"ANIO", *b"MODL", ZString),
    (*b"ANIO", *b"MODT", Opaque),
    // ---- APPA ----
    (*b"APPA", *b"DATA", Opaque), // type, value, weight, quality
    (*b"APPA", *b"EDID", ZString),
    (*b"APPA", *b"FULL", ZString),
    (*b"APPA", *b"ICON", ZString),
    (*b"APPA", *b"MODB", Opaque),
    (*b"APPA", *b"MODL", ZString),
    (*b"APPA", *b"MODT", Opaque),
    // ---- ARMO ----
    (*b"ARMO", *b"BMDT", Opaque), // u32 biped flags
    (*b"ARMO", *b"DATA", Opaque), // armour, value, health, weight
    (*b"ARMO", *b"EDID", ZString),
    (*b"ARMO", *b"ENAM", FormId), // enchantment
    (*b"ARMO", *b"FULL", ZString),
    (*b"ARMO", *b"ICO2", ZString), // female icon
    (*b"ARMO", *b"ICON", ZString),
    (*b"ARMO", *b"MO2B", Opaque),
    (*b"ARMO", *b"MO2T", Opaque),
    (*b"ARMO", *b"MO3B", Opaque),
    (*b"ARMO", *b"MO3T", Opaque),
    (*b"ARMO", *b"MO4B", Opaque),
    (*b"ARMO", *b"MO4T", Opaque),
    (*b"ARMO", *b"MOD2", ZString), // female model
    (*b"ARMO", *b"MOD3", ZString), // male ground model
    (*b"ARMO", *b"MOD4", ZString), // female ground model
    (*b"ARMO", *b"MODB", Opaque),
    (*b"ARMO", *b"MODL", ZString),
    (*b"ARMO", *b"MODT", Opaque),
    (*b"ARMO", *b"SCRI", FormId),
    // ---- BOOK ----
    (*b"BOOK", *b"ANAM", Opaque), // u16 enchantment points -- NOT DOOR/ANAM
    (*b"BOOK", *b"DATA", Opaque), // flags, teaches, value, weight
    (*b"BOOK", *b"DESC", ZString),
    (*b"BOOK", *b"EDID", ZString),
    (*b"BOOK", *b"ENAM", FormId), // enchantment
    (*b"BOOK", *b"FULL", ZString),
    (*b"BOOK", *b"ICON", ZString),
    (*b"BOOK", *b"MODB", Opaque),
    (*b"BOOK", *b"MODL", ZString),
    (*b"BOOK", *b"MODT", Opaque),
    (*b"BOOK", *b"SCRI", FormId),
    // ---- BSGN ----
    (*b"BSGN", *b"DESC", ZString),
    (*b"BSGN", *b"EDID", ZString),
    (*b"BSGN", *b"FULL", ZString),
    (*b"BSGN", *b"ICON", ZString),
    (*b"BSGN", *b"SPLO", FormId), // SPEL or LVSP
    // ---- CELL ----
    (*b"CELL", *b"DATA", Opaque), // u8 flags
    (*b"CELL", *b"EDID", ZString),
    (*b"CELL", *b"FULL", ZString),
    (*b"CELL", *b"XCCM", FormId), // climate -- distinct from XCMT, the u8 music type
    (*b"CELL", *b"XCLC", PLAIN_8), // grid x,y
    (*b"CELL", *b"XCLL", Opaque), // lighting
    (*b"CELL", *b"XCLR", FormIdArray), // regions
    (*b"CELL", *b"XCLW", Opaque), // f32 water height
    (*b"CELL", *b"XCMT", Opaque), // u8 music type
    (*b"CELL", *b"XCWT", FormId), // water
    (*b"CELL", *b"XGLB", FormId), // global
    (*b"CELL", *b"XOWN", FormId),
    (*b"CELL", *b"XRNK", Opaque), // i32 faction rank
    // ---- CLAS ----
    (*b"CLAS", *b"DATA", Opaque), // skills, flags, services -- enums, no FormIDs
    (*b"CLAS", *b"DESC", ZString),
    (*b"CLAS", *b"EDID", ZString),
    (*b"CLAS", *b"FULL", ZString),
    (*b"CLAS", *b"ICON", ZString),
    // ---- CLMT ----
    (*b"CLMT", *b"EDID", ZString),
    (*b"CLMT", *b"FNAM", ZString), // sun texture
    (*b"CLMT", *b"GNAM", ZString), // sun glare texture
    (*b"CLMT", *b"MODB", Opaque),
    (*b"CLMT", *b"MODL", ZString),
    (*b"CLMT", *b"MODT", Opaque),
    (*b"CLMT", *b"TNAM", Opaque), // sunrise/sunset/volatility timings
    (*b"CLMT", *b"WLST", WEATHER_CHANCE),
    // ---- CLOT ----
    (*b"CLOT", *b"ANAM", Opaque), // u16 enchantment points -- NOT a FormID, as WEAP/ANAM
    (*b"CLOT", *b"BMDT", Opaque), // u32 biped flags
    (*b"CLOT", *b"DATA", Opaque), // value, weight
    (*b"CLOT", *b"EDID", ZString),
    (*b"CLOT", *b"ENAM", FormId), // enchantment
    (*b"CLOT", *b"FULL", ZString),
    (*b"CLOT", *b"ICO2", ZString), // female icon
    (*b"CLOT", *b"ICON", ZString),
    (*b"CLOT", *b"MO2B", Opaque),
    (*b"CLOT", *b"MO2T", Opaque),
    (*b"CLOT", *b"MO3B", Opaque),
    (*b"CLOT", *b"MO3T", Opaque),
    (*b"CLOT", *b"MO4B", Opaque),
    (*b"CLOT", *b"MO4T", Opaque),
    (*b"CLOT", *b"MOD2", ZString), // female model
    (*b"CLOT", *b"MOD3", ZString), // male ground model
    (*b"CLOT", *b"MOD4", ZString), // female ground model
    (*b"CLOT", *b"MODB", Opaque),
    (*b"CLOT", *b"MODL", ZString),
    (*b"CLOT", *b"MODT", Opaque),
    (*b"CLOT", *b"SCRI", FormId),
    // ---- CONT ----
    (*b"CONT", *b"CNTO", FORM_ID_THEN_8), // item + u32 count
    (*b"CONT", *b"DATA", Opaque),
    (*b"CONT", *b"EDID", ZString),
    (*b"CONT", *b"FULL", ZString),
    (*b"CONT", *b"MODB", Opaque),
    (*b"CONT", *b"MODL", ZString),
    (*b"CONT", *b"MODT", Opaque),
    (*b"CONT", *b"QNAM", FormId), // close sound
    (*b"CONT", *b"SCRI", FormId),
    (*b"CONT", *b"SNAM", FormId), // open sound
    // ---- CREA ----
    (*b"CREA", *b"ACBS", Opaque), // flags, stats, level
    (*b"CREA", *b"AIDT", Opaque), // AI attributes
    (*b"CREA", *b"BNAM", Opaque), // f32 base scale -- NOT a FormID
    (*b"CREA", *b"CNTO", FORM_ID_THEN_8), // inventory item + count
    (*b"CREA", *b"CSCR", FormId), // inherits sounds from another CREA
    (*b"CREA", *b"CSDC", Opaque), // u8 sound chance
    (*b"CREA", *b"CSDI", FormId), // sound
    (*b"CREA", *b"CSDT", Opaque), // u32 sound type
    (*b"CREA", *b"DATA", Opaque), // combat/magic/stealth skills, health, attributes
    (*b"CREA", *b"EDID", ZString),
    (*b"CREA", *b"FULL", ZString),
    (*b"CREA", *b"INAM", FormId), // death item
    (*b"CREA", *b"KFFZ", ZString), // animation paths
    (*b"CREA", *b"MODB", Opaque),
    (*b"CREA", *b"MODL", ZString),
    (*b"CREA", *b"MODT", Opaque),
    (*b"CREA", *b"NAM0", ZString), // blood spray texture
    (*b"CREA", *b"NAM1", ZString), // blood decal texture
    (*b"CREA", *b"NIFT", Opaque),
    (*b"CREA", *b"NIFZ", ZString), // model file list
    (*b"CREA", *b"PKID", FormId), // AI package
    (*b"CREA", *b"RNAM", Opaque), // u8 attack reach
    (*b"CREA", *b"SCRI", FormId),
    (*b"CREA", *b"SNAM", FORM_ID_THEN_8), // faction + rank
    (*b"CREA", *b"SPLO", FormId), // spell
    (*b"CREA", *b"TNAM", Opaque), // f32 turning speed -- NOT a FormID
    (*b"CREA", *b"WNAM", Opaque), // f32 foot weight -- NOT a FormID
    (*b"CREA", *b"ZNAM", FormId), // combat style
    // ---- CSTY ----
    (*b"CSTY", *b"CSAD", Opaque), // advanced combat floats
    (*b"CSTY", *b"CSTD", Opaque), // combat style struct
    (*b"CSTY", *b"EDID", ZString),
    // ---- DIAL ----
    (*b"DIAL", *b"DATA", Opaque), // u8 dialogue type
    (*b"DIAL", *b"EDID", ZString),
    (*b"DIAL", *b"FULL", ZString),
    (*b"DIAL", *b"QSTI", FormId), // quest -- one per subrecord
    (*b"DIAL", *b"QSTR", FormIdArray), // quests
    // ---- DOOR ----
    (*b"DOOR", *b"ANAM", FormId), // loop sound -- NOT the same as WEAP/ANAM
    (*b"DOOR", *b"BNAM", FormId), // close sound
    (*b"DOOR", *b"EDID", ZString),
    (*b"DOOR", *b"FNAM", Opaque), // u8 flags
    (*b"DOOR", *b"FULL", ZString),
    (*b"DOOR", *b"MODB", Opaque),
    (*b"DOOR", *b"MODL", ZString),
    (*b"DOOR", *b"MODT", Opaque),
    (*b"DOOR", *b"SCRI", FormId),
    (*b"DOOR", *b"SNAM", FormId), // open sound
    (*b"DOOR", *b"TNAM", FormId), // random teleport destination
    // ---- EFSH ----
    (*b"EFSH", *b"DATA", Opaque), // 224 bytes of shader parameters
    (*b"EFSH", *b"EDID", ZString),
    (*b"EFSH", *b"ICO2", ZString),
    (*b"EFSH", *b"ICON", ZString),
    // ---- ENCH ----
    (*b"ENCH", *b"EDID", ZString),
    (*b"ENCH", *b"EFID", Opaque),
    (*b"ENCH", *b"EFIT", Opaque),
    (*b"ENCH", *b"ENIT", Opaque),
    (*b"ENCH", *b"FULL", ZString),
    (*b"ENCH", *b"SCIT", SCRIPT_EFFECT),
    // ---- EYES ----
    (*b"EYES", *b"DATA", Opaque), // u8 flags
    (*b"EYES", *b"EDID", ZString),
    (*b"EYES", *b"FULL", ZString),
    (*b"EYES", *b"ICON", ZString),
    // ---- FACT ----
    (*b"FACT", *b"CNAM", Opaque), // f32 crime gold multiplier
    (*b"FACT", *b"DATA", Opaque),
    (*b"FACT", *b"EDID", ZString),
    (*b"FACT", *b"FNAM", ZString), // female rank name
    (*b"FACT", *b"FULL", ZString),
    (*b"FACT", *b"INAM", ZString), // insignia texture -- NOT the CREA/NPC_ death item
    (*b"FACT", *b"MNAM", ZString), // male rank name
    (*b"FACT", *b"RNAM", Opaque), // i32 rank number
    (*b"FACT", *b"XNAM", FORM_ID_THEN_8), // faction + i32 modifier
    // ---- FLOR ----
    (*b"FLOR", *b"EDID", ZString),
    (*b"FLOR", *b"FULL", ZString),
    (*b"FLOR", *b"MODB", Opaque),
    (*b"FLOR", *b"MODL", ZString),
    (*b"FLOR", *b"MODT", Opaque), // texture hashes
    (*b"FLOR", *b"PFIG", FormId), // ingredient produced
    (*b"FLOR", *b"PFPC", Opaque), // four u8 seasonal production counts
    (*b"FLOR", *b"SCRI", FormId),
    // ---- FURN ----
    (*b"FURN", *b"EDID", ZString),
    (*b"FURN", *b"FULL", ZString),
    (*b"FURN", *b"MNAM", Opaque), // u32 active marker flags
    (*b"FURN", *b"MODB", Opaque),
    (*b"FURN", *b"MODL", ZString),
    (*b"FURN", *b"MODT", Opaque), // texture hashes
    (*b"FURN", *b"SCRI", FormId),
    // ---- GLOB ----
    (*b"GLOB", *b"EDID", ZString),
    (*b"GLOB", *b"FLTV", Opaque), // f32 value
    (*b"GLOB", *b"FNAM", Opaque), // u8 type char
    // ---- GMST ----
    (*b"GMST", *b"DATA", Opaque), // string, int or float by EDID prefix
    (*b"GMST", *b"EDID", ZString),
    // ---- GRAS ----
    (*b"GRAS", *b"DATA", Opaque), // density, slope, water, colour range
    (*b"GRAS", *b"EDID", ZString),
    (*b"GRAS", *b"MODB", Opaque),
    (*b"GRAS", *b"MODL", ZString),
    (*b"GRAS", *b"MODT", Opaque),
    // ---- HAIR ----
    (*b"HAIR", *b"DATA", Opaque), // u8 flags
    (*b"HAIR", *b"EDID", ZString),
    (*b"HAIR", *b"FULL", ZString),
    (*b"HAIR", *b"ICON", ZString),
    (*b"HAIR", *b"MODB", Opaque),
    (*b"HAIR", *b"MODL", ZString),
    (*b"HAIR", *b"MODT", Opaque),
    // ---- IDLE ----
    (*b"IDLE", *b"ANAM", Opaque), // u8 animation group section
    (*b"IDLE", *b"CTDA", CONDITION),
    (*b"IDLE", *b"DATA", FormIdArray), // two related idle animations -- 8 bytes, not a struct
    (*b"IDLE", *b"EDID", ZString),
    (*b"IDLE", *b"MODB", Opaque),
    (*b"IDLE", *b"MODL", ZString),
    // ---- INFO ----
    (*b"INFO", *b"CTDA", CONDITION),
    (*b"INFO", *b"DATA", Opaque), // type, next-speaker, flags
    (*b"INFO", *b"NAM1", ZString), // response text
    (*b"INFO", *b"NAM2", ZString), // actor notes
    (*b"INFO", *b"NAME", FormId), // added topic
    (*b"INFO", *b"PNAM", FormId), // previous INFO in the chain
    (*b"INFO", *b"QSTI", FormId), // quest
    (*b"INFO", *b"SCDA", Opaque), // compiled script -- references forms via SCRO index, not inline
    (*b"INFO", *b"SCHR", Opaque), // script header
    (*b"INFO", *b"SCRO", FormId), // script reference
    (*b"INFO", *b"SCTX", ZString), // script source
    (*b"INFO", *b"TCLF", FormId), // link-from topic
    (*b"INFO", *b"TCLT", FormId), // choice topic
    (*b"INFO", *b"TPIC", FormId), // topic
    (*b"INFO", *b"TRDT", Opaque), // emotion type/value, response number
    // ---- INGR ----
    (*b"INGR", *b"DATA", Opaque), // f32 weight
    (*b"INGR", *b"EDID", ZString),
    (*b"INGR", *b"EFID", Opaque), // 4-char effect signature -- NOT a FormID
    (*b"INGR", *b"EFIT", Opaque),
    (*b"INGR", *b"ENIT", Opaque), // value, flags
    (*b"INGR", *b"FULL", ZString),
    (*b"INGR", *b"ICON", ZString),
    (*b"INGR", *b"MODB", Opaque),
    (*b"INGR", *b"MODL", ZString),
    (*b"INGR", *b"MODT", Opaque),
    (*b"INGR", *b"SCIT", SCRIPT_EFFECT),
    (*b"INGR", *b"SCRI", FormId),
    // ---- KEYM ----
    (*b"KEYM", *b"DATA", Opaque),
    (*b"KEYM", *b"EDID", ZString),
    (*b"KEYM", *b"FULL", ZString),
    (*b"KEYM", *b"ICON", ZString),
    (*b"KEYM", *b"MODB", Opaque),
    (*b"KEYM", *b"MODL", ZString),
    (*b"KEYM", *b"MODT", Opaque),
    (*b"KEYM", *b"SCRI", FormId),
    // ---- LAND ----
    (*b"LAND", *b"ATXT", FORM_ID_THEN_8), // LTEX + quadrant/layer
    (*b"LAND", *b"BTXT", FORM_ID_THEN_8),
    (*b"LAND", *b"DATA", Opaque),
    (*b"LAND", *b"VCLR", Opaque),
    (*b"LAND", *b"VHGT", Opaque),
    (*b"LAND", *b"VNML", Opaque),
    (*b"LAND", *b"VTEX", FormIdArray), // LTEX references
    (*b"LAND", *b"VTXT", Opaque),
    // ---- LIGH ----
    (*b"LIGH", *b"DATA", Opaque), // time, radius, colour, flags, falloff, FOV, value, weight
    (*b"LIGH", *b"EDID", ZString),
    (*b"LIGH", *b"FNAM", Opaque), // f32 fade value
    (*b"LIGH", *b"FULL", ZString),
    (*b"LIGH", *b"ICON", ZString),
    (*b"LIGH", *b"MODB", Opaque), // f32 bound radius
    (*b"LIGH", *b"MODL", ZString), // model filename
    (*b"LIGH", *b"MODT", Opaque), // texture hashes
    (*b"LIGH", *b"SCRI", FormId),
    (*b"LIGH", *b"SNAM", FormId), // SOUN
    // ---- LSCR ----
    (*b"LSCR", *b"DESC", ZString),
    (*b"LSCR", *b"EDID", ZString),
    (*b"LSCR", *b"ICON", ZString),
    (*b"LSCR", *b"LNAM", LOADING_SCREEN_LOCATION),
    // ---- LTEX ----
    (*b"LTEX", *b"EDID", ZString),
    (*b"LTEX", *b"GNAM", FormIdArray), // grass
    (*b"LTEX", *b"HNAM", Opaque), // havok friction/restitution
    (*b"LTEX", *b"ICON", ZString),
    (*b"LTEX", *b"SNAM", Opaque), // u8 texture specular exponent
    // ---- LVLC ----
    (*b"LVLC", *b"EDID", ZString),
    (*b"LVLC", *b"LVLD", Opaque), // u8 chance none
    (*b"LVLC", *b"LVLF", Opaque), // u8 flags
    (*b"LVLC", *b"LVLO", LEVELLED_ENTRY),
    (*b"LVLC", *b"SCRI", FormId),
    (*b"LVLC", *b"TNAM", FormId), // template creature
    // ---- LVLI ----
    (*b"LVLI", *b"EDID", ZString),
    (*b"LVLI", *b"LVLD", Opaque), // u8 chance none
    (*b"LVLI", *b"LVLF", Opaque), // u8 flags
    (*b"LVLI", *b"LVLO", LEVELLED_ENTRY),
    // ---- LVSP ----
    (*b"LVSP", *b"EDID", ZString),
    (*b"LVSP", *b"LVLD", Opaque), // u8 chance none
    (*b"LVSP", *b"LVLF", Opaque), // u8 flags
    (*b"LVSP", *b"LVLO", LEVELLED_ENTRY),
    // ---- MGEF ----
    (*b"MGEF", *b"DATA", MAGIC_EFFECT_DATA),
    (*b"MGEF", *b"DESC", ZString),
    (*b"MGEF", *b"EDID", ZString),
    (*b"MGEF", *b"FULL", ZString),
    (*b"MGEF", *b"ICON", ZString),
    (*b"MGEF", *b"MODB", Opaque),
    (*b"MGEF", *b"MODL", ZString),
    // ---- MISC ----
    (*b"MISC", *b"DATA", Opaque), // value, weight
    (*b"MISC", *b"EDID", ZString),
    (*b"MISC", *b"FULL", ZString),
    (*b"MISC", *b"ICON", ZString),
    (*b"MISC", *b"MODB", Opaque),
    (*b"MISC", *b"MODL", ZString),
    (*b"MISC", *b"MODT", Opaque),
    (*b"MISC", *b"SCRI", FormId),
    // ---- NPC_ ----
    (*b"NPC_", *b"ACBS", Opaque), // configuration
    (*b"NPC_", *b"AIDT", Opaque), // AI data
    (*b"NPC_", *b"CNAM", FormId), // class
    (*b"NPC_", *b"CNTO", FORM_ID_THEN_8), // inventory item + count
    (*b"NPC_", *b"DATA", Opaque), // skills/attributes
    (*b"NPC_", *b"EDID", ZString),
    (*b"NPC_", *b"ENAM", FormId), // eyes
    (*b"NPC_", *b"FGGA", Opaque), // facegen geometry, asymmetric
    (*b"NPC_", *b"FGGS", Opaque), // facegen geometry, symmetric
    (*b"NPC_", *b"FGTS", Opaque), // facegen texture
    (*b"NPC_", *b"FNAM", Opaque),
    (*b"NPC_", *b"FULL", ZString),
    (*b"NPC_", *b"HCLR", Opaque), // hair COLOUR (rgba) -- NOT a FormID
    (*b"NPC_", *b"HNAM", FormId), // hair
    (*b"NPC_", *b"INAM", FormId), // death item
    (*b"NPC_", *b"KFFZ", Opaque), // animation filenames
    (*b"NPC_", *b"LNAM", Opaque), // hair LENGTH (f32) -- NOT a FormID
    (*b"NPC_", *b"MODB", Opaque),
    (*b"NPC_", *b"MODL", ZString),
    (*b"NPC_", *b"PKID", FormId), // AI package
    (*b"NPC_", *b"RNAM", FormId), // race
    (*b"NPC_", *b"SCRI", FormId),
    (*b"NPC_", *b"SNAM", FORM_ID_THEN_8), // faction + u8 rank
    (*b"NPC_", *b"SPLO", FormId), // spell
    (*b"NPC_", *b"ZNAM", FormId), // combat style
    // ---- PACK ----
    (*b"PACK", *b"CTDA", CONDITION),
    (*b"PACK", *b"EDID", ZString),
    (*b"PACK", *b"PKDT", Opaque),
    (*b"PACK", *b"PLDT", PACKAGE_LOCATION),
    (*b"PACK", *b"PSDT", Opaque),
    (*b"PACK", *b"PTDT", PACKAGE_TARGET),
    // ---- PGRD ----
    (*b"PGRD", *b"DATA", Opaque),
    (*b"PGRD", *b"PGAG", Opaque),
    (*b"PGRD", *b"PGRI", Opaque), // inter-cell point links: indices + floats
    (*b"PGRD", *b"PGRL", FormIdPrefix), // REFR FormID + point indices
    (*b"PGRD", *b"PGRP", Opaque), // point array
    (*b"PGRD", *b"PGRR", Opaque), // point-to-point connections
    // ---- QUST ----
    (*b"QUST", *b"CNAM", ZString), // log entry text
    (*b"QUST", *b"CTDA", CONDITION),
    (*b"QUST", *b"DATA", Opaque),
    (*b"QUST", *b"EDID", ZString),
    (*b"QUST", *b"FULL", ZString),
    (*b"QUST", *b"ICON", ZString),
    (*b"QUST", *b"INDX", Opaque), // i16 stage index
    (*b"QUST", *b"QSDT", Opaque),
    (*b"QUST", *b"QSTA", FORM_ID_THEN_8), // target + flags
    (*b"QUST", *b"SCDA", Opaque),
    (*b"QUST", *b"SCHR", Opaque),
    (*b"QUST", *b"SCRI", FormId),
    (*b"QUST", *b"SCRO", FormId), // script-referenced object
    (*b"QUST", *b"SCTX", ZString),
    // ---- RACE ----
    (*b"RACE", *b"ATTR", Opaque), // male/female base attributes
    (*b"RACE", *b"CNAM", Opaque), // u8 default hair colour
    (*b"RACE", *b"DATA", Opaque), // skill boosts, height, weight, flags
    (*b"RACE", *b"DESC", ZString),
    (*b"RACE", *b"DNAM", FORM_ID_PAIR_8), // default hair, male then female
    (*b"RACE", *b"EDID", ZString),
    (*b"RACE", *b"ENAM", FormIdArray), // eyes
    (*b"RACE", *b"FGGA", Opaque), // FaceGen geometry asymmetric
    (*b"RACE", *b"FGGS", Opaque), // FaceGen geometry symmetric
    (*b"RACE", *b"FGTS", Opaque), // FaceGen texture symmetric
    (*b"RACE", *b"FNAM", Opaque), // zero-length female marker
    (*b"RACE", *b"FULL", ZString),
    (*b"RACE", *b"HNAM", FormIdArray), // hairs
    (*b"RACE", *b"ICON", ZString),
    (*b"RACE", *b"INDX", Opaque), // u32 body part index
    (*b"RACE", *b"MNAM", Opaque), // zero-length male marker
    (*b"RACE", *b"MODB", Opaque),
    (*b"RACE", *b"MODL", ZString),
    (*b"RACE", *b"NAM0", Opaque), // zero-length head-data marker
    (*b"RACE", *b"NAM1", Opaque), // zero-length body-data marker
    (*b"RACE", *b"PNAM", Opaque), // f32 FaceGen main clamp
    (*b"RACE", *b"SNAM", Opaque), // two unused bytes
    (*b"RACE", *b"SPLO", FormId), // racial spell
    (*b"RACE", *b"UNAM", Opaque), // f32 FaceGen face clamp
    (*b"RACE", *b"VNAM", FORM_ID_PAIR_8), // voice, male then female
    (*b"RACE", *b"XNAM", FORM_ID_THEN_8), // faction + reaction modifier
    // ---- REFR ----
    (*b"REFR", *b"DATA", PLAIN_24),
    (*b"REFR", *b"EDID", ZString),
    (*b"REFR", *b"FNAM", Opaque),
    (*b"REFR", *b"FULL", ZString),
    (*b"REFR", *b"NAME", FormId), // base object
    (*b"REFR", *b"ONAM", Opaque), // open by default
    (*b"REFR", *b"TNAM", Opaque), // map marker type
    (*b"REFR", *b"XACT", Opaque), // action flags
    (*b"REFR", *b"XCHG", Opaque), // f32 charge -- NOT a FormID
    (*b"REFR", *b"XCNT", Opaque), // i32 count -- NOT a FormID
    (*b"REFR", *b"XESP", FORM_ID_THEN_8),
    (*b"REFR", *b"XGLB", FormId), // global
    (*b"REFR", *b"XHLT", Opaque), // i32 health -- NOT a FormID
    (*b"REFR", *b"XLCM", Opaque), // i32 level modifier -- NOT a FormID
    (*b"REFR", *b"XLOC", Struct {
        sizes: &[12],
        form_id_offsets: &[4], // lock level, then the key FormID
    }),
    (*b"REFR", *b"XLOD", Opaque),
    (*b"REFR", *b"XMRK", Opaque), // zero-length map marker flag
    (*b"REFR", *b"XOWN", FormId),
    (*b"REFR", *b"XRNK", Opaque), // i32 faction rank
    (*b"REFR", *b"XRTM", FormId), // REFR
    (*b"REFR", *b"XSCL", Opaque), // f32 scale -- NOT a FormID
    (*b"REFR", *b"XSED", Opaque), // SpeedTree seed
    (*b"REFR", *b"XTEL", FORM_ID_THEN_28), // destination door + 24B transform
    (*b"REFR", *b"XTRG", FormId), // target reference
    // ---- REGN ----
    (*b"REGN", *b"EDID", ZString),
    (*b"REGN", *b"ICON", ZString),
    (*b"REGN", *b"RCLR", Opaque), // map colour
    (*b"REGN", *b"RDAT", Opaque), // data-type header
    (*b"REGN", *b"RDGS", GRASS_ENTRY),
    (*b"REGN", *b"RDMD", Opaque), // u32 music type
    (*b"REGN", *b"RDMP", ZString), // map name
    (*b"REGN", *b"RDOT", REGION_OBJECT),
    (*b"REGN", *b"RDSD", REGION_SOUND),
    (*b"REGN", *b"RDWT", WEATHER_CHANCE),
    (*b"REGN", *b"RPLD", Opaque), // point list floats
    (*b"REGN", *b"RPLI", Opaque), // u32 edge falloff
    (*b"REGN", *b"WNAM", FormId), // WRLD
    // ---- ROAD ----
    (*b"ROAD", *b"PGRP", Opaque), // point positions
    (*b"ROAD", *b"PGRR", Opaque), // connections
    // ---- SBSP ----
    (*b"SBSP", *b"DNAM", Opaque), // x/y/z floats
    (*b"SBSP", *b"EDID", ZString),
    // ---- SCPT ----
    (*b"SCPT", *b"EDID", ZString),
    (*b"SCPT", *b"SCDA", Opaque), // see QUST/SCDA
    (*b"SCPT", *b"SCHR", Opaque),
    (*b"SCPT", *b"SCRO", FormId), // referenced object -- rewritten in place
    (*b"SCPT", *b"SCRV", Opaque), // local VARIABLE INDEX -- NOT a FormID
    (*b"SCPT", *b"SCTX", ZString),
    (*b"SCPT", *b"SCVR", ZString),
    (*b"SCPT", *b"SLSD", Opaque),
    // ---- SGST ----
    (*b"SGST", *b"DATA", Opaque), // uses, value, weight
    (*b"SGST", *b"EDID", ZString),
    (*b"SGST", *b"EFID", Opaque), // 4-char effect signature -- NOT a FormID
    (*b"SGST", *b"EFIT", Opaque),
    (*b"SGST", *b"FULL", ZString),
    (*b"SGST", *b"ICON", ZString),
    (*b"SGST", *b"MODB", Opaque),
    (*b"SGST", *b"MODL", ZString),
    (*b"SGST", *b"MODT", Opaque),
    // ---- SKIL ----
    (*b"SKIL", *b"ANAM", ZString),
    (*b"SKIL", *b"DATA", Opaque), // action, attribute, specialization
    (*b"SKIL", *b"DESC", ZString),
    (*b"SKIL", *b"EDID", ZString),
    (*b"SKIL", *b"ENAM", ZString),
    (*b"SKIL", *b"ICON", ZString),
    (*b"SKIL", *b"INDX", Opaque), // s32 skill index
    (*b"SKIL", *b"JNAM", ZString),
    (*b"SKIL", *b"MNAM", ZString),
    // ---- SLGM ----
    (*b"SLGM", *b"DATA", Opaque), // value, weight
    (*b"SLGM", *b"EDID", ZString),
    (*b"SLGM", *b"FULL", ZString),
    (*b"SLGM", *b"ICON", ZString),
    (*b"SLGM", *b"MODB", Opaque),
    (*b"SLGM", *b"MODL", ZString),
    (*b"SLGM", *b"MODT", Opaque),
    (*b"SLGM", *b"SCRI", FormId),
    (*b"SLGM", *b"SLCP", Opaque), // u8 soul capacity
    (*b"SLGM", *b"SOUL", Opaque), // u8 contained soul
    // ---- SOUN ----
    (*b"SOUN", *b"EDID", ZString),
    (*b"SOUN", *b"FNAM", ZString), // sound filename
    (*b"SOUN", *b"SNDX", Opaque),
    // ---- SPEL ----
    (*b"SPEL", *b"EDID", ZString),
    (*b"SPEL", *b"EFID", Opaque), // 4-char effect signature -- NOT a FormID
    (*b"SPEL", *b"EFIT", Opaque),
    (*b"SPEL", *b"FULL", ZString),
    (*b"SPEL", *b"SCIT", SCRIPT_EFFECT),
    (*b"SPEL", *b"SPIT", Opaque), // type, cost, level, flags
    // ---- STAT ----
    (*b"STAT", *b"EDID", ZString),
    (*b"STAT", *b"MODB", Opaque),
    (*b"STAT", *b"MODL", ZString),
    (*b"STAT", *b"MODT", Opaque),
    // ---- TES4 ----
    (*b"TES4", *b"CNAM", ZString), // author
    (*b"TES4", *b"DATA", Opaque), // master file size
    (*b"TES4", *b"DELE", Opaque),
    (*b"TES4", *b"HEDR", Opaque),
    (*b"TES4", *b"MAST", ZString),
    (*b"TES4", *b"OFST", Opaque),
    (*b"TES4", *b"SNAM", ZString), // description
    // ---- TREE ----
    (*b"TREE", *b"BNAM", Opaque), // leaf curvature floats
    (*b"TREE", *b"CNAM", Opaque), // SpeedTree growth parameters
    (*b"TREE", *b"EDID", ZString),
    (*b"TREE", *b"ICON", ZString),
    (*b"TREE", *b"MODB", Opaque),
    (*b"TREE", *b"MODL", ZString),
    (*b"TREE", *b"MODT", Opaque),
    (*b"TREE", *b"SNAM", Opaque), // SpeedTree seeds -- u32 array, NOT FormIDs
    // ---- WATR ----
    (*b"WATR", *b"ANAM", Opaque), // u8 opacity
    (*b"WATR", *b"DATA", Opaque), // water properties
    (*b"WATR", *b"EDID", ZString),
    (*b"WATR", *b"FNAM", Opaque), // u8 flags
    (*b"WATR", *b"GNAM", RELATED_WATERS),
    (*b"WATR", *b"MNAM", ZString), // material id
    (*b"WATR", *b"SNAM", FormId), // SOUN
    (*b"WATR", *b"TNAM", ZString), // texture
    // ---- WEAP ----
    (*b"WEAP", *b"ANAM", Opaque), // u16 enchantment points -- NOT DOOR/ANAM
    (*b"WEAP", *b"DATA", Opaque),
    (*b"WEAP", *b"EDID", ZString),
    (*b"WEAP", *b"ENAM", FormId), // enchantment
    (*b"WEAP", *b"FULL", ZString),
    (*b"WEAP", *b"ICON", ZString),
    (*b"WEAP", *b"MODB", Opaque),
    (*b"WEAP", *b"MODL", ZString),
    (*b"WEAP", *b"MODT", Opaque),
    (*b"WEAP", *b"SCRI", FormId),
    // ---- WRLD ----
    (*b"WRLD", *b"CNAM", FormId), // climate
    (*b"WRLD", *b"DATA", Opaque),
    (*b"WRLD", *b"EDID", ZString),
    (*b"WRLD", *b"FULL", ZString),
    (*b"WRLD", *b"ICON", ZString),
    (*b"WRLD", *b"MNAM", Opaque), // map data
    (*b"WRLD", *b"NAM0", Opaque), // min object bounds
    (*b"WRLD", *b"NAM2", FormId), // water
    (*b"WRLD", *b"NAM9", Opaque), // max object bounds
    (*b"WRLD", *b"SNAM", FormId), // water (alternate)
    (*b"WRLD", *b"WNAM", FormId), // parent worldspace
    // ---- WTHR ----
    (*b"WTHR", *b"CNAM", ZString), // cloud texture layer 0
    (*b"WTHR", *b"DATA", Opaque), // wind speed, fog, colours, flags
    (*b"WTHR", *b"DNAM", ZString), // cloud texture layer 1
    (*b"WTHR", *b"EDID", ZString),
    (*b"WTHR", *b"FNAM", Opaque), // fog distances
    (*b"WTHR", *b"HNAM", Opaque), // HDR parameters
    (*b"WTHR", *b"MODB", Opaque), // f32 bound radius
    (*b"WTHR", *b"MODL", ZString),
    (*b"WTHR", *b"NAM0", Opaque), // 160-byte colour table
    (*b"WTHR", *b"SNAM", FORM_ID_THEN_8), // sound + type, repeating
];

pub fn lookup(record: [u8; 4], field: [u8; 4]) -> Option<FieldKind> {
    FIELDS
        .binary_search_by(|(r, f, _)| (r, f).cmp(&(&record, &field)))
        .ok()
        .map(|position| FIELDS[position].2)
}

/// Whether the table describes this record type at all.
///
/// Distinguishes "unknown record" from "known record, unknown field", which
/// changes what the user has to go and look up.
pub fn knows_record(record: [u8; 4]) -> bool {
    FIELDS.iter().any(|(r, _, _)| *r == record)
}

/// Record signatures currently covered.
pub fn known_records() -> Vec<[u8; 4]> {
    let mut out: Vec<[u8; 4]> = FIELDS.iter().map(|(r, _, _)| *r).collect();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_has_no_duplicates() {
        // lookup() binary-searches, so this is a correctness requirement.
        for pair in FIELDS.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!(
                (a.0, a.1) < (b.0, b.1),
                "table out of order at {}/{} before {}/{}",
                String::from_utf8_lossy(&a.0),
                String::from_utf8_lossy(&a.1),
                String::from_utf8_lossy(&b.0),
                String::from_utf8_lossy(&b.1),
            );
        }
    }

    #[test]
    fn struct_offsets_fit_inside_every_declared_size() {
        for (record, field, kind) in FIELDS {
            if let Struct {
                sizes,
                form_id_offsets,
            } = kind
            {
                for size in *sizes {
                    for offset in *form_id_offsets {
                        assert!(
                            offset + 4 <= *size,
                            "{}/{}: FormID at offset {offset} does not fit in {size} bytes",
                            String::from_utf8_lossy(record),
                            String::from_utf8_lossy(field),
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn same_field_signature_can_differ_between_records() {
        // The reason the key is the pair: a sound FormID vs an integer.
        assert_eq!(lookup(*b"DOOR", *b"ANAM"), Some(FormId));
        assert_eq!(lookup(*b"WEAP", *b"ANAM"), Some(Opaque));
    }

    #[test]
    fn covers_every_record_type_unique_forts_uses() {
        let required: &[&[u8; 4]] = &[
            b"REFR", b"CELL", b"PGRD", b"PACK", b"ACHR", b"NPC_", b"SCPT", b"WRLD", b"LAND",
            b"QUST", b"ACRE", b"ACTI", b"FACT", b"STAT", b"BOOK", b"KEYM", b"CONT", b"DOOR",
            b"ENCH", b"WEAP", b"SOUN",
        ];
        for record in required {
            assert!(
                knows_record(**record),
                "{} is used by Unique Forts but missing from the table",
                String::from_utf8_lossy(*record)
            );
        }
    }

    #[test]
    fn unknown_pairs_return_none() {
        assert!(lookup(*b"ZZZZ", *b"EDID").is_none());
        assert!(lookup(*b"STAT", *b"ZZZZ").is_none());
    }
}
