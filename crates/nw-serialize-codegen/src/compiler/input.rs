use std::{fs, path::Path};

use serde_json::Value;

use crate::CodegenContext;
use crate::class_registration::class_registration_trace_root_from_jsonl_str;
use crate::module_descriptors::{
    is_module_descriptor_json_name, module_descriptor_capture, module_descriptors_root,
    module_descriptors_root_from_capture, module_name_from_path,
};

use super::SerializeContextCompileError;

#[derive(Debug, Clone, Copy, Default)]
pub struct SerializeContextCompileInputs<'a> {
    pub module_descriptors_root: Option<&'a Value>,
    pub serialize_porting_root: Option<&'a Value>,
    pub class_registration_trace_root: Option<&'a Value>,
}

pub(super) fn read_optional_json(
    path: Option<&Path>,
) -> Result<Option<Value>, SerializeContextCompileError> {
    path.map(read_json).transpose()
}

pub(super) fn read_optional_module_descriptors(
    path: Option<&Path>,
    context: &CodegenContext,
) -> Result<Option<Value>, SerializeContextCompileError> {
    path.map(|path| read_module_descriptors(path, context))
        .transpose()
}

pub(super) fn read_optional_class_registration_trace(
    path: Option<&Path>,
) -> Result<Option<Value>, SerializeContextCompileError> {
    path.map(read_class_registration_trace).transpose()
}

fn read_module_descriptors(
    path: &Path,
    context: &CodegenContext,
) -> Result<Value, SerializeContextCompileError> {
    if path.is_dir() {
        return read_module_descriptor_directory(path, context);
    }

    let root = read_json(path)?;
    Ok(module_descriptors_root_from_capture(
        module_name_from_path(path),
        root,
    ))
}

fn read_module_descriptor_directory(
    path: &Path,
    context: &CodegenContext,
) -> Result<Value, SerializeContextCompileError> {
    let mut entries = fs::read_dir(path)
        .map_err(|source| SerializeContextCompileError::ReadJson {
            path: path.to_path_buf(),
            source,
        })?
        .map(|entry| {
            entry.map(|entry| entry.path()).map_err(|source| {
                SerializeContextCompileError::ReadJson {
                    path: path.to_path_buf(),
                    source,
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.retain(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_module_descriptor_json_name)
    });
    entries.sort();

    let modules = context.runner().try_map(&entries, |entry| {
        read_json(entry).map(|root| module_descriptor_capture(module_name_from_path(entry), root))
    })?;

    Ok(module_descriptors_root(modules))
}

fn read_json(path: &Path) -> Result<Value, SerializeContextCompileError> {
    let bytes = fs::read(path).map_err(|source| SerializeContextCompileError::ReadJson {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| SerializeContextCompileError::ParseJson {
        path: path.to_path_buf(),
        source,
    })
}

fn read_class_registration_trace(path: &Path) -> Result<Value, SerializeContextCompileError> {
    let text =
        fs::read_to_string(path).map_err(|source| SerializeContextCompileError::ReadJson {
            path: path.to_path_buf(),
            source,
        })?;
    class_registration_trace_root_from_jsonl_str(&text).map_err(|source| {
        SerializeContextCompileError::ParseJson {
            path: path.to_path_buf(),
            source,
        }
    })
}
