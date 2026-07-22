//! Structural skip of Wwise `NodeBaseParams` for bank version **150**.
//!
//! New World ships `BKHD.dwBankGeneratorVersion == 150` (Wwise 2019.1 authoring).
//! The 3-26 client links Wwise **2023.1** (`wwise_v2023.1` SDK paths) but still
//! deserializes these banks via `CAkParameterNodeBase::SetNodeBaseParams`
//! (`NewWorld 3-26` `FUN_7ff6072edc00`). Container tails (`CAkSwitchCntr`,
//! `CAkRanSeqCntr`) sit immediately after a full `NodeBaseParams` block.
//! Sliding-byte searches for those tails can false-match and then allocate from
//! garbage counts; this module walks the authored layout instead.
//!
//! Ghidra cross-check (SwitchCntr FX path `FUN_7ff607318550`): override `u8`,
//! `numFx` `u8`, then when `numFx > 0` a bypass `u8` plus `numFx` ×
//! `{uFXIndex u8, fxID u32, bitVector u8}` (**6** bytes). Metadata
//! (`FUN_7ff607318700`) is the same 6-byte record without the bypass byte.
//!
//! Layout after the object id, for a parameter node (sound / ranseq / switch /
//! layer / actor-mixer):
//!
//! 1. `NodeInitialFxParams` — override + count; when count > 0, `bBypassAll` and
//!    `count` records of `{uFXIndex u8, fxID u32, bitVector u8}` (**6** bytes;
//!    v150 is past the v145 `bIsShareSet`/`bIsRendered` pair).
//! 2. `SetInitialMetadataParams` — override + count + `count` × `{uFXIndex, fxID,
//!    bIsShareSet}` (6 bytes each).
//! 3. `OverrideBusId` `u32`, `DirectParentID` `u32`, priority `byBitVector` `u8`.
//! 4. Two `AkPropBundle`s (value props, then ranged modifiers).
//! 5. `PositioningParams`, `AuxParams` (incl. `reflectionsAuxBus`),
//!    `AdvSettingsParams`, `StateChunk` (7-bit `var` counts), `InitialRTPC`.

/// Effect record in `NodeInitialFxParams` at bank version 150: index, id, flags.
const FX_RECORD_LEN_V150: usize = 1 + 4 + 1;
/// Metadata-effect record: index, id, share flag (no rendered byte).
const METADATA_FX_RECORD_LEN: usize = 1 + 4 + 1;
/// Hard caps so a corrupt count cannot walk off into multi-gigabyte “skips”.
const MAX_FX: usize = 16;
const MAX_PROPS: usize = 64;
const MAX_STATE_PROPS: usize = 256;
const MAX_STATE_GROUPS: usize = 256;
const MAX_STATES: usize = 1024;
const MAX_RTPC_CURVES: usize = 1024;
const MAX_PATH_VERTICES: usize = 4096;
const MAX_PATH_PLAYLIST: usize = 4096;
const MAX_GRAPH_POINTS: usize = 4096;

/// Skip a complete v150 `NodeBaseParams` starting at `cursor`. Returns the offset
/// of the first byte after the block, or `None` on truncation / absurd counts.
#[must_use]
pub fn skip_node_base_params_v150(body: &[u8], cursor: usize) -> Option<usize> {
    let cursor = skip_initial_fx_params_v150(body, cursor)?;
    let cursor = skip_initial_metadata_params_v150(body, cursor)?;
    let cursor = cursor.checked_add(4)?; // OverrideBusId
    let cursor = cursor.checked_add(4)?; // DirectParentID
    let cursor = cursor.checked_add(1)?; // byBitVector (priority / MIDI)
    let cursor = skip_prop_bundle(body, cursor)?;
    let cursor = skip_ranged_prop_bundle(body, cursor)?;
    let cursor = skip_positioning_params_v150(body, cursor)?;
    let cursor = skip_aux_params_v150(body, cursor)?;
    let cursor = skip_adv_settings_params_v150(body, cursor)?;
    let cursor = skip_state_chunk_v150(body, cursor)?;
    skip_initial_rtpc_v150(body, cursor)
}

