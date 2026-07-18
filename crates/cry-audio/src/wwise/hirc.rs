//! Typed Wwise HIRC readers and the event → media reachability walk.
//!
//! The generic [`WwiseHierarchyObject`](super::WwiseHierarchyObject) blobs give
//! each object's kind, id, and byte span. This module adds the *typed* readers
//! needed to answer one question: which encoded media does a given HIRC event
//! actually play? It walks `Event → Action → Sound / Random-Sequence / Switch /
//! Layer container` links and collects the source (media) ids reachable from the
//! event's play actions.
//!
//! # Evidence
//!
//! Offsets below are for **Wwise bank version 150** (Wwise 2019.1), the version
//! New World ships (`BKHD.dwBankGeneratorVersion == 150`). They were verified
//! byte-for-byte against real banks, e.g. `sounds/wwise/ftsp_alligator_events.bnk`,
//! where every walked object has no effects (`uNumFx == 0`):
//!
//! * `CAkAction` body: `ulActionType` `u16` @0, `idExt` (target object) `u32` @2.
//!   A *play* action has the high byte `0x04` (e.g. `0x0403`).
//! * `CAkSound` body — `AkBankSourceData`: `ulPluginID` `u32` @0, `StreamType`
//!   `u8` @4, then `AkMediaInformation` { `sourceID` `u32` @5,
//!   `uInMemoryMediaSize` `u32` @9, `uSourceBits` `u8` @13 }. `NodeBaseParams`
//!   follows at @14; its `DirectParentID` lands at @22 in these banks.
//! * Container bodies (`CAkRanSeqCntr` 5, `CAkSwitchCntr` 6, `CAkLayerCntr` 9,
//!   `CAkActorMixer` 7) begin with `NodeBaseParams`; `DirectParentID` lands at
//!   @8.
//!
//! # Why parent links
//!
//! The container → children edges are recovered from each object's authored
//! `DirectParentID` (early in `NodeBaseParams`), the exact inverse of the
//! `AkChildren` array that appears only after the full positioning / state /
//! RTPC tail. Wwise keeps the two encodings consistent, so inverting the parent
//! edges reproduces the forward child links while parsing an order of magnitude
//! less of each object. Every read is bounds-checked; an object that cannot be
//! parsed simply contributes no edge (it is never fatal), so a stray effects
//! chain on an unreachable node cannot corrupt a reachable subtree.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::ranseq::{WwiseRandomContainer, weighted_sequence_seed};
use super::{WwiseHierarchyObjectKind, WwiseMediaId, WwiseObjectId, WwiseSoundBank};

impl WwiseHierarchyObjectKind {
    /// `CAkSound`.
    pub const SOUND: Self = Self(2);
    /// `CAkAction`.
    pub const ACTION: Self = Self(3);
    /// `CAkRanSeqCntr` (random / sequence container).
    pub const RANDOM_SEQUENCE_CONTAINER: Self = Self(5);
    /// `CAkSwitchCntr`.
    pub const SWITCH_CONTAINER: Self = Self(6);
    /// `CAkActorMixer`.
    pub const ACTOR_MIXER: Self = Self(7);
    /// `CAkLayerCntr`.
    pub const LAYER_CONTAINER: Self = Self(9);
}

/// One decoded `CAkAction`: its type word and the object it drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WwiseAction {
    pub action_type: u16,
    pub target: WwiseObjectId,
}

impl WwiseAction {
    /// `ulActionType` `u16` @0, `idExt` (target) `u32` @2.
    #[must_use]
    pub fn parse(body: &[u8]) -> Option<Self> {
        let action_type = read_u16(body, 0)?;
        let target = WwiseObjectId(read_u32(body, 2)?);
        Some(Self {
            action_type,
            target,
        })
    }

    /// Play actions carry the high byte `0x04` (`Play`, `PlayAndContinue`).
    #[must_use]
    pub const fn is_play(self) -> bool {
        self.action_type & 0xff00 == 0x0400
    }
}

/// One branch of a `CAkSwitchCntr`: the switch value it fires on and the media
/// that branch plays. The surface *name* is not resolved here (a bank has no
/// name table); a caller cross-references the FX library + Wwise switch ids to
/// tag [`Self::switch_id`] with a surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WwiseSwitchBranch {
    /// The `ulSwitchID` this branch selects on (an `AK::SoundEngine` FNV of the
    /// switch state name, though the mapping must be validated before trusting).
    pub switch_id: u32,
    /// True when this is the container's authored `ulDefaultSwitch` branch.
    pub is_default: bool,
    /// Media this branch reaches, in default-descent order (deduplicated).
    pub media: Vec<WwiseMediaId>,
}

impl WwiseSoundBank {
    /// True when this bank's HIRC defines the given event object.
    #[must_use]
    pub fn defines_event(&self, event: WwiseObjectId) -> bool {
        self.hierarchy.iter().any(|object| {
            object.kind == WwiseHierarchyObjectKind::EVENT && object.object_id == event
        })
    }

    /// The set of media (source) ids reachable from `event` through its play
    /// actions and the container hierarchy. A random/sequence container's
    /// variations, and every branch of a switch/layer container, all count —
    /// they are all children of the played node. Media that no play action can
    /// reach is not collected.
    ///
    /// `bytes` must be the same buffer the bank was parsed from (object bodies
    /// are addressed by absolute offset). Pure: byte slices in, ids out.
    #[must_use]
    pub fn event_media(&self, bytes: &[u8], event: WwiseObjectId) -> BTreeSet<WwiseMediaId> {
        HircGraph::index(self, bytes).media_for_event(event)
    }

