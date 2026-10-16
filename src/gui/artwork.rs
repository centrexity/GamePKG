//! Curated, freely licensed covers for older packages with placeholder artwork.
pub(super) fn fallback(id: &str) -> Option<(&'static [u8], &'static str)> {
    let (png, notice): (&[u8], &str) = match id {
        "com.gamepkg.2048" => (
            include_bytes!("../../ports/2048/thumbnail.png"),
            include_str!("../../ports/2048/NOTICE"),
        ),
        "com.gamepkg.arkanoo" => (
            include_bytes!("../../ports/arkanoo/thumbnail.png"),
            include_str!("../../ports/arkanoo/NOTICE"),
        ),
        "com.gamepkg.doom" => (
            include_bytes!("../../ports/doom/thumbnail.png"),
            include_str!("../../ports/doom/NOTICE"),
        ),
        "com.gamepkg.zelda3" => (
            include_bytes!("../../ports/zelda3/thumbnail.png"),
            include_str!("../../ports/zelda3/NOTICE"),
        ),
        _ => return None,
    };
    Some((
        png,
        notice
            .split_once("Thumbnail artwork:")
            .map_or(notice, |(_, text)| text),
    ))
}
