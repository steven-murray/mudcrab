//! CTDA condition parameters.
//!
//! A condition is 24 bytes: type u8, unused[3], comparison f32, function u32,
//! param1 u32, param2 u32, unused u32. Whether the parameters hold FormIDs
//! depends entirely on the function index -- `GetStage` takes a QUST FormID,
//! `GetRandomPercent` takes nothing, `GetQuestVariable` takes a QUST FormID
//! *and* a plain variable index.
//!
//! The table is transcribed from xEdit's own `wbCTDAFunctions`, whose entries
//! carry a `ParamType` per parameter. Everything from `ptObjectReference`
//! onward in its `TCTDAFunctionParamType` enum names a record type and is a
//! FormID; everything before it (`ptInteger`, `ptActorValue`, `ptQuestStage`,
//! `ptVariableName`, ...) is not. All 192 functions are listed, so an unlisted
//! index means a plugin using a function TES4 does not define.
//!
//! It was previously derived empirically -- comparing each parameter against
//! zMerge's output to see whether zMerge rewrote it -- and **that method was
//! wrong for seven functions**: `GetScriptVariable`, `GetIsClass`, `GetIsRace`,
//! `GetFactionRank`, `GetDisposition`, `GetDeadCount` and `GetInCellParam` all
//! take a FormID that the MOFAM merges happened never to renumber, because the
//! records they pointed at were vanilla and so kept their ids. "Observed
//! unchanged" was true and meant nothing. A corpus can only show which fields
//! *did* change, never which ones *can*.

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
    (1, F, V), // GetDistance (ObjectReference)
    (5, V, V), // GetLocked
    (6, V, V), // GetPos (Axis)
    (8, V, V), // GetAngle (Axis)
    (10, V, V), // GetStartingPos (Axis)
    (11, V, V), // GetStartingAngle (Axis)
    (12, V, V), // GetSecondsPassed
    (14, V, V), // GetActorValue (ActorValue)
    (18, V, V), // GetCurrentTime
    (24, V, V), // GetScale
    (27, F, V), // GetLineOfSight (ObjectReference)
    (32, F, V), // GetInSameCell (ObjectReference)
    (35, V, V), // GetDisabled
    (36, V, V), // MenuMode (Integer)
    (39, V, V), // GetDisease
    (40, V, V), // GetVampire
    (41, V, V), // GetClothingValue
    (42, F, V), // SameFaction (Actor)
    (43, F, V), // SameRace (Actor)
    (44, F, V), // SameSex (Actor)
    (45, F, V), // GetDetected (Actor)
    (46, V, V), // GetDead
    (47, F, V), // GetItemCount (InventoryObject)
    (48, V, V), // GetGold
    (49, V, V), // GetSleeping
    (50, V, V), // GetTalkedToPC
    (53, F, V), // GetScriptVariable (ObjectReference, VariableName)
    (56, F, V), // GetQuestRunning (Quest)
    (58, F, V), // GetStage (Quest)
    (59, F, V), // GetStageDone (Quest, QuestStage)
    (60, F, F), // GetFactionRankDifference (Faction, Actor)
    (61, V, V), // GetAlarmed
    (62, V, V), // IsRaining
    (63, V, V), // GetAttacked
    (64, V, V), // GetIsCreature
    (65, V, V), // GetLockLevel
    (66, F, V), // GetShouldAttack (Actor)
    (67, F, V), // GetInCell (Cell)
    (68, F, V), // GetIsClass (Class)
    (69, F, V), // GetIsRace (Race)
    (70, V, V), // GetIsSex (Sex)
    (71, F, V), // GetInFaction (Faction)
    (72, F, V), // GetIsID (ReferencableObject)
    (73, F, V), // GetFactionRank (Faction)
    (74, F, V), // GetGlobalValue (Global)
    (75, V, V), // IsSnowing
    (76, F, V), // GetDisposition (Actor)
    (77, V, V), // GetRandomPercent
    (79, F, V), // GetQuestVariable (Quest, VariableName)
    (80, V, V), // GetLevel
    (81, V, V), // GetArmorRating
    (84, F, V), // GetDeadCount (ActorBase)
    (91, V, V), // GetIsAlerted
    (98, V, V), // GetPlayerControlsDisabled
    (99, F, V), // GetHeadingAngle (ObjectReference)
    (101, V, V), // IsWeaponOut
    (102, V, V), // IsTorchOut
    (103, V, V), // IsShieldOut
    (104, V, V), // IsYielding
    (106, V, V), // IsFacingUp
    (107, V, V), // GetKnockedState
    (108, V, V), // GetWeaponAnimType
    (109, V, V), // GetWeaponSkillType
    (110, V, V), // GetCurrentAIPackage
    (111, V, V), // IsWaiting
    (112, V, V), // IsIdlePlaying
    (116, V, V), // GetCrimeGold
    (122, F, V), // GetCrime (Actor, CrimeType)
    (125, V, V), // IsGuard
    (127, V, V), // CanPayCrimeGold
    (128, V, V), // GetFatiguePercentage
    (129, F, V), // GetPCIsClass (Class)
    (130, F, V), // GetPCIsRace (Race)
    (131, V, V), // GetPCIsSex (Sex)
    (132, F, V), // GetPCInFaction (Faction)
    (133, V, V), // SameFactionAsPC
    (134, V, V), // SameRaceAsPC
    (135, V, V), // SameSexAsPC
    (136, F, V), // GetIsReference (ObjectReference)
    (141, V, V), // IsTalking
    (142, V, V), // GetWalkSpeed
    (143, V, V), // GetCurrentAIProcedure
    (144, V, V), // GetTrespassWarningLevel
    (145, V, V), // IsTrespassing
    (146, V, V), // IsInMyOwnedCell
    (147, V, V), // GetWindSpeed
    (148, V, V), // GetCurrentWeatherPercent
    (149, F, V), // GetIsCurrentWeather (Weather)
    (150, V, V), // IsContinuingPackagePCNear
    (153, V, V), // CanHaveFlames
    (154, V, V), // HasFlames
    (157, V, V), // GetOpenState
    (159, V, V), // GetSitting
    (160, V, V), // GetFurnitureMarkerID
    (161, F, V), // GetIsCurrentPackage (Package)
    (162, F, V), // IsCurrentFurnitureRef (ObjectReference)
    (163, F, V), // IsCurrentFurnitureObj (Furniture)
    (170, V, V), // GetDayOfWeek
    (171, V, V), // IsPlayerInJail
    (172, F, V), // GetTalkedToPCParam (Actor)
    (175, V, V), // IsPCSleeping
    (176, V, V), // IsPCAMurderer
    (180, F, V), // GetDetectionLevel (Actor)
    (182, F, V), // GetEquipped (InventoryObject)
    (185, V, V), // IsSwimming
    (190, V, V), // GetAmountSoldStolen
    (193, F, V), // GetPCExpelled (Faction)
    (195, F, V), // GetPCFactionMurder (Faction)
    (197, F, V), // GetPCFactionSteal (Faction)
    (199, F, V), // GetPCFactionAttack (Faction)
    (201, F, V), // GetPCFactionSubmitAuthority (Faction)
    (203, V, V), // GetDestroyed
    (214, F, V), // HasMagicEffect (MagicEffect)
    (215, V, V), // GetDoorDefaultOpen
    (223, F, V), // IsSpellTarget (MagicItem)
    (224, F, V), // GetIsPlayerBirthsign (Birthsign)
    (225, V, V), // GetPersuasionNumber
    (227, V, V), // HasVampireFed
    (228, F, V), // GetIsClassDefault (Class)
    (229, V, V), // GetClassDefaultMatch
    (230, F, F), // GetInCellParam (Cell, ObjectReference)
    (237, V, V), // GetIsGhost
    (242, V, V), // GetUnconscious
    (244, V, V), // GetRestrained
    (246, F, V), // GetIsUsedItem (ReferencableObject)
    (247, V, V), // GetIsUsedItemType (FormType)
    (249, V, V), // GetPCFame
    (251, V, V), // GetPCInfamy
    (254, V, V), // GetIsPlayableRace
    (255, V, V), // GetOffersServicesNow
    (258, V, V), // GetUsedItemLevel
    (259, V, V), // GetUsedItemActivate
    (264, V, V), // GetBarterGold
    (265, V, V), // IsTimePassing
    (266, V, V), // IsPleasant
    (267, V, V), // IsCloudy
    (274, V, V), // GetArmorRatingUpperBody
    (277, V, V), // GetBaseActorValue (ActorValue)
    (278, F, V), // IsOwner (OwnerOpt)
    (280, F, F), // IsCellOwner (Cell, OwnerOpt)
    (282, V, V), // IsHorseStolen
    (285, V, V), // IsLeftUp
    (286, V, V), // IsSneaking
    (287, V, V), // IsRunning
    (288, F, V), // GetFriendHit (Actor)
    (289, V, V), // IsInCombat
    (300, V, V), // IsInInterior
    (305, V, V), // GetInvestmentGold
    (306, V, V), // IsActorUsingATorch
    (309, V, V), // IsXBox
    (310, F, V), // GetInWorldspace (WorldSpace)
    (312, V, V), // GetPCMiscStat (Integer)
    (313, V, V), // IsActorEvil
    (314, V, V), // IsActorAVictim
    (315, V, V), // GetTotalPersuasionNumber
    (318, V, V), // GetIdleDoneOnce
    (320, V, V), // GetNoRumors
    (323, V, V), // WhichServiceMenu
    (327, V, V), // IsRidingHorse
    (329, V, V), // IsTurnArrest
    (332, V, V), // IsInDangerousWater
    (338, V, V), // GetIgnoreFriendlyHits
    (339, V, V), // IsPlayersLastRiddenHorse
    (353, V, V), // IsActor
    (354, V, V), // IsEssential
    (358, V, V), // IsPlayerMovingIntoNewSpace
    (361, V, V), // GetTimeDead
    (362, V, V), // GetPlayerHasLastRiddenHorse
    (365, V, V), // GetPlayerInSEWorld
    (1107, V, V), // IsAmmo, (Integer)
    (1884, F, V), // GetPCTrainingSessionsUsed (Package)
    (2213, F, V), // GetPackageOffersServices (Package)
    (2214, F, V), // GetPackageMustReachLocation (Package)
    (2215, F, V), // GetPackageMustComplete (Package)
    (2216, F, V), // GetPackageLockDoorsAtStart (Package)
    (2217, F, V), // GetPackageLockDoorsAtEnd (Package)
    (2218, F, V), // GetPackageLockDoorsAtLocation (Package)
    (2219, F, V), // GetPackageUnlockDoorsAtStart (Package)
    (2220, F, V), // GetPackageUnlockDoorsAtEnd (Package)
    (2221, F, V), // GetPackageUnlockDoorsAtLocation (Package)
    (2222, F, V), // GetPackageContinueIfPCNear (Package)
    (2223, F, V), // GetPackageOncePerDay (Package)
    (2224, F, V), // GetPackageSkipFalloutBehavior (Package)
    (2225, F, V), // GetPackageAlwaysRun (Package)
    (2226, F, V), // GetPackageAlwaysSneak (Package)
    (2227, F, V), // GetPackageAllowSwimming (Package)
    (2228, F, V), // GetPackageAllowFalls (Package)
    (2229, F, V), // GetPackageArmorUnequipped (Package)
    (2230, F, V), // GetPackageWeaponsUnequipped (Package)
    (2231, F, V), // GetPackageDefensiveCombat (Package)
    (2232, F, V), // GetPackageUseHorse (Package)
    (2233, F, V), // GetPackageNoIdleAnims (Package)
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
