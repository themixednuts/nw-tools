//! `CAkRanSeqCntr` playlist parameters and the engine's deterministic
//! variation-selection algorithm.
//!
//! The reachability walk in [`super::hirc`] answers *which* media an event can
//! reach. This module answers the finer question the engine actually decides at
//! runtime: given a random/sequence container, in what order do its variations
//! play? It parses the full `RanSeqCntrInitialValues` block (mode, random mode,
//! avoid-repeat count, and the weighted playlist) and reproduces the selection
//! with a **seeded, deterministic** RNG so an export is byte-reproducible. No
//! system randomness or time is ever consulted — `cry-audio` stays pure.
//!
//! # Evidence (bank version 150, `sounds/wwise/ftsp_alligator_events.bnk`)
//!
//! After `NodeBaseParams`, a `CAkRanSeqCntr` body carries a fixed 24-byte
//! `RanSeqCntrInitialValues` block immediately before its `AkUInt32`-counted
//! (id-sorted) children array:
//!
//! ```text
//! sLoopCount              u16   @ children_anchor - 24
//! sLoopModMin             u16   @ -22
//! sLoopModMax             u16   @ -20
//! fTransitionTime         f32   @ -18
//! fTransitionTimeModMin   f32   @ -14
//! fTransitionTimeModMax   f32   @ -10
//! wAvoidRepeatCount       u16   @ -6
//! eTransitionMode         u8    @ -4
//! eRandomMode             u8    @ -3   (0 = Normal, 1 = Shuffle)
//! eMode                   u8    @ -2   (0 = Random, 1 = Sequence)
//! flags                   u8    @ -1   (bit0 bIsUsingWeight, bit1 bReset…,
//!                                       bit2 bRestartBackward, bit3 bContinuous,
//!                                       bit4 bIsGlobal)
//! ```
//!
//! then `u32 ulNumChilds; u32[…] childIDs` (id-sorted), then the playlist:
//! `u16 numPlaylistItem; { AkUniqueID ulPlayID (u32); AkUInt32 weight }[…]`.
//! Confirmed byte-for-byte across all nine alligator containers, e.g. id
//! `216394492` (`eMode`=0 Random, `eRandomMode`=1 Shuffle, `wAvoidRepeatCount`=5,
//! 19 items each weight 50000) and id `93576288` (`eRandomMode`=0 Normal,
//! `wAvoidRepeatCount`=32, 31 items). Every shipped container has
//! `bIsUsingWeight`=0, so the authored weights are all the Wwise default (50000)
//! and selection is uniform; the parser still honours explicit weights.

use super::{WwiseMediaId, WwiseObjectId};

/// `eMode`: how the playlist is traversed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WwiseRandomSequenceMode {
    /// `eMode == 0`: pick items at random (see [`WwiseRandomMode`]).
    Random,
    /// `eMode == 1`: play the playlist in authored order, looping.
    Sequence,
}

/// `eRandomMode`: the flavour of random selection (only meaningful when
/// [`WwiseRandomSequenceMode::Random`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WwiseRandomMode {
    /// `eRandomMode == 0`: each pick is an independent weighted draw, excluding
    /// the last `wAvoidRepeatCount` distinct picks.
    Normal,
    /// `eRandomMode == 1`: a weighted shuffle-bag — every item plays once per
    /// cycle before any repeats, with `wAvoidRepeatCount` carried across the
    /// cycle boundary so the join never repeats the last N.
    Shuffle,
}

/// A parsed `CAkRanSeqCntr` playlist: the engine's selection parameters plus its
/// weighted items (`(child node id, weight)`), in authored playlist order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WwiseRandomContainer {
    pub mode: WwiseRandomSequenceMode,
    pub random_mode: WwiseRandomMode,
    pub avoid_repeat_count: u16,
    /// `(ulPlayID, weight)` in authored playlist order. When the container's
    /// `bIsUsingWeight` flag is clear the weights are normalised to 1 (uniform),
    /// matching the engine, which ignores authored weights in that mode.
    pub items: Vec<(WwiseObjectId, u32)>,
}

/// Length of the fixed `RanSeqCntrInitialValues` block that precedes the
/// children array (see the module evidence).
const RANSEQ_PARAMS_LEN: usize = 24;

/// Playlist item: `AkUniqueID ulPlayID` (u32) + `AkUInt32 weight`.
const PLAYLIST_ITEM_LEN: usize = 8;

