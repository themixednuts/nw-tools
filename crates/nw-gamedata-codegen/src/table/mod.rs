use crate::schema::GameDataCompileMode;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GameDataTableSourceFormat {
    #[default]
    Datasheet,
}

impl GameDataTableSourceFormat {
    #[must_use]
    pub const fn default_for_mode(_mode: GameDataCompileMode) -> Self {
        Self::Datasheet
    }
}
