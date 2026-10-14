//! Project-owned support details shared by the app and public website.
use std::sync::OnceLock;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Details {
    pub paypal_url: String,
    pub bitcoin: String,
    pub monero: String,
    pub dogecoin: String,
    pub mining_url: String,
}

pub fn details() -> &'static Details {
    static DETAILS: OnceLock<Details> = OnceLock::new();
    DETAILS.get_or_init(|| {
        serde_json::from_str(include_str!("../html/support/details.json"))
            .expect("bundled support details must be valid")
    })
}
