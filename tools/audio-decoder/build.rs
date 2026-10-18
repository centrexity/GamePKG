fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GAMEPKG_AUDIO_MAX_MEMORY_BYTES");
    if std::env::var("TARGET").as_deref() == Ok("wasm32-unknown-unknown") {
        let maximum = std::env::var("GAMEPKG_AUDIO_MAX_MEMORY_BYTES")
            .map(|value| value.parse::<u32>().expect("invalid audio memory maximum"))
            .unwrap_or(16 * 1024 * 1024);
        assert!(
            (65536..=16 * 1024 * 1024).contains(&maximum) && maximum % 65536 == 0,
            "audio memory maximum must be page-aligned and no larger than 16 MiB"
        );
        println!("cargo:rustc-link-arg=--max-memory={maximum}");
    }
}
