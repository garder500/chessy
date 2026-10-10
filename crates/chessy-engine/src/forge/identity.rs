//! What a forged skill is called and how it presents itself, all derived from
//! its definition: the name and the description are made from the effect, so
//! the description can never say something the skill does not do, and the
//! icon and sound specs tell the client what to draw and play.

use serde::{Deserialize, Serialize};

use super::def::{Constraint, Effect, Side, SkillDef, SwapScope};
use crate::types::PieceKind;

/// The five families of the catalogue (the client colours them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Attack,
    Defense,
    Mobility,
    Control,
    Create,
}

fn one() -> u8 {
    1
}

fn is_one(n: &u8) -> bool {
    *n == 1
}

/// How the client draws the icon. The icon is read off the structure of the
/// definition, one layer per brick: the central glyph is the action, the
/// target mark says whom it is about, the silhouette is the piece it is about,
/// the gauge is how long it lasts, the pips are how many times it can be used
/// and the marks are the rules around it. Two definitions that differ in any of
/// these bricks get two different icons.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconSpec {
    /// The action, one of [`GLYPHS`].
    pub glyph: String,
    /// The strongest piece the skill is about: `pawn`, `knight`, `bishop`, `rook`, `queen`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piece: Option<String>,
    /// `forever` when the effect is permanent, `short` or `long` otherwise; absent for an instant effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge: Option<String>,
    /// Whom it is about: `own` (your pieces), `enemy` or `any` (both camps, or the whole game).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Exact duration in plies (2 to 8); absent when the effect is instant or permanent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plies: Option<u8>,
    /// Uses per game (1 to 3).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub uses: u8,
    /// Rules around it, in a fixed order: `in_check`, `no_mate`, `no_check`, `free`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<String>,
    /// Every kind of piece the skill names, weakest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<String>,
}

/// How the client builds the sound: the effect picks the family of bricks,
/// the rest tunes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundSpec {
    /// Index of the effect (see [`effect_index`]).
    pub effect: u8,
    /// A note of the scale, 0 to 6.
    pub degree: u8,
    /// 0 to 3: how bright the sound is.
    pub timbre: u8,
    /// 0 to 2: how long it rings.
    pub length: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub name: String,
    pub description: String,
    pub family: Family,
    pub icon: IconSpec,
    pub sound: SoundSpec,
}

/// Every glyph an [`IconSpec`] may name; the client has a drawing for each.
pub const GLYPHS: [&str; 17] = [
    "snowflake",
    "shield",
    "veil",
    "morph",
    "crown",
    "erase",
    "banner",
    "portal",
    "echo",
    "swap",
    "summon",
    "ankh",
    "dove",
    "mirror",
    "fog",
    "mute",
    "domain",
];

pub fn effect_index(effect: &Effect) -> usize {
    match effect {
        Effect::Freeze { .. } => 0,
        Effect::Shield { .. } => 1,
        Effect::Cloak { .. } => 2,
        Effect::Morph { .. } => 3,
        Effect::Promote => 4,
        Effect::Remove { .. } => 5,
        Effect::Convert => 6,
        Effect::Teleport => 7,
        Effect::Duplicate => 8,
        Effect::Swap { .. } => 9,
        Effect::Spawn { .. } => 10,
        Effect::Revive { .. } => 11,
        Effect::Truce { .. } => 12,
        Effect::Mirror => 13,
        Effect::Fog { .. } => 14,
        Effect::Silence { .. } => 15,
        Effect::Ambush { .. } => 16,
    }
}

