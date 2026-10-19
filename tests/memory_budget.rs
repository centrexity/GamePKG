//! Public budget behavior, also run in actual 32-bit host/Android test suites.
use gamepkg::memory_budget::Budget;
#[test]
fn nested_capacity_stays_charged_until_its_live_owner_is_released() {
    let game = Budget::new(16);
    let effects = game.partition(10).unwrap();
    let clips = effects.partition(6).unwrap();
    let playing = clips.reserve(6).unwrap();
    let guest = game.reserve(6).unwrap();
    assert!(game.reserve(1).is_err());
    assert!(effects.reserve(5).is_err());
    assert!(clips.reserve(1).is_err());
    assert_eq!(game.used(), 16);
    drop(clips);
    drop(effects);
    assert_eq!(game.used(), 16);
    drop(playing);
    assert_eq!(game.used(), 6);
    drop(guest);
    assert_eq!(game.used(), 0);
}
#[test]
fn full_width_counters_do_not_truncate_on_32_bit_hosts() {
    let game = Budget::new(u64::MAX);
    let cache = game.partition(u64::MAX).unwrap();
    let recording = cache.reserve(u64::MAX).unwrap();
    assert_eq!(game.used(), u64::MAX);
    assert_eq!(cache.used(), u64::MAX);
    assert!(game.reserve(1).is_err());
    assert!(cache.partition(1).is_err());
    drop(cache);
    assert_eq!(game.used(), u64::MAX);
    drop(recording);
    assert_eq!(game.used(), 0);
}
#[test]
fn another_thread_can_release_the_last_recording_after_cache_shutdown() {
    let game = Budget::new(8);
    let cache = game.partition(8).unwrap();
    let playing = cache.reserve(8).unwrap();
    drop(cache);
    let (release, wait) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        wait.recv().unwrap();
        drop(playing);
    });
    assert_eq!(game.used(), 8);
    assert!(game.reserve(1).is_err());
    release.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(game.used(), 0);
    assert!(game.partition(8).is_ok());
    assert_eq!(game.used(), 0);
}
