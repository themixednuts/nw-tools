//! Source-level naming for SSA values.

use std::collections::{BTreeMap, BTreeSet};

use bstr::BString;

use crate::{
    chunk::{LocVar, Proto},
    decompile::ast::{BindingId, FunctionId, Name},
    ir::{SsaFunction, SsaOp, SsaRef},
};

pub use super::identifier::is_valid_identifier;

/// Debug-local binding selected for a register at a program counter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalBinding {
    pub index: usize,
    pub reg: u16,
    pub name: Name,
    pub start_pc: i32,
    pub end_pc: i32,
}

#[derive(Debug)]
struct NamingResult {
    names: Vec<Vec<Option<Name>>>,
    bindings: Vec<Vec<Option<BindingId>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnonymousNameRole {
    Parameter,
    Local,
    Upvalue,
}

impl AnonymousNameRole {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Parameter => "a",
            Self::Local => "l",
            Self::Upvalue => "u",
        }
    }

    fn base(self, index: impl std::fmt::Display) -> String {
        format!("{}{index}", self.prefix())
    }

    fn name(self, index: impl std::fmt::Display) -> Name {
        Name::from(self.base(index))
    }
}

/// Name resolver backed by one precomputed `reg/version -> name` map.
#[derive(Debug)]
pub struct NameResolver<'a> {
    proto: &'a Proto,
    function_id: FunctionId,
    upvalue_overrides: Vec<Name>,
    parameter_names: Vec<Name>,
    local_bindings: Vec<LocalBinding>,
    value_names: Vec<Vec<Option<Name>>>,
    value_bindings: Vec<Vec<Option<BindingId>>>,
}

impl<'a> NameResolver<'a> {
    #[must_use]
    pub fn new(proto: &'a Proto, function: &SsaFunction) -> Self {
        Self::with_overrides(proto, function, FunctionId::root(), Vec::new(), Vec::new())
    }

    #[must_use]
    pub fn with_overrides(
        proto: &'a Proto,
        function: &SsaFunction,
        function_id: FunctionId,
        upvalue_overrides: Vec<Name>,
        param_overrides: Vec<Option<Name>>,
    ) -> Self {
        let parameter_names = parameter_names(proto, &param_overrides);
        let local_bindings = local_bindings(proto, &parameter_names);
        let naming = build_value_names(
            proto,
            function,
            &local_bindings,
            &parameter_names,
            &upvalue_overrides,
            &function_id,
        );
        Self {
            proto,
            function_id,
            upvalue_overrides,
            parameter_names,
            local_bindings,
            value_names: naming.names,
            value_bindings: naming.bindings,
        }
    }

    /// Return a visible local binding active at the use site.
    #[must_use]
    pub fn binding_for_use(&self, reg: u16, pc: i32) -> Option<LocalBinding> {
        self.binding_at(reg, pc)
    }

    /// Return a visible local binding associated with a definition.
    ///
    /// Lua debug ranges usually start after the initializer instructions, so
    /// this checks the definition boundary first and then a small lookahead.
    #[must_use]
    pub fn binding_for_def(&self, reg: u16, pc: i32) -> Option<LocalBinding> {
        self.binding_at(reg, pc + 1).or_else(|| {
            self.binding_at(reg, pc)
                .filter(|binding| binding.end_pc > pc + 1)
        })
    }

    /// Return whether this SSA destination is a named debug local.
    #[must_use]
    pub fn is_named_def(&self, reference: SsaRef, pc: i32) -> bool {
        let Some(reg) = reference.reg_index() else {
            return false;
        };
        self.binding_for_def(reg, pc).is_some()
    }

    /// Return the naming-group identity for a versioned register.
    #[must_use]
    pub fn binding_id_for_ref(&self, reference: SsaRef) -> Option<BindingId> {
        let SsaRef::Reg { reg, ver } = reference else {
            return None;
        };
        let version = usize::try_from(ver).ok()?;
        self.value_bindings
            .get(usize::from(reg))?
            .get(version)
            .cloned()
            .flatten()
    }

    /// Return whether lifetime analysis associated this SSA value with a
    /// source/debug local, including values defined at an end-PC boundary.
    #[must_use]
    pub fn has_debug_binding(&self, reference: SsaRef) -> bool {
        self.binding_id_for_ref(reference)
            .is_some_and(|binding| binding.is_debug_local())
    }

