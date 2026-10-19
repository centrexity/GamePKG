//! Actual original SDL primitive captures for the earlier point backgrounds.
use serde_json::Value;
use std::sync::OnceLock;

pub fn case(level: u32, frame: u64) -> &'static Value {
    static CASES: OnceLock<Vec<Value>> = OnceLock::new();
    let cases = CASES.get_or_init(|| {
        let mut cases = Vec::new();
        for text in [
            include_str!("../fixtures/arkanoo-canonical-particles.json"),
            include_str!("../fixtures/arkanoo-canonical-effects.json"),
            include_str!("../fixtures/arkanoo-canonical-campaign.json"),
        ] {
            let mut fixture: Value = serde_json::from_str(text).unwrap();
            cases.append(fixture["cases"].as_array_mut().unwrap());
        }
        cases
    });
    cases
        .iter()
        .find(|case| {
            case["level"].as_u64() == Some(u64::from(level))
                && case["frame"].as_u64() == Some(frame)
        })
        .unwrap_or_else(|| {
            panic!("Missing original canonical background: level {level}, frame {frame}")
        })
}