/// Read `DirectParentID` from a v150 parameter-node body that begins at `cursor`
/// (containers: 0; sounds: after `AkBankSourceData`).
#[must_use]
pub fn read_direct_parent_id_v150(body: &[u8], cursor: usize) -> Option<u32> {
    let cursor = skip_initial_fx_params_v150(body, cursor)?;
    let cursor = skip_initial_metadata_params_v150(body, cursor)?;
    let _bus = read_u32(body, cursor)?;
    let cursor = cursor.checked_add(4)?;
    read_u32(body, cursor)
}

fn skip_initial_fx_params_v150(body: &[u8], mut cursor: usize) -> Option<usize> {
    let _override = *body.get(cursor)?;
    cursor = cursor.checked_add(1)?;
    let num_fx = *body.get(cursor)? as usize;
    cursor = cursor.checked_add(1)?;
    if num_fx > MAX_FX {
        return None;
    }
    if num_fx > 0 {
        cursor = cursor.checked_add(1)?; // bBypassAll (v150)
        cursor = cursor.checked_add(num_fx.checked_mul(FX_RECORD_LEN_V150)?)?;
    }
    (cursor <= body.len()).then_some(cursor)
}

fn skip_initial_metadata_params_v150(body: &[u8], mut cursor: usize) -> Option<usize> {
    let _override = *body.get(cursor)?;
    cursor = cursor.checked_add(1)?;
    let num = *body.get(cursor)? as usize;
    cursor = cursor.checked_add(1)?;
    if num > MAX_FX {
        return None;
    }
    if num > 0 {
        cursor = cursor.checked_add(num.checked_mul(METADATA_FX_RECORD_LEN)?)?;
    }
    (cursor <= body.len()).then_some(cursor)
}

fn skip_prop_bundle(body: &[u8], mut cursor: usize) -> Option<usize> {
    let count = *body.get(cursor)? as usize;
    cursor = cursor.checked_add(1)?;
    if count > MAX_PROPS {
        return None;
    }
    // cProps × u8 pID, then cProps × u32 uni value.
    cursor = cursor.checked_add(count)?;
    cursor = cursor.checked_add(count.checked_mul(4)?)?;
    (cursor <= body.len()).then_some(cursor)
}

fn skip_ranged_prop_bundle(body: &[u8], mut cursor: usize) -> Option<usize> {
    let count = *body.get(cursor)? as usize;
    cursor = cursor.checked_add(1)?;
    if count > MAX_PROPS {
        return None;
    }
    cursor = cursor.checked_add(count)?;
    cursor = cursor.checked_add(count.checked_mul(8)?)?; // min + max
    (cursor <= body.len()).then_some(cursor)
}

fn skip_positioning_params_v150(body: &[u8], mut cursor: usize) -> Option<usize> {
    let bits = *body.get(cursor)?;
    cursor = cursor.checked_add(1)?;
    let has_positioning = bits & 1 != 0;
    let has_3d = has_positioning && (bits >> 1) & 1 != 0;
    if !(has_positioning && has_3d) {
        return Some(cursor);
    }
    let _bits3d = *body.get(cursor)?;
    cursor = cursor.checked_add(1)?;
    // e3DPositionType from uBitsPositioning[5:6]; automation when != 0.
    let e3d_position_type = (bits >> 5) & 3;
    let has_automation = e3d_position_type != 0;
    if !has_automation {
        return Some(cursor);
    }
    let _path_mode = *body.get(cursor)?;
    cursor = cursor.checked_add(1)?;
    let _transition = read_u32(body, cursor)?; // s32
    cursor = cursor.checked_add(4)?;
    let num_vertices = read_u32(body, cursor)? as usize;
    cursor = cursor.checked_add(4)?;
    if num_vertices > MAX_PATH_VERTICES {
        return None;
    }
    cursor = cursor.checked_add(num_vertices.checked_mul(16)?)?; // xyz f32 + duration s32
    let num_playlist = read_u32(body, cursor)? as usize;
    cursor = cursor.checked_add(4)?;
    if num_playlist > MAX_PATH_PLAYLIST {
        return None;
    }
    cursor = cursor.checked_add(num_playlist.checked_mul(8)?)?; // offset + count
    // Ak3DAutomationParams per playlist item: fX/Y/ZRange.
    cursor = cursor.checked_add(num_playlist.checked_mul(12)?)?;
    (cursor <= body.len()).then_some(cursor)
}