/// Nouns a skill of each effect may be called, indexed by [`effect_index`].
const NOUNS: [&[&str]; 17] = [
    &["Givre", "Gel", "Glace", "Frimas"],
    &["Égide", "Rempart", "Bastion", "Sauvegarde"],
    &["Voile", "Ombre", "Linceul", "Brume"],
    &["Métamorphose", "Mue", "Avatar", "Transmutation"],
    &["Ascension", "Couronnement", "Apothéose", "Sacre"],
    &["Effacement", "Bannissement", "Purge", "Faucheuse"],
    &["Ralliement", "Trahison", "Allégeance", "Défection"],
    &["Faille", "Bond", "Brèche", "Passage"],
    &["Écho", "Doublon", "Reflet", "Essaim"],
    &["Permutation", "Chassé-croisé", "Troc", "Échange"],
    &["Convocation", "Renfort", "Invocation", "Levée"],
    &["Nécromancie", "Résurrection", "Rappel", "Renaissance"],
    &["Armistice", "Trêve", "Paix blanche", "Cessez-le-feu"],
    &["Miroir", "Inversion", "Palindrome", "Contre-champ"],
    &[
        "Brouillard",
        "Brume épaisse",
        "Nuit blanche",
        "Purée de pois",
    ],
    &["Silence", "Mutisme", "Bâillon", "Serment du taciturne"],
    &[
        "Expansion du domaine",
        "Domaine",
        "Guet-apens",
        "Cathédrale des fous",
    ],
];

const SYLLABLES: [&str; 32] = [
    "al", "bor", "cal", "dra", "el", "fen", "gor", "hal", "il", "jor", "kar", "lor", "mor", "nyx",
    "ol", "pyr", "quel", "ras", "syl", "tor", "ul", "vor", "wyn", "xan", "yl", "zer", "ar", "eth",
    "is", "om", "un", "ka",
];

fn mix(seed: u64, salt: u64) -> u64 {
    let mut x = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// A made-up proper name, two or three syllables.
fn proper_name(seed: u64) -> String {
    let n = 2 + mix(seed, 1) % 2;
    let word: String = (0..n)
        .map(|i| SYLLABLES[(mix(seed, 10 + i) % SYLLABLES.len() as u64) as usize])
        .collect();
    capitalize(&word)
}

fn name(def: &SkillDef, seed: u64) -> String {
    let nouns = NOUNS[effect_index(&def.effect)];
    let noun = nouns[(mix(seed, 2) % nouns.len() as u64) as usize];
    let proper = proper_name(seed);
    let link = if proper.starts_with(['A', 'E', 'I', 'O', 'U', 'Y']) {
        "d'"
    } else {
        "de "
    };
    format!("{noun} {link}{proper}")
}

fn kind_name(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Pawn => "pion",
        PieceKind::Knight => "cavalier",
        PieceKind::Bishop => "fou",
        PieceKind::Rook => "tour",
        PieceKind::Queen => "dame",
        PieceKind::King => "roi",
    }
}

fn kind_plural(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Pawn => "pions",
        PieceKind::Knight => "cavaliers",
        PieceKind::Bishop => "fous",
        PieceKind::Rook => "tours",
        PieceKind::Queen => "dames",
        PieceKind::King => "rois",
    }
}

/// "pion, cavalier ou dame".
fn kinds_list(kinds: &[PieceKind]) -> String {
    let names: Vec<&str> = kinds.iter().map(|&k| kind_name(k)).collect();
    match names.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_string(),
        Some((last, rest)) => format!("{} ou {last}", rest.join(", ")),
    }
}

fn kinds_list_plural(kinds: &[PieceKind]) -> String {
    let names: Vec<&str> = kinds.iter().map(|&k| kind_plural(k)).collect();
    match names.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_string(),
        Some((last, rest)) => format!("{} ou {last}", rest.join(", ")),
    }
}

/// "4 coups (2 de chaque camp)": a ply is one move by one player.
fn duration(plies: u8) -> String {
    if plies.is_multiple_of(2) {
        format!("{plies} coups ({} de chaque camp)", plies / 2)
    } else {
        format!("{plies} coups")
    }
}