    /// Return the debug-local lifetime associated with an SSA value.
    #[must_use]
    pub fn debug_binding_for_ref(&self, reference: SsaRef) -> Option<LocalBinding> {
        let identity = self.binding_id_for_ref(reference)?;
        self.local_bindings
            .iter()
            .enumerate()
            .find(|(index, _)| self.debug_binding(*index) == identity)
            .map(|(_, binding)| binding.clone())
    }

    /// Return the local name for a use site or a deterministic fallback.
    #[must_use]
    pub fn name_for_ref(&self, reference: SsaRef, pc: i32) -> Name {
        if let SsaRef::Reg { reg, .. } = reference
            && let Some(binding) = self.binding_for_use(reg, pc)
        {
            let name = if binding.name.is_synthetic() {
                self.value_name(reference)
                    .unwrap_or_else(|| self.synthetic_value_name(reference))
            } else {
                binding.name
            };
            return name.with_binding(self.debug_binding(binding.index));
        }
        if let Some(name) = self.value_name(reference) {
            return name;
        }
        self.synthetic_value_name(reference)
    }

    /// Return a deterministic temporary name for a value.
    #[must_use]
    pub fn synthetic_value_name(&self, reference: SsaRef) -> Name {
        let name = match reference {
            SsaRef::Reg { reg, .. } => self.synthetic_reg_name(reg),
            SsaRef::Const(idx) => Name::from(format!("k{idx}")),
            SsaRef::None => Name::from("nil"),
        };
        self.binding_id_for_ref(reference)
            .map_or(name.clone(), |binding| name.with_binding(binding))
    }

    /// Return the canonical emitted name for a debug binding at a definition.
    #[must_use]
    pub fn name_for_binding_def(&self, binding: &LocalBinding, reference: SsaRef) -> Name {
        let name = if binding.name.is_synthetic() {
            self.value_name(reference)
                .unwrap_or_else(|| self.synthetic_value_name(reference))
        } else {
            binding.name.clone()
        };
        name.with_binding(self.debug_binding(binding.index))
    }

    /// Return a deterministic register fallback name.
    #[must_use]
    pub fn synthetic_reg_name(&self, reg: u16) -> Name {
        let role = if reg < u16::from(self.proto.num_params) {
            AnonymousNameRole::Parameter
        } else {
            AnonymousNameRole::Local
        };
        role.name(reg)
    }

    /// Return the single source-level name for a register SSA value.
    #[must_use]
    pub fn collapsed_name_for_ref(&self, reference: SsaRef, pc: i32) -> Name {
        self.name_for_ref(reference, pc)
    }

    /// Return an upvalue name or a deterministic fallback.
    #[must_use]
    pub fn upvalue_name(&self, idx: u16) -> Name {
        let idx_usize = usize::from(idx);
        if let Some(name) = self.upvalue_overrides.get(idx_usize)
            && is_valid_identifier(&name.0)
        {
            return name.binding().map_or_else(
                || name.clone().with_binding(self.upvalue_binding(idx_usize)),
                |_| name.clone(),
            );
        }
        if let Some(upvalue) = self.proto.upvalues.get(idx_usize)
            && is_valid_identifier(&upvalue.name)
        {
            return name_from_debug_identifier(upvalue.name.clone())
                .with_binding(self.upvalue_binding(idx_usize));
        }
        AnonymousNameRole::Upvalue
            .name(idx)
            .with_binding(self.upvalue_binding(idx_usize))
    }

    /// Return the emitted parameter name for a register index.
    #[must_use]
    pub fn parameter_name(&self, reg: u8) -> Name {
        self.parameter_names
            .get(usize::from(reg))
            .cloned()
            .unwrap_or_else(|| AnonymousNameRole::Parameter.name(reg))
            .with_binding(self.debug_binding(usize::from(reg)))
    }

    /// Binding identity for one debug-local slot in this lexical function.
    #[must_use]
    pub fn debug_binding(&self, index: usize) -> BindingId {
        BindingId::debug_local(&self.function_id, index)
    }

    /// Lexical identity for a child prototype.
    #[must_use]
    pub fn child_function_id(&self, index: usize) -> FunctionId {
        self.function_id.child(index)
    }

    fn upvalue_binding(&self, index: usize) -> BindingId {
        BindingId::upvalue(&self.function_id, index)
    }

