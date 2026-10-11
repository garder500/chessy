use super::SkillId;
use crate::forge::identity::{identity, Family};
use crate::forge::registry;

impl SkillId {
    /// The family the client colours the skill with. An unregistered forged
    /// skill falls back to `Control`, like the client's placeholder entry.
    pub fn family(self) -> Family {
        use Family::*;
        match self {
            SkillId::Teleportation
            | SkillId::Rollback
            | SkillId::DestinySwapper
            | SkillId::Bench
            | SkillId::Transposition
            | SkillId::Temporal => Mobility,
            SkillId::Imune | SkillId::Invisibility | SkillId::Forcefield | SkillId::Celestial => {
                Defense
            }
            SkillId::Clone
            | SkillId::Morph
            | SkillId::Godhelp
            | SkillId::Wall
            | SkillId::Mirage
            | SkillId::Evolve => Create,
            SkillId::Canceller
            | SkillId::Tornado
            | SkillId::Freeze
            | SkillId::Geomancy
            | SkillId::Mind
            | SkillId::Control => Control,
            SkillId::Terminator
            | SkillId::Trap
            | SkillId::Queensac
            | SkillId::Remover
            | SkillId::Switch => Attack,
            SkillId::Forged(n) => registry::def(n).map_or(Control, |def| identity(def).family),
        }
    }
}
