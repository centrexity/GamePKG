//! Feed production lease output into the unchanged original engine bindings.
use super::*;
fn step(input: &mut Input, session: &mut Session, expired: bool) -> u32 {
    let now = Instant::now();
    let at = if expired {
        input.updated = Some(now - INPUT_LEASE);
        now
    } else {
        input.updated.unwrap_or(now)
    };
    let (buttons, dx, dy, events) = input.take(at);
    for event in events {
        session.queue(event).unwrap();
    }
    session.tick(buttons, dx, dy).unwrap();
    let saved = session.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    let config = files.iter().find(|f| f.path == "config.cfg").unwrap();
    std::str::from_utf8(config.bytes)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("maxfps "))
        .unwrap()
        .parse()
        .unwrap()
}
#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_bindings_release_stale_input_and_resume_only_current_held_controls() {
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original data");
    let config=b"exec data/defaults.cfg\nmaxfps 100\nbind W [maxfps (+ $maxfps 1); onrelease [maxfps (+ $maxfps 10)]]\nbind MOUSE1 [maxfps (+ $maxfps 1); onrelease [maxfps (+ $maxfps 10)]]\nbind F7 [maxfps (+ $maxfps 1); onrelease [maxfps (+ $maxfps 10)]]\nbind F6 [mode 0; map complex; showgui \"\"]\n";
    let mut seed = b"GPS1".to_vec();
    seed.extend((config.len() as u32).to_le_bytes());
    seed.extend([0; 8]);
    seed.extend(config);
    let mut session = Session::compile_and_start_with_save(
        std::path::Path::new(&data),
        2 * 1024 * 1048576,
        Duration::from_secs(60),
        &seed,
    )
    .unwrap();
    session.tick(0, 0, 0).unwrap();
    session.key(63, true, false, 0).unwrap();
    session.key(63, false, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    session.set_deadline(Duration::from_millis(500)).unwrap();
    let mut input = Input::default();
    let mut actual = Vec::new();
    let down = || {
        vec![
            protocol::key_event(26, true, false, 0).unwrap(),
            protocol::mouse_button_event(1, true, 0).unwrap(),
        ]
    };
    input.submit(0, 0, 0, down()).unwrap();
    actual.push(step(&mut input, &mut session, false));
    actual.push(step(&mut input, &mut session, true));
    input.submit(0, 0, 0, vec![]).unwrap();
    actual.push(step(&mut input, &mut session, false));
    actual.push(step(&mut input, &mut session, false));
    input
        .submit(
            0,
            0,
            0,
            vec![
                protocol::key_event(26, false, false, 0).unwrap(),
                protocol::mouse_button_event(1, false, 0).unwrap(),
            ],
        )
        .unwrap();
    actual.push(step(&mut input, &mut session, false));
    actual.push(step(&mut input, &mut session, true));
    input
        .submit(
            0,
            0,
            0,
            vec![
                protocol::key_event(64, true, false, 0).unwrap(),
                protocol::key_event(64, false, false, 0).unwrap(),
            ],
        )
        .unwrap();
    actual.push(step(&mut input, &mut session, false));
    input.submit(0, 0, 0, vec![]).unwrap();
    actual.push(step(&mut input, &mut session, false));
    input.submit(0, 0, 0, down()).unwrap();
    actual.push(step(&mut input, &mut session, false));
    input.reset();
    actual.push(step(&mut input, &mut session, false));
    input.submit(0, 0, 0, vec![]).unwrap();
    actual.push(step(&mut input, &mut session, false));
    input
        .submit(
            0,
            0,
            0,
            vec![protocol::key_event(26, true, false, 0).unwrap()],
        )
        .unwrap();
    actual.push(step(&mut input, &mut session, false));
    actual.push(step(&mut input, &mut session, true));
    input
        .submit(
            0,
            0,
            0,
            vec![
                protocol::key_event(26, true, true, 0).unwrap(),
                protocol::key_event(26, true, true, 0).unwrap(),
            ],
        )
        .unwrap();
    actual.push(step(&mut input, &mut session, false));
    input
        .submit(
            0,
            0,
            0,
            vec![protocol::key_event(26, false, false, 0).unwrap()],
        )
        .unwrap();
    actual.push(step(&mut input, &mut session, false));
    // A fresh controller hold must wait behind the stale tail, rather than
    // being repeatedly reset/repressed while that old history is drained.
    let mut backlog = vec![protocol::key_event(26, true, false, 0).unwrap()];
    backlog.extend(vec![protocol::text_event("x").unwrap(); 200]);
    input.submit(0, 0, 0, backlog).unwrap();
    actual.push(step(&mut input, &mut session, false));
    actual.push(step(&mut input, &mut session, true));
    input.submit(1, 0, 0, vec![]).unwrap();
    actual.push(step(&mut input, &mut session, false));
    actual.push(step(&mut input, &mut session, false));
    input
        .submit(
            0,
            0,
            0,
            vec![protocol::key_event(26, false, false, 0).unwrap()],
        )
        .unwrap();
    actual.push(step(&mut input, &mut session, false));
    session.close().unwrap();
    assert_eq!(
        actual,
        [
            102, 122, 124, 124, 144, 144, 155, 155, 157, 177, 177, 178, 188, 189, 199, 200, 210,
            210, 211, 221
        ]
    );
}