    /// The ordered, deduplicated media the *default* switch branch plays for
    /// `event`. Identical to [`Self::event_media`] except that a switch container
    /// descends only the package whose `ulSwitchID` equals the authored
    /// `ulDefaultSwitch` (pure id equality — no surface-name hashing, which was
    /// found unreliable), and a random/sequence container yields its variations
    /// in authored playlist order. Order is first-encounter; media ids are
    /// unique. A container whose typed tail cannot be validated degrades to its
    /// full child set (never fatal), so this is always a subset of `event_media`.
    ///
    /// `bytes` must be the same buffer the bank was parsed from. Pure: byte
    /// slices in, ids out.
    #[must_use]
    pub fn event_default_media(&self, bytes: &[u8], event: WwiseObjectId) -> Vec<WwiseMediaId> {
        HircGraph::index(self, bytes).default_media_for_event(event)
    }

    /// Every switch-container branch reachable from `event`'s play actions, each
    /// tagged with its `ulSwitchID`, whether it is the default branch, and the
    /// media it plays (default-descent order). Empty when the event plays no
    /// switch container (e.g. a plain blend/random container). The surface *name*
    /// is not resolved — a bank carries no name table; a caller validates the FX
    /// library's surface → switch-id mapping against these ids.
    ///
    /// `bytes` must be the same buffer the bank was parsed from. Pure.
    #[must_use]
    pub fn event_switch_branches(
        &self,
        bytes: &[u8],
        event: WwiseObjectId,
    ) -> Vec<WwiseSwitchBranch> {
        HircGraph::index(self, bytes).switch_branches_for_event(event)
    }

    /// A deterministic, engine-faithful sequence of `count` media selections for
    /// `event`'s `surface_switch_id` branch, reproducing the `CAkRanSeqCntr`'s
    /// authored mode, weights, and avoid-repeat count (see
    /// [`WwiseRandomContainer::select_sequence`]). The seed is derived purely from
    /// `event`, `surface_switch_id`, and the container id, so an export is
    /// byte-reproducible — no system randomness or time.
    ///
    /// When the target is a switch container, the branch whose `ulSwitchID`
    /// matches `surface_switch_id` is descended (falling back to the default
    /// branch when it has no such package); otherwise `surface_switch_id` is
    /// ignored. If the branch has no parseable random container, the branch's
    /// ordered default media is cycled to `count` instead (never empty when the
    /// branch reaches any media).
    ///
    /// `bytes` must be the same buffer the bank was parsed from. Pure.
    #[must_use]
    pub fn event_weighted_sequence(
        &self,
        bytes: &[u8],
        event: WwiseObjectId,
        surface_switch_id: u32,
        count: usize,
    ) -> Vec<WwiseMediaId> {
        HircGraph::index(self, bytes).weighted_sequence_for_event(event, surface_switch_id, count)
    }
}

/// The typed slice of a bank's HIRC needed to walk events to media.
struct HircGraph {
    /// Event object id → its action ids.
    events: HashMap<u32, Vec<WwiseObjectId>>,
    /// Play-action id → the object it targets.
    play_targets: HashMap<u32, WwiseObjectId>,
    /// Sound object id → its source media id.
    sound_source: HashMap<u32, WwiseMediaId>,
    /// Parent object id → its child object ids (inverted `DirectParentID`).
    children: HashMap<u32, Vec<u32>>,
    /// Switch-container id → the default package's node ids (typed tail parse).
    /// Absent when the tail could not be validated (walk uses `children`).
    switch_default: HashMap<u32, Vec<u32>>,
    /// Random/sequence-container id → its children in authored playlist order.
    /// Absent when the playlist could not be validated (walk uses `children`).
    ranseq_order: HashMap<u32, Vec<u32>>,
    /// Switch-container id → its full typed tail (every branch + default switch).
    /// Absent when the tail could not be validated.
    switch_all: HashMap<u32, SwitchTail>,
    /// Random/sequence-container id → its parsed weighted playlist (mode, random
    /// mode, avoid-repeat, weighted items). Absent when the tail could not be
    /// validated (the weighted walk degrades to ordered media).
    ranseq_container: HashMap<u32, WwiseRandomContainer>,
}

impl HircGraph {
    fn index(bank: &WwiseSoundBank, bytes: &[u8]) -> Self {
        let mut graph = Self {
            events: HashMap::new(),
            play_targets: HashMap::new(),
            sound_source: HashMap::new(),
            children: HashMap::new(),
            switch_default: HashMap::new(),
            ranseq_order: HashMap::new(),
            switch_all: HashMap::new(),
            ranseq_container: HashMap::new(),
        };
        for object in &bank.hierarchy {
            // Object data spans `[object_id, body]`; the typed body starts after
            // the 4-byte object id.
            let Some(body) = object.data(bytes).and_then(|data| data.get(4..)) else {
                continue;
            };
            match object.kind {
                WwiseHierarchyObjectKind::EVENT => {
                    graph
                        .events
                        .insert(object.object_id.0, object.event_action_ids.clone());
                }
                WwiseHierarchyObjectKind::ACTION => {
                    if let Some(action) = WwiseAction::parse(body) {
                        if action.is_play() {
                            graph.play_targets.insert(object.object_id.0, action.target);
                        }
                    }
                }
                WwiseHierarchyObjectKind::SOUND => {
                    if let Some((source, parent)) = read_sound(body) {
                        graph.sound_source.insert(object.object_id.0, source);
                        graph.link(parent.0, object.object_id.0);
                    }
                }
                WwiseHierarchyObjectKind::RANDOM_SEQUENCE_CONTAINER
                | WwiseHierarchyObjectKind::SWITCH_CONTAINER
                | WwiseHierarchyObjectKind::LAYER_CONTAINER
                | WwiseHierarchyObjectKind::ACTOR_MIXER => {
                    if let Some(parent) = read_node_parent(body, 0) {
                        graph.link(parent.0, object.object_id.0);
                    }
                }
                _ => {}
            }
        }
        // Pass 2: typed switch / random-sequence tails. The default-branch walk
        // needs, per switch, the node ids of the package whose `ulSwitchID`
        // equals `ulDefaultSwitch`, and per random/sequence container, its
        // children in authored playlist order. Both are anchored on the child
        // set already recovered by parent inversion (Pass 1), then validated by
        // requiring the typed tail to consume the object body exactly — which
        // avoids re-parsing the version-fragile positioning / state / RTPC middle
        // of `NodeBaseParams`. A tail that fails to validate is simply omitted,
        // so the default walk degrades to the full child set for that node.
        for object in &bank.hierarchy {
            let Some(body) = object.data(bytes).and_then(|data| data.get(4..)) else {
                continue;
            };
            let id = object.object_id.0;
            match object.kind {
                WwiseHierarchyObjectKind::SWITCH_CONTAINER => {
                    if let Some(tail) = graph
                        .children
                        .get(&id)
                        .and_then(|children| parse_switch_tail(body, children))
                    {
                        // The default-branch walk keeps its existing input: the
                        // default package's nodes when non-empty (else it degrades
                        // to the full child set).
                        let default_nodes = tail.default_nodes();
                        if !default_nodes.is_empty() {
                            graph.switch_default.insert(id, default_nodes);
                        }
                        graph.switch_all.insert(id, tail);
                    }
                }
                WwiseHierarchyObjectKind::RANDOM_SEQUENCE_CONTAINER => {
                    if let Some(children) = graph.children.get(&id) {
                        if let Some(order) = parse_ranseq_order(body, children) {
                            graph.ranseq_order.insert(id, order);
                        }
                        if let Some(container) = WwiseRandomContainer::parse(body, children) {
                            graph.ranseq_container.insert(id, container);
                        }
                    }
                }
                _ => {}
            }
        }
        graph
    }

