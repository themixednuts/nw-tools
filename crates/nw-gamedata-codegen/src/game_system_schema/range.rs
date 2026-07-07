use super::OwnedCellValue;

#[must_use]
pub fn range_f32_from_cell_value(value: &OwnedCellValue) -> Option<::core::range::Range<f32>> {
    let range = range_inclusive_f32_from_cell_value(value)?;
    Some(::core::range::Range {
        start: range.start,
        end: range.last,
    })
}

#[must_use]
pub fn range_f32_from_text(value: &str) -> ::core::range::Range<f32> {
    let range = range_inclusive_f32_from_text(value);
    ::core::range::Range {
        start: range.start,
        end: range.last,
    }
}

#[must_use]
pub fn range_inclusive_f32_from_cell_value(
    value: &OwnedCellValue,
) -> Option<::core::range::RangeInclusive<f32>> {
    match value {
        OwnedCellValue::Number(value) if value.is_finite() => Some(::core::range::RangeInclusive {
            start: *value,
            last: *value,
        }),
        OwnedCellValue::String(value) => Some(range_inclusive_f32_from_text(value)),
        OwnedCellValue::Number(_) | OwnedCellValue::Boolean(_) => None,
    }
}

#[must_use]
pub fn range_u32_from_cell_value(value: &OwnedCellValue) -> Option<::core::range::Range<u32>> {
    let range = range_inclusive_u32_from_cell_value(value)?;
    Some(::core::range::Range {
        start: range.start,
        end: range.last,
    })
}

#[must_use]
pub fn range_u32_from_text(value: &str) -> Option<::core::range::Range<u32>> {
    let range = range_inclusive_u32_from_text(value)?;
    Some(::core::range::Range {
        start: range.start,
        end: range.last,
    })
}

#[must_use]
pub fn range_inclusive_u32_from_cell_value(
    value: &OwnedCellValue,
) -> Option<::core::range::RangeInclusive<u32>> {
    match value {
        OwnedCellValue::Number(value) => {
            let value = range_u32_component_from_f32(*value)?;
            Some(::core::range::RangeInclusive {
                start: value,
                last: value,
            })
        }
        OwnedCellValue::String(value) => range_inclusive_u32_from_text(value),
        OwnedCellValue::Boolean(_) => None,
    }
}

#[must_use]
pub fn range_inclusive_u32_from_text(value: &str) -> Option<::core::range::RangeInclusive<u32>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let parts = value.split('-').map(str::trim).collect::<Vec<_>>();
    match parts.as_slice() {
        [value] => {
            let value = range_u32_component_from_str(value)?;
            Some(::core::range::RangeInclusive {
                start: value,
                last: value,
            })
        }
        [start, last] => Some(::core::range::RangeInclusive {
            start: range_u32_component_from_str(start)?,
            last: range_u32_component_from_str(last)?,
        }),
        _ => None,
    }
}

#[must_use]
pub fn range_i32_from_cell_value(value: &OwnedCellValue) -> Option<::core::range::Range<i32>> {
    let range = range_inclusive_i32_from_cell_value(value)?;
    Some(::core::range::Range {
        start: range.start,
        end: range.last,
    })
}

#[must_use]
pub fn range_i32_from_text(value: &str) -> Option<::core::range::Range<i32>> {
    let range = range_inclusive_i32_from_text(value)?;
    Some(::core::range::Range {
        start: range.start,
        end: range.last,
    })
}

#[must_use]
pub fn range_inclusive_i32_from_cell_value(
    value: &OwnedCellValue,
) -> Option<::core::range::RangeInclusive<i32>> {
    match value {
        OwnedCellValue::Number(value) => {
            let value = range_i32_component_from_f32(*value)?;
            Some(::core::range::RangeInclusive {
                start: value,
                last: value,
            })
        }
        OwnedCellValue::String(value) => range_inclusive_i32_from_text(value),
        OwnedCellValue::Boolean(_) => None,
    }
}