fn live_frames(worker: &Worker, frames: u64) {
    let target = worker.poll().unwrap().ticks + frames;
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        worker.submit(0, 0, 0, vec![]).unwrap();
        let output = worker.poll().unwrap();
        assert!(output.error.is_none(), "{:?}", output.error);
        if output.ticks >= target {
            return;
        }
        assert!(Instant::now() < until, "worker frames stopped");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn live_expiry(worker: &Worker) {
    let until = Instant::now() + Duration::from_secs(10);
    let mut expired_at = None;
    loop {
        let state = worker.shared.lock().unwrap();
        assert!(state.output.error.is_none(), "{:?}", state.output.error);
        if state.input.expired && expired_at.is_none() {
            expired_at = Some(state.output.ticks);
        }
        if expired_at.is_some_and(|at| state.output.ticks >= at + 2) {
            return;
        }
        drop(state);
        assert!(Instant::now() < until, "worker did not expire stale input");
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_worker_expires_without_ui_updates_then_recovers_without_replaying_taps() {
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original data");
    let config=b"exec data/defaults.cfg\nmaxfps 100\nbind W [maxfps (+ $maxfps 1); onrelease [maxfps (+ $maxfps 10)]]\nbind MOUSE1 [maxfps (+ $maxfps 1); onrelease [maxfps (+ $maxfps 10)]]\nbind F7 [maxfps (+ $maxfps 1); onrelease [maxfps (+ $maxfps 10)]]\nbind F6 [mode 0; map complex; showgui \"\"]\n";
    let mut seed = b"GPS1".to_vec();
    seed.extend((config.len() as u32).to_le_bytes());
    seed.extend([0; 8]);
    seed.extend(config);
    let mut session = Session::compile_and_start_with_save(
        std::path::Path::new(&data),
        2 * 1024 * 1048576,
        Duration::from_secs(60),
        &seed,
    )
    .unwrap();
    session.tick(0, 0, 0).unwrap();
    session.key(63, true, false, 0).unwrap();
    session.key(63, false, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    session.set_deadline(Duration::from_millis(500)).unwrap();
    let root = tempfile::tempdir().unwrap();
    let store = super::super::save::Store::open(root.path(), "test.lease", "fixture", 1).unwrap();
    let mut worker = Worker::start(session, store, None).unwrap();
    worker
        .submit(
            0,
            0,
            0,
            vec![
                protocol::key_event(26, true, false, 0).unwrap(),
                protocol::mouse_button_event(1, true, 0).unwrap(),
            ],
        )
        .unwrap();
    live_frames(&worker, 2);
    live_expiry(&worker);
    live_frames(&worker, 2); // Still-held controls resume once on a fresh UI report.
    worker
        .submit(
            0,
            0,
            0,
            vec![
                protocol::key_event(26, false, false, 0).unwrap(),
                protocol::mouse_button_event(1, false, 0).unwrap(),
            ],
        )
        .unwrap();
    live_frames(&worker, 2);
    live_expiry(&worker);
    worker
        .submit(
            0,
            0,
            0,
            vec![
                protocol::key_event(64, true, false, 0).unwrap(),
                protocol::key_event(64, false, false, 0).unwrap(),
            ],
        )
        .unwrap();
    live_frames(&worker, 3);
    worker.finish().unwrap();
    let saved = super::super::save::Store::open(root.path(), "test.lease", "fixture", 1).unwrap();
    let files = protocol::save_files(&saved.seed).unwrap();
    let config = files.iter().find(|f| f.path == "config.cfg").unwrap();
    assert!(std::str::from_utf8(config.bytes)
        .unwrap()
        .lines()
        .any(|line| line == "maxfps 155"));
}
