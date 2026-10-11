//! Forged skills: a skill described by data instead of code.
//!
//! A [`SkillDef`] is a small tree (an effect, constraints, how often it can be
//! used) that one generic [`composite::Composite`] interprets. Definitions are
//! registered by the server under a database id and then behave like any other
//! skill, through [`crate::skills::SkillId::Forged`].

pub mod at_least;
pub mod composite;
pub mod def;
pub mod generate;
pub mod identity;
pub mod measure;
pub mod rarity;
pub mod registry;

pub use def::{Constraint, DefError, Effect, Side, SkillDef, SwapScope};
pub use rarity::{Graded, Rarity};
