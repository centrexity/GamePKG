//! Authoritative WIT world and component encoding for host-generated guest bindings.
use anyhow::Result;
pub const WIT: &str = include_str!("../wit/game.wit");
pub fn componentize(mut module: Vec<u8>) -> Result<Vec<u8>> {
    let mut resolve = wit_parser::Resolve::default();
    let package = resolve.push_str("game.wit", WIT)?;
    let world = resolve.select_world(package, Some("game"))?;
    wit_component::embed_component_metadata(
        &mut module,
        &resolve,
        world,
        wit_component::StringEncoding::UTF8,
    )?;
    wit_component::ComponentEncoder::default()
        .module(&module)?
        .validate(true)
        .encode()
}
