//! Titles: a chapter's title is earned with the three stars of its boss.

use super::{CHAPTERS, STAR_CHALLENGE, STAR_OBJECTIVE, STAR_WIN};
use crate::campaign_store::CampaignRow;

const ALL_STARS: u8 = STAR_WIN | STAR_OBJECTIVE | STAR_CHALLENGE;

/// Whether the boss of a chapter has been beaten with its three stars.
pub fn title_earned(rows: &[CampaignRow], chapter: u8) -> bool {
    rows.iter()
        .any(|r| r.at.chapter == chapter && r.at.is_boss() && r.stars == ALL_STARS)
}

/// Chapters whose title is earned, with the title.
pub fn titles_earned(rows: &[CampaignRow]) -> Vec<(u8, &'static str)> {
    CHAPTERS
        .iter()
        .enumerate()
        .map(|(chapter, c)| (chapter as u8, c.title))
        .filter(|&(chapter, _)| title_earned(rows, chapter))
        .collect()
}

/// Title of the highest chapter earned.
pub fn best_title(rows: &[CampaignRow]) -> Option<&'static str> {
    titles_earned(rows).last().map(|&(_, title)| title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::{LevelRef, BOSS_LEVEL};

    fn boss_row(chapter: u8, stars: u8) -> CampaignRow {
        CampaignRow {
            at: LevelRef {
                chapter,
                level: BOSS_LEVEL,
            },
            stars,
            rewarded: false,
        }
    }

    #[test]
    fn titles_need_the_full_boss_mask() {
        assert_eq!(best_title(&[]), None);
        let rows = [
            boss_row(0, ALL_STARS),
            boss_row(2, ALL_STARS),
            boss_row(1, STAR_WIN),
        ];
        assert!(title_earned(&rows, 0) && !title_earned(&rows, 1));
        assert_eq!(
            titles_earned(&rows),
            vec![(0, "Tombeur du Bélier"), (2, "Maître des Routes")]
        );
        assert_eq!(best_title(&rows), Some("Maître des Routes"));
    }

    #[test]
    fn a_non_boss_level_gives_no_title() {
        let row = CampaignRow {
            at: LevelRef {
                chapter: 1,
                level: 2,
            },
            stars: ALL_STARS,
            rewarded: false,
        };
        assert_eq!(best_title(&[row]), None);
    }

    #[test]
    fn titles_are_named_after_the_chapters() {
        let names: Vec<_> = CHAPTERS.iter().map(|c| c.title).collect();
        assert_eq!(
            names,
            [
                "Tombeur du Bélier",
                "Briseur de Muraille",
                "Maître des Routes",
                "Démasqueur",
                "Maître de forge"
            ]
        );
    }
}