fn effect_sentence(effect: &Effect) -> String {
    match effect {
        Effect::Freeze { plies } => format!(
            "Immobilise une pièce ennemie (pas le roi) pendant {}. Une pièce gelée ne peut pas bouger et ne donne pas échec.",
            duration(*plies)
        ),
        Effect::Shield { plies } => format!(
            "Une de tes pièces (pas le roi) ne peut pas être capturée pendant {}.",
            duration(*plies)
        ),
        Effect::Cloak { plies } => format!(
            "Cache une de tes pièces (pas le roi) à l'adversaire pendant {}.",
            duration(*plies)
        ),
        Effect::Morph { side, into, plies } => {
            let whose = match side {
                Side::Own => "une de tes pièces",
                Side::Enemy => "une pièce ennemie",
            };
            format!(
                "Transforme {whose} (pas le roi) en {} pendant {}, puis elle reprend sa forme.",
                kind_name(*into),
                duration(*plies)
            )
        }
        Effect::Promote => {
            "Une de tes pièces (ni le roi ni une dame) devient une dame, pour de bon.".to_string()
        }
        Effect::Remove { kinds } => format!(
            "Retire de l'échiquier un(e) {} ennemi(e).",
            kinds_list(kinds)
        ),
        Effect::Convert => {
            "Une pièce ennemie (pas le roi) qui n'en attaque aucune des tiennes passe de ton côté, pour de bon.".to_string()
        }
        Effect::Teleport => {
            "Déplace une de tes pièces (pas le roi) sur n'importe quelle case vide, sans tenir compte des obstacles.".to_string()
        }
        Effect::Duplicate => {
            "Copie une de tes pièces (pas le roi) sur une case vide voisine.".to_string()
        }
        Effect::Swap { scope } => match scope {
            SwapScope::Own => "Échange la place de deux de tes pièces.".to_string(),
            SwapScope::Any => {
                "Échange la place de deux pièces, quel que soit leur camp (pas les rois).".to_string()
            }
        },
        Effect::Spawn { kinds, plies } => format!(
            "Fait apparaître un(e) {} temporaire sur une case libre des rangées 3 à 6, pendant {}.",
            kinds_list(kinds),
            duration(*plies)
        ),
        Effect::Revive { kinds } => format!(
            "Ramène sur l'échiquier une de tes pièces capturées ({}), sur sa case d'origine ou la plus proche.",
            kinds_list_plural(kinds)
        ),
        Effect::Truce { plies } => format!(
            "Armistice : pendant {}, plus rien n'attaque rien. Aucune capture, aucun échec.",
            duration(*plies)
        ),
        Effect::Mirror => {
            "Les deux camps échangent leurs armées : chaque joueur hérite des pièces de l'autre, à la place symétrique.".to_string()
        }
        Effect::Fog { plies } => format!(
            "Brouillard : pendant {}, chaque joueur ne voit que les pièces ennemies à deux cases ou moins des siennes.",
            duration(*plies)
        ),
        Effect::Silence { plies } => format!(
            "L'adversaire ne peut utiliser aucun pouvoir pendant {}.",
            duration(*plies)
        ),
        Effect::Ambush { plies } => format!(
            "Expansion de domaine : pendant {}, la première fois que l'adversaire met ton roi en échec, des fous surgissent et capturent les pièces qui l'attaquent.",
            duration(*plies)
        ),
    }
}

fn description(def: &SkillDef) -> String {
    let mut out = effect_sentence(&def.effect);
    for c in &def.constraints {
        out.push(' ');
        out.push_str(match c {
            Constraint::OnlyInCheck => "Utilisable seulement quand ton roi est en échec.",
            Constraint::ForbidMate => "Refusé s'il met l'adversaire échec et mat.",
            Constraint::ForbidCheck => "Refusé s'il laisse un roi en échec.",
        });
    }
    if matches!(
        def.effect,
        Effect::Swap {
            scope: SwapScope::Any
        }
    ) {
        out.push_str(" Refusé s'il laisse un roi en échec.");
    }
    if def.free_action {
        out.push_str(" Ne consomme pas ton tour.");
    }
    if def.max_uses > 1 {
        out.push_str(&format!(" Utilisable {} fois par partie.", def.max_uses));
    }
    out
}

fn family(effect: &Effect) -> Family {
    match effect {
        Effect::Remove { .. } | Effect::Convert => Family::Attack,
        Effect::Shield { .. } | Effect::Cloak { .. } => Family::Defense,
        Effect::Teleport | Effect::Swap { .. } => Family::Mobility,
        Effect::Promote | Effect::Duplicate | Effect::Spawn { .. } | Effect::Revive { .. } => {
            Family::Create
        }
        Effect::Freeze { .. }
        | Effect::Morph { .. }
        | Effect::Truce { .. }
        | Effect::Mirror
        | Effect::Fog { .. }
        | Effect::Silence { .. }
        | Effect::Ambush { .. } => Family::Control,
    }
}

