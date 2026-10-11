//! The skill a chapter boss gives: forged once, placed in the deck by the player.

mod common;

use std::sync::Arc;
use std::time::Duration;

use chessy_engine::SkillId;
use chessy_server::campaign::{
    forge_table, LevelRef, BOSS_LEVEL, CHAPTERS, STAR_CHALLENGE, STAR_OBJECTIVE, STAR_WIN,
};
use chessy_server::protocol::{ClientMsg, DevResult};
use chessy_server::store::Store;
use chessy_server::App;
use common::*;
use serde_json::Value;

const ATTACK: u8 = 0;
const FORGE_POLLS: u32 = 2400;
const ALL_STARS: u8 = STAR_WIN | STAR_OBJECTIVE | STAR_CHALLENGE;

fn open_boss(store: &Store, player: &str, chapter: u8) {
    for level in 0..BOSS_LEVEL {
        let at = LevelRef { chapter, level };
        store.record_campaign(player, at, ALL_STARS).unwrap();
    }
}

fn beat_boss(c: &Client, chapter: u8) {
    c.send(ClientMsg::CampaignStart {
        chapter,
        level: BOSS_LEVEL,
        deck: None,
    });
    c.send(ClientMsg::DevFinish {
        result: DevResult::Win,
    });
}

fn skill_of(info: &Value) -> SkillId {
    serde_json::from_value(info["skill"].clone()).unwrap()
}

fn deck_of(store: &Store, player: &str) -> Vec<SkillId> {
    store.deck(player).unwrap()
}

/// An account whose forge for `chapter` is waiting to be claimed.
fn with_forging_row(app: &Arc<App>, store: &Store, chapter: u8) -> Client {
    let (id, token) = store.register("ana", "unused-hash", None).unwrap();
    assert!(store.begin_boss_forge(&id, chapter).unwrap());
    Client::connect(app, Some(token))
}

async fn pending_forge(c: &mut Client, chapter: u8) -> Value {
    c.send(ClientMsg::BossForgeClaim { chapter });
    wait_pending(c, None).await
}

