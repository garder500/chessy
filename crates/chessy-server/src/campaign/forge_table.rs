//! What the boss of each chapter may forge: a family, a rarity floor and the
//! drop weights (percent) of the rarities from that floor up.

use chessy_engine::forge::identity::Family;
use chessy_engine::forge::Rarity;

pub struct ForgeTable {
    /// `None`: any family.
    pub family: Option<Family>,
    pub floor: Rarity,
    pub weights: &'static [(Rarity, u8)],
}

const ATTACK: ForgeTable = ForgeTable {
    family: Some(Family::Attack),
    floor: Rarity::Uncommon,
    weights: &[
        (Rarity::Uncommon, 57),
        (Rarity::Rare, 29),
        (Rarity::Epic, 14),
    ],
};
const DEFENSE: ForgeTable = ForgeTable {
    family: Some(Family::Defense),
    floor: Rarity::Rare,
    weights: &[(Rarity::Rare, 68), (Rarity::Epic, 32)],
};
const MOBILITY: ForgeTable = ForgeTable {
    family: Some(Family::Mobility),
    ..DEFENSE
};
const CONTROL: ForgeTable = ForgeTable {
    family: Some(Family::Control),
    floor: Rarity::Epic,
    weights: &[(Rarity::Epic, 100)],
};
const CREATE: ForgeTable = ForgeTable {
    family: None,
    floor: Rarity::Epic,
    weights: &[(Rarity::Epic, 86), (Rarity::Legendary, 14)],
};

/// The table of a chapter (the last one's for any chapter past the end).
pub fn forge_table(chapter: u8) -> ForgeTable {
    match chapter {
        0 => ATTACK,
        1 => DEFENSE,
        2 => MOBILITY,
        3 => CONTROL,
        _ => CREATE,
    }
}

#[cfg(test)]
mod tests {
    use chessy_engine::forge::generate::DROP_WEIGHTS;

    use super::*;

    /// Engine drop weights from `floor` up, as whole percents summing to 100
    /// (largest remainder).
    fn expected(floor: Rarity, with_legendary: bool) -> Vec<(Rarity, u8)> {
        let kept: Vec<(Rarity, u64)> = Rarity::ALL
            .into_iter()
            .zip(DROP_WEIGHTS)
            .filter(|&(r, _)| r >= floor && (with_legendary || r != Rarity::Legendary))
            .collect();
        let total: u64 = kept.iter().map(|&(_, w)| w).sum();
        let mut percents: Vec<(Rarity, u64, u64)> = kept
            .iter()
            .map(|&(r, w)| (r, w * 100 / total, w * 100 % total))
            .collect();
        let missing = 100 - percents.iter().map(|p| p.1).sum::<u64>();
        let mut order: Vec<usize> = (0..percents.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(percents[i].2));
        for &i in order.iter().take(missing as usize) {
            percents[i].1 += 1;
        }
        percents.into_iter().map(|(r, p, _)| (r, p as u8)).collect()
    }

    #[test]
    fn tables_are_the_engine_drops_above_the_floor() {
        for chapter in 0..5 {
            let table = forge_table(chapter);
            assert_eq!(
                table.weights,
                expected(table.floor, chapter == 4),
                "chapter {chapter}"
            );
            assert_eq!(table.weights[0].0, table.floor);
        }
    }

    #[test]
    fn only_the_last_chapter_drops_legendaries_and_picks_any_family() {
        for chapter in 0..4 {
            let table = forge_table(chapter);
            assert!(table.weights.iter().all(|&(r, _)| r != Rarity::Legendary));
            assert!(table.family.is_some());
        }
        assert!(forge_table(4).family.is_none());
    }
}
