use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{NetworkSchema, NetworkSchemaImportError, invalidate_fields_for_handler_vtables};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkGhidraOverlayMergeReport {
    pub source_type_count: usize,
    pub ignored_registry_type_count: usize,
    pub source_field_registration_function_count: usize,
    pub replaced_field_registration_function_count: usize,
    pub added_field_registration_function_count: usize,
    pub source_field_handler_vtable_count: usize,
    pub replaced_field_handler_vtable_count: usize,
    pub added_field_handler_vtable_count: usize,
    pub rebound_dependent_field_count: usize,
}

impl NetworkSchema {
    /// Overlay focused Ghidra analysis onto a full normalized schema.
    ///
    /// Only address-keyed analysis catalogs are overlaid. Registry rows remain
    /// owned by the first report, preventing a handler-only run from erasing
    /// type and field metadata it did not analyze.
    pub fn merge_ghidra_static_network_overlay(
        &mut self,
        report: &Value,
    ) -> Result<NetworkGhidraOverlayMergeReport, NetworkSchemaImportError> {
        let overlay = Self::from_ghidra_static_network_report(report)?;
        self.merge_normalized_ghidra_overlay(overlay)
    }

    pub fn merge_normalized_ghidra_overlay(
        &mut self,
        overlay: NetworkSchema,
    ) -> Result<NetworkGhidraOverlayMergeReport, NetworkSchemaImportError> {
        validate_program_identity(self, &overlay)?;
        let mut result = NetworkGhidraOverlayMergeReport {
            source_type_count: overlay.types.len(),
            ignored_registry_type_count: overlay.types.len(),
            source_field_registration_function_count: overlay.field_registration_functions.len(),
            source_field_handler_vtable_count: overlay.field_handler_vtables.len(),
            ..NetworkGhidraOverlayMergeReport::default()
        };
        let updated_handler_addresses = overlay
            .field_handler_vtables
            .iter()
            .filter_map(|entry| entry.address.clone())
            .collect::<BTreeSet<_>>();

        merge_address_catalog(
            &mut self.field_registration_functions,
            overlay.field_registration_functions,
            |entry| entry.address.as_deref(),
            &mut result.replaced_field_registration_function_count,
            &mut result.added_field_registration_function_count,
            "field registration function",
        )?;
        merge_address_catalog(
            &mut self.field_handler_vtables,
            overlay.field_handler_vtables,
            |entry| entry.address.as_deref(),
            &mut result.replaced_field_handler_vtable_count,
            &mut result.added_field_handler_vtable_count,
            "field handler vtable",
        )?;
        result.rebound_dependent_field_count = self
            .types
            .iter_mut()
            .map(|network_type| {
                invalidate_fields_for_handler_vtables(
                    &mut network_type.fields,
                    &updated_handler_addresses,
                )
            })
            .chain(
                self.field_registration_functions
                    .iter_mut()
                    .map(|function| {
                        invalidate_fields_for_handler_vtables(
                            &mut function.fields,
                            &updated_handler_addresses,
                        )
                    }),
            )
            .sum();
        self.normalize_derived_shapes();
        Ok(result)
    }
}

fn validate_program_identity(
    base: &NetworkSchema,
    overlay: &NetworkSchema,
) -> Result<(), NetworkSchemaImportError> {
    let base_program = base
        .sources
        .iter()
        .find_map(|source| source.program.as_deref());
    let overlay_program = overlay
        .sources
        .iter()
        .find_map(|source| source.program.as_deref());
    if let (Some(base_program), Some(overlay_program)) = (base_program, overlay_program)
        && base_program != overlay_program
    {
        return Err(NetworkSchemaImportError::IncompatibleOverlay(format!(
            "program mismatch: `{base_program}` != `{overlay_program}`"
        )));
    }

    let base_image = base
        .sources
        .iter()
        .find_map(|source| source.image_base.as_deref());
    let overlay_image = overlay
        .sources
        .iter()
        .find_map(|source| source.image_base.as_deref());
    if let (Some(base_image), Some(overlay_image)) = (base_image, overlay_image)
        && base_image != overlay_image
    {
        return Err(NetworkSchemaImportError::IncompatibleOverlay(format!(
            "image-base mismatch: `{base_image}` != `{overlay_image}`"
        )));
    }
    Ok(())
}