/// The piece a skill is about, when it has a favourite.
fn icon_piece(effect: &Effect) -> Option<PieceKind> {
    let strongest = |kinds: &[PieceKind]| {
        kinds
            .iter()
            .copied()
            .max_by_key(|k| *k as u8 /* pawn < ... < queen */)
    };
    match effect {
        Effect::Morph { into, .. } => Some(*into),
        Effect::Remove { kinds } | Effect::Spawn { kinds, .. } | Effect::Revive { kinds } => {
            strongest(kinds)
        }
        Effect::Promote => Some(PieceKind::Queen),
        _ => None,
    }
}

fn plies_of(effect: &Effect) -> Option<u8> {
    match effect {
        Effect::Freeze { plies }
        | Effect::Shield { plies }
        | Effect::Cloak { plies }
        | Effect::Morph { plies, .. }
        | Effect::Spawn { plies, .. }
        | Effect::Truce { plies }
        | Effect::Fog { plies }
        | Effect::Silence { plies }
        | Effect::Ambush { plies } => Some(*plies),
        _ => None,
    }
}

fn badge(def: &SkillDef) -> Option<&'static str> {
    if def.irreversible() {
        return Some("forever");
    }
    plies_of(&def.effect).map(|p| if p <= 4 { "short" } else { "long" })
}

fn kind_key(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Pawn => "pawn",
        PieceKind::Knight => "knight",
        PieceKind::Bishop => "bishop",
        PieceKind::Rook => "rook",
        PieceKind::Queen => "queen",
        PieceKind::King => "king",
    }
}

fn icon_target(effect: &Effect) -> &'static str {
    match effect {
        Effect::Freeze { .. }
        | Effect::Remove { .. }
        | Effect::Convert
        | Effect::Silence { .. } => "enemy",
        Effect::Morph { side, .. } => match side {
            Side::Own => "own",
            Side::Enemy => "enemy",
        },
        Effect::Swap { scope } => match scope {
            SwapScope::Own => "own",
            SwapScope::Any => "any",
        },
        Effect::Truce { .. } | Effect::Mirror | Effect::Fog { .. } => "any",
        _ => "own",
    }
}

/// Every kind of piece an effect names, weakest first.
fn icon_kinds(effect: &Effect) -> Vec<PieceKind> {
    let mut kinds = match effect {
        Effect::Morph { into, .. } => vec![*into],
        Effect::Remove { kinds } | Effect::Spawn { kinds, .. } | Effect::Revive { kinds } => {
            kinds.clone()
        }
        Effect::Promote => vec![PieceKind::Queen],
        _ => Vec::new(),
    };
    kinds.sort_by_key(|k| *k as u8);
    kinds.dedup();
    kinds
}

/// The icon of a skill, one layer per brick of its definition.
pub fn icon(def: &SkillDef) -> IconSpec {
    let def = def.clone().canonical();
    let marks = def
        .constraints
        .iter()
        .map(|c| match c {
            Constraint::OnlyInCheck => "in_check",
            Constraint::ForbidMate => "no_mate",
            Constraint::ForbidCheck => "no_check",
        })
        .chain(def.free_action.then_some("free"))
        .map(str::to_string)
        .collect();
    IconSpec {
        glyph: GLYPHS[effect_index(&def.effect)].to_string(),
        piece: icon_piece(&def.effect).map(|k| kind_key(k).to_string()),
        badge: badge(&def).map(str::to_string),
        target: Some(icon_target(&def.effect).to_string()),
        plies: plies_of(&def.effect),
        uses: def.max_uses,
        marks,
        kinds: icon_kinds(&def.effect)
            .into_iter()
            .map(|k| kind_key(k).to_string())
            .collect(),
    }
}

/// Everything about how the skill presents itself.
pub fn identity(def: &SkillDef) -> Identity {
    let seed = def.fingerprint();
    let index = effect_index(&def.effect);
    Identity {
        name: name(def, seed),
        description: description(def),
        family: family(&def.effect),
        icon: icon(def),
        sound: SoundSpec {
            effect: index as u8,
            degree: (mix(seed, 3) % 7) as u8,
            timbre: (mix(seed, 4) % 4) as u8,
            length: (mix(seed, 5) % 3) as u8,
        },
    }
}
