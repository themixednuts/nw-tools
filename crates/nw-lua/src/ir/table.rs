//! Lua table-allocation facts retained in SSA.

/// A `NEWTABLE` array/hash size operand encoded as Lua's "floating byte".
///
/// Lua 5.1's parser derives these operands from the number of list and record
/// fields in the source constructor. The VM decodes them as allocation hints,
/// so larger values may represent a rounded capacity rather than an exact field
/// count. Keeping the encoded value prevents reconstruction from treating a
/// capacity estimate as source truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableSizeHint(u16);

impl TableSizeHint {
    /// Retains one decoded instruction operand.
    #[must_use]
    pub const fn from_encoded(encoded: u16) -> Self {
        Self(encoded)
    }

    /// Returns the bytecode operand exactly as encoded.
    #[must_use]
    pub const fn encoded(self) -> u16 {
        self.0
    }

    /// Returns the allocation capacity produced by Lua's `luaO_fb2int`.
    #[must_use]
    pub const fn decoded_capacity(self) -> usize {
        let exponent = (self.0 >> 3) & 31;
        if exponent == 0 {
            self.0 as usize
        } else {
            (((self.0 & 7) + 8) as usize) << (exponent - 1)
        }
    }

    /// Returns the source field count when the floating-byte encoding is
    /// injective for this operand.
    ///
    /// Lua 5.1 encodes counts through 16 without rounding. Above 16, multiple
    /// source counts can map to the same allocation hint, so reconstruction
    /// must use other bytecode facts or decline the richer source shape.
    #[must_use]
    pub const fn exact_field_count(self) -> Option<usize> {
        if self.0 <= 16 {
            Some(self.decoded_capacity())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TableSizeHint;

    #[test]
    fn small_table_hints_retain_exact_source_counts() {
        for count in 0..=16 {
            let hint = TableSizeHint::from_encoded(count);
            assert_eq!(hint.exact_field_count(), Some(usize::from(count)));
            assert_eq!(hint.decoded_capacity(), usize::from(count));
        }
    }

    #[test]
    fn rounded_table_hints_are_not_claimed_as_exact() {
        let hint = TableSizeHint::from_encoded(17);
        assert_eq!(hint.decoded_capacity(), 18);
        assert_eq!(hint.exact_field_count(), None);
    }
}
