use crate::CodegenContext;
use crate::ir::SerializeCodegenUnit;

use super::super::support;
use super::{TypeScriptSourceEmitError, TypeScriptSourceEmitter, format_typescript_source};

mod index;
mod package;
mod type_file;

use package::{TYPESCRIPT_VITEPLUS_TSCONFIG, viteplus_config, viteplus_package_json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptStandaloneProject {
    pub files: Vec<TypeScriptStandaloneProjectFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptStandaloneProjectOptions {
    pub package_name: String,
    pub pack_entries: Vec<String>,
}

impl Default for TypeScriptStandaloneProjectOptions {
    fn default() -> Self {
        Self {
            package_name: "az-typescript-validation".to_owned(),
            pack_entries: vec!["src/index.ts".to_owned()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptStandaloneProjectFile {
    pub path: String,
    pub source: String,
}

impl TypeScriptSourceEmitter {
    pub fn emit_standalone_project(
        &self,
        unit: &SerializeCodegenUnit,
        context: &CodegenContext,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        self.emit_standalone_project_with_context(unit, unit, context)
    }

    pub fn emit_standalone_project_with_context(
        &self,
        emitted_unit: &SerializeCodegenUnit,
        context_unit: &SerializeCodegenUnit,
        context: &CodegenContext,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        self.emit_standalone_project_with_options_and_context(
            emitted_unit,
            context_unit,
            &TypeScriptStandaloneProjectOptions::default(),
            context,
        )
    }

    pub fn emit_standalone_project_with_options(
        &self,
        unit: &SerializeCodegenUnit,
        options: &TypeScriptStandaloneProjectOptions,
        context: &CodegenContext,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        self.emit_standalone_project_with_options_and_context(unit, unit, options, context)
    }

    pub fn emit_standalone_project_with_options_and_context(
        &self,
        emitted_unit: &SerializeCodegenUnit,
        context_unit: &SerializeCodegenUnit,
        options: &TypeScriptStandaloneProjectOptions,
        context: &CodegenContext,
    ) -> Result<TypeScriptStandaloneProject, TypeScriptSourceEmitError> {
        let mut files = vec![
            TypeScriptStandaloneProjectFile {
                path: "package.json".to_owned(),
                source: viteplus_package_json(&options.package_name),
            },
            TypeScriptStandaloneProjectFile {
                path: "tsconfig.json".to_owned(),
                source: TYPESCRIPT_VITEPLUS_TSCONFIG.to_owned(),
            },
            TypeScriptStandaloneProjectFile {
                path: "vite.config.ts".to_owned(),
                source: format_typescript_source(&viteplus_config(&options.pack_entries))?,
            },
            TypeScriptStandaloneProjectFile {
                path: "src/index.ts".to_owned(),
                source: format_typescript_source(
                    "export { Asset, AssetId } from \"./az/asset.js\";\nexport { Crc32 } from \"./az/crc.js\";\nexport { AzRtti, Rtti, RttiRegistry, registerType, rttiFor, rttiRegistry } from \"./az/rtti.js\";\nexport type { RttiRegistration, RttiTarget } from \"./az/rtti.js\";\nexport { Uuid, typeIds } from \"./az/uuid.js\";\nexport * as az from \"./az/index.js\";\nexport * from \"./types/index.js\";\n",
                )?,
            },
            TypeScriptStandaloneProjectFile {
                path: "src/az/index.ts".to_owned(),
                source: format_typescript_source(
                    "export * from \"./asset.js\";\nexport * from \"./collection.js\";\nexport * from \"./crc.js\";\nexport * from \"./math.js\";\nexport * from \"./rtti.js\";\nexport * from \"./uuid.js\";\n",
                )?,
            },
            TypeScriptStandaloneProjectFile {
                path: "src/az/uuid.ts".to_owned(),
                source: format_typescript_source(support::uuid_module_source())?,
            },
            TypeScriptStandaloneProjectFile {
                path: "src/az/crc.ts".to_owned(),
                source: format_typescript_source(support::crc_module_source())?,
            },
            TypeScriptStandaloneProjectFile {
                path: "src/az/rtti.ts".to_owned(),
                source: format_typescript_source(support::rtti_module_source())?,
            },
            TypeScriptStandaloneProjectFile {
                path: "src/az/asset.ts".to_owned(),
                source: format_typescript_source(support::asset_module_source())?,
            },
            TypeScriptStandaloneProjectFile {
                path: "src/az/collection.ts".to_owned(),
                source: format_typescript_source(support::collection_module_source())?,
            },
            TypeScriptStandaloneProjectFile {
                path: "src/az/math.ts".to_owned(),
                source: format_typescript_source(support::math_module_source())?,
            },
        ];
        files.extend(self.emit_project_type_files_with_context(
            emitted_unit,
            context_unit,
            context,
        )?);
        Ok(TypeScriptStandaloneProject { files })
    }
}