    /// Return initial local declarations that are already in scope as params or
    /// later-phase internal locals.
    #[must_use]
    pub fn initially_declared_locals(&self) -> Vec<usize> {
        let mut declared = (0..usize::from(self.proto.num_params)).collect::<Vec<_>>();
        declared.extend(
            self.proto
                .loc_vars
                .iter()
                .enumerate()
                .filter_map(|(index, loc)| {
                    let is_param = index < usize::from(self.proto.num_params);
                    (is_param || is_internal_local(loc)).then_some(index)
                }),
        );
        declared.sort_unstable();
        declared.dedup();
        declared
    }

    /// Return whether a debug binding is in scope before SSA statements lower.
    #[must_use]
    pub fn is_declared_at_entry(&self, index: usize) -> bool {
        self.initially_declared_locals().contains(&index)
            || self
                .local_bindings
                .get(index)
                .is_some_and(|binding| binding.start_pc == 0)
    }

    /// Return leading locals whose explicit `nil` initializer has no VM instruction.
    #[must_use]
    pub fn implicit_nil_prefix(&self, first: &LocalBinding) -> Vec<(SsaRef, Name)> {
        self.local_bindings
            .iter()
            .filter(|binding| {
                binding.start_pc == first.start_pc
                    && binding.reg < first.reg
                    && !self.is_declared_at_entry(binding.index)
            })
            .map(|binding| {
                let reference = SsaRef::Reg {
                    reg: binding.reg,
                    ver: 0,
                };
                (reference, self.name_for_binding_def(binding, reference))
            })
            .collect()
    }

    /// Return named locals whose lexical lifetime begins at function entry but
    /// which are not parameters. Lua 5.1 represents an uninitialized local in
    /// this position only through debug lifetime metadata; there is no
    /// `LOADNIL` instruction to reconstruct.
    #[must_use]
    pub fn implicit_entry_declarations(&self) -> Vec<Name> {
        self.local_bindings
            .iter()
            .filter(|binding| {
                binding.index >= usize::from(self.proto.num_params) && binding.start_pc == 0
            })
            .map(|binding| {
                binding
                    .name
                    .clone()
                    .with_binding(self.debug_binding(binding.index))
            })
            .collect()
    }

    fn value_name(&self, reference: SsaRef) -> Option<Name> {
        let SsaRef::Reg { reg, ver } = reference else {
            return None;
        };
        self.value_names
            .get(usize::from(reg))
            .and_then(|versions| versions.get(usize::try_from(ver).ok()?))
            .cloned()
            .flatten()
    }

