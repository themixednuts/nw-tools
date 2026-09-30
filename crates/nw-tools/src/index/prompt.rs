//! Copy for the explicit `nw-tools index` confirm step.

/// Rounded-up first-build cost from the 2026-09-14 reference run.
pub const FIRST_BUILD_HOURS: u32 = 3;
pub const FIRST_BUILD_GIB: u32 = 20;

/// Warning shown before a cold or incomplete build.
#[must_use]
pub fn first_build_warning() -> String {
    format!(
        "First build takes about {FIRST_BUILD_HOURS} hours and about {FIRST_BUILD_GIB} GB.\nLater runs only redo paks that changed."
    )
}

/// `y` / `yes`, ignoring case and surrounding space.
#[must_use]
pub fn confirm_accepted(line: &str) -> bool {
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_rounds_up_to_clean_numbers() {
        let text = first_build_warning();
        assert!(text.contains("3 hours"), "{text}");
        assert!(text.contains("20 GB"), "{text}");
    }

    #[test]
    fn only_yes_starts_the_build() {
        assert!(confirm_accepted("y"));
        assert!(confirm_accepted(" Yes \n"));
        assert!(!confirm_accepted("n"));
        assert!(!confirm_accepted(""));
    }
}
