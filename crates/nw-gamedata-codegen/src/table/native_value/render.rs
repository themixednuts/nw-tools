use std::{fmt::Write as _, range::RangeInclusive};

use anyhow::{Context, Result};
use serde::Serialize;

use super::{
    NativeDevCellValue, NativeDevPairValue, NativeDevRange, NativeDevRangeValue,
    NativeDevScalarValue,
};

pub(in crate::table) fn render_native_dev_cell_value(value: &NativeDevCellValue) -> Result<String> {
    match value {
        NativeDevCellValue::Scalar(value) => render_native_dev_scalar(value),
        NativeDevCellValue::Range(value) => render_native_dev_range(value),
        NativeDevCellValue::Pair(value) => render_native_dev_pair_value(value),
        NativeDevCellValue::List(values) => {
            render_native_dev_list_items(values, render_native_dev_cell_value)
        }
    }
}

fn render_native_dev_scalar(value: &NativeDevScalarValue) -> Result<String> {
    match value {
        NativeDevScalarValue::String(value) => ron::ser::to_string(value)
            .with_context(|| format!("serialize RON string value {value:?}")),
        NativeDevScalarValue::F32(value) => {
            ron::ser::to_string(value).context("serialize RON f32 value")
        }
        NativeDevScalarValue::U8(value) => {
            ron::ser::to_string(value).context("serialize RON u8 value")
        }
        NativeDevScalarValue::NonZeroU8(value) => {
            ron::ser::to_string(&value.get()).context("serialize RON non-zero u8 value")
        }
        NativeDevScalarValue::U16(value) => {
            ron::ser::to_string(value).context("serialize RON u16 value")
        }
        NativeDevScalarValue::NonZeroU16(value) => {
            ron::ser::to_string(&value.get()).context("serialize RON non-zero u16 value")
        }
        NativeDevScalarValue::I64(value) => {
            ron::ser::to_string(value).context("serialize RON i64 value")
        }
        NativeDevScalarValue::U64(value) => {
            ron::ser::to_string(value).context("serialize RON u64 value")
        }
        NativeDevScalarValue::Crc32(value) => {
            ron::ser::to_string(value).context("serialize RON crc32 value")
        }
        NativeDevScalarValue::Boolean(value) => {
            ron::ser::to_string(value).context("serialize RON boolean value")
        }
    }
}

fn render_native_dev_range(value: &NativeDevRangeValue) -> Result<String> {
    match value {
        NativeDevRangeValue::F32(value) => render_native_dev_typed_range(value),
        NativeDevRangeValue::I32(value) => render_native_dev_typed_range(value),
        NativeDevRangeValue::U32(value) => render_native_dev_typed_range(value),
    }
}

fn render_native_dev_pair_value(value: &NativeDevPairValue) -> Result<String> {
    Ok(format!(
        "(first: {}, second: {})",
        render_native_dev_cell_value(&value.first)?,
        render_native_dev_cell_value(&value.second)?
    ))
}

fn render_native_dev_list_items<T>(
    values: &[T],
    mut render: impl FnMut(&T) -> Result<String>,
) -> Result<String> {
    if values.is_empty() {
        return Ok("[]".to_owned());
    }

    let mut out = String::new();
    writeln!(out, "[")?;
    for value in values {
        writeln!(out, "            {},", render(value)?)?;
    }
    write!(out, "        ]")?;
    Ok(out)
}

fn render_native_dev_typed_range<T>(value: &NativeDevRange<T>) -> Result<String>
where
    T: Serialize,
    RangeInclusive<T>: NativeDevRangeInclusiveFields<T>,
{
    match value {
        NativeDevRange::Exclusive(value) => Ok(format!(
            "(start: {}, end: {})",
            ron::ser::to_string(&value.start).context("serialize RON range start")?,
            ron::ser::to_string(&value.end).context("serialize RON range end")?
        )),
        NativeDevRange::Inclusive(value) => Ok(format!(
            "(start: {}, last: {})",
            ron::ser::to_string(value.native_dev_start()).context("serialize RON range start")?,
            ron::ser::to_string(value.native_dev_last()).context("serialize RON range last")?
        )),
    }
}

trait NativeDevRangeInclusiveFields<T> {
    fn native_dev_start(&self) -> &T;
    fn native_dev_last(&self) -> &T;
}

impl<T> NativeDevRangeInclusiveFields<T> for RangeInclusive<T> {
    fn native_dev_start(&self) -> &T {
        &self.start
    }

    fn native_dev_last(&self) -> &T {
        &self.last
    }
}
