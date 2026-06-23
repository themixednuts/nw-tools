#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustCodegenOptions {
    pub mode: RustCodegenMode,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RustCodegenMode {
    #[default]
    Integrated,
    Standalone,
}

impl Default for RustCodegenOptions {
    fn default() -> Self {
        Self {
            mode: RustCodegenMode::Integrated,
        }
    }
}