#[must_use]
pub fn range_inclusive_i32_from_text(value: &str) -> Option<::core::range::RangeInclusive<i32>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let Some((start, last)) = split_signed_range_text(value) else {
        let value = range_i32_component_from_str(value)?;
        return Some(::core::range::RangeInclusive {
            start: value,
            last: value,
        });
    };
    Some(::core::range::RangeInclusive {
        start: range_i32_component_from_str(start)?,
        last: range_i32_component_from_str(last)?,
    })
}

#[must_use]
pub fn range_inclusive_f32_from_text(value: &str) -> ::core::range::RangeInclusive<f32> {
    match parse_range_inclusive_f32_text(value) {
        RangeInclusiveF32TextValue::Parsed(range) => range,
        RangeInclusiveF32TextValue::ZeroFallback => zero_range_inclusive_f32(),
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum RangeInclusiveF32TextValue {
    Parsed(::core::range::RangeInclusive<f32>),
    ZeroFallback,
}

impl RangeInclusiveF32TextValue {
    pub(super) const fn parsed(self) -> Option<::core::range::RangeInclusive<f32>> {
        match self {
            Self::Parsed(range) => Some(range),
            Self::ZeroFallback => None,
        }
    }
}

pub(super) fn parse_range_inclusive_f32_text(value: &str) -> RangeInclusiveF32TextValue {
    let value = value.trim();
    if value.is_empty() {
        return RangeInclusiveF32TextValue::ZeroFallback;
    }
    let parts = value.split('-').map(str::trim).collect::<Vec<_>>();
    match parts.as_slice() {
        [value] => {
            range_f32_component(value).map_or(RangeInclusiveF32TextValue::ZeroFallback, |value| {
                RangeInclusiveF32TextValue::Parsed(::core::range::RangeInclusive {
                    start: value,
                    last: value,
                })
            })
        }
        [start, last] => match (range_f32_component(start), range_f32_component(last)) {
            (Some(start), Some(last)) => {
                let (start, last) = if start <= last {
                    (start, last)
                } else {
                    (last, start)
                };
                RangeInclusiveF32TextValue::Parsed(::core::range::RangeInclusive { start, last })
            }
            _ => RangeInclusiveF32TextValue::ZeroFallback,
        },
        _ => RangeInclusiveF32TextValue::ZeroFallback,
    }
}

pub(super) fn range_f32_component(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().filter(|value| value.is_finite())
}

pub(super) fn range_u32_component_from_str(value: &str) -> Option<u32> {
    value.parse::<u32>().ok()
}

pub(super) fn range_u32_component_from_f32(value: f32) -> Option<u32> {
    if value == 0.0 {
        return Some(0);
    }
    if !value.is_finite() || value.fract() != 0.0 || value < 0.0 {
        return None;
    }
    format!("{value:.0}").parse::<u32>().ok()
}

pub(super) fn range_i32_component_from_str(value: &str) -> Option<i32> {
    value.parse::<i32>().ok()
}

pub(super) fn range_i32_component_from_f32(value: f32) -> Option<i32> {
    if value == 0.0 {
        return Some(0);
    }
    if !value.is_finite() || value.fract() != 0.0 {
        return None;
    }
    format!("{value:.0}").parse::<i32>().ok()
}

pub(super) fn split_signed_range_text(value: &str) -> Option<(&str, &str)> {
    let bytes = value.as_bytes();
    let mut candidate = None;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte != b'-' || index == 0 {
            continue;
        }
        let (left, right) = value.split_at(index);
        let left = left.trim();
        let right = right[1..].trim();
        if range_i32_component_from_str(left).is_none()
            || range_i32_component_from_str(right).is_none()
        {
            continue;
        }
        if candidate.replace((left, right)).is_some() {
            return None;
        }
    }
    candidate
}

pub(super) const fn zero_range_inclusive_f32() -> ::core::range::RangeInclusive<f32> {
    ::core::range::RangeInclusive {
        start: 0.0,
        last: 0.0,
    }
}