    fn link(&mut self, parent: u32, child: u32) {
        self.children.entry(parent).or_default().push(child);
    }

    fn media_for_event(&self, event: WwiseObjectId) -> BTreeSet<WwiseMediaId> {
        let mut media = BTreeSet::new();
        let mut visited = HashSet::new();
        if let Some(actions) = self.events.get(&event.0) {
            for action in actions {
                if let Some(target) = self.play_targets.get(&action.0) {
                    self.collect(target.0, &mut media, &mut visited);
                }
            }
        }
        media
    }

    fn collect(&self, node: u32, media: &mut BTreeSet<WwiseMediaId>, visited: &mut HashSet<u32>) {
        if !visited.insert(node) {
            return;
        }
        if let Some(source) = self.sound_source.get(&node) {
            media.insert(*source);
        }
        if let Some(children) = self.children.get(&node) {
            for child in children {
                self.collect(*child, media, visited);
            }
        }
    }

    fn default_media_for_event(&self, event: WwiseObjectId) -> Vec<WwiseMediaId> {
        let mut media = Vec::new();
        let mut media_seen = HashSet::new();
        let mut visited = HashSet::new();
        if let Some(actions) = self.events.get(&event.0) {
            for action in actions {
                if let Some(target) = self.play_targets.get(&action.0) {
                    self.collect_default(target.0, &mut media, &mut media_seen, &mut visited);
                }
            }
        }
        media
    }

    fn collect_default(
        &self,
        node: u32,
        media: &mut Vec<WwiseMediaId>,
        media_seen: &mut HashSet<u32>,
        visited: &mut HashSet<u32>,
    ) {
        if !visited.insert(node) {
            return;
        }
        if let Some(source) = self.sound_source.get(&node)
            && media_seen.insert(source.0)
        {
            media.push(*source);
        }
        // A switch descends only its default package; a random/sequence container
        // descends its authored playlist order; every other node (layer, actor
        // mixer, or a container whose typed tail did not validate) descends its
        // full child set, exactly as the reachability walk does.
        let branch = self
            .switch_default
            .get(&node)
            .or_else(|| self.ranseq_order.get(&node))
            .or_else(|| self.children.get(&node));
        if let Some(children) = branch {
            for child in children {
                self.collect_default(*child, media, media_seen, visited);
            }
        }
    }

    /// Every switch-container branch reachable from the event's play actions.
    fn switch_branches_for_event(&self, event: WwiseObjectId) -> Vec<WwiseSwitchBranch> {
        let mut branches = Vec::new();
        let Some(actions) = self.events.get(&event.0) else {
            return branches;
        };
        for action in actions {
            let Some(target) = self.play_targets.get(&action.0) else {
                continue;
            };
            let Some(tail) = self.switch_all.get(&target.0) else {
                continue;
            };
            for (switch_id, nodes) in &tail.packages {
                let mut media = Vec::new();
                let mut media_seen = HashSet::new();
                let mut visited = HashSet::new();
                for node in nodes {
                    self.collect_default(*node, &mut media, &mut media_seen, &mut visited);
                }
                branches.push(WwiseSwitchBranch {
                    switch_id: *switch_id,
                    is_default: *switch_id == tail.default_switch,
                    media,
                });
            }
        }
        branches
    }

    /// The deterministic weighted media sequence for a surface branch.
    fn weighted_sequence_for_event(
        &self,
        event: WwiseObjectId,
        surface_switch_id: u32,
        count: usize,
    ) -> Vec<WwiseMediaId> {
        let Some(actions) = self.events.get(&event.0) else {
            return Vec::new();
        };
        for action in actions {
            let Some(target) = self.play_targets.get(&action.0) else {
                continue;
            };
            // Resolve the branch node ids and the switch id that actually selected
            // them (the surface's package, else the container default, else the
            // whole target when it is not a switch).
            let (branch_nodes, resolved_switch) = match self.switch_all.get(&target.0) {
                Some(tail) => {
                    let nodes = tail
                        .package_nodes(surface_switch_id)
                        .or_else(|| tail.package_nodes(tail.default_switch));
                    match nodes {
                        Some(nodes) => (nodes.to_vec(), surface_switch_id),
                        None => (Vec::new(), surface_switch_id),
                    }
                }
                None => (vec![target.0], surface_switch_id),
            };

            // Prefer the first parseable random container in the branch; its
            // authored mode/weights drive an engine-faithful stream.
            let mut visited = HashSet::new();
            let container_id = branch_nodes
                .iter()
                .find_map(|node| self.first_random_container(*node, &mut visited));
            if let Some(container_id) = container_id
                && let Some(container) = self.ranseq_container.get(&container_id)
            {
                let seed = weighted_sequence_seed(event.0, resolved_switch, container_id);
                let sequence =
                    container.select_media(seed, count, |node| self.node_first_media(node));
                if !sequence.is_empty() {
                    return sequence;
                }
            }

            // No parseable random container: cycle the branch's ordered media.
            let mut media = Vec::new();
            let mut media_seen = HashSet::new();
            let mut branch_visited = HashSet::new();
            for node in &branch_nodes {
                self.collect_default(*node, &mut media, &mut media_seen, &mut branch_visited);
            }
            if !media.is_empty() {
                return (0..count).map(|k| media[k % media.len()]).collect();
            }
        }
        Vec::new()
    }

