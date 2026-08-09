use super::*;

pub(super) fn reorder_message_fields_by_signature(
    fields: &mut Vec<NetworkField>,
    signatures: &[NetworkMessageFieldSignature],
    serialize_types: &[NetworkSerializeType],
) -> usize {
    if fields.len() <= 1 || fields.len() != signatures.len() {
        return 0;
    }

    let signature_shapes = signatures
        .iter()
        .map(|signature| message_signature_field_shapes(signature, serialize_types))
        .collect::<Option<Vec<_>>>();
    let field_shapes = fields
        .iter()
        .map(message_machine_field_shapes)
        .collect::<Option<Vec<_>>>();
    let (Some(signature_shapes), Some(field_shapes)) = (signature_shapes, field_shapes) else {
        return 0;
    };
    if field_shapes
        .iter()
        .zip(&signature_shapes)
        .all(|(field, signature)| wire_shape_sequences_machine_compatible(field, signature))
    {
        return 0;
    }

    let mut assignments = vec![None; signatures.len()];
    let mut remaining = (0..fields.len()).collect::<Vec<_>>();
    for (signature_index, signature) in signatures.iter().enumerate() {
        let exact = remaining
            .iter()
            .copied()
            .filter(|field_index| {
                wire_shape_sequences_machine_compatible(
                    &field_shapes[*field_index],
                    &signature_shapes[signature_index],
                )
            })
            .filter(|field_index| field_has_signature_native_type(&fields[*field_index], signature))
            .collect::<Vec<_>>();
        let [field_index] = exact.as_slice() else {
            continue;
        };
        assignments[signature_index] = Some(*field_index);
        remaining.retain(|candidate| candidate != field_index);
    }

    for signature_index in 0..signatures.len() {
        if assignments[signature_index].is_some() {
            continue;
        }
        let Some(position) = remaining.iter().position(|field_index| {
            wire_shape_sequences_machine_compatible(
                &field_shapes[*field_index],
                &signature_shapes[signature_index],
            )
        }) else {
            return 0;
        };
        assignments[signature_index] = Some(remaining.remove(position));
    }
    if !remaining.is_empty() {
        return 0;
    }

    let Some(order) = assignments.into_iter().collect::<Option<Vec<_>>>() else {
        return 0;
    };
    let reordered_count = order
        .iter()
        .enumerate()
        .filter(|(index, field_index)| *index != **field_index)
        .count();
    if reordered_count == 0 {
        return 0;
    }

    let source = std::mem::take(fields);
    *fields = order
        .into_iter()
        .enumerate()
        .map(|(index, field_index)| {
            let mut field = source[field_index].clone();
            field.index = signatures[index]
                .index
                .or_else(|| u32::try_from(index).ok());
            field
        })
        .collect();
    reordered_count
}

fn field_has_signature_native_type(
    field: &NetworkField,
    signature: &NetworkMessageFieldSignature,
) -> bool {
    let Some(expected) = signature.native_type.as_deref() else {
        return false;
    };
    field_has_proven_native_type_identity(field, expected)
        || [
            field.native_type.as_deref(),
            field.source_type_name.as_deref(),
            field
                .nested_type_shape
                .as_ref()
                .and_then(|shape| shape.type_name_full.as_deref()),
            field
                .nested_type_shape
                .as_ref()
                .and_then(|shape| shape.type_name.as_deref()),
        ]
        .into_iter()
        .flatten()
        .any(|candidate| equivalent_native_type(candidate, expected))
}
