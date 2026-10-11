//! `chessy-forge calibrate [--samples N] [--seed S] [--positions P] [--write]`
//! recomputes the rarity thresholds from random skills (and writes
//! `calibration.json` with `--write`).
//!
//! `chessy-forge report [--positions P]` grades the hand-written skills with
//! the forge's own yardstick, a sanity check on the thresholds: the unique
//! skills should land high, Mind Reading nowhere.
//!
//! `chessy-forge sample [--count N] [--seed S] [--rarity R]` forges a few
//! skills and prints them.
//!
//! `chessy-forge bench-floor [--runs N] [--attempts A] [--seed S]` times
//! `forge_at_least` for each chapter's (family, floor, target).

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use chessy_engine::ai::Rng;
use chessy_engine::forge::at_least::{forge_at_least, ForgeOutcome};
use chessy_engine::forge::generate::{self, Budget};
use chessy_engine::forge::identity::Family;
use chessy_engine::forge::measure;
use chessy_engine::forge::Rarity;
use chessy_engine::{skill, SkillId, SkillKind};

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn number<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> T {
    flag(args, name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn calibration_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../chessy-engine/src/forge/calibration.json")
}

fn calibrate(args: &[String]) -> ExitCode {
    let samples = number(args, "--samples", 600u32);
    let seed = number(args, "--seed", 1u64);
    let positions = number(args, "--positions", 24usize);
    let result = generate::calibrate(samples, seed, positions);
    let json = serde_json::to_string(&result).expect("calibration serializes");
    println!("{json}");
    if args.iter().any(|a| a == "--write") {
        let path = calibration_path();
        if let Err(e) = std::fs::write(&path, format!("{json}\n")) {
            eprintln!("cannot write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        eprintln!("wrote {}", path.display());
    }
    ExitCode::SUCCESS
}

fn report(args: &[String]) -> ExitCode {
    let positions = number(args, "--positions", 24usize);
    let th = generate::calibration().thresholds;
    let mut rows: Vec<(f64, String)> = SkillId::ALL
        .into_iter()
        .map(|id| {
            let m = measure::measure_skill(skill(id), positions);
            let tone = m.tone_index(false);
            let tier = th.tier(0.6 * tone + 0.4 * tone);
            let kind = if skill(id).kind() == SkillKind::Unique {
                "unique"
            } else {
                "classic"
            };
            (
                tone,
                format!(
                    "{id:<16} {kind:<8} tone {tone:5.1}  avail {:4.2}  swing {:6.1}  flips {:4.2}  mobility {:+5.1}  ~{}",
                    m.availability,
                    m.mean_swing,
                    m.flip_rate,
                    m.mobility_shift,
                    tier.as_str()
                ),
            )
        })
        .collect();
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, line) in rows {
        println!("{line}");
    }
    ExitCode::SUCCESS
}

fn sample(args: &[String]) -> ExitCode {
    let count = number(args, "--count", 10usize);
    let mut rng = Rng::new(number(args, "--seed", 7u64));
    let wanted = flag(args, "--rarity").and_then(|r| Rarity::parse(&r));
    let mut known = HashSet::new();
    for _ in 0..count {
        let target = wanted.unwrap_or_else(|| generate::roll_rarity(&mut rng));
        let forged = generate::forge(&mut rng, target, &known, Budget::live());
        known.insert(forged.def.signature());
        println!(
            "{:<10} (asked {:<9}) score {:5.1} cost {:5.1} tone {:5.1}{}  {}",
            forged.graded.rarity.as_str(),
            target.as_str(),
            forged.graded.score,
            forged.graded.cost,
            forged.graded.tone,
            if forged.graded.redundant {
                " redundant"
            } else {
                ""
            },
            serde_json::to_string(&forged.def).expect("serializes")
        );
    }
    ExitCode::SUCCESS
}

/// (family, floor, targets) of the five campaign chapters.
const CHAPTERS: [(Option<Family>, Rarity, &[Rarity]); 5] = [
    (
        Some(Family::Attack),
        Rarity::Uncommon,
        &[Rarity::Uncommon, Rarity::Rare, Rarity::Epic],
    ),
    (
        Some(Family::Defense),
        Rarity::Rare,
        &[Rarity::Rare, Rarity::Epic],
    ),
    (
        Some(Family::Mobility),
        Rarity::Rare,
        &[Rarity::Rare, Rarity::Epic],
    ),
    (Some(Family::Control), Rarity::Epic, &[Rarity::Epic]),
    (None, Rarity::Epic, &[Rarity::Epic, Rarity::Legendary]),
];

fn bench_floor(args: &[String]) -> ExitCode {
    let runs = number(args, "--runs", 20u32);
    let attempts = number(args, "--attempts", Budget::campaign().attempts);
    let budget = Budget {
        attempts,
        ..Budget::campaign()
    };
    let mut rng = Rng::new(number(args, "--seed", 1u64));
    let known = HashSet::new();
    println!("attempts {attempts}, {runs} forges per row");
    for (family, floor, targets) in CHAPTERS {
        for &target in targets {
            let start = Instant::now();
            let outcomes: Vec<_> = (0..runs)
                .map(|_| forge_at_least(&mut rng, target, floor, family, &known, budget))
                .collect();
            let mean = start.elapsed().as_secs_f64() / f64::from(runs);
            let share = |keep: &dyn Fn(&ForgeOutcome) -> bool| {
                100.0 * outcomes.iter().filter(|o| keep(o)).count() as f64 / f64::from(runs)
            };
            let exact = share(&|o| o.forged.as_ref().is_some_and(|f| f.graded.rarity == target));
            let none = share(&|o| o.forged.is_none());
            println!(
                "{:<9} floor {:<9} target {:<9} {:6.2}s/forge  exact {exact:5.1}%  fallback {:5.1}%  none {none:5.1}%",
                family.map_or("Free".into(), |f| format!("{f:?}")),
                floor.as_str(),
                target.as_str(),
                mean,
                100.0 - exact - none,
            );
        }
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("calibrate") => calibrate(&args),
        Some("report") => report(&args),
        Some("sample") => sample(&args),
        Some("bench-floor") => bench_floor(&args),
        _ => {
            eprintln!("usage: chessy-forge calibrate|report|sample|bench-floor [options]");
            ExitCode::FAILURE
        }
    }
}
