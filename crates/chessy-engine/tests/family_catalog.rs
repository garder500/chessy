use chessy_engine::SkillId;
use std::collections::HashMap;

fn catalog_families() -> HashMap<String, String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/src/catalog.ts");
    let source = std::fs::read_to_string(path).expect("web/src/catalog.ts is readable");
    source
        .lines()
        .filter_map(|line| {
            let args = line.trim().strip_prefix("e(\"")?;
            let mut quoted = args.split('"');
            let wire = quoted.next()?;
            let family = quoted.nth(1)?;
            Some((wire.to_string(), family.to_string()))
        })
        .collect()
}

#[test]
fn engine_families_match_the_client_catalog() {
    let catalog = catalog_families();
    assert_eq!(catalog.len(), SkillId::ALL.len());
    for skill in SkillId::ALL {
        let family = format!("{:?}", skill.family()).to_lowercase();
        assert_eq!(
            Some(family.as_str()),
            catalog.get(&skill.to_string()).map(String::as_str),
            "{skill}"
        );
    }
}