fn skip_aux_params_v150(body: &[u8], mut cursor: usize) -> Option<usize> {
    let bits = *body.get(cursor)?;
    cursor = cursor.checked_add(1)?;
    let has_aux = (bits >> 3) & 1 != 0;
    if has_aux {
        cursor = cursor.checked_add(16)?; // four auxID u32
    }
    // reflectionsAuxBus (v150 > 134).
    cursor = cursor.checked_add(4)?;
    (cursor <= body.len()).then_some(cursor)
}

fn skip_adv_settings_params_v150(body: &[u8], mut cursor: usize) -> Option<usize> {
    // byBitVector, eVirtualQueueBehavior, u16MaxNumInstance,
    // eBelowThresholdBehavior, byBitVector.
    cursor = cursor.checked_add(1 + 1 + 2 + 1 + 1)?;
    (cursor <= body.len()).then_some(cursor)
}

fn skip_state_chunk_v150(body: &[u8], mut cursor: usize) -> Option<usize> {
    let (num_props, next) = read_var(body, cursor)?;
    cursor = next;
    if num_props > MAX_STATE_PROPS {
        return None;
    }
    for _ in 0..num_props {
        let (_, next) = read_var(body, cursor)?;
        cursor = next;
        cursor = cursor.checked_add(1)?; // accumType
        cursor = cursor.checked_add(1)?; // inDb
    }
    let (num_groups, next) = read_var(body, cursor)?;
    cursor = next;
    if num_groups > MAX_STATE_GROUPS {
        return None;
    }
    for _ in 0..num_groups {
        cursor = cursor.checked_add(4)?; // ulStateGroupID
        cursor = cursor.checked_add(1)?; // eStateSyncType
        let (num_states, next) = read_var(body, cursor)?;
        cursor = next;
        if num_states > MAX_STATES {
            return None;
        }
        for _ in 0..num_states {
            cursor = cursor.checked_add(4)?; // ulStateID
            // v150 > 145: inline AkPropBundle<float, unsigned short> instead of
            // a bare ulStateInstanceID.
            cursor = skip_float_u16_prop_bundle(body, cursor)?;
        }
    }
    (cursor <= body.len()).then_some(cursor)
}

fn skip_float_u16_prop_bundle(body: &[u8], mut cursor: usize) -> Option<usize> {
    let count = read_u16(body, cursor)? as usize;
    cursor = cursor.checked_add(2)?;
    if count > MAX_PROPS {
        return None;
    }
    cursor = cursor.checked_add(count.checked_mul(2)?)?; // pID u16
    cursor = cursor.checked_add(count.checked_mul(4)?)?; // pValue f32
    (cursor <= body.len()).then_some(cursor)
}

fn skip_initial_rtpc_v150(body: &[u8], mut cursor: usize) -> Option<usize> {
    let num_curves = read_u16(body, cursor)? as usize;
    cursor = cursor.checked_add(2)?;
    if num_curves > MAX_RTPC_CURVES {
        return None;
    }
    for _ in 0..num_curves {
        cursor = cursor.checked_add(4)?; // RTPCID
        cursor = cursor.checked_add(1)?; // rtpcType
        cursor = cursor.checked_add(1)?; // rtpcAccum
        let (_, next) = read_var(body, cursor)?;
        cursor = next; // ParamID
        cursor = cursor.checked_add(4)?; // rtpcCurveID
        cursor = cursor.checked_add(1)?; // eScaling
        let points = read_u16(body, cursor)? as usize;
        cursor = cursor.checked_add(2)?;
        if points > MAX_GRAPH_POINTS {
            return None;
        }
        // AkRTPCGraphPoint: From f32, To f32, Interp u32.
        cursor = cursor.checked_add(points.checked_mul(12)?)?;
    }
    (cursor <= body.len()).then_some(cursor)
}

