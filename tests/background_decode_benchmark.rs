//! Maintainer comparison of lossless background storage encodings.
use gamepkg::drawing::image::Image;
#[test]
#[ignore = "set GAMEPKG_BACKGROUND_CANDIDATE to a candidate background1-0.png"]
fn compare_background_decoding_without_changing_pixel_or_image_limits() {
    let baseline_path = std::env::var("GAMEPKG_BACKGROUND_BASELINE").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/ports/arkanoo/assets/background1-0.png"
        )
        .to_owned()
    });
    let baseline = std::fs::read(baseline_path).unwrap();
    let candidate = std::fs::read(std::env::var("GAMEPKG_BACKGROUND_CANDIDATE").unwrap()).unwrap();
    let original = Image::from_png(&baseline).unwrap();
    let improved = Image::from_png(&candidate).unwrap();
    assert_eq!(
        original, improved,
        "Storage optimization changed original pixels"
    );
    assert!(candidate.len() <= gamepkg::drawing::image::MAX_PNG_BYTES);
    assert_eq!(improved.size(), [512, 512]);
    for (label, bytes) in [("baseline", baseline), ("candidate", candidate)] {
        let start = std::time::Instant::now();
        for _ in 0..40 {
            std::hint::black_box(Image::from_png(&bytes).unwrap());
        }
        eprintln!(
            "{label}: {} encoded bytes; {:.3} ms/decode",
            bytes.len(),
            start.elapsed().as_secs_f64() * 1000.0 / 40.0
        );
    }
}
