//! Maintainer-only actual runtime/ABI checks; never injected as game source.
#[allow(dead_code)]
#[path = "../engines/native-services/client.rs"]
mod client;
#[path = "../engines/native-services/local_bindings.rs"]
mod local;
use local::runtime::{Error, MutexState, Runtime, TaskContext, WaitObject};
use std::{
    thread,
    time::{Duration, Instant},
};
fn fresh(entries: Vec<local::runtime::Entry>) -> Runtime {
    Runtime::new(&[], entries, 4 * 1024 * 1024, 4, 65536).unwrap()
}
fn state(locked: bool, owned: bool, abandoned: bool, depth: u32) -> MutexState {
    MutexState {
        locked,
        owned_by_caller: owned,
        abandoned,
        depth,
    }
}
fn abandon(context: TaskContext, mutex: u64) -> u64 {
    context.runtime().mutex_lock(mutex, 0, true).unwrap();
    context.runtime().mutex_lock(mutex, 0, true).unwrap();
    assert_eq!(
        context.runtime().mutex_state(mutex),
        Ok(state(true, true, false, 2))
    );
    37
}
#[test]
fn observation_never_acquires_unlocks_clears_abandonment_or_grants_foreign_ownership() {
    let owner = fresh(vec![abandon]);
    let other = fresh(vec![]);
    let mutex = owner.mutex().unwrap();
    for id in [0, u64::MAX] {
        assert_eq!(owner.mutex_state(id), Err(Error::Denied));
    }
    assert_eq!(other.mutex_state(mutex), Err(Error::Denied));
    assert_eq!(owner.mutex_state(mutex), Ok(state(false, false, false, 0)));
    owner.mutex_lock(mutex, 0, true).unwrap();
    owner.mutex_lock(mutex, 0, true).unwrap();
    for _ in 0..64 {
        assert_eq!(owner.mutex_state(mutex), Ok(state(true, true, false, 2)));
    }
    let copy = owner.clone();
    thread::spawn(move || {
        assert_eq!(copy.mutex_state(mutex), Ok(state(true, false, false, 2)));
        assert_eq!(copy.mutex_lock(mutex, 0, true), Err(Error::Busy));
        assert_eq!(copy.unlock(mutex), Err(Error::Denied));
    })
    .join()
    .unwrap();
    assert_eq!(owner.mutex_state(mutex), Ok(state(true, true, false, 2)));
    owner.unlock(mutex).unwrap();
    assert_eq!(owner.mutex_state(mutex), Ok(state(true, true, false, 1)));
    assert_eq!(owner.release_sync(mutex), Err(Error::Busy));
    assert_eq!(owner.release_sync_typed(mutex, 2), Err(Error::Busy));
    assert_eq!(owner.release_sync_typed(mutex, 1), Err(Error::Invalid));
    assert_eq!(owner.mutex_state(mutex), Ok(state(true, true, false, 1)));
    owner.unlock(mutex).unwrap();
    let job = owner.spawn(0, mutex, false).unwrap();
    owner
        .wait_many(&[WaitObject { id: job, kind: 4 }], false, 1000, true)
        .unwrap();
    assert_eq!(owner.join(job), Ok(37));
    for _ in 0..64 {
        assert_eq!(owner.mutex_state(mutex), Ok(state(false, false, true, 0)));
    }
    assert_eq!(owner.mutex_lock(mutex, 0, true), Err(Error::Denied));
    let result = owner
        .wait_many(&[WaitObject { id: mutex, kind: 2 }], false, 0, true)
        .unwrap();
    assert!(result.abandoned);
    assert_eq!(owner.mutex_state(mutex), Ok(state(true, true, false, 1)));
    owner.unlock(mutex).unwrap();
    owner.release_sync(mutex).unwrap();
    assert_eq!(owner.mutex_state(mutex), Err(Error::Denied));
    let mut held = Vec::new();
    for _ in 0..128 {
        held.push(owner.mutex().unwrap());
    }
    for id in &held {
        assert_eq!(owner.mutex_state(*id), Ok(state(false, false, false, 0)));
    }
    assert_eq!(owner.mutex(), Err(Error::Busy));
    for id in held {
        owner.release_sync(id).unwrap();
    }
    let mutex = owner.mutex().unwrap();
    let copy = owner.clone();
    owner.close().unwrap();
    assert_eq!(copy.mutex_state(mutex), Err(Error::Denied));
}
fn cancelled_observer(context: TaskContext, record: u64) -> u64 {
    let (_, bytes) = context.runtime().record_snapshot(record, 16).unwrap();
    let mutex = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let ready = u64::from_le_bytes(bytes[8..].try_into().unwrap());
    context.runtime().mutex_lock(mutex, 0, true).unwrap();
    context.runtime().signal(ready, true).unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    while !context.cancelled() {
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(context.runtime().mutex_state(mutex), Err(Error::Denied));
    // Observation denial does not remove the actual owner's cleanup ability.
    assert_eq!(
        context.runtime().release_sync_typed(mutex, 2),
        Err(Error::Busy)
    );
    assert_eq!(
        context.runtime().release_sync_typed(mutex, 1),
        Err(Error::Invalid)
    );
    context.runtime().unlock(mutex).unwrap();
    context.runtime().release_sync_typed(mutex, 2).unwrap();
    51
}
#[test]
fn cancelled_owner_cannot_query_but_can_release_its_actual_depth() {
    let owner = fresh(vec![cancelled_observer]);
    let mutex = owner.mutex().unwrap();
    let ready = owner.event(true, false).unwrap();
    let record = owner
        .record_create(&[mutex.to_le_bytes(), ready.to_le_bytes()].concat())
        .unwrap();
    let job = owner.spawn(0, record, false).unwrap();
    owner.event_wait(ready, 1000, true).unwrap();
    assert_eq!(owner.mutex_state(mutex), Ok(state(true, false, false, 1)));
    owner.cancel(job).unwrap();
    owner
        .wait_many(&[WaitObject { id: job, kind: 4 }], false, 1000, true)
        .unwrap();
    assert_eq!(owner.join(job), Ok(51));
    assert_eq!(owner.mutex_state(mutex), Err(Error::Denied));
    assert_eq!(owner.release_sync_typed(mutex, 2), Err(Error::Denied));
    owner.record_release(record).unwrap();
    owner.release_sync(ready).unwrap();
}
#[test]
fn exact_mutex_query_wire_rejects_wrong_kinds_stale_handles_and_malformed_requests() {
    local::shutdown();
    assert_eq!(client::local_mutex_state(1), Err(-4));
    local::install(fresh(vec![])).unwrap();
    let bytes = local::call(21, &[], 8).unwrap();
    let id = u64::from_le_bytes(bytes[..].try_into().unwrap());
    let request = id.to_le_bytes();
    for end in 0..8 {
        assert_eq!(local::call(48, &request[..end], 16), Err(Error::Invalid));
    }
    let mut trailing = request.to_vec();
    trailing.push(0);
    assert_eq!(local::call(48, &trailing, 16), Err(Error::Invalid));
    for capacity in 0..16 {
        assert_eq!(local::call(48, &request, capacity), Err(Error::Invalid));
    }
    assert_eq!(local::call(48, &request, 16).unwrap(), [0; 16]);
    let retire = [request.as_slice(), &2u32.to_le_bytes()].concat();
    for end in 0..retire.len() {
        assert_eq!(local::call(49, &retire[..end], 0), Err(Error::Invalid));
    }
    let mut extra = retire.clone();
    extra.push(0);
    assert_eq!(local::call(49, &extra, 0), Err(Error::Invalid));
    for kind in [0, 4, u32::MAX] {
        assert_eq!(client::local_sync_release(id, kind), Err(-1));
    }
    for kind in [1, 3] {
        assert_eq!(client::local_sync_release(id, kind), Err(-1));
    }
    client::local_mutex_lock(id, 0).unwrap();
    client::local_mutex_lock(id, 0).unwrap();
    assert_eq!(client::local_sync_release(id, 2), Err(-2));
    assert_eq!(
        local::call(48, &request, 16).unwrap(),
        [
            1u32.to_le_bytes(),
            1u32.to_le_bytes(),
            0u32.to_le_bytes(),
            2u32.to_le_bytes()
        ]
        .concat()
    );
    assert_eq!(
        client::local_mutex_state(id),
        Ok(client::LocalMutexState {
            locked: true,
            owned_by_caller: true,
            abandoned: false,
            depth: 2
        })
    );
    let cross = thread::spawn(move || client::local_mutex_state(id))
        .join()
        .unwrap()
        .unwrap();
    assert_eq!(
        cross,
        client::LocalMutexState {
            locked: true,
            owned_by_caller: false,
            abandoned: false,
            depth: 2
        }
    );
    for manual in [0u32, 1] {
        let event =
            local::call(20, &[manual.to_le_bytes(), 1u32.to_le_bytes()].concat(), 8).unwrap();
        let event = u64::from_le_bytes(event.try_into().unwrap());
        assert_eq!(client::local_mutex_state(event), Err(-1));
        assert!(client::local_event_state(event).unwrap().signaled);
        assert_eq!(client::local_sync_release(event, 2), Err(-1));
        assert!(client::local_event_state(event).unwrap().signaled);
        client::local_sync_release(event, 1).unwrap();
    }
    let sem = client::local_semaphore_create(1, 2).unwrap();
    assert_eq!(client::local_mutex_state(sem), Err(-1));
    assert_eq!(client::local_semaphore_state(sem).unwrap().count, 1);
    assert_eq!(client::local_sync_release(sem, 2), Err(-1));
    assert_eq!(client::local_semaphore_state(sem).unwrap().count, 1);
    client::local_sync_release(sem, 3).unwrap();
    local::call(24, &request, 0).unwrap();
    local::call(24, &request, 0).unwrap();
    client::local_sync_release(id, 2).unwrap();
    assert_eq!(client::local_sync_release(id, 2), Err(-4));
    assert_eq!(client::local_mutex_state(id), Err(-4));
    assert_eq!(client::local_mutex_state(0), Err(-4));
    assert_eq!(client::local_mutex_state(u64::MAX), Err(-4));
    local::shutdown();
    assert_eq!(client::local_mutex_state(id), Err(-4));
}