fn merge_address_catalog<T, F>(
    destination: &mut Vec<T>,
    source: Vec<T>,
    address: F,
    replaced_count: &mut usize,
    added_count: &mut usize,
    catalog_name: &str,
) -> Result<(), NetworkSchemaImportError>
where
    F: for<'a> Fn(&'a T) -> Option<&'a str>,
{
    let mut source_addresses = BTreeSet::new();
    for entry in &source {
        let entry_address = address(entry).ok_or_else(|| {
            NetworkSchemaImportError::IncompatibleOverlay(format!(
                "{catalog_name} is missing its address"
            ))
        })?;
        if !source_addresses.insert(entry_address.to_owned()) {
            return Err(NetworkSchemaImportError::IncompatibleOverlay(format!(
                "duplicate {catalog_name} address `{entry_address}`"
            )));
        }
    }

    let mut destination_indices = destination
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| Some((address(entry)?.to_owned(), index)))
        .collect::<BTreeMap<_, _>>();
    for entry in source {
        let entry_address = address(&entry)
            .expect("overlay address was validated")
            .to_owned();
        if let Some(index) = destination_indices.get(&entry_address).copied() {
            destination[index] = entry;
            *replaced_count += 1;
        } else {
            destination_indices.insert(entry_address, destination.len());
            destination.push(entry);
            *added_count += 1;
        }
    }
    destination.sort_by(|left, right| address(left).cmp(&address(right)));
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn report(wire_shape: &str) -> Value {
        json!({
            "program": "NewWorld.exe",
            "imageBase": "NewWorld+0x0",
            "registryEntries": [],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81e2338",
                "fieldCount": 1,
                "wireShape": wire_shape,
                "wireShapeSource": "test-evidence",
                "slots": []
            }]
        })
    }

    fn report_with_dependent_field(wire_shape: &str, source: &str, layout: &str) -> Value {
        let mut report = report(wire_shape);
        report["registryEntries"] = json!([{
            "typeIndex": 13,
            "fields": [{
                "index": 0,
                "name": "position",
                "handlerVtable": "NewWorld+0x81e2338",
                "handlerKind": "replicated-field",
                "handlerVtableSlots": 14,
                "physicalFieldCount": 1,
                "wireShape": wire_shape,
                "wireShapeSource": source,
                "wireLayout": layout,
                "wireLayoutSource": source,
                "sourceTypeId": "99a32353-e595-4d5c-86cb-dc80318228d1",
                "sourceTypeIdSource": "unproven-handler-value",
                "sourceTypeIdentityProven": false,
                "nestedTypeShape": {
                    "typeId": "99a32353-e595-4d5c-86cb-dc80318228d1",
                    "typeIdSource": "unproven-handler-value",
                    "identityProven": false,
                    "typeName": "StaleValue",
                    "members": []
                },
                "confidence": "register-field-call"
            }]
        }]);
        report["fieldHandlerVtables"][0]["handlerKind"] = json!("replicated-field");
        report["fieldHandlerVtables"][0]["handlerKindSource"] = json!(source);
        report["fieldHandlerVtables"][0]["vtableSlots"] = json!(14);
        report["fieldHandlerVtables"][0]["physicalFieldCount"] = json!(1);
        report["fieldHandlerVtables"][0]["wireShapeSource"] = json!(source);
        report["fieldHandlerVtables"][0]["wireLayout"] = json!(layout);
        report["fieldHandlerVtables"][0]["wireLayoutSource"] = json!(source);
        report
    }

    #[test]
    fn replaces_only_the_exact_address_keyed_handler() {
        let mut schema =
            NetworkSchema::from_ghidra_static_network_report(&report("vlq-u32")).unwrap();
        let merge = schema
            .merge_ghidra_static_network_overlay(&report("fixed-vector<fixed-bytes-16,10>"))
            .unwrap();

        assert_eq!(merge.replaced_field_handler_vtable_count, 1);
        assert_eq!(schema.field_handler_vtables.len(), 1);
        assert_eq!(
            schema.field_handler_vtables[0]
                .wire_shape
                .as_ref()
                .unwrap()
                .wire_string(),
            "fixed-vector<fixed-bytes-16,10>"
        );
    }

    #[test]
    fn rebinds_fields_that_depend_on_replaced_handlers() {
        let mut schema =
            NetworkSchema::from_ghidra_static_network_report(&report_with_dependent_field(
                "f32",
                "marshal-call:marshal-function-name",
                "fixed-bytes-4",
            ))
            .unwrap();
        let merge = schema
            .merge_ghidra_static_network_overlay(&report_with_dependent_field(
                "packed-position<0xc2c80000,0x44fa0000>",
                "marshal+unmarshal-pcode-agreement",
                "fixed-bytes-10",
            ))
            .unwrap();

        assert_eq!(merge.rebound_dependent_field_count, 1);
        let field = &schema.types[0].fields[0];
        assert_eq!(
            field.wire_shape.as_ref().unwrap().wire_string(),
            "packed-position<0xc2c80000,0x44fa0000>"
        );
        assert_eq!(
            field.wire_shape_source.as_deref(),
            Some("marshal+unmarshal-pcode-agreement")
        );
        assert_eq!(field.wire_layout.as_deref(), Some("fixed-bytes-10"));
        assert_eq!(
            field.handler_kind_source.as_deref(),
            Some("marshal+unmarshal-pcode-agreement")
        );
        assert_eq!(field.wire_shape_raw.as_deref(), Some("f32"));
        assert_eq!(field.source_type_id, None);
        assert_eq!(field.nested_type_shape, None);
    }

    #[test]
    fn rejects_duplicate_overlay_addresses() {
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&report("u32")).unwrap();
        let mut duplicate = report("u64");
        let repeated = duplicate["fieldHandlerVtables"][0].clone();
        duplicate["fieldHandlerVtables"]
            .as_array_mut()
            .unwrap()
            .push(repeated);

        assert!(
            schema
                .merge_ghidra_static_network_overlay(&duplicate)
                .is_err()
        );
    }
}