    /// The first random/sequence container reachable from `node`, following the
    /// default branch of any switch on the way. `None` when the subtree has none.
    fn first_random_container(&self, node: u32, visited: &mut HashSet<u32>) -> Option<u32> {
        if !visited.insert(node) {
            return None;
        }
        if self.ranseq_container.contains_key(&node) {
            return Some(node);
        }
        let branch = self
            .switch_default
            .get(&node)
            .or_else(|| self.ranseq_order.get(&node))
            .or_else(|| self.children.get(&node))?;
        branch
            .iter()
            .find_map(|child| self.first_random_container(*child, visited))
    }

    /// The first media id a node plays, following default branches. Used to map a
    /// selected playlist node (usually a `CAkSound`) to its source media.
    fn node_first_media(&self, node: u32) -> Option<WwiseMediaId> {
        let mut visited = HashSet::new();
        self.first_media(node, &mut visited)
    }

    fn first_media(&self, node: u32, visited: &mut HashSet<u32>) -> Option<WwiseMediaId> {
        if !visited.insert(node) {
            return None;
        }
        if let Some(source) = self.sound_source.get(&node) {
            return Some(*source);
        }
        let branch = self
            .switch_default
            .get(&node)
            .or_else(|| self.ranseq_order.get(&node))
            .or_else(|| self.children.get(&node))?;
        branch
            .iter()
            .find_map(|child| self.first_media(*child, visited))
    }
}

/// A `CAkSwitchCntr`'s validated typed tail: its authored default switch and
/// every `(ulSwitchID, node ids)` package.
struct SwitchTail {
    default_switch: u32,
    packages: Vec<(u32, Vec<u32>)>,
}

impl SwitchTail {
    /// The default package's node ids (empty when the default switch names no
    /// package — the caller then degrades to the full child set).
    fn default_nodes(&self) -> Vec<u32> {
        self.package_nodes(self.default_switch)
            .map(<[u32]>::to_vec)
            .unwrap_or_default()
    }

    fn package_nodes(&self, switch_id: u32) -> Option<&[u32]> {
        self.packages
            .iter()
            .find(|(id, _)| *id == switch_id)
            .map(|(_, nodes)| nodes.as_slice())
    }
}

/// `CAkSound` → (source media id, `DirectParentID`).
///
/// `AkBankSourceData` is 14 bytes for a bank/streamed source; a *source plugin*
/// (plugin type nibble `0x02`, e.g. a tone generator) inserts a `u32` size and
/// its payload before `NodeBaseParams`.
fn read_sound(body: &[u8]) -> Option<(WwiseMediaId, WwiseObjectId)> {
    let plugin_id = read_u32(body, 0)?;
    let _stream_type = *body.get(4)?;
    let source_id = read_u32(body, 5)?;
    // `uInMemoryMediaSize` @9, `uSourceBits` @13 are not needed for the walk.
    let mut cursor = 14usize;
    if plugin_id & 0x0f == 0x02 {
        let size = read_u32(body, cursor)? as usize;
        cursor = cursor.checked_add(4)?.checked_add(size)?;
    }
    let parent = read_node_parent(body, cursor)?;
    Some((WwiseMediaId(source_id), parent))
}

/// Skip the head of `NodeBaseParams` and read `DirectParentID`.
///
/// `NodeInitialFxParams`: `bIsOverrideParentFX` `u8`, `uNumFx` `u8`, then when
/// `uNumFx > 0` a `bitsFXBypass` `u8` and `uNumFx` effect records; then the
/// Wwise 2019 metadata-effects pair `bIsOverrideParentMetadata` `u8`,
/// `uNumFxMetadata` `u8` with its own records. `OverrideBusId` `u32` and
/// `DirectParentID` `u32` follow. Each effect record is 7 bytes: `uFXIndex`
/// `u8`, `fxID` `u32`, `bIsShareSet` `u8`, `bIsRendered` `u8`.
fn read_node_parent(body: &[u8], mut cursor: usize) -> Option<WwiseObjectId> {
    let _override_fx = *body.get(cursor)?;
    cursor = cursor.checked_add(1)?;
    let num_fx = *body.get(cursor)?;
    cursor = cursor.checked_add(1)?;
    if num_fx > 0 {
        cursor = cursor.checked_add(1)?; // bitsFXBypass
        cursor = cursor.checked_add((num_fx as usize).checked_mul(FX_RECORD_LEN)?)?;
    }
    let _override_metadata = *body.get(cursor)?;
    cursor = cursor.checked_add(1)?;
    let num_fx_metadata = *body.get(cursor)?;
    cursor = cursor.checked_add(1)?;
    if num_fx_metadata > 0 {
        cursor = cursor.checked_add((num_fx_metadata as usize).checked_mul(FX_RECORD_LEN)?)?;
    }
    let _override_bus = read_u32(body, cursor)?;
    cursor = cursor.checked_add(4)?; // OverrideBusId
    Some(WwiseObjectId(read_u32(body, cursor)?))
}

const FX_RECORD_LEN: usize = 1 + 4 + 1 + 1;

/// `AkSwitchNodeParams` record length in a `CAkSwitchCntr` tail: `ulNodeID`
/// `u32`, two playback/mode bit-vector bytes, `FadeOutTime` `u32`, `FadeInTime`
/// `u32`. Verified by exact body consumption on the alligator switch container
/// (2 params over its final 28 bytes).
const SWITCH_PARAM_LEN: usize = 4 + 1 + 1 + 4 + 4;

