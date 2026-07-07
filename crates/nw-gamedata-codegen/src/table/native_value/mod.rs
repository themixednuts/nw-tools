use std::num::{NonZeroU8, NonZeroU16};
use std::range::{Range, RangeInclusive};

mod convert;
mod render;

pub(super) use convert::native_dev_cell_value;
#[cfg(test)]
pub(super) use convert::native_dev_string_cell_value;
pub(super) use render::render_native_dev_cell_value;

#[derive(Debug)]
pub(super) enum NativeDevCellValue {
    Scalar(NativeDevScalarValue),
    Range(NativeDevRangeValue),
    Pair(Box<NativeDevPairValue>),
    List(Vec<NativeDevCellValue>),
}

#[derive(Debug)]
pub(super) enum NativeDevScalarValue {
    String(String),
    F32(f32),
    U8(u8),
    NonZeroU8(NonZeroU8),
    U16(u16),
    NonZeroU16(NonZeroU16),
    I64(i64),
    U64(u64),
    Crc32(u32),
    Boolean(bool),
}

#[derive(Debug)]
pub(super) enum NativeDevRangeValue {
    F32(NativeDevRange<f32>),
    I32(NativeDevRange<i32>),
    U32(NativeDevRange<u32>),
}

#[derive(Debug)]
pub(super) enum NativeDevRange<T> {
    Exclusive(Range<T>),
    Inclusive(RangeInclusive<T>),
}

#[derive(Debug)]
pub(super) struct NativeDevPairValue {
    first: NativeDevCellValue,
    second: NativeDevCellValue,
}

impl NativeDevCellValue {
    pub(super) fn string(value: impl Into<String>) -> Self {
        Self::Scalar(NativeDevScalarValue::String(value.into()))
    }

    pub(super) const fn f32(value: f32) -> Self {
        Self::Scalar(NativeDevScalarValue::F32(value))
    }

    pub(super) const fn u8(value: u8) -> Self {
        Self::Scalar(NativeDevScalarValue::U8(value))
    }

    pub(super) const fn nonzero_u8(value: NonZeroU8) -> Self {
        Self::Scalar(NativeDevScalarValue::NonZeroU8(value))
    }

    pub(super) const fn u16(value: u16) -> Self {
        Self::Scalar(NativeDevScalarValue::U16(value))
    }

    pub(super) const fn nonzero_u16(value: NonZeroU16) -> Self {
        Self::Scalar(NativeDevScalarValue::NonZeroU16(value))
    }

    pub(super) const fn i64(value: i64) -> Self {
        Self::Scalar(NativeDevScalarValue::I64(value))
    }

    pub(super) const fn u64(value: u64) -> Self {
        Self::Scalar(NativeDevScalarValue::U64(value))
    }

    pub(super) const fn crc32(value: u32) -> Self {
        Self::Scalar(NativeDevScalarValue::Crc32(value))
    }

    pub(super) const fn boolean(value: bool) -> Self {
        Self::Scalar(NativeDevScalarValue::Boolean(value))
    }

    pub(super) const fn range_f32(value: Range<f32>) -> Self {
        Self::Range(NativeDevRangeValue::F32(NativeDevRange::Exclusive(value)))
    }

    pub(super) const fn range_inclusive_f32(value: RangeInclusive<f32>) -> Self {
        Self::Range(NativeDevRangeValue::F32(NativeDevRange::Inclusive(value)))
    }

    pub(super) const fn range_i32(value: Range<i32>) -> Self {
        Self::Range(NativeDevRangeValue::I32(NativeDevRange::Exclusive(value)))
    }

    pub(super) const fn range_inclusive_i32(value: RangeInclusive<i32>) -> Self {
        Self::Range(NativeDevRangeValue::I32(NativeDevRange::Inclusive(value)))
    }

    pub(super) const fn range_u32(value: Range<u32>) -> Self {
        Self::Range(NativeDevRangeValue::U32(NativeDevRange::Exclusive(value)))
    }

    pub(super) const fn range_inclusive_u32(value: RangeInclusive<u32>) -> Self {
        Self::Range(NativeDevRangeValue::U32(NativeDevRange::Inclusive(value)))
    }

    pub(super) fn pair(first: NativeDevCellValue, second: NativeDevCellValue) -> Self {
        Self::Pair(Box::new(NativeDevPairValue { first, second }))
    }

    pub(super) fn list(values: Vec<NativeDevCellValue>) -> Self {
        Self::List(values)
    }
}