/// Rare forges are slow in debug builds: wait longer than `wait_for` does.
/// A forge that found nothing resends `forging`: with `retry`, claim again.
async fn wait_pending(c: &mut Client, retry: Option<u8>) -> Value {
    for _ in 0..FORGE_POLLS {
        while let Some(msg) = c.try_next("boss_forge") {
            if msg["info"]["state"] == "pending" {
                return msg["info"].clone();
            }
            if let Some(chapter) = retry {
                c.send(ClientMsg::BossForgeClaim { chapter });
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the boss forge never became pending");
}

#[tokio::test]
async fn a_boss_win_forges_a_skill_of_its_chapter_family() {
    let (app, store) = new_app(Default::default());
    let mut c = account(&app, &store, "ana");
    for (chapter, content) in (0u8..).zip(CHAPTERS.iter()) {
        if !content.available() {
            continue;
        }
        open_boss(&store, &c.id, chapter);
        beat_boss(&c, chapter);
        assert_eq!(c.next("boss_forge")["info"]["state"], "forging");
        let pending = wait_pending(&mut c, Some(chapter)).await;
        assert_eq!(pending["chapter"], chapter);

        let table = forge_table(chapter);
        let view = &store
            .forged_views(&[forged_id(&skill_of(&pending))])
            .unwrap()[0];
        assert!(
            view.rarity >= table.floor,
            "chapter {chapter}: below the floor"
        );
        if let Some(family) = table.family {
            assert_eq!(view.family, family, "chapter {chapter}");
        }
    }
}

fn forged_id(skill: &SkillId) -> u32 {
    match skill {
        SkillId::Forged(n) => *n,
        other => panic!("not a forged skill: {other:?}"),
    }
}

#[tokio::test]
async fn replaying_the_boss_forges_nothing_more() {
    let (app, store) = new_app(Default::default());
    let mut c = account(&app, &store, "ana");
    open_boss(&store, &c.id, ATTACK);
    beat_boss(&c, ATTACK);
    c.wait_for("boss_forge").await;
    c.wait_for("boss_forge").await;
    let forged = store.forged_signatures().unwrap().len();

    beat_boss(&c, ATTACK);
    c.next("game_over");
    c.next("game_over");
    assert!(c.try_next("boss_forge").is_none());
    assert_eq!(store.forged_signatures().unwrap().len(), forged);
}

#[tokio::test]
async fn two_simultaneous_claims_run_one_job() {
    let (app, store) = new_app(Default::default());
    let mut c = with_forging_row(&app, &store, ATTACK);
    assert_eq!(c.next("boss_forge")["info"]["state"], "forging");

    c.send(ClientMsg::BossForgeClaim { chapter: ATTACK });
    c.send(ClientMsg::BossForgeClaim { chapter: ATTACK });
    assert_eq!(c.error_code(), "forging");
    assert!(
        c.try_next("error").is_none(),
        "only the second claim is refused"
    );

    assert_eq!(c.wait_for("boss_forge").await["info"]["state"], "forging");
    assert_eq!(c.wait_for("boss_forge").await["info"]["state"], "pending");
    assert_eq!(store.forged_signatures().unwrap().len(), 1);
}

#[tokio::test]
async fn a_claim_after_a_restart_relaunches_the_forge() {
    let (app, store) = new_app(Default::default());
    let mut c = with_forging_row(&app, &store, ATTACK);
    assert_eq!(
        c.next("boss_forge")["info"]["state"],
        "forging",
        "pushed on connect"
    );

    let pending = pending_forge(&mut c, ATTACK).await;
    assert!(pending["skill"].is_string());
    let row = store.boss_forge(&c.id, ATTACK).unwrap().unwrap();
    assert_eq!(row.skill_id, Some(forged_id(&skill_of(&pending))));
}

#[tokio::test]
async fn placing_is_refused_outside_pending() {
    let (app, store) = new_app(Default::default());
    let mut c = with_forging_row(&app, &store, ATTACK);
    c.send(ClientMsg::BossForgePlace {
        chapter: ATTACK,
        replace: None,
    });
    assert_eq!(c.error_code(), "not_pending");
}

#[tokio::test]
async fn a_full_deck_replaces_a_skill_or_waits() {
    let (app, store) = new_app(Default::default());
    let mut c = with_forging_row(&app, &store, ATTACK);
    let deck: Vec<SkillId> = SkillId::ALL.iter().copied().take(7).collect();
    store.set_deck(&c.id, &deck).unwrap();
    let pending = pending_forge(&mut c, ATTACK).await;
    assert_eq!(pending["deck_full"], true);
    let gained = skill_of(&pending);

    c.send(ClientMsg::BossForgePlace {
        chapter: ATTACK,
        replace: None,
    });
    assert_eq!(c.error_code(), "deck_full");
    c.send(ClientMsg::BossForgePlace {
        chapter: ATTACK,
        replace: Some(SkillId::Forged(u32::MAX)),
    });
    assert_eq!(
        c.error_code(),
        "deck_full",
        "the replaced skill must be in the deck"
    );

    c.send(ClientMsg::BossForgePlace {
        chapter: ATTACK,
        replace: Some(deck[0]),
    });
    let update = c.next("deck_update");
    assert_eq!(update["lost"], serde_json::to_value(deck[0]).unwrap());
    assert_eq!(c.next("boss_forge")["info"]["state"], "placed");
    let after = deck_of(&store, &c.id);
    assert!(after.contains(&gained) && !after.contains(&deck[0]));
    assert_eq!(after.len(), 7);
}

#[tokio::test]
async fn placing_with_room_costs_nobody_a_skill() {
    let (app, store) = new_app(Default::default());
    let mut c = with_forging_row(&app, &store, ATTACK);
    let deck: Vec<SkillId> = SkillId::ALL.iter().copied().take(3).collect();
    store.set_deck(&c.id, &deck).unwrap();
    let gained = skill_of(&pending_forge(&mut c, ATTACK).await);

    c.send(ClientMsg::BossForgePlace {
        chapter: ATTACK,
        replace: None,
    });
    assert_eq!(c.next("deck_update")["lost"], Value::Null);
    let after = deck_of(&store, &c.id);
    assert!(deck.iter().all(|s| after.contains(s)));
    assert!(after.contains(&gained));

    c.send(ClientMsg::BossForgePlace {
        chapter: ATTACK,
        replace: None,
    });
    assert_eq!(c.error_code(), "not_pending", "placed once only");
}

#[tokio::test]
async fn a_guest_has_no_boss_forge() {
    let (app, _store) = new_app(Default::default());
    let mut g = guest(&app);
    g.send(ClientMsg::BossForgeClaim { chapter: ATTACK });
    assert_eq!(g.error_code(), "account_required");
    g.send(ClientMsg::BossForgePlace {
        chapter: ATTACK,
        replace: None,
    });
    assert_eq!(g.error_code(), "account_required");
}