/// `CAkRanSeqCntr` playlist item length: `AkUInt32` node id + `AkUInt32` weight.
const PLAYLIST_ITEM_LEN: usize = 4 + 4;

/// A `CAkSwitchCntr`'s full typed tail (default switch + every branch package),
/// or `None` when it cannot be validated (caller degrades to the full child set).
///
/// # Evidence (bank version 150, `sounds/wwise/ftsp_alligator_events.bnk`)
///
/// After `NodeBaseParams`, a `CAkSwitchCntr` body carries `eGroupType` `u8`,
/// `ulGroupID` `u32`, `ulDefaultSwitch` `u32`, `bIsContinuousValidation` `u8`,
/// then the `AkUInt32`-counted children array, then `ulNumSwitchGroups` `u32`
/// packages (each `ulSwitchID` `u32` + an `AkUInt32`-counted node-id list), then
/// `ulNumSwitchParams` `u32` records of [`SWITCH_PARAM_LEN`] bytes. In the
/// alligator switch (`id 61188014`, body length 142) the children array lands at
/// body offset 46, so `ulDefaultSwitch` (`2108779966`) sits at 41; the container
/// has four packages over two child blend containers.
///
/// Rather than skip the version-fragile positioning / state / RTPC tail of
/// `NodeBaseParams`, the children array is located by matching the child set
/// already recovered by parent inversion; the whole switch tail is then required
/// to consume the body exactly, which uniquely pins the anchor.
fn parse_switch_tail(body: &[u8], known_children: &[u32]) -> Option<SwitchTail> {
    let expected: HashSet<u32> = known_children.iter().copied().collect();
    let count = expected.len();
    if count == 0 {
        return None;
    }
    // `ulDefaultSwitch` sits at `p - 5`, `ulGroupID` at `p - 9`, `eGroupType`
    // at `p - 10`, so the children array cannot begin before offset 10.
    let stride = count.checked_mul(4)?.checked_add(4)?;
    let mut p = 10usize;
    while p.checked_add(stride)? <= body.len() {
        if read_u32(body, p)? as usize == count
            && let Some(tail) = try_switch_tail(body, p, &expected)
        {
            return Some(tail);
        }
        p += 1;
    }
    None
}

fn try_switch_tail(body: &[u8], p: usize, expected: &HashSet<u32>) -> Option<SwitchTail> {
    let count = expected.len();
    let mut children = HashSet::with_capacity(count);
    for index in 0..count {
        children.insert(read_u32(body, p + 4 + index * 4)?);
    }
    if &children != expected {
        return None;
    }
    // Read (and thereby validate the reach to) the authored group header.
    let _group_type = *body.get(p.checked_sub(10)?)?;
    let _group_id = read_u32(body, p.checked_sub(9)?)?;
    let default_switch = read_u32(body, p.checked_sub(5)?)?;

    let mut cursor = p + 4 + count * 4;
    let num_groups = read_u32(body, cursor)? as usize;
    cursor = cursor.checked_add(4)?;
    let mut packages = Vec::with_capacity(num_groups);
    for _ in 0..num_groups {
        let switch_id = read_u32(body, cursor)?;
        cursor = cursor.checked_add(4)?;
        let items = read_u32(body, cursor)? as usize;
        cursor = cursor.checked_add(4)?;
        let end = cursor.checked_add(items.checked_mul(4)?)?;
        if end > body.len() {
            return None;
        }
        let nodes = (0..items)
            .map(|index| read_u32(body, cursor + index * 4))
            .collect::<Option<Vec<u32>>>()?;
        cursor = end;
        packages.push((switch_id, nodes));
    }
    let num_params = read_u32(body, cursor)? as usize;
    cursor = cursor.checked_add(4)?;
    let params_end = cursor.checked_add(num_params.checked_mul(SWITCH_PARAM_LEN)?)?;
    if params_end != body.len() {
        return None;
    }
    Some(SwitchTail {
        default_switch,
        packages,
    })
}

/// A random/sequence container's children in authored playlist order, or `None`
/// when the typed tail cannot be validated (caller keeps parent-inversion order).
///
/// # Evidence (bank version 150, `sounds/wwise/ftsp_alligator_events.bnk`)
///
/// After `NodeBaseParams` and the random/sequence parameters, a `CAkRanSeqCntr`
/// body carries the `AkUInt32`-counted (id-sorted) children array, then an
/// `AkUInt16`-counted playlist of [`PLAYLIST_ITEM_LEN`]-byte items (`AkUInt32`
/// node id + `AkUInt32` weight). The playlist is the authored order (children
/// are stored sorted); in all eight alligator containers it consumes the body
/// exactly.
fn parse_ranseq_order(body: &[u8], known_children: &[u32]) -> Option<Vec<u32>> {
    let expected: HashSet<u32> = known_children.iter().copied().collect();
    let count = expected.len();
    if count == 0 {
        return None;
    }
    let stride = count.checked_mul(4)?.checked_add(4)?;
    let mut p = 0usize;
    while p.checked_add(stride)? <= body.len() {
        if read_u32(body, p)? as usize == count {
            let mut children = HashSet::with_capacity(count);
            for index in 0..count {
                children.insert(read_u32(body, p + 4 + index * 4)?);
            }
            if children == expected
                && let Some(order) =
                    try_ranseq_playlist(body, p + stride, &expected, known_children)
            {
                return Some(order);
            }
        }
        p += 1;
    }
    None
}

