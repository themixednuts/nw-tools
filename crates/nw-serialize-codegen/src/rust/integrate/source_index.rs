use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use heck::ToShoutySnakeCase;
use syn::{
    Attribute, Expr, Item, Lit, LitStr, Path as SynPath, Visibility, punctuated::Punctuated,
};
use uuid::Uuid;

use crate::CodegenContext;
use crate::rust::integrate::RustIntegrationError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RustSourceTypeIndex {
    by_name: BTreeMap<String, Vec<RustSourceTypeLocation>>,
    by_type_id: BTreeMap<Uuid, RustSourceTypeLocation>,
}

impl RustSourceTypeIndex {
    pub fn from_root(
        root: impl AsRef<Path>,
        context: &CodegenContext,
    ) -> Result<Self, RustIntegrationError> {
        let root = root.as_ref();
        let mut paths = Vec::new();
        super::collect_rust_files(root, &mut paths)?;
        paths.sort();

        let mut index = Self::default();
        let scans = context.runner().try_map(&paths, |path| {
            let source =
                std::fs::read_to_string(path).map_err(|source| RustIntegrationError::Read {
                    path: path.clone(),
                    source,
                })?;
            Self::from_source(root, path, &source)
        })?;
        for scan in scans {
            index.merge(scan);
        }
        Ok(index)
    }

