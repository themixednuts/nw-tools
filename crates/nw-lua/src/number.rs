//! Lua 5.1 number literal formatting.

const MAX_COMPACT_INTEGER_LEN: usize = 30;

/// Formats a non-NaN `f64` as a Lua 5.1 decimal literal.
///
/// Integers keep plain decimal form while it is reasonably compact. Other
/// values use the shorter exact round-trip candidate between Rust's default
/// decimal display and scientific notation.
pub(crate) fn lua51_number_literal(value: f64) -> Option<String> {
    if value.is_nan() {
        return None;
    }
    if !value.is_finite() {
        return Some(if value.is_sign_negative() {
            "-1e9999".to_string()
        } else {
            "1e9999".to_string()
        });
    }

    let decimal = value.to_string();
    if is_compact_integer(value, &decimal) {
        return Some(decimal);
    }

    let scientific = format!("{value:e}");
    Some(shorter_exact_literal(value, decimal, scientific))
}

fn is_compact_integer(value: f64, text: &str) -> bool {
    value.fract() == 0.0 && !text.contains(['.', 'e', 'E']) && text.len() <= MAX_COMPACT_INTEGER_LEN
}

fn shorter_exact_literal(value: f64, decimal: String, scientific: String) -> String {
    let decimal_exact = reparses_exact(&decimal, value);
    let scientific_exact = reparses_exact(&scientific, value);

    match (decimal_exact, scientific_exact) {
        (true, true) if scientific.len() < decimal.len() => scientific,
        (true, _) => decimal,
        (false, true) => scientific,
        (false, false) => decimal,
    }
}

fn reparses_exact(text: &str, value: f64) -> bool {
    text.parse::<f64>()
        .is_ok_and(|parsed| parsed.to_bits() == value.to_bits())
}

#[cfg(test)]
mod tests {
    use super::lua51_number_literal;

    #[test]
    fn large_and_small_literals_use_compact_scientific_notation() {
        assert_eq!(lua51_number_literal(1e308).as_deref(), Some("1e308"));
        assert_eq!(lua51_number_literal(1e-300).as_deref(), Some("1e-300"));
    }

    #[test]
    fn compact_integer_values_stay_plain_decimal() {
        assert_eq!(lua51_number_literal(3.0).as_deref(), Some("3"));
        assert_eq!(lua51_number_literal(-7.0).as_deref(), Some("-7"));
        assert_eq!(
            lua51_number_literal(9_007_199_254_740_992.0).as_deref(),
            Some("9007199254740992")
        );
    }

    #[test]
    fn selected_literals_reparse_to_identical_bits() {
        for value in [
            -0.0,
            0.1,
            -12345.6789,
            1.234_567_890_123_456_7,
            1e308,
            1e-300,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
        ] {
            let literal = lua51_number_literal(value).expect("non-NaN literal");
            let reparsed = literal.parse::<f64>().expect("Rust parses literal");
            assert_eq!(reparsed.to_bits(), value.to_bits(), "{literal}");
        }
    }
}
