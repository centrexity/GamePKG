use gamepkg::runtime::Game;
use std::time::{Duration, Instant};

fn drive_network_fixture() -> std::collections::BTreeSet<u32> {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_NETWORK_TEST_ROOT").unwrap());
    let id = std::env::var("GAMEPKG_NETWORK_TEST_ID").unwrap();
    let mut game = Game::load(&root, &id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut observed = std::collections::BTreeSet::new();
    let mut ticks = 0;
    while Instant::now() < deadline {
        game.step(0).unwrap();
        ticks += 1;
        observed.insert(game.score());
        match game.score() {
            42 => break,
            999 => panic!("guest rejected an invalid transport result"),
            _ => {}
        }
        // Socket completion runs on host workers. Bound real elapsed time,
        // rather than assuming an arbitrary number of immediate ticks waits.
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        game.score(),
        42,
        "{id}: state {} after {ticks} ticks",
        game.state
    );
    game.finish().unwrap();
    println!("{id}: completed in {ticks} real async ticks; observed {observed:?}");
    observed
}

#[test]
#[ignore = "requires signed fixtures prepared by tests/network_game.py"]
fn fragmented_tcp_round_trip_completes_across_real_async_ticks() {
    let observed = drive_network_fixture();
    assert!(
        observed.contains(&101) && observed.contains(&103),
        "both deliberately separated reads must reach the guest"
    );
}

#[test]
#[ignore = "requires signed fixtures prepared by tests/network_listener_game.py"]
fn tcp_listener_partial_io_closes_and_retires_handles() {
    drive_network_fixture();
}
