use std::{fmt, io, path::PathBuf};

use thiserror::Error;
use uuid::Uuid;

use crate::rust::integrate::identity::{
    AzIdentityReconcileReport, SkippedAzIdentityReconcileReport, SourceReconcileError,
};
use crate::{ReflectedTypeCatalog, RustIntegrationError};

/// Inputs for the tooling-only component scaffold integration stage.
#[derive(Debug)]
pub struct ComponentScaffoldRequest<'a> {
    pub catalog: &'a ReflectedTypeCatalog,
    pub source_label: Option<&'a str>,
    pub facet_owner_evidence: &'a [FacetOwnerEvidence],
    pub components_root: PathBuf,
    pub module_file: PathBuf,
    pub plugin_file: Option<PathBuf>,
    pub apply: bool,
}

#[derive(Debug, Clone)]
pub struct ComponentScaffoldReport {
    pub source_label: String,
    pub applied: bool,
    pub components_seen: usize,
    pub existing_components: Vec<ExistingComponentReport>,
    pub created_or_extended_modules: Vec<ModuleScaffoldReport>,
    pub missing_existing_fields: Vec<ExistingFieldReport>,
    pub skipped_existing_fields: Vec<SkippedFieldReport>,
    pub az_identity_reconciliations: Vec<AzIdentityReconcileReport>,
    pub skipped_az_identity_reconciliations: Vec<SkippedAzIdentityReconcileReport>,
    pub facet_owner_evidence: Vec<FacetOwnerEvidence>,
}

#[derive(Debug, Clone)]
pub struct ExistingComponentReport {
    pub component_name: String,
    pub type_id: Uuid,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ModuleScaffoldReport {
    pub module_name: String,
    pub file_path: PathBuf,
    pub component_names: Vec<String>,
    pub action: ModuleScaffoldAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleScaffoldAction {
    CreateFile,
    AppendToExistingFile,
}

#[derive(Debug, Clone)]
pub struct ExistingFieldReport {
    pub component_name: String,
    pub file_path: PathBuf,
    pub field_name: String,
    pub field_type: String,
}

#[derive(Debug, Clone)]
pub struct SkippedFieldReport {
    pub component_name: String,
    pub file_path: PathBuf,
    pub field_name: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct FacetOwnerEvidence {
    pub facet_name: String,
    pub facet_type_id: Uuid,
    pub owner_name: String,
    pub owner_type_id: Uuid,
    pub field_name: String,
}

#[derive(Debug, Error)]
pub enum ComponentScaffoldError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to create directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to walk {path}: {source}")]
    Walk {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    SourceReconcile(#[from] SourceReconcileError),
    #[error(transparent)]
    RustIntegration(Box<RustIntegrationError>),
}

impl From<RustIntegrationError> for ComponentScaffoldError {
    fn from(source: RustIntegrationError) -> Self {
        Self::RustIntegration(Box::new(source))
    }
}

impl fmt::Display for ComponentScaffoldReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "component scaffold: {}", self.source_label)?;
        writeln!(f, "  applied: {}", self.applied)?;
        writeln!(f, "  component types seen: {}", self.components_seen)?;
        writeln!(
            f,
            "  existing component types: {}",
            self.existing_components.len()
        )?;
        writeln!(
            f,
            "  modules to create/extend: {}",
            self.created_or_extended_modules.len()
        )?;
        writeln!(
            f,
            "  missing existing fields: {}",
            self.missing_existing_fields.len()
        )?;
        writeln!(
            f,
            "  skipped existing fields: {}",
            self.skipped_existing_fields.len()
        )?;
        writeln!(
            f,
            "  az identity reconciliations: {}",
            self.az_identity_reconciliations.len()
        )?;
        writeln!(
            f,
            "  skipped az identity reconciliations: {}",
            self.skipped_az_identity_reconciliations.len()
        )?;
        writeln!(
            f,
            "  facet owner evidence: {}",
            self.facet_owner_evidence.len()
        )?;

        for fix in &self.az_identity_reconciliations {
            writeln!(f, "{fix}")?;
        }

        for fix in self.skipped_az_identity_reconciliations.iter().take(20) {
            writeln!(
                f,
                "  skip {}::{}: {}",
                fix.file_path.display(),
                fix.component_name,
                fix.reason
            )?;
        }
        if self.skipped_az_identity_reconciliations.len() > 20 {
            writeln!(
                f,
                "  ... {} more skipped az identity reconciliations",
                self.skipped_az_identity_reconciliations.len() - 20
            )?;
        }

        for facet in self.facet_owner_evidence.iter().take(20) {
            writeln!(
                f,
                "  facet-owner {} -> {} via {} ({})",
                facet.facet_name, facet.owner_name, facet.field_name, facet.facet_type_id
            )?;
        }
        if self.facet_owner_evidence.len() > 20 {
            writeln!(
                f,
                "  ... {} more facet-owner evidence rows",
                self.facet_owner_evidence.len() - 20
            )?;
        }

        for module in &self.created_or_extended_modules {
            let action = match module.action {
                ModuleScaffoldAction::CreateFile => "create",
                ModuleScaffoldAction::AppendToExistingFile => "append",
            };
            writeln!(
                f,
                "  {action} {}: {}",
                module.file_path.display(),
                module.component_names.join(", ")
            )?;
        }

        for field in self.missing_existing_fields.iter().take(50) {
            writeln!(
                f,
                "  missing {}::{}: {}",
                field.component_name, field.field_name, field.field_type
            )?;
        }
        if self.missing_existing_fields.len() > 50 {
            writeln!(
                f,
                "  ... {} more missing fields",
                self.missing_existing_fields.len() - 50
            )?;
        }

        for field in self.skipped_existing_fields.iter().take(50) {
            writeln!(
                f,
                "  skipped {}::{}: {}",
                field.component_name, field.field_name, field.reason
            )?;
        }
        if self.skipped_existing_fields.len() > 50 {
            writeln!(
                f,
                "  ... {} more skipped fields",
                self.skipped_existing_fields.len() - 50
            )?;
        }

        Ok(())
    }
}
