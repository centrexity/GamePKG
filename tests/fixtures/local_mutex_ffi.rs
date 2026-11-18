// Maintainer-only C/Rust ABI driver. Unsafe extern is rejected as game source.
#[allow(dead_code)]
#[path = "../../engines/native-services/client.rs"]
mod client;
#[path = "../../engines/native-services/local_bindings.rs"]
mod local;
unsafe extern "C" {
    fn mutex_c_query(
        id: u64,
        locked: u32,
        owned: u32,
        abandoned: u32,
        depth: u32,
        error: i32,
    ) -> i32;
    fn mutex_c_retire(id: u64, kind: u32, expected: i32) -> i32;
}
fn check(id: u64, locked: u32, owned: u32, abandoned: u32, depth: u32, error: i32) {
    assert_eq!(
        unsafe { mutex_c_query(id, locked, owned, abandoned, depth, error) },
        0,
        "query id={id} expected=({locked},{owned},{abandoned},{depth}) error={error}"
    );
}
fn retire(id: u64, kind: u32, expected: i32) {
    assert_eq!(unsafe { mutex_c_retire(id, kind, expected) }, 0);
}
fn abandon(context: local::runtime::TaskContext, id: u64) -> u64 {
    context.runtime().mutex_lock(id, 0, true).unwrap();
    context.runtime().mutex_lock(id, 0, true).unwrap();
    84
}
fn main() {
    local::install(
        local::runtime::Runtime::new(&[], vec![abandon], 4 * 1024 * 1024, 2, 65536).unwrap(),
    )
    .unwrap();
    let bytes = local::call(21, &[], 8).unwrap();
    let id = u64::from_le_bytes(bytes.try_into().unwrap());
    check(id, 0, 0, 0, 0, 0);
    // A forged high half must not truncate to the same actual i686 object.
    let foreign_bits = id | 0x8000000000000000;
    check(foreign_bits, 0, 0, 0, 0, -4);
    retire(foreign_bits, 2, -4);
    check(id, 0, 0, 0, 0, 0);
    client::local_mutex_lock(id, 0).unwrap();
    client::local_mutex_lock(id, 0).unwrap();
    check(id, 1, 1, 0, 2, 0);
    std::thread::spawn(move || {
        check(id, 1, 0, 0, 2, 0);
        retire(id, 2, -2);
    })
    .join()
    .unwrap();
    retire(id, 1, -1);
    check(id, 1, 1, 0, 2, 0);
    local::call(24, &id.to_le_bytes(), 0).unwrap();
    local::call(24, &id.to_le_bytes(), 0).unwrap();
    let task = local::call(
        10,
        &[
            0u32.to_le_bytes().as_slice(),
            &id.to_le_bytes(),
            &0u32.to_le_bytes(),
        ]
        .concat(),
        8,
    )
    .unwrap();
    let task = u64::from_le_bytes(task.try_into().unwrap());
    client::local_wait_many(
        &[client::LocalWaitObject { id: task, kind: 4 }],
        false,
        1000,
    )
    .unwrap();
    assert_eq!(client::local_task_join(task).unwrap().result(), 84);
    for _ in 0..3 {
        check(id, 0, 0, 1, 0, 0);
    }
    assert!(
        client::local_wait_many(&[client::LocalWaitObject { id, kind: 2 }], false, 0)
            .unwrap()
            .abandoned
    );
    check(id, 1, 1, 0, 1, 0);
    local::call(24, &id.to_le_bytes(), 0).unwrap();
    retire(id, 2, 0);
    check(id, 0, 0, 0, 0, -4);
    let event = local::call(20, &[0u32.to_le_bytes(), 1u32.to_le_bytes()].concat(), 8).unwrap();
    let event = u64::from_le_bytes(event.try_into().unwrap());
    check(event, 0, 0, 0, 0, -1);
    retire(event, 2, -1);
    assert!(client::local_event_state(event).unwrap().signaled);
    retire(event, 1, 0);
    let semaphore = client::local_semaphore_create(1, 2).unwrap();
    check(semaphore, 0, 0, 0, 0, -1);
    retire(semaphore, 2, -1);
    assert_eq!(client::local_semaphore_state(semaphore).unwrap().count, 1);
    retire(semaphore, 3, 0);
    local::shutdown();
    check(id, 0, 0, 0, 0, -4);
    println!("PASS exact unaligned C/Rust mutex observation/typed retirement ABI; actual local runtime, not title or private-broker-pipe gameplay");
}