/// Wwise `var` / 7-bit continuation integer (same encoding as HIRC event action
/// counts).
fn read_var(body: &[u8], mut cursor: usize) -> Option<(usize, usize)> {
    let mut byte = *body.get(cursor)?;
    cursor = cursor.checked_add(1)?;
    let mut value = u32::from(byte & 0x7f);
    let mut loops = 0u8;
    while byte & 0x80 != 0 {
        byte = *body.get(cursor)?;
        cursor = cursor.checked_add(1)?;
        if value > u32::MAX >> 7 || loops >= 10 {
            return None;
        }
        value = (value << 7) | u32::from(byte & 0x7f);
        loops += 1;
    }
    Some((value as usize, cursor))
}

fn read_u16(body: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(body.get(offset..end)?.try_into().ok()?))
}

fn read_u32(body: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(body.get(offset..end)?.try_into().ok()?))
}

/// Empty v150 `NodeBaseParams` with the given `DirectParentID` (no FX, props,
/// positioning, state, or RTPC). Length is fixed at 31 bytes.
#[cfg(test)]
#[must_use]
pub fn empty_node_base_params_v150(parent: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(31);
    body.extend_from_slice(&[0, 0]); // FX override + count
    body.extend_from_slice(&[0, 0]); // metadata override + count
    body.extend_from_slice(&0u32.to_le_bytes()); // OverrideBusId
    body.extend_from_slice(&parent.to_le_bytes());
    body.push(0); // priority byBitVector
    body.push(0); // prop cProps
    body.push(0); // ranged cProps
    body.push(0); // positioning bits
    body.push(0); // aux bits
    body.extend_from_slice(&0u32.to_le_bytes()); // reflectionsAuxBus
    body.push(0); // adv byBitVector
    body.push(0); // eVirtualQueueBehavior
    body.extend_from_slice(&0u16.to_le_bytes()); // u16MaxNumInstance
    body.push(0); // eBelowThresholdBehavior
    body.push(0); // adv byBitVector
    body.push(0); // state props var
    body.push(0); // state groups var
    body.extend_from_slice(&0u16.to_le_bytes()); // RTPC curves
    debug_assert_eq!(body.len(), 31);
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_node_base_is_31_bytes_and_skips_cleanly() {
        let body = empty_node_base_params_v150(42);
        assert_eq!(skip_node_base_params_v150(&body, 0), Some(31));
        assert_eq!(read_direct_parent_id_v150(&body, 0), Some(42));
    }

    #[test]
    fn alligator_switch_node_base_ends_before_group_header() {
        // Body of switch 61188014 from ftsp_alligator_events.bnk (version 150).
        let body = hex_literal(
            "0000000000000000dcb0f53a00010c0000a0c1000002000000001002000003000000000000\
             bcb3c650be6db17d0002000000c375590241871f2304000000ff60b76f0100000041871f23\
             be6db17d0100000041871f23a8b9722a01000000c3755902c17c464d01000000c375590202\
             000000c37559020001000000000000000041871f2300010000000000000000",
        );
        assert_eq!(body.len(), 142);
        let end = skip_node_base_params_v150(&body, 0).expect("skip");
        assert_eq!(end, 36);
        assert_eq!(body[end], 0); // eGroupType
        assert_eq!(read_u32(&body, end + 5), Some(2_108_779_966)); // ulDefaultSwitch
        assert_eq!(read_u32(&body, end + 10), Some(2)); // ulNumChilds
        assert_eq!(read_direct_parent_id_v150(&body, 0), Some(989_180_124));
    }

    #[test]
    fn gorilla_switch_with_one_fx_skips_six_byte_records() {
        // Head of switch 351325259 from ftsp_gorilla_events.bnk: one FX slot.
        let head = [
            1u8, 1, // override + numFx
            0, // bBypassAll
            0, 0, 0, 0, 0, 4, // FX record (6 bytes)
            0, 0, // metadata
            0, 0, 0, 0, // bus
            0x07, 0x83, 0x26, 0x35, // parent
        ];
        assert_eq!(read_direct_parent_id_v150(&head, 0), Some(0x3526_8307));
        let mut body = head.to_vec();
        body.extend_from_slice(&[
            0, // priority
            0, 0, // prop bundles
            0, // positioning
            0, 0, 0, 0, 0, // aux + reflections
            0, 0, 0, 0, 0, 0, // adv (6)
            0, 0, // state
            0, 0, // rtpc
        ]);
        assert_eq!(skip_node_base_params_v150(&body, 0), Some(body.len()));
    }

    #[test]
    fn skips_switch_and_ranseq_tails_on_shipped_event_banks() {
        let root = std::path::Path::new(r"C:\nwt\sounds\wwise");
        if !root.is_dir() {
            return;
        }
        let mut switch_ok = 0usize;
        let mut switch_total = 0usize;
        let mut ranseq_ok = 0usize;
        let mut ranseq_total = 0usize;
        let mut failures = Vec::new();
        for entry in std::fs::read_dir(root).expect("read wwise dir") {
            let path = entry.expect("entry").path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.contains("events") || path.extension().and_then(|e| e.to_str()) != Some("bnk")
            {
                continue;
            }
            let bytes = std::fs::read(&path).expect("read bank");
            let bank = crate::WwiseSoundBank::parse(&bytes).expect("parse bank");
            for object in &bank.hierarchy {
                let Some(body) = object.data(&bytes).and_then(|data| data.get(4..)) else {
                    continue;
                };
                if object.kind == crate::WwiseHierarchyObjectKind::SWITCH_CONTAINER {
                    switch_total += 1;
                    match skip_node_base_params_v150(body, 0) {
                        Some(end) if switch_tail_consumes(body, end) => switch_ok += 1,
                        other => failures.push(format!(
                            "{name} switch {} skip={other:?}",
                            object.object_id.0
                        )),
                    }
                } else if object.kind == crate::WwiseHierarchyObjectKind::RANDOM_SEQUENCE_CONTAINER
                {
                    ranseq_total += 1;
                    match skip_node_base_params_v150(body, 0) {
                        Some(end) if ranseq_tail_consumes(body, end) => ranseq_ok += 1,
                        other => failures.push(format!(
                            "{name} ranseq {} skip={other:?}",
                            object.object_id.0
                        )),
                    }
                }
            }
        }
        assert!(
            failures.is_empty(),
            "switch {switch_ok}/{switch_total} ranseq {ranseq_ok}/{ranseq_total}; first failures: {:?}",
            &failures[..failures.len().min(20)]
        );
        assert!(switch_total > 0 || ranseq_total > 0);
    }

    fn switch_tail_consumes(body: &[u8], mut cursor: usize) -> bool {
        (|| {
            cursor = cursor.checked_add(1 + 4 + 4 + 1)?; // group header
            let children = read_u32(body, cursor)? as usize;
            cursor = cursor
                .checked_add(4)?
                .checked_add(children.checked_mul(4)?)?;
            let groups = read_u32(body, cursor)? as usize;
            cursor = cursor.checked_add(4)?;
            for _ in 0..groups {
                cursor = cursor.checked_add(4)?;
                let items = read_u32(body, cursor)? as usize;
                cursor = cursor.checked_add(4)?.checked_add(items.checked_mul(4)?)?;
            }
            let params = read_u32(body, cursor)? as usize;
            cursor = cursor
                .checked_add(4)?
                .checked_add(params.checked_mul(14)?)?;
            Some(cursor == body.len())
        })()
        .unwrap_or(false)
    }

    fn ranseq_tail_consumes(body: &[u8], mut cursor: usize) -> bool {
        (|| {
            cursor = cursor.checked_add(24)?;
            let children = read_u32(body, cursor)? as usize;
            cursor = cursor
                .checked_add(4)?
                .checked_add(children.checked_mul(4)?)?;
            let items = read_u16(body, cursor)? as usize;
            cursor = cursor.checked_add(2)?.checked_add(items.checked_mul(8)?)?;
            Some(cursor == body.len())
        })()
        .unwrap_or(false)
    }

    fn hex_literal(hex: &str) -> Vec<u8> {
        let hex: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }
}