fn try_ranseq_playlist(
    body: &[u8],
    cursor: usize,
    expected: &HashSet<u32>,
    known_children: &[u32],
) -> Option<Vec<u32>> {
    let items = read_u16(body, cursor)? as usize;
    let start = cursor.checked_add(2)?;
    let end = start.checked_add(items.checked_mul(PLAYLIST_ITEM_LEN)?)?;
    if end != body.len() {
        return None;
    }
    let mut order = Vec::with_capacity(known_children.len());
    let mut seen = HashSet::new();
    for index in 0..items {
        let id = read_u32(body, start + index * PLAYLIST_ITEM_LEN)?;
        if expected.contains(&id) && seen.insert(id) {
            order.push(id);
        }
    }
    // Any authored child the playlist omits still plays; append it so the default
    // walk never loses a variation.
    for &child in known_children {
        if seen.insert(child) {
            order.push(child);
        }
    }
    Some(order)
}

fn read_u16(body: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(body.get(offset..end)?.try_into().ok()?))
}

fn read_u32(body: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(body.get(offset..end)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WwiseSoundBank;

    /// Build one HIRC object: `type` `u8`, `size` `u32` (= 4 + body), `id` `u32`,
    /// then the typed body.
    fn object(kind: u8, id: u32, body: &[u8]) -> Vec<u8> {
        let mut bytes = vec![kind];
        bytes.extend_from_slice(&((body.len() as u32 + 4).to_le_bytes()));
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(body);
        bytes
    }

    /// Minimal `NodeBaseParams` head: no effects, `OverrideBusId` 0, the given
    /// `DirectParentID`.
    fn node_base(parent: u32) -> Vec<u8> {
        let mut body = vec![0u8, 0, 0, 0]; // override fx, num fx, override meta, num meta
        body.extend_from_slice(&0u32.to_le_bytes()); // OverrideBusId
        body.extend_from_slice(&parent.to_le_bytes()); // DirectParentID
        body
    }

    fn sound(id: u32, source: u32, parent: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&0x0004_0001u32.to_le_bytes()); // plugin (vorbis)
        body.push(0); // StreamType
        body.extend_from_slice(&source.to_le_bytes()); // sourceID
        body.extend_from_slice(&0u32.to_le_bytes()); // uInMemoryMediaSize
        body.push(0); // uSourceBits
        body.extend_from_slice(&node_base(parent));
        object(WwiseHierarchyObjectKind::SOUND.0, id, &body)
    }

    fn action(id: u32, target: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&0x0403u16.to_le_bytes()); // Play
        body.extend_from_slice(&target.to_le_bytes());
        body.push(0); // idExt bus flag (unused by the reader)
        object(WwiseHierarchyObjectKind::ACTION.0, id, &body)
    }

    fn event(id: u32, actions: &[u32]) -> Vec<u8> {
        let mut body = vec![actions.len() as u8]; // packed count (< 0x80)
        for action in actions {
            body.extend_from_slice(&action.to_le_bytes());
        }
        object(WwiseHierarchyObjectKind::EVENT.0, id, &body)
    }

    fn container(kind: u8, id: u32, parent: u32) -> Vec<u8> {
        object(kind, id, &node_base(parent))
    }

    /// A `CAkSwitchCntr` with a full typed tail: group header, the (set-matched)
    /// children array, one package per `(switch_id, nodes)`, and zero switch
    /// params — laid out exactly as bank version 150 stores it.
    fn switch_container(
        id: u32,
        parent: u32,
        group_id: u32,
        default_switch: u32,
        children: &[u32],
        packages: &[(u32, &[u32])],
    ) -> Vec<u8> {
        let mut body = node_base(parent);
        body.push(0); // eGroupType
        body.extend_from_slice(&group_id.to_le_bytes());
        body.extend_from_slice(&default_switch.to_le_bytes());
        body.push(0); // bIsContinuousValidation
        body.extend_from_slice(&(children.len() as u32).to_le_bytes());
        for child in children {
            body.extend_from_slice(&child.to_le_bytes());
        }
        body.extend_from_slice(&(packages.len() as u32).to_le_bytes());
        for (switch_id, nodes) in packages {
            body.extend_from_slice(&switch_id.to_le_bytes());
            body.extend_from_slice(&(nodes.len() as u32).to_le_bytes());
            for node in *nodes {
                body.extend_from_slice(&node.to_le_bytes());
            }
        }
        body.extend_from_slice(&0u32.to_le_bytes()); // ulNumSwitchParams
        object(WwiseHierarchyObjectKind::SWITCH_CONTAINER.0, id, &body)
    }

    /// A `CAkRanSeqCntr` with the id-sorted children array and an authored
    /// playlist (all weights equal) in the given order.
    fn ranseq_container(id: u32, parent: u32, playlist: &[u32]) -> Vec<u8> {
        let mut sorted: Vec<u32> = playlist.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        let mut body = node_base(parent);
        body.extend_from_slice(&(sorted.len() as u32).to_le_bytes());
        for child in &sorted {
            body.extend_from_slice(&child.to_le_bytes());
        }
        body.extend_from_slice(&(playlist.len() as u16).to_le_bytes());
        for node in playlist {
            body.extend_from_slice(&node.to_le_bytes());
            body.extend_from_slice(&50_000u32.to_le_bytes()); // weight
        }
        object(
            WwiseHierarchyObjectKind::RANDOM_SEQUENCE_CONTAINER.0,
            id,
            &body,
        )
    }

    /// A `CAkRanSeqCntr` with the full 24-byte `RanSeqCntrInitialValues` block
    /// (so the weighted parser accepts it), the id-sorted children array, and a
    /// weighted playlist. `e_mode`: 0 Random / 1 Sequence; `e_random`: 0 Normal /
    /// 1 Shuffle; `flags` bit0 sets `bIsUsingWeight`.
    fn ranseq_container_full(
        id: u32,
        parent: u32,
        e_mode: u8,
        e_random: u8,
        avoid: u16,
        using_weight: bool,
        playlist: &[(u32, u32)],
    ) -> Vec<u8> {
        let mut sorted: Vec<u32> = playlist.iter().map(|(node, _)| *node).collect();
        sorted.sort_unstable();
        sorted.dedup();
        let mut body = node_base(parent);
        // 24-byte params block.
        body.extend_from_slice(&1u16.to_le_bytes()); // sLoopCount
        body.extend_from_slice(&0u16.to_le_bytes()); // sLoopModMin
        body.extend_from_slice(&0u16.to_le_bytes()); // sLoopModMax
        body.extend_from_slice(&1000.0f32.to_le_bytes()); // fTransitionTime
        body.extend_from_slice(&0.0f32.to_le_bytes()); // fTransitionTimeModMin
        body.extend_from_slice(&0.0f32.to_le_bytes()); // fTransitionTimeModMax
        body.extend_from_slice(&avoid.to_le_bytes()); // wAvoidRepeatCount
        body.push(0); // eTransitionMode
        body.push(e_random); // eRandomMode
        body.push(e_mode); // eMode
        body.push(if using_weight { 0x01 } else { 0x00 }); // flags
        body.extend_from_slice(&(sorted.len() as u32).to_le_bytes());
        for child in &sorted {
            body.extend_from_slice(&child.to_le_bytes());
        }
        body.extend_from_slice(&(playlist.len() as u16).to_le_bytes());
        for (node, weight) in playlist {
            body.extend_from_slice(&node.to_le_bytes());
            body.extend_from_slice(&weight.to_le_bytes());
        }
        object(
            WwiseHierarchyObjectKind::RANDOM_SEQUENCE_CONTAINER.0,
            id,
            &body,
        )
    }

    fn bank_with_hirc(objects: &[Vec<u8>]) -> Vec<u8> {
        let mut hirc = (objects.len() as u32).to_le_bytes().to_vec();
        for object in objects {
            hirc.extend_from_slice(object);
        }
        let mut bytes = Vec::new();
        // BKHD (version 150, bank id 1).
        bytes.extend_from_slice(b"BKHD");
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&150u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(b"HIRC");
        bytes.extend_from_slice(&(hirc.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&hirc);
        bytes
    }

    #[test]
    fn event_media_walks_actions_containers_and_ignores_unreachable() {
        // Event 100 plays a random-sequence container (300 → sounds 400, 401)
        // and a switch container (500 → sound 600). Sound 700 hangs off an
        // unreferenced container (800) and must never be collected.
        let objects = vec![
            event(100, &[200, 201]),
            action(200, 300),
            action(201, 500),
            container(
                WwiseHierarchyObjectKind::RANDOM_SEQUENCE_CONTAINER.0,
                300,
                0,
            ),
            sound(400, 4001, 300),
            sound(401, 4002, 300),
            container(WwiseHierarchyObjectKind::SWITCH_CONTAINER.0, 500, 0),
            sound(600, 4003, 500),
            container(WwiseHierarchyObjectKind::LAYER_CONTAINER.0, 800, 0),
            sound(700, 9999, 800),
        ];
        let bytes = bank_with_hirc(&objects);
        let bank = WwiseSoundBank::parse(&bytes).unwrap();

        assert!(bank.defines_event(WwiseObjectId(100)));
        let media = bank.event_media(&bytes, WwiseObjectId(100));
        let ids: Vec<u32> = media.iter().map(|media| media.0).collect();
        assert_eq!(ids, vec![4001, 4002, 4003]);
        assert!(!ids.contains(&9999));
    }

    #[test]
    fn stop_actions_do_not_reach_media() {
        // Action 200 is a Stop (0x02xx), not a Play — its target's media must
        // not be collected.
        let mut stop = Vec::new();
        stop.extend_from_slice(&0x0203u16.to_le_bytes());
        stop.extend_from_slice(&300u32.to_le_bytes());
        stop.push(0);
        let objects = vec![
            event(100, &[200]),
            object(WwiseHierarchyObjectKind::ACTION.0, 200, &stop),
            container(
                WwiseHierarchyObjectKind::RANDOM_SEQUENCE_CONTAINER.0,
                300,
                0,
            ),
            sound(400, 4001, 300),
        ];
        let bytes = bank_with_hirc(&objects);
        let bank = WwiseSoundBank::parse(&bytes).unwrap();
        assert!(bank.event_media(&bytes, WwiseObjectId(100)).is_empty());
    }

    #[test]
    fn event_default_media_descends_only_default_switch_in_playlist_order() {
        // Event 100 plays a switch (300) whose `ulDefaultSwitch` is 7. Package 7
        // is the default branch → random container 400 (variations 402, 401 in
        // authored order); package 9 → random container 500 (variation 501).
        // Both containers are direct children of the switch, so the full walk
        // reaches every variation; the default walk reaches only branch 400's, in
        // playlist order.
        let objects = vec![
            event(100, &[200]),
            action(200, 300),
            switch_container(300, 0, 1000, 7, &[400, 500], &[(7, &[400]), (9, &[500])]),
            ranseq_container(400, 300, &[402, 401]),
            sound(401, 4001, 400),
            sound(402, 4002, 400),
            ranseq_container(500, 300, &[501]),
            sound(501, 5001, 500),
        ];
        let bytes = bank_with_hirc(&objects);
        let bank = WwiseSoundBank::parse(&bytes).unwrap();

        // Full reachability: every variation of both branches (sorted set).
        let full: Vec<u32> = bank
            .event_media(&bytes, WwiseObjectId(100))
            .iter()
            .map(|media| media.0)
            .collect();
        assert_eq!(full, vec![4001, 4002, 5001]);

        // Default branch only, in authored playlist order (402 → 4002, 401 →
        // 4001), and never the non-default branch's 5001.
        let default: Vec<u32> = bank
            .event_default_media(&bytes, WwiseObjectId(100))
            .into_iter()
            .map(|media| media.0)
            .collect();
        assert_eq!(default, vec![4002, 4001]);
    }

    #[test]
    fn event_default_media_degrades_to_full_children_when_tail_unparsable() {
        // A switch container with no typed tail (just NodeBaseParams) cannot be
        // validated, so the default walk falls back to every child — matching the
        // full reachability set rather than going silent.
        let objects = vec![
            event(100, &[200]),
            action(200, 300),
            container(WwiseHierarchyObjectKind::SWITCH_CONTAINER.0, 300, 0),
            ranseq_container(400, 300, &[401]),
            sound(401, 4001, 400),
            ranseq_container(500, 300, &[501]),
            sound(501, 5001, 500),
        ];
        let bytes = bank_with_hirc(&objects);
        let bank = WwiseSoundBank::parse(&bytes).unwrap();

        let mut default: Vec<u32> = bank
            .event_default_media(&bytes, WwiseObjectId(100))
            .into_iter()
            .map(|media| media.0)
            .collect();
        default.sort_unstable();
        assert_eq!(default, vec![4001, 5001]);
    }

    #[test]
    fn event_switch_branches_enumerates_all_branches_with_media() {
        // Four switch packages over two random containers, mirroring the alligator
        // creature footstep (switch id 7 is the default). Every branch is reported,
        // not only the default, each with the media it plays.
        let objects = vec![
            event(100, &[200]),
            action(200, 300),
            switch_container(
                300,
                0,
                1000,
                7,
                &[400, 500],
                &[(6, &[400]), (7, &[500]), (8, &[400]), (9, &[500])],
            ),
            ranseq_container(400, 300, &[401]),
            sound(401, 4001, 400),
            ranseq_container(500, 300, &[501]),
            sound(501, 5001, 500),
        ];
        let bytes = bank_with_hirc(&objects);
        let bank = WwiseSoundBank::parse(&bytes).unwrap();

        let branches = bank.event_switch_branches(&bytes, WwiseObjectId(100));
        let ids: Vec<(u32, bool, Vec<u32>)> = branches
            .iter()
            .map(|branch| {
                (
                    branch.switch_id,
                    branch.is_default,
                    branch.media.iter().map(|media| media.0).collect(),
                )
            })
            .collect();
        assert_eq!(
            ids,
            vec![
                (6, false, vec![4001]),
                (7, true, vec![5001]),
                (8, false, vec![4001]),
                (9, false, vec![5001]),
            ]
        );

        // An event with no switch container yields no branches.
        let objects = vec![
            event(110, &[210]),
            action(210, 310),
            ranseq_container(310, 0, &[311]),
            sound(311, 6001, 310),
        ];
        let bytes = bank_with_hirc(&objects);
        let bank = WwiseSoundBank::parse(&bytes).unwrap();
        assert!(
            bank.event_switch_branches(&bytes, WwiseObjectId(110))
                .is_empty()
        );
    }

    #[test]
    fn event_weighted_sequence_follows_selected_surface_branch() {
        // A switch (default 7) over two full random containers. In Sequence mode
        // the stream is the playlist order, looping — fully deterministic — and it
        // follows whichever surface switch id the caller selects.
        let objects = vec![
            event(100, &[200]),
            action(200, 300),
            switch_container(300, 0, 1000, 7, &[400, 500], &[(7, &[400]), (9, &[500])]),
            // Sequence mode (e_mode=1) → playlist order [402, 401] → media [4002, 4001].
            ranseq_container_full(400, 300, 1, 0, 0, false, &[(402, 1), (401, 1)]),
            sound(401, 4001, 400),
            sound(402, 4002, 400),
            ranseq_container_full(500, 300, 1, 0, 0, false, &[(501, 1)]),
            sound(501, 5001, 500),
        ];
        let bytes = bank_with_hirc(&objects);
        let bank = WwiseSoundBank::parse(&bytes).unwrap();

        // Default surface (switch id 7): [4002, 4001] cycled to length 5.
        let seq: Vec<u32> = bank
            .event_weighted_sequence(&bytes, WwiseObjectId(100), 7, 5)
            .into_iter()
            .map(|media| media.0)
            .collect();
        assert_eq!(seq, vec![4002, 4001, 4002, 4001, 4002]);

        // A different surface (switch id 9) follows its own branch.
        let seq: Vec<u32> = bank
            .event_weighted_sequence(&bytes, WwiseObjectId(100), 9, 3)
            .into_iter()
            .map(|media| media.0)
            .collect();
        assert_eq!(seq, vec![5001, 5001, 5001]);
    }

    #[test]
    fn event_weighted_sequence_is_deterministic_and_weight_aware() {
        // Random-Normal with a zero-weight variation: only the weighted media can
        // ever appear, and the same event/surface/count reproduces exactly.
        let objects = vec![
            event(100, &[200]),
            action(200, 300),
            ranseq_container_full(300, 0, 0, 0, 0, true, &[(301, 100), (302, 0)]),
            sound(301, 3001, 300),
            sound(302, 3002, 300),
        ];
        let bytes = bank_with_hirc(&objects);
        let bank = WwiseSoundBank::parse(&bytes).unwrap();

        let first = bank.event_weighted_sequence(&bytes, WwiseObjectId(100), 0, 6);
        let second = bank.event_weighted_sequence(&bytes, WwiseObjectId(100), 0, 6);
        assert_eq!(first, second, "same inputs → same sequence");
        let ids: Vec<u32> = first.iter().map(|media| media.0).collect();
        assert_eq!(ids, vec![3001, 3001, 3001, 3001, 3001, 3001]);
    }

    #[test]
    fn event_weighted_sequence_cycles_media_without_a_parseable_random_container() {
        // The random container here is order-only (no 24-byte params block), so the
        // weighted parser rejects it and the walk cycles the branch's ordered media.
        let objects = vec![
            event(100, &[200]),
            action(200, 300),
            ranseq_container(300, 0, &[402, 401]),
            sound(401, 4001, 300),
            sound(402, 4002, 300),
        ];
        let bytes = bank_with_hirc(&objects);
        let bank = WwiseSoundBank::parse(&bytes).unwrap();

        let seq: Vec<u32> = bank
            .event_weighted_sequence(&bytes, WwiseObjectId(100), 0, 3)
            .into_iter()
            .map(|media| media.0)
            .collect();
        // Ordered default media [4002, 4001] (playlist order) cycled to length 3.
        assert_eq!(seq, vec![4002, 4001, 4002]);
    }
}
