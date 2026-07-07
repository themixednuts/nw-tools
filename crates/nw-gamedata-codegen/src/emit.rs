use std::path::PathBuf;

use crate::compiler::GameDataCompileUnit;
use crate::target::{
    GameDataAssetSourcePlan, GameDataDataFormat, GameDataRuntimeProfile, GameDataTargetLanguage,
    GameDataTargetPlan,
};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GameDataEmitterConfigError {
    #[error(
        "{emitter:?} emitter does not support target {target_language:?}/{runtime:?}/{asset_source:?}/{data_format:?}"
    )]
    UnsupportedTarget {
        emitter: GameDataTargetLanguage,
        target_language: GameDataTargetLanguage,
        runtime: GameDataRuntimeProfile,
        asset_source: GameDataAssetSourcePlan,
        data_format: GameDataDataFormat,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDataCodegenOutput {
    target: GameDataTargetPlan,
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
    fn target(&self) -> GameDataTargetPlan;

    fn target_language(&self) -> GameDataTargetLanguage {
        self.target().language()
    }

    fn supports_target(&self, target: &GameDataTargetPlan) -> bool {
        target.supports_language(self.target_language())
    }

    fn emit(&self, unit: &GameDataCompileUnit) -> anyhow::Result<GameDataCodegenOutput>;
}

impl GameDataCodegenOutput {
    #[must_use]
    pub fn new(target: GameDataTargetPlan, files: Vec<GameDataCodegenFile>) -> Self {
        Self { target, files }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    #[must_use]
    pub const fn target(&self) -> &GameDataTargetPlan {
        &self.target
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

impl GameDataEmitterConfigError {
    #[must_use]
    pub fn unsupported(emitter: GameDataTargetLanguage, target: &GameDataTargetPlan) -> Self {
        Self::UnsupportedTarget {
            emitter,
            target_language: target.language(),
            runtime: target.runtime(),
            asset_source: target.source(),
            data_format: target.data_format(),
        }
    }
}