    fn binding_at(&self, reg: u16, pc: i32) -> Option<LocalBinding> {
        self.local_bindings
            .iter()
            .rev()
            .find(|binding| binding.reg == reg && binding_in_scope(binding, pc))
            .cloned()
            .or_else(|| {
                (usize::from(reg) < usize::from(self.proto.num_params)).then(|| LocalBinding {
                    index: usize::from(reg),
                    reg,
                    name: self.parameter_name(reg.try_into().unwrap_or(u8::MAX)),
                    start_pc: 0,
                    end_pc: i32::try_from(self.proto.code.len()).unwrap_or(i32::MAX),
                })
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ValueId {
    reg: u16,
    ver: u32,
}

#[derive(Debug, Clone)]
struct ValueSlot {
    value: ValueId,
    def_pc: Option<i32>,
    use_pcs: Vec<i32>,
}

#[derive(Debug)]
struct NamingPass<'a> {
    proto: &'a Proto,
    function: &'a SsaFunction,
    local_bindings: &'a [LocalBinding],
    parameter_names: &'a [Name],
    upvalue_overrides: &'a [Name],
    function_id: &'a FunctionId,
    slots: Vec<Vec<Option<usize>>>,
    values: Vec<ValueSlot>,
    parents: Vec<usize>,
}

impl<'a> NamingPass<'a> {
    fn new(
        proto: &'a Proto,
        function: &'a SsaFunction,
        local_bindings: &'a [LocalBinding],
        parameter_names: &'a [Name],
        upvalue_overrides: &'a [Name],
        function_id: &'a FunctionId,
    ) -> Self {
        Self {
            proto,
            function,
            local_bindings,
            parameter_names,
            upvalue_overrides,
            function_id,
            slots: vec![Vec::new(); function.num_regs],
            values: Vec::new(),
            parents: Vec::new(),
        }
    }

    fn run(mut self) -> NamingResult {
        self.collect_values();
        self.union_phi_values();
        self.union_debug_locals();
        self.assign_names()
    }

    fn collect_values(&mut self) {
        for reg in 0..self.proto.num_params {
            let id = self.ensure_ref(SsaRef::Reg {
                reg: u16::from(reg),
                ver: 0,
            });
            if let Some(id) = id {
                self.set_def_pc(id, 0);
            }
        }

        for block in &self.function.blocks {
            for node in &block.nodes {
                node.visit_defs(|reference| {
                    if let Some(id) = self.ensure_ref(reference) {
                        self.set_def_pc(id, node.pc);
                    }
                });
                node.op.visit_uses(|reference, _| {
                    if let Some(id) = self.ensure_ref(reference) {
                        self.record_use(id, node.pc);
                    }
                });
            }
        }
    }

    fn union_phi_values(&mut self) {
        for block in &self.function.blocks {
            for node in &block.nodes {
                let SsaOp::Phi { operands, .. } = &node.op else {
                    continue;
                };
                let Some(dest) = self.ensure_ref(node.dest) else {
                    continue;
                };
                for operand in operands {
                    if let Some(operand) = self.ensure_ref(*operand) {
                        self.union(dest, operand);
                    }
                }
            }
        }
    }

    fn union_debug_locals(&mut self) {
        for binding in self.local_bindings {
            let ids = self
                .values
                .iter()
                .enumerate()
                .filter_map(|(id, slot)| {
                    (slot.value.reg == binding.reg && self.value_in_binding(slot, binding))
                        .then_some(id)
                })
                .collect::<Vec<_>>();
            let Some((&first, rest)) = ids.split_first() else {
                continue;
            };
            for id in rest {
                self.union(first, *id);
            }
        }
    }

    fn assign_names(mut self) -> NamingResult {
        let debug_bindings = self.debug_bindings_by_root();
        let mut groups = BTreeMap::<usize, GroupInfo>::new();
        for id in 0..self.values.len() {
            let root = self.find(id);
            let slot = &self.values[id];
            let info = groups.entry(root).or_insert_with(|| GroupInfo {
                root,
                first_pc: slot.def_pc.unwrap_or(i32::MAX),
                first_reg: slot.value.reg,
                has_param: false,
                debug_binding: debug_bindings.get(&root).cloned(),
            });
            let pc = slot.def_pc.unwrap_or(i32::MAX);
            if (pc, slot.value.reg) < (info.first_pc, info.first_reg) {
                info.first_pc = pc;
                info.first_reg = slot.value.reg;
            }
            if usize::from(slot.value.reg) < usize::from(self.proto.num_params)
                && slot.value.ver == 0
            {
                info.has_param = true;
            }
        }

        let mut groups = groups.into_values().collect::<Vec<_>>();
        groups.sort_by_key(|info| (info.first_pc, info.first_reg, info.root));

        let mut used = reserved_names(
            self.parameter_names,
            self.local_bindings,
            self.upvalue_overrides,
        );
        let mut anonymous_locals = BTreeMap::<u16, Name>::new();
        let mut names_by_root = BTreeMap::<usize, (Name, BindingId)>::new();
        for group in groups {
            let (name, binding) = if let Some((index, name)) = group.debug_binding {
                (name, BindingId::debug_local(self.function_id, index))
            } else if group.has_param {
                (
                    self.parameter_names
                        .get(usize::from(group.first_reg))
                        .cloned()
                        .unwrap_or_else(|| AnonymousNameRole::Parameter.name(group.first_reg)),
                    BindingId::debug_local(self.function_id, usize::from(group.first_reg)),
                )
            } else {
                let name = if let Some(name) = anonymous_locals.get(&group.first_reg) {
                    name.clone()
                } else {
                    let name =
                        unique_name(AnonymousNameRole::Local.base(group.first_reg), &mut used);
                    anonymous_locals.insert(group.first_reg, name.clone());
                    name
                };
                (name, BindingId::synthetic(self.function_id, group.root))
            };
            names_by_root.insert(group.root, (name, binding));
        }

        let mut value_names = vec![Vec::new(); self.function.num_regs];
        let mut value_bindings = vec![Vec::new(); self.function.num_regs];
        for id in 0..self.values.len() {
            let root = self.find(id);
            let Some((name, binding)) = names_by_root.get(&root).cloned() else {
                continue;
            };
            let value = self.values[id].value;
            let reg = usize::from(value.reg);
            let Ok(version) = usize::try_from(value.ver) else {
                continue;
            };
            if reg >= value_names.len() {
                value_names.resize_with(reg + 1, Vec::new);
                value_bindings.resize_with(reg + 1, Vec::new);
            }
            if version >= value_names[reg].len() {
                value_names[reg].resize(version + 1, None);
                value_bindings[reg].resize(version + 1, None);
            }
            value_names[reg][version] = Some(name.with_binding(binding.clone()));
            value_bindings[reg][version] = Some(binding);
        }
        NamingResult {
            names: value_names,
            bindings: value_bindings,
        }
    }

    fn debug_bindings_by_root(&mut self) -> BTreeMap<usize, (usize, Name)> {
        let mut names = BTreeMap::new();
        for binding in self.local_bindings {
            if binding.name.is_synthetic() {
                continue;
            }
            let mut matching_id = None;
            for (id, slot) in self.values.iter().enumerate() {
                if slot.value.reg == binding.reg && self.value_in_binding(slot, binding) {
                    matching_id = Some(id);
                    break;
                }
            }
            if let Some(id) = matching_id {
                let root = self.find(id);
                names
                    .entry(root)
                    .or_insert_with(|| (binding.index, binding.name.clone()));
            }
        }
        names
    }

    fn value_in_binding(&self, slot: &ValueSlot, binding: &LocalBinding) -> bool {
        if usize::from(binding.reg) < usize::from(self.proto.num_params) && slot.value.ver == 0 {
            return true;
        }
        if slot
            .use_pcs
            .iter()
            .copied()
            .any(|use_pc| binding_in_scope(binding, use_pc))
        {
            return true;
        }
        let Some(pc) = slot.def_pc else {
            return false;
        };
        binding_in_scope(binding, pc) || pc.checked_add(1) == Some(binding.start_pc)
    }

    fn ensure_ref(&mut self, reference: SsaRef) -> Option<usize> {
        let SsaRef::Reg { reg, ver } = reference else {
            return None;
        };
        let reg_index = usize::from(reg);
        if reg_index >= self.slots.len() {
            self.slots.resize_with(reg_index + 1, Vec::new);
        }
        let version = usize::try_from(ver).ok()?;
        if version >= self.slots[reg_index].len() {
            self.slots[reg_index].resize(version + 1, None);
        }
        if let Some(id) = self.slots[reg_index][version] {
            return Some(id);
        }
        let id = self.values.len();
        self.values.push(ValueSlot {
            value: ValueId { reg, ver },
            def_pc: None,
            use_pcs: Vec::new(),
        });
        self.parents.push(id);
        self.slots[reg_index][version] = Some(id);
        Some(id)
    }

    fn set_def_pc(&mut self, id: usize, pc: i32) {
        if let Some(slot) = self.values.get_mut(id)
            && slot.def_pc.is_none_or(|current| pc < current)
        {
            slot.def_pc = Some(pc);
        }
    }

    fn record_use(&mut self, id: usize, pc: i32) {
        if let Some(slot) = self.values.get_mut(id)
            && !slot.use_pcs.contains(&pc)
        {
            slot.use_pcs.push(pc);
        }
    }

    fn union(&mut self, a: usize, b: usize) {
        let a = self.find(a);
        let b = self.find(b);
        if a != b {
            self.parents[b] = a;
        }
    }

    fn find(&mut self, id: usize) -> usize {
        let parent = self.parents[id];
        if parent == id {
            return id;
        }
        let root = self.find(parent);
        self.parents[id] = root;
        root
    }
}

#[derive(Debug, Clone)]
struct GroupInfo {
    root: usize,
    first_pc: i32,
    first_reg: u16,
    has_param: bool,
    debug_binding: Option<(usize, Name)>,
}

fn build_value_names(
    proto: &Proto,
    function: &SsaFunction,
    local_bindings: &[LocalBinding],
    parameter_names: &[Name],
    upvalue_overrides: &[Name],
    function_id: &FunctionId,
) -> NamingResult {
    NamingPass::new(
        proto,
        function,
        local_bindings,
        parameter_names,
        upvalue_overrides,
        function_id,
    )
    .run()
}

fn parameter_names(proto: &Proto, overrides: &[Option<Name>]) -> Vec<Name> {
    (0..proto.num_params)
        .map(|reg| parameter_name_for(proto, overrides, reg))
        .collect()
}

fn parameter_name_for(proto: &Proto, overrides: &[Option<Name>], reg: u8) -> Name {
    let index = usize::from(reg);
    if let Some(Some(name)) = overrides.get(index)
        && is_valid_identifier(&name.0)
    {
        return name.clone();
    }
    if let Some(loc) = proto.loc_vars.get(index)
        && is_visible_local(loc)
    {
        return name_from_debug_identifier(loc.name.clone());
    }
    AnonymousNameRole::Parameter.name(reg)
}

fn local_bindings(proto: &Proto, parameter_names: &[Name]) -> Vec<LocalBinding> {
    proto
        .loc_vars
        .iter()
        .enumerate()
        .filter_map(|(index, loc)| {
            if !is_visible_local(loc) {
                return None;
            }
            let reg = locvar_reg(proto, index)?;
            let name = if index < usize::from(proto.num_params) {
                parameter_names
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| AnonymousNameRole::Parameter.name(index))
            } else {
                name_from_debug_identifier(loc.name.clone())
            };
            Some(LocalBinding {
                index,
                reg,
                name,
                start_pc: loc.start_pc,
                end_pc: loc.end_pc,
            })
        })
        .collect()
}

fn locvar_reg(proto: &Proto, index: usize) -> Option<u16> {
    let loc = proto.loc_vars.get(index)?;
    let closed = proto.loc_vars[..index]
        .iter()
        .filter(|previous| previous.end_pc < loc.start_pc)
        .count();
    u16::try_from(index.checked_sub(closed)?).ok()
}

fn binding_in_scope(binding: &LocalBinding, pc: i32) -> bool {
    (binding.start_pc <= pc && pc < binding.end_pc)
        || (binding.start_pc == binding.end_pc
            && binding.start_pc > 0
            && pc == binding.start_pc - 1)
}

fn reserved_names(
    parameter_names: &[Name],
    local_bindings: &[LocalBinding],
    upvalue_overrides: &[Name],
) -> BTreeSet<String> {
    let mut used = BTreeSet::new();
    for name in parameter_names {
        used.insert(name.0.to_string());
    }
    for binding in local_bindings {
        if !binding.name.is_synthetic() {
            used.insert(binding.name.0.to_string());
        }
    }
    for name in upvalue_overrides {
        if is_valid_identifier(&name.0) {
            used.insert(name.0.to_string());
        }
    }
    used
}

fn unique_name(base: String, used: &mut BTreeSet<String>) -> Name {
    let mut candidate = base.clone();
    let mut suffix = 2;
    while used.contains(&candidate) || !is_valid_identifier(&BString::from(candidate.as_str())) {
        candidate = format!("{base}_{suffix}");
        suffix += 1;
    }
    used.insert(candidate.clone());
    Name::from(candidate)
}

fn is_visible_local(loc: &LocVar) -> bool {
    !is_internal_local(loc) && is_valid_identifier(&loc.name)
}

fn name_from_debug_identifier(bytes: BString) -> Name {
    if is_synthetic_identifier(bytes.as_slice()) {
        Name::synthetic(bytes)
    } else {
        Name::new(bytes)
    }
}

fn is_synthetic_identifier(bytes: &[u8]) -> bool {
    has_prefixed_number(bytes, b"a", true)
        || has_prefixed_number(bytes, b"l", true)
        || has_prefixed_number(bytes, b"u", true)
        || has_prefixed_number(bytes, b"v", true)
        || has_prefixed_number(bytes, b"arg", true)
        || has_prefixed_number(bytes, b"up", true)
        || has_prefixed_number(bytes, b"k", false)
        || has_prefixed_number(bytes, b"__nw_lua_pack_", false)
        || has_prefixed_number(bytes, b"__nw_lua_values_", false)
        || has_prefixed_number(bytes, b"__nw_lua_index_", false)
}

fn has_prefixed_number(bytes: &[u8], prefix: &[u8], allow_number_suffixes: bool) -> bool {
    let Some(rest) = bytes.strip_prefix(prefix) else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    if !allow_number_suffixes {
        return rest.iter().copied().all(|byte| byte.is_ascii_digit());
    }
    let mut saw_digit = false;
    let mut previous_was_underscore = false;
    for byte in rest {
        match *byte {
            b'0'..=b'9' => {
                saw_digit = true;
                previous_was_underscore = false;
            }
            b'_' if saw_digit && !previous_was_underscore => {
                saw_digit = false;
                previous_was_underscore = true;
            }
            _ => return false,
        }
    }
    saw_digit && !previous_was_underscore
}

fn is_internal_local(loc: &LocVar) -> bool {
    loc.name.is_empty() || loc.name.as_slice().first().copied() == Some(b'(')
}
