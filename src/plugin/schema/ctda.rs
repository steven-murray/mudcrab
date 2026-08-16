//! CTDA condition parameters.
//!
//! A condition is 24 bytes: type u8, unused[3], comparison f32, function u32,
//! param1 u32, param2 u32, unused u32. Whether the parameters hold FormIDs
//! depends entirely on the function index -- `GetStage` takes a QUST FormID,
//! `GetRandomPercent` takes nothing, `GetQuestVariable` takes a QUST FormID
//! *and* a plain variable index.
//!
//! Every classification below was confirmed empirically: for each function,
//! comparing the parameter in a merge's source plugin against the same
//! parameter in zMerge's output shows whether zMerge rewrote it. See
//! `MOFAM-test/notes/merge-recon.md`.
//!
//! An unlisted function index is an error. The table is filled demand-driven
//! from what `plugin-audit` actually finds, because guessing which parameters
//! are FormIDs is exactly how a merge corrupts a plugin.

use super::SchemaError;
use crate::plugin::formid::FormId;

pub const CTDA_SIZE: usize = 24;
const PARAM1_OFFSET: usize = 12;
const PARAM2_OFFSET: usize = 16;

/// What a condition parameter holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// Not a FormID: an integer, an actor value, a variable index, or unused.
    Value,
    /// A FormID that must be rewritten when the plugin is renumbered.
    FormId,
}

use ParamKind::{FormId as F, Value as V};

/// `(function index, param1, param2)`, sorted by function index.
///
/// Confirmed-FormID entries are marked; the rest were observed unchanged
/// across all six merges *and* carry values inconsistent with a FormID.
static FUNCTIONS: &[(u32, ParamKind, ParamKind)] = &[
    (1, F, V),   // GetDistance(ObjectRef)                  -- rewritten in corpus
    (14, V, V),  // GetActorValue(ActorValue)
    (46, V, V),  // GetDead -- takes no parameters
    (47, F, V),  // GetItemCount(InventoryObject)           -- rewritten in corpus
    (48, V, V),  // GetGold
    (50, V, V),  // GetSleeping
    (53, V, V),  // GetScriptVariable
    (56, F, V),  // GetQuestRunning(Quest)                  -- rewritten in corpus
    (58, F, V),  // GetStage(Quest)                         -- rewritten in corpus
    (59, F, V),  // GetStageDone(Quest)                     -- rewritten in corpus
    (67, F, V),  // GetInCell(Cell)                         -- rewritten in corpus
    (68, V, V),  // GetIsClass(Class)
    (69, V, V),  // GetIsRace(Race)
    (70, V, V),  // GetIsSex
    (71, F, V),  // GetInFaction(Faction)                   -- rewritten in corpus
    (72, F, V),  // GetIsID(Object)                         -- rewritten in corpus
    (73, V, V),  // GetFactionRank
    (74, F, V),  // GetGlobalValue(Global)                  -- rewritten in corpus
    (76, V, V),  // GetDisposition
    (77, V, V),  // GetRandomPercent
    (79, F, V),  // GetQuestVariable(Quest, variable index) -- rewritten in corpus
    (80, V, V),  // GetLevel
    (84, V, V),  // GetDeadCount
    (101, V, V), // IsWeaponOut
    (107, V, V), // GetKnockedState
    (110, V, V), // GetCurrentAIPackage
    (125, V, V), // IsGuard
    (131, V, V), // GetPCInFaction
    (141, V, V), // IsInMyOwnedCell
    (143, V, V), // GetCurrentWeatherPercent
    (145, V, V), // IsContinuingPackagePCNear
    (146, V, V), // CanHaveFlames
    (185, V, V), // IsSneaking
    (230, V, V), // GetTimeDead
    (251, V, V), // IsLeftUp
    (254, V, V), // IsEssential -- takes no parameters
    (286, V, V), // GetPersuasionNumber
    (365, V, V), // OBSE/extended -- observed with both parameters always zero
];

fn lookup(function: u32) -> Option<(ParamKind, ParamKind)> {
    FUNCTIONS
        .binary_search_by_key(&function, |(index, _, _)| *index)
        .ok()
        .map(|position| (FUNCTIONS[position].1, FUNCTIONS[position].2))
}

/// FormID byte offsets inside one CTDA payload.
pub fn form_id_offsets(
    record_sig: [u8; 4],
    data: &[u8],
    form_id: FormId,
) -> Result<Vec<usize>, SchemaError> {
    // Truncated CTDAs exist in the wild; without a function index we cannot
    // know what the parameters are, so refuse rather than assume.
    if data.len() < PARAM2_OFFSET + 4 {
        return Err(SchemaError::FieldSizeMismatch {
            record: record_sig,
            field: *b"CTDA",
            form_id,
            expected: &[CTDA_SIZE],
            actual: data.len(),
        });
    }

    let function = u32::from_le_bytes(data[8..12].try_into().unwrap());
    let (param1, param2) =
        lookup(function).ok_or(SchemaError::UnknownConditionFunction {
            record: record_sig,
            form_id,
            function,
        })?;

    let mut offsets = Vec::new();
    if param1 == F {
        offsets.push(PARAM1_OFFSET);
    }
    if param2 == F {
        offsets.push(PARAM2_OFFSET);
    }
    Ok(offsets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctda(function: u32) -> Vec<u8> {
        let mut data = vec![0u8; CTDA_SIZE];
        data[8..12].copy_from_slice(&function.to_le_bytes());
        data
    }

    #[test]
    fn table_is_sorted_and_unique() {
        // binary_search depends on this.
        for pair in FUNCTIONS.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "CTDA function table out of order at {} / {}",
                pair[0].0,
                pair[1].0
            );
        }
    }

    #[test]
    fn quest_functions_expose_param1_only() {
        // GetStage(Quest) -- param2 is a comparison value, not a FormID.
        assert_eq!(
            form_id_offsets(*b"QUST", &ctda(58), FormId(0)).unwrap(),
            vec![PARAM1_OFFSET]
        );
        // GetQuestVariable(Quest, variable index) -- param2 is the index.
        assert_eq!(
            form_id_offsets(*b"PACK", &ctda(79), FormId(0)).unwrap(),
            vec![PARAM1_OFFSET]
        );
    }

    #[test]
    fn parameterless_functions_expose_nothing() {
        for function in [46, 77, 254, 365] {
            assert!(
                form_id_offsets(*b"PACK", &ctda(function), FormId(0))
                    .unwrap()
                    .is_empty(),
                "function {function} should have no FormID parameters"
            );
        }
    }

    #[test]
    fn unknown_function_is_an_error_not_a_guess() {
        let err = form_id_offsets(*b"PACK", &ctda(9999), FormId(0x0100_0801)).unwrap_err();
        assert!(matches!(
            err,
            SchemaError::UnknownConditionFunction { function: 9999, .. }
        ));
        assert!(err.to_string().contains("9999"));
    }

    #[test]
    fn truncated_condition_is_an_error() {
        assert!(form_id_offsets(*b"PACK", &[0u8; 8], FormId(0)).is_err());
    }

    #[test]
    fn every_function_unique_forts_uses_is_covered() {
        // Measured from the merge's source plugins.
        for function in [1u32, 46, 58, 67, 79, 254, 365] {
            assert!(
                lookup(function).is_some(),
                "condition function {function} is used by Unique Forts but missing"
            );
        }
    }
}
