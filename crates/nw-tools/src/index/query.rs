//! CRC lookups: token index first, then facts. No raw SQL.

use drizzle::core::expr::{eq, in_array};
use drizzle::sqlite::prelude::*;

use crate::cache::{Cache, CacheFutureExt, IdxToken, Schema, SelectIdxToken};
use crate::grep::adapter::FactKind;

use super::{CrcCase, EntryFormat, IndexedHit, Lookup, LookupTarget};

tag!(ValTok, "vt");
tag!(FldTok, "ft");
tag!(PakTok, "pt");
tag!(NamTok, "nt");

type HitRow = (i64, i64, String, String, String, String, i64, i64);

const QUERY_CHUNK: usize = 4_000;

pub fn lookup(cache: &Cache, lookup: Lookup) -> anyhow::Result<Vec<IndexedHit>> {
    let Schema { idx_token, .. } = Schema::new();
    let crc = i64::from(lookup.crc.value());
    let tokens: Vec<(i64,)> = match lookup.case {
        CrcCase::Original => cache
            .db()
            .select((idx_token.id,))
            .from(idx_token)
            .r#where(eq(idx_token.crc, crc))
            .all()
            .wait()?,
        CrcCase::Lower => cache
            .db()
            .select((idx_token.id,))
            .from(idx_token)
            .r#where(eq(idx_token.crc_lower, crc))
            .all()
            .wait()?,
    };
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<i64> = tokens.into_iter().map(|(id,)| id).collect();
    let name_only = matches!(lookup.target, LookupTarget::Name);
    let role = match lookup.target {
        LookupTarget::Field => TokenRole::Field,
        LookupTarget::Value | LookupTarget::Name => TokenRole::Value,
    };
    Ok(hits_for_ids(cache, &ids, role, name_only)?
        .into_iter()
        .map(into_hit)
        .collect())
}

/// Score interned token text (fff or substring), then join matching facts.
pub fn search(cache: &Cache, queries: &[String], exact: bool) -> anyhow::Result<Vec<IndexedHit>> {
    let Schema { idx_token, .. } = Schema::new();
    let tokens: Vec<SelectIdxToken> = cache.db().select(()).from(idx_token).all().wait()?;
    let lowered: Vec<String> = queries
        .iter()
        .map(|query| query.to_ascii_lowercase())
        .collect();
    let matched = matching_token_ids(&tokens, &lowered, exact);
    if matched.is_empty() {
        return Ok(Vec::new());
    }
    let mut hits = hits_for_ids(cache, &matched, TokenRole::Value, false)?;
    hits.extend(hits_for_ids(cache, &matched, TokenRole::Field, false)?);
    hits.sort_by_key(|row| row.7);
    hits.dedup_by_key(|row| row.7);
    Ok(hits.into_iter().map(into_hit).collect())
}

fn matching_token_ids(tokens: &[SelectIdxToken], lowered: &[String], exact: bool) -> Vec<i64> {
    if exact {
        return tokens
            .iter()
            .filter(|token| {
                let hay = token.text.to_ascii_lowercase();
                lowered.iter().any(|query| hay.contains(query.as_str()))
            })
            .map(|token| token.id)
            .collect();
    }
    let haystacks: Vec<String> = tokens
        .iter()
        .map(|token| token.text.to_ascii_lowercase())
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut ids = Vec::new();
    for query in lowered {
        for (index, _) in crate::fuzzy::rank(query, &haystacks) {
            let id = tokens[index].id;
            if seen.insert(id) {
                ids.push(id);
            }
        }
    }
    ids
}

#[derive(Clone, Copy)]
enum TokenRole {
    Value,
    Field,
}

fn hits_for_ids(
    cache: &Cache,
    ids: &[i64],
    role: TokenRole,
    name_only: bool,
) -> anyhow::Result<Vec<HitRow>> {
    let mut rows = Vec::new();
    for chunk in ids.chunks(QUERY_CHUNK) {
        rows.extend(hits_for_chunk(cache, chunk, role, name_only)?);
    }
    Ok(rows)
}

fn hits_for_chunk(
    cache: &Cache,
    ids: &[i64],
    role: TokenRole,
    name_only: bool,
) -> anyhow::Result<Vec<HitRow>> {
    let Schema {
        idx_entry,
        idx_fact,
        ..
    } = Schema::new();
    let vt = IdxToken::alias::<ValTok>();
    let ft = IdxToken::alias::<FldTok>();
    let pt = IdxToken::alias::<PakTok>();
    let nt = IdxToken::alias::<NamTok>();
    let id_filter = match role {
        TokenRole::Value => in_array(idx_fact.value_id, ids.iter().copied()),
        TokenRole::Field => in_array(idx_fact.field_id, ids.iter().copied()),
    };
    let name_filter = name_only.then_some(eq(idx_fact.kind, FactKind::FileName as i64));
    cache
        .db()
        .select((
            idx_fact.kind,
            idx_fact.loc_a,
            vt.text,
            ft.text,
            pt.text,
            nt.text,
            idx_entry.format,
            idx_fact.id,
        ))
        .from(idx_fact)
        .inner_join((idx_entry, eq(idx_fact.entry_id, idx_entry.id)))
        .inner_join((vt, eq(idx_fact.value_id, vt.id)))
        .inner_join((ft, eq(idx_fact.field_id, ft.id)))
        .inner_join((pt, eq(idx_entry.pak_id, pt.id)))
        .inner_join((nt, eq(idx_entry.name_id, nt.id)))
        .r#where((id_filter, name_filter))
        .all()
        .wait()
        .map_err(Into::into)
}

fn into_hit((kind, loc_a, value, field, pak, entry, format, _): HitRow) -> IndexedHit {
    let kind = FactKind::from_i64(kind);
    IndexedHit {
        pak,
        entry,
        format: EntryFormat::from_i64(format),
        location: location(kind, loc_a, &field, &value),
        field,
        value,
        kind,
    }
}

pub fn location(kind: FactKind, loc_a: i64, field: &str, value: &str) -> String {
    match kind {
        FactKind::FileName => "name".to_owned(),
        FactKind::Cell => format!("row {loc_a} · column {field}"),
        FactKind::Fragment => format!("fragment {value}"),
        FactKind::Tag => format!("fragment {field} tags"),
        FactKind::Animation => format!("animation {value}"),
        FactKind::ObjectValue => {
            if field.is_empty() {
                format!("value {value}")
            } else {
                format!("field {field}")
            }
        }
        FactKind::ObjectField => format!("field {value}"),
        FactKind::Catalog => format!("entry {loc_a} · {field}"),
        FactKind::LuaLine => format!("line {loc_a}"),
        FactKind::AudioName => {
            if field.is_empty() {
                value.to_owned()
            } else {
                format!("{field} {value}")
            }
        }
        FactKind::AnimEvent => format!("event {value}"),
        FactKind::JsonString => {
            if field.is_empty() {
                format!("json {value}")
            } else {
                format!("field {field}")
            }
        }
        FactKind::TextLine => format!("line {loc_a}"),
        FactKind::Xml => {
            if field.is_empty() {
                value.to_owned()
            } else {
                field.to_owned()
            }
        }
    }
}