impl WwiseRandomContainer {
    /// Parse a `CAkRanSeqCntr` body given the child node ids already recovered by
    /// parent inversion. Returns `None` unless the full typed tail — the 24-byte
    /// params block, the (set-matched) children array, and the weighted playlist
    /// — validates by consuming the body exactly. Pure: bytes in, values out.
    #[must_use]
    pub fn parse(body: &[u8], known_children: &[u32]) -> Option<Self> {
        let mut expected = known_children.to_vec();
        expected.sort_unstable();
        expected.dedup();
        let count = expected.len();
        if count == 0 {
            return None;
        }
        let stride = count.checked_mul(4)?.checked_add(4)?;
        // The children-count anchor cannot begin before the 24-byte params block.
        let mut p = RANSEQ_PARAMS_LEN;
        while p.checked_add(stride)? <= body.len() {
            if read_u32(body, p)? as usize == count {
                let mut children: Vec<u32> = (0..count)
                    .map(|index| read_u32(body, p + 4 + index * 4))
                    .collect::<Option<Vec<u32>>>()?;
                children.sort_unstable();
                if children == expected
                    && let Some(container) = Self::try_tail(body, p, count)
                {
                    return Some(container);
                }
            }
            p += 1;
        }
        None
    }

    fn try_tail(body: &[u8], p: usize, count: usize) -> Option<Self> {
        // Params block sits immediately before the children count.
        let avoid_repeat_count = read_u16(body, p.checked_sub(6)?)?;
        let e_random_mode = *body.get(p.checked_sub(3)?)?;
        let e_mode = *body.get(p.checked_sub(2)?)?;
        let flags = *body.get(p.checked_sub(1)?)?;
        let using_weight = flags & 0x01 != 0;

        // Playlist follows the children array; it must consume the body exactly.
        let cursor = p + 4 + count * 4;
        let items_count = read_u16(body, cursor)? as usize;
        let start = cursor.checked_add(2)?;
        let end = start.checked_add(items_count.checked_mul(PLAYLIST_ITEM_LEN)?)?;
        if end != body.len() {
            return None;
        }
        let mut items = Vec::with_capacity(items_count);
        for index in 0..items_count {
            let base = start + index * PLAYLIST_ITEM_LEN;
            let id = read_u32(body, base)?;
            let raw_weight = read_u32(body, base + 4)?;
            let weight = if using_weight { raw_weight } else { 1 };
            items.push((WwiseObjectId(id), weight));
        }
        Some(Self {
            mode: if e_mode == 1 {
                WwiseRandomSequenceMode::Sequence
            } else {
                WwiseRandomSequenceMode::Random
            },
            random_mode: if e_random_mode == 1 {
                WwiseRandomMode::Shuffle
            } else {
                WwiseRandomMode::Normal
            },
            avoid_repeat_count,
            items,
        })
    }

    /// Produce `count` node-id selections reproducing the engine's algorithm from
    /// a deterministic `seed` (see [`weighted_sequence_seed`]). Empty when the
    /// container has no items.
    ///
    /// * Sequence mode: the playlist in order, looping — the `seed` is ignored.
    /// * Random-Normal: independent weighted draws, excluding the last
    ///   `avoid_repeat_count` distinct picks.
    /// * Random-Shuffle: a weighted shuffle-bag; each cycle plays every item once
    ///   before repeats, and `avoid_repeat_count` items carry across the cycle
    ///   boundary so the join never repeats the last N.
    #[must_use]
    pub fn select_sequence(&self, seed: u64, count: usize) -> Vec<WwiseObjectId> {
        let n = self.items.len();
        if n == 0 || count == 0 {
            return Vec::new();
        }
        if self.mode == WwiseRandomSequenceMode::Sequence {
            return (0..count).map(|k| self.items[k % n].0).collect();
        }
        let shuffle = self.random_mode == WwiseRandomMode::Shuffle;
        let cap = (self.avoid_repeat_count as usize).min(n.saturating_sub(1));
        let mut rng = SplitMix64::new(seed);
        let mut recent: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        let mut bag: Vec<usize> = Vec::new();
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let candidates: Vec<usize> = if shuffle {
                if bag.is_empty() {
                    bag = (0..n).filter(|index| !recent.contains(index)).collect();
                    if bag.is_empty() {
                        bag = (0..n).collect();
                    }
                }
                bag.clone()
            } else {
                let live: Vec<usize> = (0..n).filter(|index| !recent.contains(index)).collect();
                if live.is_empty() {
                    (0..n).collect()
                } else {
                    live
                }
            };
            let chosen = self.weighted_pick(&candidates, &mut rng);
            out.push(self.items[chosen].0);
            if shuffle {
                bag.retain(|&index| index != chosen);
            }
            recent.push_back(chosen);
            while recent.len() > cap {
                recent.pop_front();
            }
        }
        out
    }

    /// Weighted choice of one index from `candidates` (each an index into
    /// `items`). Falls back to a uniform draw when every candidate weight is 0.
    fn weighted_pick(&self, candidates: &[usize], rng: &mut SplitMix64) -> usize {
        debug_assert!(!candidates.is_empty());
        let total: u64 = candidates
            .iter()
            .map(|&index| u64::from(self.items[index].1))
            .sum();
        if total == 0 {
            return candidates[(rng.below(candidates.len() as u64)) as usize];
        }
        let mut ticket = rng.below(total);
        for &index in candidates {
            let weight = u64::from(self.items[index].1);
            if ticket < weight {
                return index;
            }
            ticket -= weight;
        }
        *candidates.last().expect("non-empty candidates")
    }

    /// Resolve `count` selections to media ids via `node_media` (a node id → its
    /// first reachable media id). Nodes that resolve to no media are skipped, so
    /// the result length may be below `count`.
    #[must_use]
    pub fn select_media(
        &self,
        seed: u64,
        count: usize,
        node_media: impl Fn(u32) -> Option<WwiseMediaId>,
    ) -> Vec<WwiseMediaId> {
        self.select_sequence(seed, count)
            .into_iter()
            .filter_map(|node| node_media(node.0))
            .collect()
    }
}

