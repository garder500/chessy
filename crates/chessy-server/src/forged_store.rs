//! Forged skills in the database (docs/spec-forge.md): storing a definition
//! the forge made up, loading them all into the engine's registry at start-up
//! and describing them to clients.

use std::collections::HashSet;

use chessy_engine::forge::bricks::Bricks;
use chessy_engine::forge::identity::{identity, Family, IconSpec, SoundSpec};
use chessy_engine::forge::{registry, Graded, Rarity, SkillDef};
use chessy_engine::SkillId;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::store::{Store, StoreError, StoreResult};

/// The generation of the forge that made a skill (bumped when its rules change).
pub const GEN_VERSION: u8 = 1;

/// Everything a client needs to show a forged skill.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SkillDefView {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub family: Family,
    pub rarity: Rarity,
    /// Legendary: exists in one deck only.
    pub unique: bool,
    /// Another skill was already like it when it was forged.
    pub redundant: bool,
    pub max_uses: u8,
    pub icon: IconSpec,
    pub sound: SoundSpec,
    /// The flat view of the definition, which the client reads to animate it.
    pub bricks: Bricks,
}

struct Row {
    id: u32,
    def: SkillDef,
    rarity: Rarity,
    redundant: bool,
}

fn fingerprint_hex(def: &SkillDef) -> String {
    format!("{:016x}", def.fingerprint())
}

fn view(row: &Row) -> SkillDefView {
    let id = identity(&row.def);
    SkillDefView {
        id: SkillId::Forged(row.id),
        name: id.name,
        description: id.description,
        family: id.family,
        rarity: row.rarity,
        unique: row.def.unique,
        redundant: row.redundant,
        max_uses: row.def.max_uses,
        icon: id.icon,
        sound: id.sound,
        bricks: row.def.bricks(),
    }
}

fn read_row(r: &rusqlite::Row) -> rusqlite::Result<Result<Row, StoreError>> {
    let id: u32 = r.get(0)?;
    let json: String = r.get(1)?;
    let rarity: String = r.get(2)?;
    let redundant: bool = r.get(3)?;
    Ok(
        match (
            serde_json::from_str::<SkillDef>(&json),
            Rarity::parse(&rarity),
        ) {
            (Ok(def), Some(rarity)) => Ok(Row {
                id,
                def,
                rarity,
                redundant,
            }),
            _ => Err(StoreError::Invalid("a stored forged skill is unreadable")),
        },
    )
}

const COLUMNS: &str = "id, def_json, rarity, redundant";

impl Store {
    /// Stores a forged skill and registers it with the engine; returns its id.
    /// A definition that is already stored (the same skill, however it was
    /// reached) keeps the id it has.
    pub fn insert_forged(&self, def: &SkillDef, graded: &Graded) -> StoreResult<u32> {
        let def = def.clone().canonical();
        let json = serde_json::to_string(&def).expect("a definition serializes");
        let fingerprint = fingerprint_hex(&def);
        let id = {
            let conn = self.db();
            let existing = |conn: &rusqlite::Connection| {
                conn.query_row(
                    "SELECT id FROM forged_skill WHERE fingerprint = ?1",
                    params![fingerprint],
                    |r| r.get::<_, u32>(0),
                )
                .optional()
            };
            match existing(&conn)? {
                Some(id) => id,
                None => {
                    // The id is the low half of the fingerprint, so one
                    // definition has one id wherever it is stored; a clash
                    // between two definitions takes the next free id.
                    let mut id = def.fingerprint() as u32;
                    let mut tries = 0;
                    loop {
                        let inserted = conn.execute(
                            "INSERT OR IGNORE INTO forged_skill
                                 (id, fingerprint, signature, def_json, rarity, score, cost,
                                  tone, redundant, gen_version)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                            params![
                                id,
                                fingerprint,
                                def.signature(),
                                json,
                                graded.rarity.as_str(),
                                graded.score,
                                graded.cost,
                                graded.tone,
                                graded.redundant,
                                GEN_VERSION,
                            ],
                        )?;
                        if inserted == 1 {
                            break id;
                        }
                        // Ignored: someone else took the id (or the same
                        // definition was stored a moment ago).
                        if let Some(id) = existing(&conn)? {
                            break id;
                        }
                        tries += 1;
                        if tries > 16 {
                            return Err(StoreError::Invalid("no free forged skill id"));
                        }
                        id = id.wrapping_add(1);
                    }
                }
            }
        };
        // Register what was stored: on a clash it is the stored definition that counts.
        let stored = self
            .forged_rows(&[id])?
            .pop()
            .ok_or(StoreError::Invalid("forged skill vanished"))?;
        registry::register(id, stored.def)
            .map_err(|_| StoreError::Invalid("a stored forged skill is invalid"))?;
        Ok(id)
    }

    /// The signatures of every forged skill, redundant or not: what a new
    /// skill must not duplicate to stay out of Common.
    pub fn forged_signatures(&self) -> StoreResult<HashSet<String>> {
        let conn = self.db();
        let mut stmt = conn.prepare("SELECT signature FROM forged_skill")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn forged_rows(&self, ids: &[u32]) -> StoreResult<Vec<Row>> {
        let conn = self.db();
        let mut out = Vec::new();
        for id in ids {
            let row = conn
                .query_row(
                    &format!("SELECT {COLUMNS} FROM forged_skill WHERE id = ?1"),
                    params![id],
                    read_row,
                )
                .optional()?;
            if let Some(row) = row {
                out.push(row?);
            }
        }
        Ok(out)
    }

    /// What clients need to show the forged skills among `ids` (unknown ids
    /// are left out).
    pub fn forged_views(&self, ids: &[u32]) -> StoreResult<Vec<SkillDefView>> {
        Ok(self.forged_rows(ids)?.iter().map(view).collect())
    }

    /// Registers every stored forged skill with the engine. Called once when
    /// the database is opened, before any deck is read.
    pub(crate) fn load_forged(&self) -> StoreResult<()> {
        let rows = {
            let conn = self.db();
            let mut stmt =
                conn.prepare(&format!("SELECT {COLUMNS} FROM forged_skill ORDER BY id"))?;
            let rows = stmt.query_map([], read_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for row in rows {
            let row = row?;
            // A definition this build no longer accepts stays in the database
            // (decks may hold it) but cannot be played: it resolves to nothing.
            let _ = registry::register(row.id, row.def);
        }
        Ok(())
    }
}
