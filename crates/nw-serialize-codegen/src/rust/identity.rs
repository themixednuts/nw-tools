use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustTypeIdentityPlan {
    pub kind: RustTypeIdentityKind,
    pub type_id: Uuid,
    pub name: Option<String>,
    /// Immediate serialized native-component base that owns the component id.
    /// Integrated output delegates [`AzComponent::component_id`] through this
    /// field and publishes the ordinary component-lowering registration.
    pub native_component_base_field: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustTypeIdentityKind {
    AzTypeInfo,
    AzRtti,
}

impl RustTypeIdentityPlan {
    #[must_use]
    pub fn az_type_info(type_id: Uuid, name: Option<String>) -> Self {
        Self {
            kind: RustTypeIdentityKind::AzTypeInfo,
            type_id,
            name,
            native_component_base_field: None,
        }
    }

    #[must_use]
    pub fn az_rtti(type_id: Uuid, name: Option<String>) -> Self {
        Self {
            kind: RustTypeIdentityKind::AzRtti,
            type_id,
            name,
            native_component_base_field: None,
        }
    }

    #[must_use]
    pub fn with_native_component_base_field(mut self, field: impl Into<String>) -> Self {
        self.native_component_base_field = Some(field.into());
        self
    }
}

impl RustTypeIdentityKind {
    #[must_use]
    pub const fn integrated_derive_name(self) -> &'static str {
        match self {
            Self::AzTypeInfo => "AzTypeInfo",
            Self::AzRtti => "AzRtti",
        }
    }

    #[must_use]
    pub const fn integrated_attr_name(self) -> &'static str {
        match self {
            Self::AzTypeInfo => "az_type_info",
            Self::AzRtti => "az_rtti",
        }
    }
}
