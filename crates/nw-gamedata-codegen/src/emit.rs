use std::path::PathBuf;

use crate::compiler::GameDataCompileUnit;
use crate::target::GameDataTargetLanguage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataCodegenOutput {
    language: GameDataTargetLanguage,
    files: Vec<GameDataCodegenFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataCodegenFile {
    path: PathBuf,
    contents: GameDataCodegenFileContents,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GameDataCodegenFileContents {
    Text(String),
    Binary(Vec<u8>),
}

pub trait GameDataEmitter {
    fn target_language(&self) -> GameDataTargetLanguage;

    fn emit(&self, unit: &GameDataCompileUnit) -> anyhow::Result<GameDataCodegenOutput>;
}

impl GameDataCodegenOutput {
    #[must_use]
    pub fn new(language: GameDataTargetLanguage, files: Vec<GameDataCodegenFile>) -> Self {
        Self { language, files }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    #[must_use]
    pub const fn language(&self) -> GameDataTargetLanguage {
        self.language
    }

    #[must_use]
    pub fn files(&self) -> &[GameDataCodegenFile] {
        &self.files
    }

    #[must_use]
    pub fn into_files(self) -> Vec<GameDataCodegenFile> {
        self.files
    }
}

impl GameDataCodegenFile {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, contents: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            contents: GameDataCodegenFileContents::Text(contents.into()),
        }
    }

    #[must_use]
    pub fn binary(path: impl Into<PathBuf>, contents: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            contents: GameDataCodegenFileContents::Binary(contents.into()),
        }
    }

    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    #[must_use]
    pub fn contents(&self) -> &str {
        match &self.contents {
            GameDataCodegenFileContents::Text(contents) => contents,
            GameDataCodegenFileContents::Binary(_) => {
                panic!("{} is a binary codegen file", self.path.display())
            }
        }
    }

    #[must_use]
    pub fn contents_bytes(&self) -> &[u8] {
        match &self.contents {
            GameDataCodegenFileContents::Text(contents) => contents.as_bytes(),
            GameDataCodegenFileContents::Binary(contents) => contents,
        }
    }

    #[must_use]
    pub fn into_parts(self) -> (PathBuf, String) {
        let GameDataCodegenFileContents::Text(contents) = self.contents else {
            panic!("{} is a binary codegen file", self.path.display());
        };
        (self.path, contents)
    }
}