/// Derive a deterministic 64-bit seed for [`WwiseRandomContainer::select_sequence`]
/// from the event id, the selected surface switch id, and the container id. Two
/// exports with the same inputs produce the same sequence; different surfaces or
/// containers diverge. Folded through [`SplitMix64`] so nearby ids do not yield
/// nearby streams.
#[must_use]
pub fn weighted_sequence_seed(event: u32, switch_id: u32, container: u32) -> u64 {
    let mut mix = SplitMix64::new(u64::from(event));
    mix.mix(u64::from(switch_id));
    mix.mix(u64::from(container));
    mix.next_u64()
}

/// A `const`, dependency-free SplitMix64 PRNG. Deterministic and documented so a
/// seeded selection is fully reproducible.
struct SplitMix64(u64);

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Fold another value into the state (used only for seed derivation).
    fn mix(&mut self, value: u64) {
        self.0 ^= self.next_u64() ^ value;
    }

    /// Uniform in `[0, n)`; returns 0 when `n == 0`. The modulo bias is negligible
    /// for the small playlists (≤ 64 items) this selects over.
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }
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

    /// A `CAkRanSeqCntr` body laid out exactly as bank version 150 stores it:
    /// the 24-byte params block, the id-sorted children array, then the weighted
    /// playlist. `flags` bit0 sets `bIsUsingWeight`.
    fn ranseq_body(
        e_mode: u8,
        e_random_mode: u8,
        avoid_repeat: u16,
        using_weight: bool,
        playlist: &[(u32, u32)],
    ) -> Vec<u8> {
        let mut children: Vec<u32> = playlist.iter().map(|(id, _)| *id).collect();
        children.sort_unstable();
        children.dedup();

        let mut body = Vec::new();
        // NodeBaseParams stand-in (any bytes; the parser anchors on the children
        // count, and the params block is read relative to that anchor). Use a
        // 12-byte head like the real node base so the anchor is unambiguous.
        body.extend_from_slice(&[0u8; 12]);
        // 24-byte RanSeqCntrInitialValues block.
        body.extend_from_slice(&1u16.to_le_bytes()); // sLoopCount
        body.extend_from_slice(&0u16.to_le_bytes()); // sLoopModMin
        body.extend_from_slice(&0u16.to_le_bytes()); // sLoopModMax
        body.extend_from_slice(&1000.0f32.to_le_bytes()); // fTransitionTime
        body.extend_from_slice(&0.0f32.to_le_bytes()); // fTransitionTimeModMin
        body.extend_from_slice(&0.0f32.to_le_bytes()); // fTransitionTimeModMax
        body.extend_from_slice(&avoid_repeat.to_le_bytes()); // wAvoidRepeatCount
        body.push(0); // eTransitionMode
        body.push(e_random_mode); // eRandomMode
        body.push(e_mode); // eMode
        body.push(if using_weight { 0x01 } else { 0x00 }); // flags
        // Children array (id-sorted).
        body.extend_from_slice(&(children.len() as u32).to_le_bytes());
        for child in &children {
            body.extend_from_slice(&child.to_le_bytes());
        }
        // Playlist.
        body.extend_from_slice(&(playlist.len() as u16).to_le_bytes());
        for (id, weight) in playlist {
            body.extend_from_slice(&id.to_le_bytes());
            body.extend_from_slice(&weight.to_le_bytes());
        }
        body
    }

    #[test]
    fn parses_full_ranseq_params_and_weighted_playlist() {
        let playlist = [(300u32, 10u32), (100, 20), (200, 70)];
        let children: Vec<u32> = vec![100, 200, 300];
        // eMode=0 Random, eRandomMode=1 Shuffle, avoid=5, using_weight=true.
        let body = ranseq_body(0, 1, 5, true, &playlist);
        let container = WwiseRandomContainer::parse(&body, &children).expect("parses");
        assert_eq!(container.mode, WwiseRandomSequenceMode::Random);
        assert_eq!(container.random_mode, WwiseRandomMode::Shuffle);
        assert_eq!(container.avoid_repeat_count, 5);
        assert_eq!(
            container.items,
            vec![
                (WwiseObjectId(300), 10),
                (WwiseObjectId(100), 20),
                (WwiseObjectId(200), 70),
            ]
        );
    }

    #[test]
    fn clears_weights_when_bisusingweight_is_false() {
        // Real alligator containers store weight 50000 with bIsUsingWeight=0; the
        // parser normalises those to uniform 1 (matching the engine).
        let playlist = [(1u32, 50_000u32), (2, 50_000), (3, 50_000)];
        let body = ranseq_body(0, 0, 0, false, &playlist);
        let container = WwiseRandomContainer::parse(&body, &[1, 2, 3]).unwrap();
        assert!(container.items.iter().all(|(_, weight)| *weight == 1));
    }

    #[test]
    fn sequence_mode_loops_playlist_in_order() {
        let playlist = [(10u32, 1u32), (20, 1), (30, 1)];
        // eMode=1 Sequence.
        let body = ranseq_body(1, 0, 0, false, &playlist);
        let container = WwiseRandomContainer::parse(&body, &[10, 20, 30]).unwrap();
        let ids: Vec<u32> = container
            .select_sequence(12345, 7)
            .into_iter()
            .map(|id| id.0)
            .collect();
        assert_eq!(ids, vec![10, 20, 30, 10, 20, 30, 10]);
    }

    #[test]
    fn normal_mode_never_picks_zero_weight_items() {
        // Only item 10 carries weight; 20 and 30 are weight 0 → unreachable.
        let playlist = [(10u32, 100u32), (20, 0), (30, 0)];
        let body = ranseq_body(0, 0, 0, true, &playlist);
        let container = WwiseRandomContainer::parse(&body, &[10, 20, 30]).unwrap();
        let ids: Vec<u32> = container
            .select_sequence(999, 6)
            .into_iter()
            .map(|id| id.0)
            .collect();
        assert_eq!(ids, vec![10, 10, 10, 10, 10, 10]);
    }

    #[test]
    fn normal_mode_avoid_repeat_forces_alternation() {
        // Two equal-weight items, avoid_repeat=1 → no item repeats consecutively.
        let playlist = [(1u32, 1u32), (2, 1)];
        let body = ranseq_body(0, 0, 1, true, &playlist);
        let container = WwiseRandomContainer::parse(&body, &[1, 2]).unwrap();
        let ids: Vec<u32> = container
            .select_sequence(7, 6)
            .into_iter()
            .map(|id| id.0)
            .collect();
        for window in ids.windows(2) {
            assert_ne!(
                window[0], window[1],
                "avoid_repeat=1 must alternate: {ids:?}"
            );
        }
        // With two items the alternation is fully determined by the first pick.
        assert_eq!(ids[0], ids[2]);
        assert_eq!(ids[1], ids[3]);
    }

    #[test]
    fn shuffle_mode_plays_every_item_once_per_cycle() {
        let playlist = [(1u32, 1u32), (2, 1), (3, 1), (4, 1)];
        let body = ranseq_body(0, 1, 0, true, &playlist);
        let container = WwiseRandomContainer::parse(&body, &[1, 2, 3, 4]).unwrap();
        let ids: Vec<u32> = container
            .select_sequence(42, 8)
            .into_iter()
            .map(|id| id.0)
            .collect();
        let mut first: Vec<u32> = ids[0..4].to_vec();
        first.sort_unstable();
        assert_eq!(first, vec![1, 2, 3, 4], "cycle 1 is a full permutation");
        let mut second: Vec<u32> = ids[4..8].to_vec();
        second.sort_unstable();
        assert_eq!(second, vec![1, 2, 3, 4], "cycle 2 is a full permutation");
    }

    #[test]
    fn selection_is_deterministic_for_a_seed() {
        let playlist = [(1u32, 3u32), (2, 5), (3, 2), (4, 9)];
        let body = ranseq_body(0, 1, 2, true, &playlist);
        let container = WwiseRandomContainer::parse(&body, &[1, 2, 3, 4]).unwrap();
        let seed = weighted_sequence_seed(0xABCD, 7, 61188014);
        let first = container.select_sequence(seed, 20);
        let second = container.select_sequence(seed, 20);
        assert_eq!(first, second, "same seed → same sequence");
        // A different surface switch id yields a different stream.
        let other = weighted_sequence_seed(0xABCD, 8, 61188014);
        assert_ne!(container.select_sequence(other, 20), first);
    }
}
