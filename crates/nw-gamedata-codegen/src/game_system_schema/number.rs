use super::{GameSystemListAtomShape, GameSystemListElementShape, GameSystemNumberShape};

#[derive(Debug, Default)]
pub(super) struct NumberStats {
    has_float: bool,
    has_negative: bool,
    has_zero: bool,
}

impl NumberStats {
    pub(super) fn observe(&mut self, value: f32) {
        if value.fract() != 0.0 {
            self.has_float = true;
        }
        if value < 0.0 {
            self.has_negative = true;
        }
        if value == 0.0 {
            self.has_zero = true;
        }
    }

    pub(super) const fn finish_observed(self) -> GameSystemNumberShape {
        if self.has_float {
            GameSystemNumberShape::Float
        } else if self.has_negative {
            GameSystemNumberShape::Integer
        } else if self.has_zero {
            GameSystemNumberShape::NonNegativeInteger
        } else {
            GameSystemNumberShape::PositiveInteger
        }
    }
}

pub(super) const fn combine_number_shapes(
    left: GameSystemNumberShape,
    right: GameSystemNumberShape,
) -> GameSystemNumberShape {
    use GameSystemNumberShape::{
        Float, Integer, NonNegativeInteger, NonZeroU8, NonZeroU16, PositiveInteger, U8, U16,
    };

    match (left, right) {
        (Float, _) | (_, Float) => Float,
        (Integer, _) | (_, Integer) => Integer,
        (NonNegativeInteger, _) | (_, NonNegativeInteger) => NonNegativeInteger,
        (U8, U8) => U8,
        (NonZeroU8, NonZeroU8) => NonZeroU8,
        (U8, NonZeroU8) | (NonZeroU8, U8) => U8,
        (U8, U16) | (U16, U8) => U16,
        (U8, NonZeroU16) | (NonZeroU16, U8) => U16,
        (NonZeroU8, U16) | (U16, NonZeroU8) => U16,
        (NonZeroU8, NonZeroU16) | (NonZeroU16, NonZeroU8) => NonZeroU16,
        (U16, U16) => U16,
        (NonZeroU16, NonZeroU16) => NonZeroU16,
        (U16, NonZeroU16) | (NonZeroU16, U16) => U16,
        (PositiveInteger, NonZeroU8) | (NonZeroU8, PositiveInteger) => PositiveInteger,
        (PositiveInteger, U8) | (U8, PositiveInteger) => NonNegativeInteger,
        (PositiveInteger, NonZeroU16) | (NonZeroU16, PositiveInteger) => PositiveInteger,
        (PositiveInteger, U16) | (U16, PositiveInteger) => NonNegativeInteger,
        (PositiveInteger, PositiveInteger) => PositiveInteger,
    }
}

pub(super) fn combine_list_element_shapes(
    left: GameSystemListElementShape,
    right: GameSystemListElementShape,
) -> GameSystemListElementShape {
    match (left, right) {
        (GameSystemListElementShape::Crc32, GameSystemListElementShape::Crc32) => {
            GameSystemListElementShape::Crc32
        }
        (GameSystemListElementShape::Crc32, _) | (_, GameSystemListElementShape::Crc32) => {
            GameSystemListElementShape::String
        }
        (GameSystemListElementShape::String, _) | (_, GameSystemListElementShape::String) => {
            GameSystemListElementShape::String
        }
        (
            GameSystemListElementShape::Color { color_shape: left },
            GameSystemListElementShape::Color { color_shape: right },
        ) if left == right => GameSystemListElementShape::Color { color_shape: left },
        (GameSystemListElementShape::Color { .. }, _)
        | (_, GameSystemListElementShape::Color { .. }) => GameSystemListElementShape::String,
        (
            GameSystemListElementShape::Enum {
                enum_shape: left_enum,
            },
            GameSystemListElementShape::Enum {
                enum_shape: right_enum,
            },
        ) if left_enum == right_enum => GameSystemListElementShape::Enum {
            enum_shape: left_enum,
        },
        (GameSystemListElementShape::Enum { .. }, _)
        | (_, GameSystemListElementShape::Enum { .. }) => GameSystemListElementShape::String,
        (GameSystemListElementShape::Boolean, GameSystemListElementShape::Boolean) => {
            GameSystemListElementShape::Boolean
        }
        (
            GameSystemListElementShape::Number { number_shape: left },
            GameSystemListElementShape::Number {
                number_shape: right,
            },
        ) => GameSystemListElementShape::Number {
            number_shape: combine_number_shapes(left, right),
        },
        (
            GameSystemListElementShape::Range {
                bounds: left_bounds,
                number_shape: left_number_shape,
            },
            GameSystemListElementShape::Range {
                bounds: right_bounds,
                number_shape: right_number_shape,
            },
        ) if left_bounds == right_bounds => GameSystemListElementShape::Range {
            bounds: left_bounds,
            number_shape: combine_number_shapes(left_number_shape, right_number_shape),
        },
        (GameSystemListElementShape::Range { .. }, _)
        | (_, GameSystemListElementShape::Range { .. }) => GameSystemListElementShape::String,
        (
            GameSystemListElementShape::Pair {
                separator: left_separator,
                first: left_first,
                second: left_second,
                default_second_source_token: left_default_second_source_token,
            },
            GameSystemListElementShape::Pair {
                separator: right_separator,
                first: right_first,
                second: right_second,
                default_second_source_token: right_default_second_source_token,
            },
        ) if left_separator == right_separator
            && left_default_second_source_token == right_default_second_source_token =>
        {
            let Some(first) = combine_list_atom_shapes(left_first, right_first) else {
                return GameSystemListElementShape::String;
            };
            let Some(second) = combine_list_atom_shapes(left_second, right_second) else {
                return GameSystemListElementShape::String;
            };
            GameSystemListElementShape::Pair {
                separator: left_separator,
                first,
                second,
                default_second_source_token: left_default_second_source_token,
            }
        }
        (GameSystemListElementShape::Pair { .. }, _)
        | (_, GameSystemListElementShape::Pair { .. }) => GameSystemListElementShape::String,
        (GameSystemListElementShape::Boolean, GameSystemListElementShape::Number { .. })
        | (GameSystemListElementShape::Number { .. }, GameSystemListElementShape::Boolean) => {
            GameSystemListElementShape::String
        }
    }
}