    pub fn from_source(
        root: &Path,
        file_path: &Path,
        source: &str,
    ) -> Result<Self, RustIntegrationError> {
        let Some(module_path) = module_path_for_file(root, file_path) else {
            return Ok(Self::default());
        };
        let file = syn::parse_file(source).map_err(|source| RustIntegrationError::Parse {
            path: file_path.to_path_buf(),
            source: crate::rust::analyze::RustSourceAnalyzeError::Parse(source),
        })?;
        Ok(Self::from_syn_file(&module_path, file_path, &file))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    #[must_use]
    pub fn locations_for_name(&self, name: &str) -> &[RustSourceTypeLocation] {
        self.by_name.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    #[must_use]
    pub fn location_for(
        &self,
        rust_name: &str,
        type_id: Uuid,
        current_module: &str,
    ) -> Option<&RustSourceTypeLocation> {
        if let Some(location) = self.by_type_id.get(&type_id) {
            return Some(location);
        }

        if rust_name.starts_with("Unknown") {
            return None;
        }

        let locations = self.by_name.get(rust_name)?;
        if let Some(location) = locations
            .iter()
            .find(|location| location.module_path == current_module)
        {
            return Some(location);
        }
        if locations.len() == 1 {
            return locations.first();
        }

        None
    }

    #[must_use]
    pub fn location_for_type_id(&self, type_id: Uuid) -> Option<&RustSourceTypeLocation> {
        self.by_type_id.get(&type_id)
    }

    #[must_use]
    pub fn reference_for_type_id(&self, type_id: Uuid, current_module: &str) -> Option<String> {
        self.location_for_type_id(type_id)
            .map(|location| location.reference_from(current_module))
    }

    pub fn merge(&mut self, other: Self) {
        for (name, mut locations) in other.by_name {
            self.by_name.entry(name).or_default().append(&mut locations);
        }
        for (type_id, location) in other.by_type_id {
            self.by_type_id.entry(type_id).or_insert(location);
        }
    }

    fn from_syn_file(module_path: &str, file_path: &Path, file: &syn::File) -> Self {
        let const_type_ids = public_uuid_consts(file);
        let mut index = Self::default();

        for item in &file.items {
            let Some(type_item) = RustSourceTypeItem::from_syn_item(item) else {
                continue;
            };
            let type_id = type_item.type_id_from_attrs(&const_type_ids).or_else(|| {
                const_type_ids
                    .get(&format!(
                        "{}_TYPE_ID",
                        type_item.name.to_shouty_snake_case()
                    ))
                    .copied()
            });
            let location = RustSourceTypeLocation {
                name: type_item.name,
                module_path: module_path.to_owned(),
                file_path: file_path.to_path_buf(),
                derive_capabilities: type_item.derive_capabilities,
                maps_entities: type_item.maps_entities,
            };
            index.insert(location, type_id);
        }

        index
    }

    fn insert(&mut self, location: RustSourceTypeLocation, type_id: Option<Uuid>) {
        self.by_name
            .entry(location.name.clone())
            .or_default()
            .push(location.clone());
        if let Some(type_id) = type_id {
            self.by_type_id.entry(type_id).or_insert(location);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSourceTypeLocation {
    pub name: String,
    pub module_path: String,
    pub file_path: PathBuf,
    pub derive_capabilities: RustDeriveCapabilities,
    pub maps_entities: bool,
}

impl RustSourceTypeLocation {
    #[must_use]
    pub fn reference_from(&self, current_module: &str) -> String {
        if self.module_path == current_module {
            self.name.clone()
        } else {
            format!("crate::{}::{}", self.module_path, self.name)
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RustDeriveCapabilities {
    pub copy: bool,
    pub eq: bool,
}

impl RustDeriveCapabilities {
    pub const NONE: Self = Self {
        copy: false,
        eq: false,
    };
    pub const COPY_ONLY: Self = Self {
        copy: true,
        eq: false,
    };
    pub const EQ_ONLY: Self = Self {
        copy: false,
        eq: true,
    };
    pub const COPY_EQ: Self = Self {
        copy: true,
        eq: true,
    };
}

struct RustSourceTypeItem {
    name: String,
    attrs: Vec<Attribute>,
    derive_capabilities: RustDeriveCapabilities,
    maps_entities: bool,
}

impl RustSourceTypeItem {
    fn from_syn_item(item: &Item) -> Option<Self> {
        match item {
            Item::Struct(item) if is_public(&item.vis) => {
                Some(Self::new(item.ident.to_string(), item.attrs.clone()))
            }
            Item::Enum(item) if is_public(&item.vis) => {
                Some(Self::new(item.ident.to_string(), item.attrs.clone()))
            }
            Item::Type(item) if is_public(&item.vis) => {
                Some(Self::new(item.ident.to_string(), item.attrs.clone()))
            }
            _ => None,
        }
    }

    fn new(name: String, attrs: Vec<Attribute>) -> Self {
        let derive_tokens = derive_leaf_names(&attrs);
        Self {
            name,
            attrs,
            derive_capabilities: RustDeriveCapabilities {
                copy: derive_tokens.iter().any(|token| token == "Copy"),
                eq: derive_tokens.iter().any(|token| token == "Eq"),
            },
            maps_entities: derive_tokens.iter().any(|token| token == "MapEntities"),
        }
    }

    fn type_id_from_attrs(&self, const_type_ids: &BTreeMap<String, Uuid>) -> Option<Uuid> {
        self.attrs
            .iter()
            .filter(|attr| is_identity_attr(attr))
            .find_map(|attr| attr_type_id(attr, const_type_ids))
    }
}

fn public_uuid_consts(file: &syn::File) -> BTreeMap<String, Uuid> {
    file.items
        .iter()
        .filter_map(|item| {
            let Item::Const(item) = item else {
                return None;
            };
            if !is_public(&item.vis) {
                return None;
            }
            Some((item.ident.to_string(), uuid_from_expr(&item.expr)?))
        })
        .collect()
}

fn uuid_from_expr(expr: &Expr) -> Option<Uuid> {
    match expr {
        Expr::Lit(expr) => {
            let Lit::Str(lit) = &expr.lit else {
                return None;
            };
            parse_uuid(&lit.value())
        }
        Expr::Macro(expr) => {
            let lit = syn::parse2::<LitStr>(expr.mac.tokens.clone()).ok()?;
            parse_uuid(&lit.value())
        }
        _ => None,
    }
}

fn attr_type_id(attr: &Attribute, const_type_ids: &BTreeMap<String, Uuid>) -> Option<Uuid> {
    let args = attr
        .parse_args_with(Punctuated::<Expr, syn::Token![,]>::parse_terminated)
        .ok()?;

    for (index, arg) in args.into_iter().enumerate() {
        match &arg {
            Expr::Assign(assign)
                if expr_assign_lhs_is(&assign.left, "uuid")
                    || expr_assign_lhs_is(&assign.left, "type_id") =>
            {
                return uuid_from_identity_expr(&assign.right, const_type_ids);
            }
            _ if index == 0 => {
                if let Some(type_id) = uuid_from_identity_expr(&arg, const_type_ids) {
                    return Some(type_id);
                }
            }
            _ => {}
        }
    }

    None
}

fn uuid_from_identity_expr(expr: &Expr, const_type_ids: &BTreeMap<String, Uuid>) -> Option<Uuid> {
    uuid_from_expr(expr).or_else(|| type_id_const_from_expr(expr, const_type_ids))
}

fn expr_assign_lhs_is(expr: &Expr, name: &str) -> bool {
    let Expr::Path(expr) = expr else {
        return false;
    };
    expr.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

fn type_id_const_from_expr(expr: &Expr, const_type_ids: &BTreeMap<String, Uuid>) -> Option<Uuid> {
    let Expr::Path(expr) = expr else {
        return None;
    };
    let name = expr.path.segments.last()?.ident.to_string();
    const_type_ids.get(&name).copied()
}

fn derive_leaf_names(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .filter_map(|attr| {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<SynPath, syn::Token![,]>::parse_terminated,
            )
            .ok()
        })
        .flat_map(|paths| {
            paths.into_iter().filter_map(|path| {
                path.segments
                    .last()
                    .map(|segment| segment.ident.to_string())
            })
        })
        .collect()
}

fn is_identity_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("az_type_info")
        || attr.path().is_ident("type_info")
        || attr.path().is_ident("az_rtti")
        || attr.path().is_ident("rtti")
}

fn is_public(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

fn module_path_for_file(root: &Path, file_path: &Path) -> Option<String> {
    let relative = file_path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let value = component.as_os_str().to_str()?;
        let path = Path::new(value);
        if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            let stem = path.file_stem()?.to_str()?;
            if stem != "mod" {
                parts.push(stem.to_owned());
            }
        } else {
            parts.push(value.to_owned());
        }
    }

    (!parts.is_empty()).then(|| parts.join("::"))
}

fn parse_uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value.trim_matches(['{', '}'])).ok()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use uuid::uuid;

    use super::*;

    #[test]
    fn scans_public_type_id_attrs_consts_derives_and_module_paths() {
        let source = r#"
use bevy::ecs::entity::MapEntities;

pub const SHARED_TYPE_TYPE_ID: uuid::Uuid =
    uuid::uuid!("11111111-2222-3333-4444-555555555555");
pub const ALIAS_TYPE_ID: &str = "22222222-3333-4444-5555-666666666666";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, MapEntities)]
#[az_type_info(SHARED_TYPE_TYPE_ID)]
pub struct SharedType {
    pub entity: Option<bevy::ecs::entity::Entity>,
}

#[derive(core::marker::Copy, std::cmp::Eq)]
#[az_type_info("33333333-4444-5555-6666-777777777777")]
pub enum SharedMode {}

pub type Alias = u32;
"#;

        let index = RustSourceTypeIndex::from_source(
            Path::new("components"),
            Path::new("components/groups/shared.rs"),
            source,
        )
        .expect("source type index");

        let shared = index
            .location_for(
                "SharedType",
                uuid!("11111111-2222-3333-4444-555555555555"),
                "groups::owner",
            )
            .expect("shared type location");
        assert_eq!(shared.name, "SharedType");
        assert_eq!(shared.module_path, "groups::shared");
        assert_eq!(shared.derive_capabilities, RustDeriveCapabilities::COPY_EQ);
        assert!(shared.maps_entities);
        assert_eq!(
            shared.reference_from("groups::owner"),
            "crate::groups::shared::SharedType"
        );
        assert_eq!(shared.reference_from("groups::shared"), "SharedType");

        let mode = index
            .location_for(
                "SharedMode",
                uuid!("33333333-4444-5555-6666-777777777777"),
                "groups::owner",
            )
            .expect("enum type location");
        assert_eq!(mode.derive_capabilities, RustDeriveCapabilities::COPY_EQ);

        let alias = index
            .location_for(
                "Alias",
                uuid!("22222222-3333-4444-5555-666666666666"),
                "groups::owner",
            )
            .expect("alias type location");
        assert_eq!(alias.name, "Alias");
    }

    #[test]
    fn ambiguous_names_do_not_resolve_without_matching_module_or_type_id() {
        let mut index = RustSourceTypeIndex::default();
        index.insert(
            RustSourceTypeLocation {
                name: "SharedType".to_owned(),
                module_path: "a".to_owned(),
                file_path: PathBuf::from("a.rs"),
                derive_capabilities: RustDeriveCapabilities::NONE,
                maps_entities: false,
            },
            None,
        );
        index.insert(
            RustSourceTypeLocation {
                name: "SharedType".to_owned(),
                module_path: "b".to_owned(),
                file_path: PathBuf::from("b.rs"),
                derive_capabilities: RustDeriveCapabilities::NONE,
                maps_entities: false,
            },
            None,
        );

        assert!(
            index
                .location_for("SharedType", Uuid::nil(), "owner")
                .is_none()
        );
        assert_eq!(
            index
                .location_for("SharedType", Uuid::nil(), "a")
                .expect("same-module location")
                .module_path,
            "a"
        );
    }
}
