//! Dictionary encoder: unique strings → integer ids + dual CRC32.

use std::collections::HashMap;

use nw_datasheet::game_system::Crc32;

use crate::cache::{Cache, CacheFutureExt, InsertIdxToken, Schema, SelectIdxToken};

const TOKEN_CHUNK: usize = 512;

struct PendingToken {
    id: i64,
    text: String,
    crc: i64,
    crc_lower: i64,
}

pub struct Interner {
    ids: HashMap<String, i64>,
    next_id: i64,
    pending: Vec<PendingToken>,
}

impl Interner {
    pub fn load(cache: &Cache) -> anyhow::Result<Self> {
        let Schema { idx_token, .. } = Schema::new();
        let rows: Vec<SelectIdxToken> = cache.db().select(()).from(idx_token).all().wait()?;
        let mut ids = HashMap::with_capacity(rows.len());
        let mut next_id = 1i64;
        for row in rows {
            next_id = next_id.max(row.id + 1);
            ids.insert(row.text, row.id);
        }
        Ok(Self {
            ids,
            next_id,
            pending: Vec::new(),
        })
    }

    pub fn intern(&mut self, text: &str) -> i64 {
        if let Some(&id) = self.ids.get(text) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        let crc = i64::from(Crc32::from_str(text).value());
        let crc_lower = i64::from(Crc32::from_str_lower(text).value());
        self.pending.push(PendingToken {
            id,
            text: text.to_owned(),
            crc,
            crc_lower,
        });
        self.ids.insert(text.to_owned(), id);
        id
    }

    pub fn flush(&mut self, cache: &mut Cache) -> anyhow::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let Schema { idx_token, .. } = Schema::new();
        let pending = std::mem::take(&mut self.pending);
        cache
            .db_mut()
            .transaction(
                drizzle::sqlite::connection::SQLiteTransactionType::Deferred,
                async |tx| {
                    for chunk in pending.chunks(TOKEN_CHUNK) {
                        let rows: Vec<_> = chunk
                            .iter()
                            .map(|token| {
                                InsertIdxToken::new(token.text.clone(), token.crc, token.crc_lower)
                                    .with_id(token.id)
                            })
                            .collect();
                        tx.insert(idx_token).values(rows).execute().await?;
                    }
                    Ok(())
                },
            )
            .wait()?;
        Ok(())
    }
}