pub(super) fn combine_list_atom_shapes(
    left: GameSystemListAtomShape,
    right: GameSystemListAtomShape,
) -> Option<GameSystemListAtomShape> {
    match (left, right) {
        (GameSystemListAtomShape::String, GameSystemListAtomShape::String) => {
            Some(GameSystemListAtomShape::String)
        }
        (GameSystemListAtomShape::Boolean, GameSystemListAtomShape::Boolean) => {
            Some(GameSystemListAtomShape::Boolean)
        }
        (GameSystemListAtomShape::Crc32, GameSystemListAtomShape::Crc32) => {
            Some(GameSystemListAtomShape::Crc32)
        }
        (
            GameSystemListAtomShape::Color { color_shape: left },
            GameSystemListAtomShape::Color { color_shape: right },
        ) if left == right => Some(GameSystemListAtomShape::Color { color_shape: left }),
        (
            GameSystemListAtomShape::Enum {
                enum_shape: left_enum,
            },
            GameSystemListAtomShape::Enum {
                enum_shape: right_enum,
            },
        ) if left_enum == right_enum => Some(GameSystemListAtomShape::Enum {
            enum_shape: left_enum,
        }),
        (
            GameSystemListAtomShape::Number { number_shape: left },
            GameSystemListAtomShape::Number {
                number_shape: right,
            },
        ) => Some(GameSystemListAtomShape::Number {
            number_shape: combine_number_shapes(left, right),
        }),
        (
            GameSystemListAtomShape::Range {
                bounds: left_bounds,
                number_shape: left_number_shape,
            },
            GameSystemListAtomShape::Range {
                bounds: right_bounds,
                number_shape: right_number_shape,
            },
        ) if left_bounds == right_bounds => Some(GameSystemListAtomShape::Range {
            bounds: left_bounds,
            number_shape: combine_number_shapes(left_number_shape, right_number_shape),
        }),
        _ => None,
    }
}

pub(super) fn string_token_as_number(value: &str) -> Option<f32> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    value.parse::<f32>().ok().filter(|value| value.is_finite())
}

pub(super) fn number_matches_shape(value: f32, number_shape: GameSystemNumberShape) -> bool {
    if !value.is_finite() {
        return false;
    }
    match number_shape {
        GameSystemNumberShape::Float => true,
        GameSystemNumberShape::Integer => value.fract() == 0.0,
        GameSystemNumberShape::NonNegativeInteger => value.fract() == 0.0 && value >= 0.0,
        GameSystemNumberShape::PositiveInteger => value.fract() == 0.0 && value > 0.0,
        GameSystemNumberShape::U8 => value.fract() == 0.0 && value >= 0.0 && value <= 255.0,
        GameSystemNumberShape::NonZeroU8 => value.fract() == 0.0 && value > 0.0 && value <= 255.0,
        GameSystemNumberShape::U16 => value.fract() == 0.0 && value >= 0.0 && value <= 65_535.0,
        GameSystemNumberShape::NonZeroU16 => {
            value.fract() == 0.0 && value > 0.0 && value <= 65_535.0
        }
    }
}

pub(super) fn usize_ratio(numerator: usize, denominator: usize) -> f64 {
    debug_assert!(denominator > 0);
    let numerator = u32::try_from(numerator).expect("schema ratio numerator fits in u32");
    let denominator = u32::try_from(denominator).expect("schema ratio denominator fits in u32");
    f64::from(numerator) / f64::from(denominator)
}
