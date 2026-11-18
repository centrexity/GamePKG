//! Compiles independently for i686/Windows as well as normal Cargo tests.
#[path = "../engines/native-services/local_runtime.rs"]
mod runtime;
use runtime::{Error, Runtime, TaskContext, Window};
use std::{
    thread,
    time::{Duration, Instant},
};

#[test]
fn fixed_i686_window_protection_zeroing_budget_and_collision() {
    let window = Window {
        address: 0x80000000,
        bytes: 0x20000000,
    };
    assert!(matches!(
        Runtime::new(&[window], vec![], 1024, 0, 65536),
        Err(Error::Busy)
    ));
    let owner = Runtime::new(&[window], vec![], 0x21000000, 0, 65536).unwrap();
    assert!(matches!(
        Runtime::new(&[window], vec![], 0x21000000, 0, 65536),
        Err(Error::Busy)
    ));
    let (id, size) = owner.windows().unwrap()[0];
    assert_eq!(size, 0x20000000);
    assert_eq!(owner.write(id, 0, &[1]), Err(Error::Denied));
    assert_eq!(owner.commit(id, 1, 4096, 2), Err(Error::Invalid));
    assert_eq!(owner.commit(id, u64::MAX, 4096, 2), Err(Error::Invalid));
    assert_eq!(owner.commit(id, 0, 4096, 4), Err(Error::Invalid));
    owner.commit(id, 0, 8192, 2).unwrap();
    owner.write(id, 4095, &[7, 8]).unwrap();
    let mut bytes = [0; 2];
    owner.read(id, 4095, &mut bytes).unwrap();
    assert_eq!(bytes, [7, 8]);
    owner.protect(id, 4096, 4096, 1).unwrap();
    assert_eq!(owner.write(id, 4095, &[1, 2]), Err(Error::Denied));
    owner.read(id, 4095, &mut bytes).unwrap();
    assert_eq!(bytes, [7, 8]);
    owner.protect(id, 0, 4096, 0).unwrap();
    assert_eq!(owner.read(id, 0, &mut bytes), Err(Error::Denied));
    owner.protect(id, 0, 4096, 2).unwrap();
    owner.decommit(id, 0, 8192).unwrap();
    owner.commit(id, 0, 8192, 2).unwrap();
    owner.read(id, 4095, &mut bytes).unwrap();
    assert_eq!(bytes, [0, 0]);
    assert_eq!(owner.read(id, size, &mut bytes), Err(Error::Invalid));
    let other = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    assert_eq!(other.read(id, 0, &mut bytes), Err(Error::Denied));
    let clone = owner.clone();
    drop(owner);
    assert_eq!(clone.windows(), Err(Error::Denied));
    assert!(Runtime::new(&[window], vec![], 0x21000000, 0, 65536).is_ok());
}
fn task(context: TaskContext, argument: u64) -> u64 {
    assert_ne!(context.runtime().current_task().unwrap().0, 0);
    while context
        .runtime()
        .current_task()
        .is_ok_and(|(_, cancelled)| !cancelled)
    {
        let _ = context.runtime().wait(argument, 5, true);
    }
    0x1234
}
fn immediate(_: TaskContext, argument: u64) -> u64 {
    argument + 1
}

#[test]
fn typed_mutex_acquisition_rejects_events_without_consuming_and_preserves_recursive_ownership() {
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let other = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    for manual in [false, true] {
        let event = owner.event(manual, true).unwrap();
        assert_eq!(owner.mutex_lock(event, 0, true), Err(Error::Invalid));
        owner.wait(event, 0, true).unwrap();
        assert_eq!(
            owner.wait(event, 0, true),
            if manual { Ok(()) } else { Err(Error::Busy) }
        );
        owner.signal(event, false).unwrap();
        assert_eq!(owner.mutex_lock(event, 1000, true), Err(Error::Invalid));
        assert_eq!(owner.wait(event, 0, true), Err(Error::Busy));
        owner.release_sync(event).unwrap();
        assert_eq!(owner.mutex_lock(event, 0, true), Err(Error::Denied));
    }
    let mutex = owner.mutex().unwrap();
    assert_eq!(owner.mutex_lock(mutex, 0, false), Err(Error::Busy));
    assert_eq!(owner.mutex_lock(mutex, 1001, true), Err(Error::Invalid));
    assert_eq!(owner.mutex_lock(0, 0, true), Err(Error::Denied));
    assert_eq!(other.mutex_lock(mutex, 0, true), Err(Error::Denied));
    owner.mutex_lock(mutex, 0, true).unwrap();
    owner.mutex_lock(mutex, 0, true).unwrap();
    assert_eq!(owner.release_sync(mutex), Err(Error::Busy));
    let copy = owner.clone();
    assert_eq!(
        thread::spawn(move || {
            assert_eq!(copy.unlock(mutex), Err(Error::Denied));
            copy.mutex_lock(mutex, 0, true)
        })
        .join()
        .unwrap(),
        Err(Error::Busy)
    );
    owner.unlock(mutex).unwrap();
    let copy = owner.clone();
    assert_eq!(
        thread::spawn(move || copy.mutex_lock(mutex, 0, true))
            .join()
            .unwrap(),
        Err(Error::Busy)
    );
    owner.unlock(mutex).unwrap();
    let copy = owner.clone();
    thread::spawn(move || {
        copy.mutex_lock(mutex, 1000, true).unwrap();
        copy.unlock(mutex).unwrap();
    })
    .join()
    .unwrap();
    owner.release_sync(mutex).unwrap();
    let next = owner.mutex().unwrap();
    assert_ne!(next, mutex);
    assert_eq!(owner.mutex_lock(mutex, 0, true), Err(Error::Denied));
    owner.close().unwrap();
    assert_eq!(owner.mutex_lock(next, 0, true), Err(Error::Denied));
}

#[test]
fn typed_event_wait_and_state_reject_mutexes_and_preserve_event_semantics() {
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let other = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let mutex = owner.mutex().unwrap();
    assert_eq!(owner.event_state(mutex), Err(Error::Invalid));
    assert_eq!(owner.event_wait(mutex, 0, true), Err(Error::Invalid));
    let copy = owner.clone();
    thread::spawn(move || {
        copy.mutex_lock(mutex, 0, true).unwrap();
        copy.unlock(mutex).unwrap();
    })
    .join()
    .unwrap();
    for manual in [false, true] {
        let event = owner.event(manual, true).unwrap();
        let expected = runtime::EventState {
            manual,
            signaled: true,
        };
        assert_eq!(owner.event_state(event), Ok(expected));
        assert_eq!(owner.event_state(event), Ok(expected));
        assert_eq!(other.event_state(event), Err(Error::Denied));
        assert_eq!(other.event_wait(event, 0, true), Err(Error::Denied));
        assert_eq!(owner.event_wait(event, 0, false), Err(Error::Busy));
        assert_eq!(owner.event_wait(event, 1001, true), Err(Error::Invalid));
        assert_eq!(owner.event_state(event), Ok(expected));
        owner.event_wait(event, 0, true).unwrap();
        assert_eq!(
            owner.event_state(event),
            Ok(runtime::EventState {
                manual,
                signaled: manual
            })
        );
        assert_eq!(
            owner.event_wait(event, 0, true),
            if manual { Ok(()) } else { Err(Error::Busy) }
        );
        owner.signal(event, false).unwrap();
        assert_eq!(
            owner.event_state(event),
            Ok(runtime::EventState {
                manual,
                signaled: false
            })
        );
        owner.release_sync(event).unwrap();
        assert_eq!(owner.event_state(event), Err(Error::Denied));
        assert_eq!(owner.event_wait(event, 0, true), Err(Error::Denied));
    }
    let event = owner.event(false, true).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let jobs = (0..2)
        .map(|_| {
            let copy = owner.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                copy.event_wait(event, 0, true)
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let results = jobs
        .into_iter()
        .map(|job| job.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(Error::Busy))
            .count(),
        1
    );
    owner.release_sync(event).unwrap();
    owner.release_sync(mutex).unwrap();
    let mut events = Vec::new();
    for _ in 0..128 {
        events.push(owner.event(false, false).unwrap());
    }
    assert_eq!(owner.event(false, false), Err(Error::Busy));
    for event in events {
        assert_eq!(owner.event_wait(event, 0, true), Err(Error::Busy));
        owner.event_state(event).unwrap();
        owner.release_sync(event).unwrap();
    }
    assert_eq!(owner.event_state(0), Err(Error::Denied));
    owner.close().unwrap();
    assert_eq!(owner.event_state(0), Err(Error::Denied));
}

fn typed_event_cancelled(context: TaskContext, request: u64) -> u64 {
    let b = context
        .runtime()
        .mailbox_receive(request, 16, 1000)
        .unwrap();
    let event = u64::from_le_bytes(b[..8].try_into().unwrap());
    let ready = u64::from_le_bytes(b[8..].try_into().unwrap());
    context.runtime().signal(ready, true).unwrap();
    assert_eq!(
        context.runtime().event_wait(event, 1000, true),
        Err(Error::Denied)
    );
    assert_eq!(context.runtime().event_state(event), Err(Error::Denied));
    37
}
#[test]
fn typed_event_wait_wakes_on_cancel_release_and_runtime_close() {
    let owner = Runtime::new(&[], vec![typed_event_cancelled], 4 * 1024 * 1024, 1, 65536).unwrap();
    let event = owner.event(false, false).unwrap();
    let ready = owner.event(false, false).unwrap();
    let request = owner.mailbox(16, 1).unwrap();
    owner
        .mailbox_send(
            request,
            &[event.to_le_bytes(), ready.to_le_bytes()].concat(),
        )
        .unwrap();
    let task = owner.spawn(0, request, false).unwrap();
    owner.event_wait(ready, 1000, true).unwrap();
    owner.cancel(task).unwrap();
    owner.signal(event, true).unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    while !owner.task_finished(task).unwrap() {
        assert!(Instant::now() < end);
        thread::yield_now();
    }
    assert_eq!(owner.join(task), Ok(37));
    assert!(owner.event_state(event).unwrap().signaled);
    owner.release_sync(event).unwrap();
    owner.release_sync(ready).unwrap();
    owner.mailbox_release(request).unwrap();
    for close in [false, true] {
        let event = owner.event(false, false).unwrap();
        let copy = owner.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let job = thread::spawn(move || {
            tx.send(()).unwrap();
            copy.event_wait(event, 1000, true)
        });
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        if close {
            owner.close().unwrap();
        } else {
            owner.release_sync(event).unwrap();
        }
        assert_eq!(job.join().unwrap(), Err(Error::Denied));
    }
}

fn typed_cancelled_waiter(context: TaskContext, request: u64) -> u64 {
    let bytes = context
        .runtime()
        .mailbox_receive(request, 16, 1000)
        .unwrap();
    let mutex = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let reply = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    context.runtime().mailbox_send(reply, &[1]).unwrap();
    assert_eq!(
        context.runtime().mutex_lock(mutex, 1000, true),
        Err(Error::Denied)
    );
    assert_eq!(
        context.runtime().mutex_lock(mutex, 0, true),
        Err(Error::Denied)
    );
    29
}

#[test]
fn typed_mutex_waiters_cancel_or_close_without_acquiring_the_held_lock() {
    let owner = Runtime::new(&[], vec![typed_cancelled_waiter], 4 * 1024 * 1024, 1, 65536).unwrap();
    let mutex = owner.mutex().unwrap();
    owner.mutex_lock(mutex, 0, true).unwrap();
    let request = owner.mailbox(16, 1).unwrap();
    let reply = owner.mailbox(1, 1).unwrap();
    owner
        .mailbox_send(
            request,
            &[mutex.to_le_bytes(), reply.to_le_bytes()].concat(),
        )
        .unwrap();
    let task = owner.spawn(0, request, false).unwrap();
    assert_eq!(owner.mailbox_receive(reply, 1, 1000).unwrap(), [1]);
    owner.cancel(task).unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        match owner.join(task) {
            Ok(value) => {
                assert_eq!(value, 29);
                break;
            }
            Err(Error::Busy) => thread::yield_now(),
            value => panic!("{value:?}"),
        }
        assert!(Instant::now() < end);
    }
    owner.unlock(mutex).unwrap();
    owner.release_sync(mutex).unwrap();
    let mutex = owner.mutex().unwrap();
    owner.mutex_lock(mutex, 0, true).unwrap();
    let copy = owner.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let waiter = thread::spawn(move || {
        ready_tx.send(()).unwrap();
        copy.mutex_lock(mutex, 1000, true)
    });
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    owner.close().unwrap();
    assert_eq!(waiter.join().unwrap(), Err(Error::Denied));
    assert_eq!(owner.mutex_lock(mutex, 0, true), Err(Error::Denied));
}
#[test]
fn local_threads_events_mutexes_cancellation_bounds_and_owner_drop() {
    let owner = Runtime::new(&[], vec![task, immediate], 4 * 1024 * 1024, 2, 65536).unwrap();
    assert_eq!(owner.current_task().unwrap(), (0, false));
    let event = owner.event(false, false).unwrap();
    assert_eq!(owner.wait(event, 0, false), Err(Error::Busy));
    assert_eq!(owner.wait(event, 1001, true), Err(Error::Invalid));
    assert_eq!(owner.wait(event, 0, true), Err(Error::Busy));
    owner.signal(event, true).unwrap();
    owner.wait(event, 0, true).unwrap();
    assert_eq!(owner.wait(event, 0, true), Err(Error::Busy));
    let manual = owner.event(true, true).unwrap();
    owner.wait(manual, 0, true).unwrap();
    owner.wait(manual, 0, true).unwrap();
    let mutex = owner.mutex().unwrap();
    owner.wait(mutex, 0, true).unwrap();
    owner.wait(mutex, 0, true).unwrap();
    assert_eq!(owner.release_sync(mutex), Err(Error::Busy));
    let copy = owner.clone();
    thread::spawn(move || {
        assert_eq!(copy.unlock(mutex), Err(Error::Denied));
        assert_eq!(copy.wait(mutex, 0, true), Err(Error::Busy));
    })
    .join()
    .unwrap();
    owner.unlock(mutex).unwrap();
    owner.unlock(mutex).unwrap();
    owner.release_sync(mutex).unwrap();
    assert_eq!(owner.unlock(mutex), Err(Error::Denied));
    assert_eq!(owner.spawn(2, 0, false), Err(Error::Denied));
    let a = owner.spawn(0, event, false).unwrap();
    let b = owner.spawn(0, event, false).unwrap();
    assert_eq!(owner.spawn(0, event, false), Err(Error::Busy));
    assert_eq!(owner.join(a), Err(Error::Busy));
    owner.cancel(a).unwrap();
    owner.cancel(b).unwrap();
    for id in [a, b] {
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            match owner.join(id) {
                Ok(v) => {
                    // Cancellation before entry returns zero without running
                    // game code; an already-started entry returns its result.
                    assert!(matches!(v, 0 | 0x1234));
                    break;
                }
                Err(Error::Busy) => {
                    assert!(Instant::now() < end);
                    thread::yield_now()
                }
                r => panic!("{r:?}"),
            }
        }
        assert_eq!(owner.join(id), Err(Error::Denied));
    }
    let completed = owner.spawn(1, 41, true).unwrap();
    assert_eq!(owner.join(completed), Err(Error::Busy));
    assert!(owner.resume(completed).unwrap());
    assert!(!owner.resume(completed).unwrap());
    let end = Instant::now() + Duration::from_secs(2);
    loop {
        match owner.join(completed) {
            Ok(v) => {
                assert_eq!(v, 42);
                break;
            }
            Err(Error::Busy) => {
                assert!(Instant::now() < end);
                thread::yield_now()
            }
            r => panic!("{r:?}"),
        }
    }
    let other = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    assert_eq!(other.signal(event, true), Err(Error::Denied));
    for _ in 2..128 {
        owner.event(false, false).unwrap();
    }
    assert_eq!(owner.event(false, false), Err(Error::Busy));
    let copy = owner.clone();
    let wait = thread::spawn(move || copy.wait(event, 1000, true));
    thread::sleep(Duration::from_millis(10));
    let end = Instant::now();
    drop(owner);
    assert_eq!(wait.join().unwrap(), Err(Error::Denied));
    assert!(end.elapsed() < Duration::from_millis(250));
}

#[test]
fn mailboxes_preserve_fifo_and_messages_on_errors_with_scoped_nonreused_ids() {
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    for (maximum, depth) in [(0, 1), (65537, 1), (1, 0), (1, 33)] {
        assert_eq!(owner.mailbox(maximum, depth), Err(Error::Invalid));
    }
    let queue = owner.mailbox(4, 2).unwrap();
    assert_eq!(owner.mailbox_send(queue, &[0; 5]), Err(Error::Invalid));
    assert_eq!(owner.mailbox_receive(queue, 4, 0), Err(Error::Busy));
    owner.mailbox_send(queue, &[1, 2, 3, 4]).unwrap();
    owner.mailbox_send(queue, &[]).unwrap();
    assert_eq!(owner.mailbox_send(queue, &[9]), Err(Error::Busy));
    assert_eq!(owner.mailbox_receive(queue, 4, 1001), Err(Error::Invalid));
    assert_eq!(owner.mailbox_receive(queue, 65537, 0), Err(Error::Invalid));
    assert_eq!(owner.mailbox_receive(queue, 3, 0), Err(Error::Invalid));
    assert_eq!(owner.mailbox_receive(queue, 4, 0).unwrap(), [1, 2, 3, 4]);
    assert_eq!(owner.mailbox_receive(queue, 0, 0).unwrap(), []);
    let other = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    assert_eq!(other.mailbox_send(queue, &[]), Err(Error::Denied));
    assert_eq!(other.mailbox_receive(queue, 4, 0), Err(Error::Denied));
    assert_eq!(other.mailbox_release(queue), Err(Error::Denied));
    owner.mailbox_release(queue).unwrap();
    assert_eq!(owner.mailbox_release(queue), Err(Error::Denied));
    assert_eq!(owner.mailbox_send(queue, &[]), Err(Error::Denied));
    assert_ne!(owner.mailbox(4, 2).unwrap(), queue);
}

#[test]
fn mailbox_capacity_and_slot_budgets_are_reserved_and_reclaimed() {
    // Mailbox capacity is reserved before game entry even without fixed windows.
    assert!(matches!(
        Runtime::new(&[], vec![], 3 * 1024 * 1024, 0, 65536),
        Err(Error::Busy)
    ));
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    assert_eq!(owner.mailbox(65536, 17), Err(Error::Busy));
    let full = owner.mailbox(65536, 16).unwrap();
    assert_eq!(owner.mailbox(1, 1), Err(Error::Busy));
    owner.mailbox_release(full).unwrap();
    let mut queues = Vec::new();
    for _ in 0..16 {
        queues.push(owner.mailbox(1, 1).unwrap());
    }
    assert_eq!(owner.mailbox(1, 1), Err(Error::Busy));
    for queue in queues {
        owner.mailbox_release(queue).unwrap();
    }
    assert!(owner.mailbox(65536, 16).is_ok());
}

fn mailbox_waiter(context: TaskContext, request: u64) -> u64 {
    let bytes = context.runtime().mailbox_receive(request, 8, 1000).unwrap();
    let reply = u64::from_le_bytes(bytes.try_into().unwrap());
    context.runtime().mailbox_send(reply, &[42]).unwrap();
    assert_eq!(
        context.runtime().mailbox_receive(request, 8, 1000),
        Err(Error::Denied)
    );
    77
}

#[test]
fn mailbox_waiters_wake_on_send_release_close_and_task_cancel() {
    // External test threads stand in for callers of the child-local binding.
    // These calls never involve the host window's owner thread.
    let owner = Runtime::new(&[], vec![mailbox_waiter], 4 * 1024 * 1024, 1, 65536).unwrap();
    let queue = owner.mailbox(4, 2).unwrap();
    let copy = owner.clone();
    let receiver = thread::spawn(move || copy.mailbox_receive(queue, 4, 1000));
    owner.mailbox_send(queue, &[7, 8]).unwrap();
    assert_eq!(receiver.join().unwrap().unwrap(), [7, 8]);
    let copy = owner.clone();
    let receiver = thread::spawn(move || copy.mailbox_receive(queue, 4, 1000));
    owner.mailbox_release(queue).unwrap();
    assert_eq!(receiver.join().unwrap(), Err(Error::Denied));
    let queue = owner.mailbox(8, 2).unwrap();
    let reply = owner.mailbox(1, 1).unwrap();
    owner.mailbox_send(queue, &reply.to_le_bytes()).unwrap();
    let task = owner.spawn(0, queue, false).unwrap();
    assert_eq!(owner.mailbox_receive(reply, 1, 1000).unwrap(), [42]);
    thread::sleep(Duration::from_millis(10));
    let start = Instant::now();
    owner.cancel(task).unwrap();
    loop {
        match owner.join(task) {
            Ok(value) => {
                assert_eq!(value, 77);
                break;
            }
            Err(Error::Busy) => {
                assert!(start.elapsed() < Duration::from_millis(500));
                thread::yield_now();
            }
            result => panic!("{result:?}"),
        }
    }
    let copy = owner.clone();
    let receiver = thread::spawn(move || copy.mailbox_receive(queue, 4, 1000));
    drop(owner);
    assert_eq!(receiver.join().unwrap(), Err(Error::Denied));
}

#[test]
fn opaque_bulk_copy_preserves_overlap_and_validates_all_pages_before_writes() {
    let owner = Runtime::new(
        &[
            Window {
                address: 0x31000000,
                bytes: 0x10000,
            },
            Window {
                address: 0x32000000,
                bytes: 0x10000,
            },
        ],
        vec![],
        4 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let handles = owner.windows().unwrap();
    let a = handles[0].0;
    let b = handles[1].0;
    owner.commit(a, 0, 0x10000, 2).unwrap();
    owner.commit(b, 0, 8192, 2).unwrap();
    let initial = (0..16384).map(|n| (n * 31) as u8).collect::<Vec<_>>();
    owner.write(a, 0, &initial).unwrap();
    let mut expected = initial.clone();
    expected.copy_within(0..10000, 1);
    owner.copy(a, 0, a, 1, 10000).unwrap();
    let mut actual = vec![0u8; 16384];
    owner.read(a, 0, &mut actual).unwrap();
    assert_eq!(actual, expected);
    expected.copy_within(1..10001, 0);
    owner.copy(a, 1, a, 0, 10000).unwrap();
    owner.read(a, 0, &mut actual).unwrap();
    assert_eq!(actual, expected);
    owner.copy(a, 17, b, 19, 4096).unwrap();
    let mut copied = vec![0; 4096];
    owner.read(b, 19, &mut copied).unwrap();
    assert_eq!(copied, &expected[17..4113]);
    owner.write(b, 4090, &[0x55; 12]).unwrap();
    owner.protect(b, 4096, 4096, 1).unwrap();
    assert_eq!(owner.copy(a, 0, b, 4090, 12), Err(Error::Denied));
    let mut preserved = [0; 12];
    owner.read(b, 4090, &mut preserved).unwrap();
    assert_eq!(preserved, [0x55; 12]);
    assert_eq!(owner.copy(a, 65530, b, 0, 12), Err(Error::Invalid));
    assert_eq!(owner.copy(a, u64::MAX, b, 0, 2), Err(Error::Invalid));
    assert_eq!(owner.copy(a, 0, b, u64::MAX, 2), Err(Error::Invalid));
    assert_eq!(
        owner.copy(a, 0, b, 0, 64 * 1024 * 1024 + 1),
        Err(Error::Invalid)
    );
    assert_eq!(owner.copy(a, 0, b, 0, 0), Ok(()));
    assert_eq!(owner.copy(0, 0, b, 0, 0), Err(Error::Denied));
    owner.protect(a, 4096, 4096, 0).unwrap();
    assert_eq!(owner.copy(a, 4090, b, 0, 12), Err(Error::Denied));
    let mut target = [0; 12];
    owner.read(b, 0, &mut target).unwrap();
    assert_eq!(target, [0; 12]);
    owner.decommit(a, 8192, 4096).unwrap();
    assert_eq!(owner.copy(a, 8192, b, 0, 1), Err(Error::Denied));
    let clone = owner.clone();
    drop(owner);
    assert_eq!(clone.copy(a, 0, b, 0, 0), Err(Error::Denied));
}

#[test]
fn maximum_bulk_copy_uses_fixed_request_space_and_retains_read_write_transfer_caps() {
    let length = 64 * 1024 * 1024u64;
    let owner = Runtime::new(
        &[
            Window {
                address: 0x40000000,
                bytes: length,
            },
            Window {
                address: 0x45000000,
                bytes: length,
            },
        ],
        vec![],
        144 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let windows = owner.windows().unwrap();
    let a = windows[0].0;
    let b = windows[1].0;
    owner.commit(a, 0, length, 2).unwrap();
    owner.commit(b, 0, length, 2).unwrap();
    owner.write(a, 0, &[1, 2, 3, 4]).unwrap();
    owner.write(a, length - 4, &[5, 6, 7, 8]).unwrap();
    owner.copy(a, 0, b, 0, length).unwrap();
    let mut words = [0; 4];
    owner.read(b, 0, &mut words).unwrap();
    assert_eq!(words, [1, 2, 3, 4]);
    owner.read(b, length - 4, &mut words).unwrap();
    assert_eq!(words, [5, 6, 7, 8]);
    let mut oversized = vec![0; 1024 * 1024 + 1];
    assert_eq!(owner.read(a, 0, &mut oversized), Err(Error::Invalid));
    assert_eq!(owner.write(a, 0, &oversized), Err(Error::Invalid));
}

#[test]
fn opaque_records_are_atomic_copied_scoped_bounded_reclaimed_and_revocable() {
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    for bytes in [vec![], vec![0; 4097]] {
        assert_eq!(owner.record_create(&bytes), Err(Error::Invalid));
    }
    let record = owner.record_create(&0u64.to_le_bytes()).unwrap();
    assert_eq!(owner.record_snapshot(record, 7), Err(Error::Invalid));
    let mut copy = owner.record_snapshot(record, 8).unwrap().1;
    copy.fill(255);
    assert_eq!(owner.record_snapshot(record, 8).unwrap(), (0, vec![0; 8]));
    let mut jobs = Vec::new();
    for _ in 0..4 {
        let runtime = owner.clone();
        jobs.push(thread::spawn(move || {
            for _ in 0..100 {
                let end = Instant::now() + Duration::from_secs(5);
                loop {
                    let (version, data) = runtime.record_snapshot(record, 8).unwrap();
                    let value = u64::from_le_bytes(data.try_into().unwrap());
                    match runtime.record_compare_exchange(
                        record,
                        version,
                        &(value + 1).to_le_bytes(),
                    ) {
                        Ok(next) => {
                            assert_eq!(next, version + 1);
                            break;
                        }
                        Err(Error::Busy) => assert!(Instant::now() < end),
                        other => panic!("{other:?}"),
                    }
                }
            }
        }));
    }
    for job in jobs {
        job.join().unwrap();
    }
    assert_eq!(
        owner.record_snapshot(record, 8).unwrap(),
        (400, 400u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        owner.record_compare_exchange(record, 0, &[0; 8]),
        Err(Error::Busy)
    );
    assert_eq!(
        owner.record_compare_exchange(record, 400, &[0; 7]),
        Err(Error::Invalid)
    );
    assert_eq!(owner.record_snapshot(record, 8).unwrap().0, 400);
    let other = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    assert_eq!(other.record_snapshot(record, 8), Err(Error::Denied));
    assert_eq!(other.record_release(record), Err(Error::Denied));
    owner.record_release(record).unwrap();
    assert_eq!(owner.record_snapshot(record, 8), Err(Error::Denied));
    let mut full = Vec::new();
    for _ in 0..16 {
        full.push(owner.record_create(&[0; 4096]).unwrap());
    }
    assert_eq!(owner.record_create(&[0]), Err(Error::Busy));
    owner.record_release(full.pop().unwrap()).unwrap();
    let replacement = owner.record_create(&[0; 4096]).unwrap();
    full.push(replacement);
    for id in full {
        owner.record_release(id).unwrap();
    }
    let mut slots = Vec::new();
    for _ in 0..64 {
        slots.push(owner.record_create(&[1]).unwrap());
    }
    assert_eq!(owner.record_create(&[1]), Err(Error::Busy));
    let old = slots.pop().unwrap();
    owner.record_release(old).unwrap();
    let new = owner.record_create(&[2]).unwrap();
    assert_ne!(old, new);
    assert_eq!(owner.record_snapshot(old, 1), Err(Error::Denied));
    let alias = owner.clone();
    owner.close().unwrap();
    assert_eq!(alias.record_snapshot(new, 1), Err(Error::Denied));
    assert_eq!(alias.record_create(&[1]), Err(Error::Denied));
}

#[test]
fn opaque_fill_checks_every_page_and_bounds_before_any_write() {
    let owner = Runtime::new(
        &[Window {
            address: 0x33000000,
            bytes: 65536,
        }],
        vec![],
        4 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let window = owner.windows().unwrap()[0].0;
    assert_eq!(owner.fill(window, 0, 1, 7), Err(Error::Denied));
    owner.commit(window, 0, 8192, 2).unwrap();
    owner.fill(window, 0, 8192, 0x5a).unwrap();
    owner.protect(window, 4096, 4096, 1).unwrap();
    assert_eq!(owner.fill(window, 4090, 12, 1), Err(Error::Denied));
    let mut bytes = [0u8; 12];
    owner.read(window, 4090, &mut bytes).unwrap();
    assert_eq!(bytes, [0x5a; 12]);
    owner.protect(window, 4096, 4096, 0).unwrap();
    assert_eq!(owner.fill(window, 4090, 12, 2), Err(Error::Denied));
    let mut prefix = [0u8; 6];
    owner.read(window, 4090, &mut prefix).unwrap();
    assert_eq!(prefix, [0x5a; 6]);
    owner.protect(window, 4096, 4096, 2).unwrap();
    owner.decommit(window, 4096, 4096).unwrap();
    assert_eq!(owner.fill(window, 4090, 12, 3), Err(Error::Denied));
    owner.read(window, 4090, &mut prefix).unwrap();
    assert_eq!(prefix, [0x5a; 6]);
    owner.commit(window, 4096, 4096, 2).unwrap();
    for (offset, length) in [
        (65535, 2),
        (u64::MAX, 2),
        (65536, 1),
        (0, 64 * 1024 * 1024 + 1),
    ] {
        assert_eq!(
            owner.fill(window, offset, length, 0xff),
            Err(Error::Invalid)
        );
    }
    assert_eq!(owner.fill(window, 65536, 0, 0), Ok(()));
    assert_eq!(owner.fill(window, 65537, 0, 0), Err(Error::Invalid));
    assert_eq!(owner.fill(0, 0, 0, 0), Err(Error::Denied));
    let other = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    assert_eq!(other.fill(window, 0, 1, 9), Err(Error::Denied));
    owner.fill(window, 4093, 7, 0xa5).unwrap();
    owner.read(window, 4090, &mut bytes).unwrap();
    assert_eq!(
        bytes,
        [0x5a, 0x5a, 0x5a, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0, 0]
    );
    let revoked = owner.clone();
    drop(owner);
    assert_eq!(revoked.fill(window, 0, 0, 0), Err(Error::Denied));
    assert_eq!(revoked.fill(window, 0, 1, 0), Err(Error::Denied));
}

#[test]
fn maximum_fill_has_fixed_request_space_and_preserves_transfer_limits() {
    let length = 64 * 1024 * 1024u64;
    let owner = Runtime::new(
        &[Window {
            address: 0x38000000,
            bytes: length,
        }],
        vec![],
        72 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let window = owner.windows().unwrap()[0].0;
    owner.commit(window, 0, length, 2).unwrap();
    for value in [0xffu8, 0, 0x7e] {
        owner.fill(window, 0, length, value).unwrap();
        let mut chunk = vec![0; 1024 * 1024];
        for offset in (0..length).step_by(chunk.len()) {
            owner.read(window, offset, &mut chunk).unwrap();
            assert!(chunk.iter().all(|b| *b == value));
        }
    }
    let mut oversized = vec![0; 1024 * 1024 + 1];
    assert_eq!(owner.read(window, 0, &mut oversized), Err(Error::Invalid));
    assert_eq!(owner.write(window, 0, &oversized), Err(Error::Invalid));
}

#[test]
fn fill_is_atomic_against_concurrent_page_protection_changes() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let owner = Runtime::new(
        &[Window {
            address: 0x35000000,
            bytes: 65536,
        }],
        vec![],
        4 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let window = owner.windows().unwrap()[0].0;
    owner.commit(window, 0, 8192, 2).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let protection = owner.clone();
    let worker = thread::spawn(move || {
        while !stopped.load(Ordering::Acquire) {
            protection.protect(window, 4096, 4096, 1).unwrap();
            thread::yield_now();
            protection.protect(window, 4096, 4096, 2).unwrap();
        }
    });
    let mut last = 0u8;
    let mut bytes = vec![0; 8192];
    for value in 1..=255u8 {
        match owner.fill(window, 0, 8192, value) {
            Ok(()) => last = value,
            Err(Error::Denied) => {}
            result => panic!("{result:?}"),
        }
        owner.read(window, 0, &mut bytes).unwrap();
        assert!(
            bytes.iter().all(|b| *b == last),
            "partial fill after a permission race"
        );
    }
    stop.store(true, Ordering::Release);
    worker.join().unwrap();
}

#[test]
fn segmented_access_validates_all_ranges_before_mutation_and_preserves_order() {
    let owner = Runtime::new(
        &[
            Window {
                address: 0x36000000,
                bytes: 65536,
            },
            Window {
                address: 0x37000000,
                bytes: 65536,
            },
        ],
        vec![],
        4 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let windows = owner.windows().unwrap();
    let a = windows[0].0;
    let b = windows[1].0;
    owner.commit(a, 0, 8192, 2).unwrap();
    owner.commit(b, 0, 4096, 2).unwrap();
    // Emulate an unaligned access at a guest alias boundary: last two backing
    // bytes followed by the first two bytes, without masking arbitrary addresses.
    let spans = [[a, 8190, 2], [a, 0, 2], [b, 7, 2]];
    owner.write_segments(&spans, &[1, 2, 3, 4, 5, 6]).unwrap();
    let mut output = [0; 6];
    owner.read_segments(&spans, &mut output).unwrap();
    assert_eq!(output, [1, 2, 3, 4, 5, 6]);
    // Aliased destinations deliberately preserve descriptor order.
    owner
        .write_segments(&[[a, 30, 3], [a, 31, 3]], &[1, 2, 3, 4, 5, 6])
        .unwrap();
    let mut alias = [0; 4];
    owner.read(a, 30, &mut alias).unwrap();
    assert_eq!(alias, [1, 4, 5, 6]);
    owner.protect(b, 0, 4096, 1).unwrap();
    assert_eq!(owner.write_segments(&spans, &[9; 6]), Err(Error::Denied));
    owner.read_segments(&spans, &mut output).unwrap();
    assert_eq!(output, [1, 2, 3, 4, 5, 6]);
    owner.protect(b, 0, 4096, 0).unwrap();
    output.fill(0x5a);
    assert_eq!(owner.read_segments(&spans, &mut output), Err(Error::Denied));
    assert_eq!(output, [0x5a; 6]);
    for (bad, error) in [
        (vec![], Error::Invalid),
        (vec![[a, 0, 1]; 33], Error::Invalid),
        (vec![[a, 0, 0]], Error::Invalid),
        (vec![[a, 0, 1048577]], Error::Invalid),
        (vec![[a, 0, u64::MAX]], Error::Invalid),
        (vec![[a, 0, 1], [a, u64::MAX, 1]], Error::Invalid),
        (vec![[a, 0, 1], [a, 65536, 1]], Error::Invalid),
        (vec![[a, 0, 1], [a, 8192, 1]], Error::Denied),
        (vec![[a, 0, 1], [0, 0, 1]], Error::Denied),
    ] {
        let length = bad
            .iter()
            .try_fold(0usize, |n, s| n.checked_add(s[2] as usize))
            .unwrap_or(0);
        // Deliberately bounded buffer: absurd totals fail before touching it.
        let mut result = vec![0x5a; length.min(1048577)];
        assert_eq!(owner.read_segments(&bad, &mut result), Err(error));
        assert!(result.iter().all(|b| *b == 0x5a));
        assert_eq!(owner.write_segments(&bad, &result), Err(error));
        let mut first = [0];
        owner.read(a, 0, &mut first).unwrap();
        assert_eq!(first, [3]);
    }
    assert_eq!(
        owner.write_segments(&[[a, 0, 2]], &[1]),
        Err(Error::Invalid)
    );
    let mut short = [0x5a];
    assert_eq!(
        owner.read_segments(&[[a, 0, 2]], &mut short),
        Err(Error::Invalid)
    );
    assert_eq!(short, [0x5a]);
    let stranger = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    assert_eq!(
        stranger.read_segments(&[[a, 0, 1]], &mut short),
        Err(Error::Denied)
    );
    owner.decommit(a, 4096, 4096).unwrap();
    let mut denied = [0x5a; 2];
    assert_eq!(
        owner.read_segments(&[[a, 0, 1], [a, 4096, 1]], &mut denied),
        Err(Error::Denied)
    );
    assert_eq!(denied, [0x5a; 2]);
    owner.close().unwrap();
    assert_eq!(owner.write_segments(&[[a, 0, 1]], &[1]), Err(Error::Denied));
    assert_eq!(
        owner.read_segments(&[[a, 0, 1]], &mut short),
        Err(Error::Denied)
    );
}

#[test]
fn segmented_writes_are_atomic_against_concurrent_protection_changes() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let owner = Runtime::new(
        &[Window {
            // Keep outside the concurrent maximum-fill test's 0x38000000..0x3c000000.
            address: 0x3c000000,
            bytes: 65536,
        }],
        vec![],
        4 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let window = owner.windows().unwrap()[0].0;
    owner.commit(window, 0, 8192, 2).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let protection = owner.clone();
    let worker = thread::spawn(move || {
        while !stopped.load(Ordering::Acquire) {
            protection.protect(window, 4096, 4096, 1).unwrap();
            thread::yield_now();
            protection.protect(window, 4096, 4096, 2).unwrap();
        }
    });
    let spans = [[window, 0, 4096], [window, 4096, 4096]];
    let mut last = 0u8;
    let mut bytes = [0; 8192];
    for value in 1..=255u8 {
        match owner.write_segments(&spans, &[value; 8192]) {
            Ok(()) => last = value,
            Err(Error::Denied) => {}
            result => panic!("{result:?}"),
        }
        owner.read_segments(&spans, &mut bytes).unwrap();
        assert!(
            bytes.iter().all(|b| *b == last),
            "partial scatter after permission race"
        );
    }
    stop.store(true, Ordering::Release);
    worker.join().unwrap();
}

#[test]
fn segmented_transfer_caps_and_maximum_descriptor_count_are_preserved() {
    let owner = Runtime::new(
        &[Window {
            address: 0x3d000000,
            bytes: 1048576,
        }],
        vec![],
        5 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let id = owner.windows().unwrap()[0].0;
    owner.commit(id, 0, 1048576, 2).unwrap();
    let mut segments = [[0; 3]; 32];
    for (n, span) in segments.iter_mut().enumerate() {
        *span = [id, n as u64 * 32768, 32768];
    }
    let mut bytes = vec![0x5a; 1048576];
    owner.write_segments(&segments, &bytes).unwrap();
    bytes.fill(0);
    owner.read_segments(&segments, &mut bytes).unwrap();
    assert!(bytes.iter().all(|b| *b == 0x5a));
    segments[31][2] += 1;
    assert_eq!(owner.write_segments(&segments, &bytes), Err(Error::Invalid));
}

#[test]
fn task_completion_snapshots_do_not_resume_consume_or_cross_runtime_boundaries() {
    let owner = Runtime::new(&[], vec![immediate], 4 * 1024 * 1024, 1, 65536).unwrap();
    let other = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    assert_eq!(owner.task_finished(0), Err(Error::Denied));
    let job = owner.spawn(0, 41, true).unwrap();
    for _ in 0..32 {
        assert_eq!(owner.task_finished(job), Ok(false));
        assert_eq!(owner.join(job), Err(Error::Busy));
    }
    assert_eq!(other.task_finished(job), Err(Error::Denied));
    assert_eq!(owner.resume(job), Ok(true));
    let deadline = Instant::now() + Duration::from_secs(2);
    while !owner.task_finished(job).unwrap() {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    for _ in 0..32 {
        assert_eq!(owner.task_finished(job), Ok(true));
    }
    assert_eq!(owner.spawn(0, 0, false), Err(Error::Busy));
    assert_eq!(owner.join(job), Ok(42));
    assert_eq!(owner.task_finished(job), Err(Error::Denied));
    let another = owner.spawn(0, 17, true).unwrap();
    assert_ne!(another, job);
    assert_eq!(owner.task_finished(job), Err(Error::Denied));
    assert_eq!(owner.task_finished(another), Ok(false));
    owner.cancel(another).unwrap();
    while !owner.task_finished(another).unwrap() {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert_eq!(owner.task_finished(another), Ok(true));
    assert_eq!(owner.join(another), Ok(0));
    owner.close().unwrap();
    assert_eq!(owner.task_finished(another), Err(Error::Denied));
}

#[test]
fn separate_takedawn_backings_charge_all_banks_and_revoke_together() {
    let banks = [
        Window {
            address: 0x20000000,
            bytes: 64 * 1024 * 1024,
        },
        Window {
            address: 0x24000000,
            bytes: 64 * 1024 * 1024,
        },
        Window {
            address: 0x28000000,
            bytes: 1024 * 1024,
        },
    ];
    // Reserving inaccessible pages still requires their complete budget.
    assert!(matches!(
        Runtime::new(&banks, vec![], 128 * 1024 * 1024, 0, 65536),
        Err(Error::Busy)
    ));
    // A later-bank collision must release the earlier reservation and keep
    // the independently owned conflicting bank intact.
    let blocker = Runtime::new(&banks[1..2], vec![], 128 * 1024 * 1024, 0, 65536).unwrap();
    let blocked = blocker.windows().unwrap()[0].0;
    blocker.commit(blocked, 0, 4096, 2).unwrap();
    blocker.write(blocked, 0, &[0x7b]).unwrap();
    assert!(matches!(
        Runtime::new(&banks, vec![], 256 * 1024 * 1024, 0, 65536),
        Err(Error::Busy)
    ));
    let earlier = Runtime::new(&banks[..1], vec![], 128 * 1024 * 1024, 0, 65536).unwrap();
    let mut sentinel = [0];
    blocker.read(blocked, 0, &mut sentinel).unwrap();
    assert_eq!(sentinel, [0x7b]);
    drop(earlier);
    drop(blocker);
    let owner = Runtime::new(&banks, vec![], 256 * 1024 * 1024, 0, 65536).unwrap();
    let windows = owner.windows().unwrap();
    assert_eq!(windows.len(), 3);
    assert!(matches!(
        Runtime::new(&banks, vec![], 256 * 1024 * 1024, 0, 65536),
        Err(Error::Busy)
    ));
    for (index, &(id, length)) in windows.iter().enumerate() {
        assert_eq!(length, banks[index].bytes);
        assert_eq!(owner.page_states(id, 0, 256).unwrap(), vec![0; 256]);
        assert_eq!(owner.read(id, 0, &mut [0]), Err(Error::Denied));
        owner.commit(id, 0, length, 2).unwrap();
        owner.fill(id, 0, length, index as u8 + 1).unwrap();
        assert_eq!(owner.page_states(id, 0, 256).unwrap(), vec![3; 256]);
    }
    for (index, &(id, length)) in windows.iter().enumerate() {
        for offset in [0, length - 4] {
            let mut bytes = [0; 4];
            owner.read(id, offset, &mut bytes).unwrap();
            assert_eq!(bytes, [index as u8 + 1; 4]);
        }
    }
    assert_eq!(owner.close().unwrap(), 0);
    for &(id, _) in &windows {
        assert_eq!(owner.read(id, 0, &mut [0]), Err(Error::Denied));
        assert_eq!(owner.page_states(id, 0, 256), Err(Error::Denied));
    }
    assert!(Runtime::new(&banks, vec![], 256 * 1024 * 1024, 0, 65536).is_ok());
}

#[test]
fn page_state_snapshots_are_copied_bounded_and_revoked_without_changing_memory() {
    let owner = Runtime::new(
        &[Window {
            address: 0x3e000000,
            bytes: 65536,
        }],
        vec![],
        4 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let id = owner.windows().unwrap()[0].0;
    assert_eq!(owner.page_states(id, 0, 16).unwrap(), vec![0; 16]);
    owner.commit(id, 0, 4096, 1).unwrap();
    owner.commit(id, 4096, 8192, 2).unwrap();
    owner.write(id, 4096, &[0xa5; 8]).unwrap();
    owner.protect(id, 8192, 4096, 0).unwrap();
    assert_eq!(owner.page_states(id, 0, 4).unwrap(), [2, 3, 1, 0]);
    let mut copied = owner.page_states(id, 0, 4).unwrap();
    copied.fill(3);
    assert_eq!(owner.page_states(id, 0, 4).unwrap(), [2, 3, 1, 0]);
    for (offset, count) in [
        (0, 0),
        (0, 257),
        (0, u32::MAX),
        (1, 1),
        (u64::MAX - 4095, 1),
        (65536, 1),
        (4096, 16),
        (0, 256),
    ] {
        assert_eq!(owner.page_states(id, offset, count), Err(Error::Invalid));
    }
    assert_eq!(owner.page_states(id, 61440, 1).unwrap(), vec![0]);
    for bad in [0, u64::MAX, id.wrapping_add(1)] {
        assert_eq!(owner.page_states(bad, 0, 1), Err(Error::Denied));
    }
    let other = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    assert_eq!(other.page_states(id, 0, 1), Err(Error::Denied));
    let mut bytes = [0u8; 8];
    owner.read(id, 4096, &mut bytes).unwrap();
    assert_eq!(bytes, [0xa5; 8]);
    assert_eq!(owner.write(id, 0, &[1]), Err(Error::Denied));
    assert_eq!(owner.read(id, 8192, &mut bytes), Err(Error::Denied));
    assert_eq!(owner.read(id, 12288, &mut bytes), Err(Error::Denied));
    owner.protect(id, 8192, 4096, 1).unwrap();
    let snapshot = owner.page_states(id, 0, 4).unwrap();
    owner.decommit(id, 8192, 4096).unwrap();
    assert_eq!(snapshot, [2, 3, 2, 0]);
    assert_eq!(owner.page_states(id, 0, 4).unwrap(), [2, 3, 0, 0]);
    owner.commit(id, 8192, 4096, 2).unwrap();
    assert_eq!(owner.page_states(id, 0, 4).unwrap(), [2, 3, 3, 0]);
    let clone = owner.clone();
    drop(owner);
    assert_eq!(clone.page_states(id, 0, 4), Err(Error::Denied));
}

#[test]
fn page_state_snapshot_is_atomic_with_complete_range_protection_updates() {
    let owner = Runtime::new(
        &[Window {
            address: 0x3f000000,
            bytes: 65536,
        }],
        vec![],
        4 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let id = owner.windows().unwrap()[0].0;
    owner.commit(id, 0, 65536, 2).unwrap();
    let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
    let ready = gate.clone();
    let writer = owner.clone();
    let worker = thread::spawn(move || {
        ready.wait();
        for index in 0..1000 {
            writer.protect(id, 0, 65536, index % 3).unwrap();
            thread::yield_now();
        }
    });
    gate.wait();
    for _ in 0..1000 {
        let states = owner.page_states(id, 0, 16).unwrap();
        assert_eq!(states.len(), 16);
        assert!((1..=3).contains(&states[0]));
        assert!(states.iter().all(|state| *state == states[0]));
        thread::yield_now();
    }
    worker.join().unwrap();
}

#[test]
fn performance_samples_preserve_units_and_stop_with_the_local_owner() {
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let first = owner.performance_sample().unwrap();
    assert!(first[0] <= i64::MAX as u64);
    assert!(first[1] > 0 && first[1] <= i64::MAX as u64);
    for _ in 0..100 {
        let sample = owner.performance_sample().unwrap();
        assert_eq!(sample[1], first[1]);
        assert!(sample[0] >= first[0] && sample[2] >= first[2]);
        #[cfg(target_os = "linux")]
        {
            assert_eq!(sample[1], 1_000_000_000);
            assert_eq!(sample[2], sample[0] / 1_000_000);
        }
    }
    let observer = owner.clone();
    drop(owner);
    assert_eq!(observer.performance_sample(), Err(Error::Denied));
}

#[test]
fn local_clock_is_runtime_owned_and_independent_of_remote_timing() {
    let first = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let second = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let state = first.clock_snapshot().unwrap();
    assert_eq!(state.generation, 0);
    assert_eq!(first.clock_wake(), Ok(1));
    assert_eq!(second.clock_snapshot().unwrap().generation, 0);
    let wake = first.clock_wait(0, state.frequency, 0, true).unwrap();
    assert_eq!(wake.reason, 1);
    assert_eq!(wake.snapshot.generation, 1);
    assert_eq!(
        first.clock_wait(0, state.frequency, 1, false),
        Err(Error::Busy)
    );
    let observer = first.clone();
    drop(first);
    assert_eq!(observer.clock_snapshot(), Err(Error::Denied));
    assert_eq!(observer.clock_wake(), Err(Error::Denied));
    assert_eq!(
        observer.clock_wait(0, state.frequency, 1, true),
        Err(Error::Denied)
    );
}

#[test]
fn memory_cas_checks_the_whole_range_and_is_atomic_between_workers() {
    let owner = Runtime::new(
        &[Window {
            address: 0x2a000000,
            bytes: 65536,
        }],
        vec![],
        8 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let id = owner.windows().unwrap()[0].0;
    assert_eq!(
        owner.compare_exchange_memory(id, 0, &[0], &[1]),
        Err(Error::Denied)
    );
    owner.commit(id, 0, 65536, 2).unwrap();
    for (offset, a, b) in [
        (0, vec![], vec![]),
        (0, vec![0], vec![1, 2]),
        (0, vec![0; 65537], vec![1; 65537]),
        (u64::MAX, vec![0], vec![1]),
        (65536, vec![0], vec![1]),
    ] {
        assert_eq!(
            owner.compare_exchange_memory(id, offset, &a, &b),
            Err(Error::Invalid)
        );
    }
    assert_eq!(
        owner.compare_exchange_memory(id | 0x100000000, 0, &[0], &[1]),
        Err(Error::Denied)
    );
    owner.protect(id, 4096, 4096, 1).unwrap();
    assert_eq!(
        owner.compare_exchange_memory(id, 4094, &[0; 4], &[1; 4]),
        Err(Error::Denied)
    );
    assert_eq!(
        owner.compare_exchange_memory(id, 4094, &[9; 4], &[1; 4]),
        Err(Error::Denied)
    );
    let mut unchanged = [9u8; 8];
    owner.read(id, 4092, &mut unchanged).unwrap();
    assert_eq!(unchanged, [0; 8]);
    owner.protect(id, 4096, 4096, 2).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(5));
    let mut workers = vec![];
    for _ in 0..4 {
        let copy = owner.clone();
        let gate = barrier.clone();
        workers.push(thread::spawn(move || {
            gate.wait();
            copy.compare_exchange_memory(id, 0, &[0; 8], &1u64.to_le_bytes())
                .unwrap()
        }));
    }
    barrier.wait();
    assert_eq!(
        workers
            .into_iter()
            .map(|t| u32::from(t.join().unwrap()))
            .sum::<u32>(),
        1
    );
    owner.fill(id, 0, 65536, 0).unwrap();
    let mut workers = vec![];
    for _ in 0..4 {
        let copy = owner.clone();
        workers.push(thread::spawn(move || {
            for _ in 0..128 {
                loop {
                    let mut bytes = [0u8; 8];
                    copy.read(id, 0, &mut bytes).unwrap();
                    let n = u64::from_le_bytes(bytes);
                    if copy
                        .compare_exchange_memory(id, 0, &bytes, &(n + 1).to_le_bytes())
                        .unwrap()
                    {
                        break;
                    }
                }
            }
        }));
    }
    for worker in workers {
        worker.join().unwrap();
    }
    let mut result = [0u8; 8];
    owner.read(id, 0, &mut result).unwrap();
    assert_eq!(u64::from_le_bytes(result), 512);
    owner.fill(id, 0, 65536, 0).unwrap();
    let expected = vec![0; 65536];
    let replacement = vec![0x5a; 65536];
    assert_eq!(
        owner.compare_exchange_memory(id, 0, &expected, &replacement),
        Ok(true)
    );
    assert_eq!(
        owner.compare_exchange_memory(id, 0, &expected, &expected),
        Ok(false)
    );
    let mut observed = vec![0; 65536];
    owner.read(id, 0, &mut observed).unwrap();
    assert_eq!(observed, replacement);
    owner.decommit(id, 4096, 4096).unwrap();
    assert_eq!(
        owner.compare_exchange_memory(id, 4094, &[0x5a; 4], &[0; 4]),
        Err(Error::Denied)
    );
    assert_eq!(owner.record_create(&[0; 4097]), Err(Error::Invalid));
    owner.close().unwrap();
    assert_eq!(
        owner.compare_exchange_memory(id, 0, &[0x5a], &[0]),
        Err(Error::Denied)
    );
}

#[test]
fn counted_semaphores_are_typed_bounded_atomic_and_share_the_sync_quota() {
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let other = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    for (count, maximum) in [(0, 0), (1, 0), (3, 2), (0, 0x80000000), (0, u32::MAX)] {
        assert_eq!(owner.semaphore(count, maximum), Err(Error::Invalid));
    }
    let semaphore = owner.semaphore(1, 2).unwrap();
    let state = runtime::SemaphoreState {
        count: 1,
        maximum: 2,
    };
    assert_eq!(owner.semaphore_state(semaphore), Ok(state));
    assert_eq!(owner.semaphore_state(semaphore), Ok(state));
    assert_eq!(other.semaphore_state(semaphore), Err(Error::Denied));
    assert_eq!(other.semaphore_wait(semaphore, 0, true), Err(Error::Denied));
    assert_eq!(other.semaphore_release(semaphore, 1), Err(Error::Denied));
    assert_eq!(owner.semaphore_wait(semaphore, 0, false), Err(Error::Busy));
    assert_eq!(
        owner.semaphore_wait(semaphore, 1001, true),
        Err(Error::Invalid)
    );
    assert_eq!(owner.wait(semaphore, 0, true), Err(Error::Invalid));
    assert_eq!(owner.mutex_lock(semaphore, 0, true), Err(Error::Invalid));
    assert_eq!(owner.event_wait(semaphore, 0, true), Err(Error::Invalid));
    assert_eq!(owner.event_state(semaphore), Err(Error::Invalid));
    assert_eq!(owner.signal(semaphore, false), Err(Error::Invalid));
    assert_eq!(owner.signal(semaphore, true), Err(Error::Invalid));
    assert_eq!(owner.unlock(semaphore), Err(Error::Invalid));
    for permits in [0, 2, 0x80000000, u32::MAX] {
        assert_eq!(
            owner.semaphore_release(semaphore, permits),
            Err(Error::Invalid)
        );
        assert_eq!(owner.semaphore_state(semaphore), Ok(state));
    }
    assert_eq!(owner.semaphore_release(semaphore, 1), Ok(1));
    owner.semaphore_wait(semaphore, 0, true).unwrap();
    owner.semaphore_wait(semaphore, 0, true).unwrap();
    assert_eq!(owner.semaphore_wait(semaphore, 1, true), Err(Error::Busy));
    assert_eq!(owner.semaphore_state(semaphore).unwrap().count, 0);
    owner.release_sync(semaphore).unwrap();
    assert_eq!(owner.semaphore_state(semaphore), Err(Error::Denied));
    assert_eq!(owner.semaphore_release(semaphore, 1), Err(Error::Denied));
    assert_eq!(owner.semaphore_wait(semaphore, 0, true), Err(Error::Denied));
    let event = owner.event(false, true).unwrap();
    let mutex = owner.mutex().unwrap();
    for object in [event, mutex] {
        assert_eq!(owner.semaphore_state(object), Err(Error::Invalid));
        assert_eq!(owner.semaphore_release(object, 1), Err(Error::Invalid));
        assert_eq!(owner.semaphore_wait(object, 0, true), Err(Error::Invalid));
    }
    assert!(owner.event_state(event).unwrap().signaled);
    let copy = owner.clone();
    thread::spawn(move || {
        copy.mutex_lock(mutex, 0, true).unwrap();
        copy.unlock(mutex).unwrap();
    })
    .join()
    .unwrap();
    let maximum = owner.semaphore(i32::MAX as u32, i32::MAX as u32).unwrap();
    assert_eq!(owner.semaphore_release(maximum, 1), Err(Error::Invalid));
    owner.semaphore_wait(maximum, 0, true).unwrap();
    assert_eq!(owner.semaphore_release(maximum, 1), Ok(i32::MAX as u32 - 1));
    for object in [event, mutex, maximum] {
        owner.release_sync(object).unwrap();
    }
    let mut objects = Vec::new();
    for index in 0..128 {
        objects.push(match index % 3 {
            0 => owner.event(false, false).unwrap(),
            1 => owner.mutex().unwrap(),
            _ => owner.semaphore(0, 1).unwrap(),
        });
    }
    assert_eq!(owner.semaphore(0, 1), Err(Error::Busy));
    assert_eq!(owner.event(false, false), Err(Error::Busy));
    assert_eq!(owner.mutex(), Err(Error::Busy));
    for object in objects {
        owner.release_sync(object).unwrap();
    }
    let next = owner.semaphore(0, 1).unwrap();
    assert_ne!(next, semaphore);
    assert_eq!(owner.semaphore_state(semaphore), Err(Error::Denied));
    owner.close().unwrap();
    assert_eq!(owner.semaphore_state(next), Err(Error::Denied));
    assert_eq!(owner.semaphore_release(next, 1), Err(Error::Denied));
    assert_eq!(owner.semaphore_wait(next, 0, true), Err(Error::Denied));
    assert_eq!(owner.semaphore(0, 1), Err(Error::Denied));
}

#[test]
fn counted_semaphore_concurrent_release_and_acquisition_are_linearizable() {
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let semaphore = owner.semaphore(0, 16).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(17));
    let jobs = (0..16)
        .map(|_| {
            let copy = owner.clone();
            let gate = barrier.clone();
            thread::spawn(move || {
                gate.wait();
                copy.semaphore_release(semaphore, 1).unwrap()
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let mut previous = jobs
        .into_iter()
        .map(|job| job.join().unwrap())
        .collect::<Vec<_>>();
    previous.sort_unstable();
    assert_eq!(previous, (0..16).collect::<Vec<_>>());
    assert_eq!(owner.semaphore_state(semaphore).unwrap().count, 16);
    for _ in 0..11 {
        owner.semaphore_wait(semaphore, 0, true).unwrap();
    }
    let jobs = (0..16)
        .map(|_| {
            let copy = owner.clone();
            let gate = barrier.clone();
            thread::spawn(move || {
                gate.wait();
                copy.semaphore_wait(semaphore, 0, true)
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let outcomes = jobs
        .into_iter()
        .map(|job| job.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 5);
    assert_eq!(
        outcomes.iter().filter(|r| **r == Err(Error::Busy)).count(),
        11
    );
    assert_eq!(owner.semaphore_state(semaphore).unwrap().count, 0);
    owner.release_sync(semaphore).unwrap();
}

fn semaphore_cancelled(context: TaskContext, request: u64) -> u64 {
    let b = context
        .runtime()
        .mailbox_receive(request, 16, 1000)
        .unwrap();
    let semaphore = u64::from_le_bytes(b[..8].try_into().unwrap());
    let ready = u64::from_le_bytes(b[8..].try_into().unwrap());
    context.runtime().signal(ready, true).unwrap();
    assert_eq!(
        context.runtime().semaphore_wait(semaphore, 1000, true),
        Err(Error::Denied)
    );
    assert_eq!(
        context.runtime().semaphore_state(semaphore),
        Err(Error::Denied)
    );
    assert_eq!(
        context.runtime().semaphore_release(semaphore, 1),
        Err(Error::Denied)
    );
    assert_eq!(context.runtime().semaphore(0, 1), Err(Error::Denied));
    47
}
#[test]
fn counted_semaphore_wait_wakes_on_release_cancel_close_and_revocation() {
    let owner = Runtime::new(&[], vec![semaphore_cancelled], 4 * 1024 * 1024, 1, 65536).unwrap();
    let semaphore = owner.semaphore(0, 1).unwrap();
    let ready = owner.event(false, false).unwrap();
    let request = owner.mailbox(16, 1).unwrap();
    owner
        .mailbox_send(
            request,
            &[semaphore.to_le_bytes(), ready.to_le_bytes()].concat(),
        )
        .unwrap();
    let task = owner.spawn(0, request, false).unwrap();
    owner.event_wait(ready, 1000, true).unwrap();
    owner.cancel(task).unwrap();
    owner.semaphore_release(semaphore, 1).unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    while !owner.task_finished(task).unwrap() {
        assert!(Instant::now() < end);
        thread::yield_now();
    }
    assert_eq!(owner.join(task), Ok(47));
    assert_eq!(owner.semaphore_state(semaphore).unwrap().count, 1);
    owner.semaphore_wait(semaphore, 0, true).unwrap();
    let copy = owner.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let job = thread::spawn(move || {
        tx.send(()).unwrap();
        copy.semaphore_wait(semaphore, 1000, true)
    });
    rx.recv_timeout(Duration::from_secs(3)).unwrap();
    owner.semaphore_release(semaphore, 1).unwrap();
    assert_eq!(job.join().unwrap(), Ok(()));
    assert_eq!(owner.semaphore_state(semaphore).unwrap().count, 0);
    owner.release_sync(semaphore).unwrap();
    owner.release_sync(ready).unwrap();
    owner.mailbox_release(request).unwrap();
    for close in [false, true] {
        let semaphore = owner.semaphore(0, 1).unwrap();
        let copy = owner.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let job = thread::spawn(move || {
            tx.send(()).unwrap();
            copy.semaphore_wait(semaphore, 1000, true)
        });
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        if close {
            owner.close().unwrap();
        } else {
            owner.release_sync(semaphore).unwrap();
        }
        assert_eq!(job.join().unwrap(), Err(Error::Denied));
    }
}

fn wait_object(id: u64, kind: u32) -> runtime::WaitObject {
    runtime::WaitObject { id, kind }
}
#[test]
fn wait_sets_commit_all_mixed_states_atomically_and_keep_manual_events() {
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let event = owner.event(false, true).unwrap();
    let manual = owner.event(true, true).unwrap();
    let semaphore = owner.semaphore(1, 2).unwrap();
    let mutex = owner.mutex().unwrap();
    let objects = [
        wait_object(semaphore, 3),
        wait_object(event, 1),
        wait_object(mutex, 2),
        wait_object(manual, 1),
    ];
    owner.signal(event, false).unwrap();
    assert_eq!(owner.wait_many(&objects, true, 0, true), Err(Error::Busy));
    assert_eq!(owner.semaphore_state(semaphore).unwrap().count, 1);
    let copy = owner.clone();
    thread::spawn(move || {
        copy.mutex_lock(mutex, 0, true).unwrap();
        copy.unlock(mutex).unwrap();
    })
    .join()
    .unwrap();
    owner.signal(event, true).unwrap();
    assert_eq!(owner.wait_many(&objects, true, 0, false), Err(Error::Busy));
    assert_eq!(
        owner.wait_many(&objects, true, 0, true),
        Ok(runtime::WaitResult {
            index: 0,
            abandoned: false
        })
    );
    assert!(!owner.event_state(event).unwrap().signaled);
    assert!(owner.event_state(manual).unwrap().signaled);
    assert_eq!(owner.semaphore_state(semaphore).unwrap().count, 0);
    owner.mutex_lock(mutex, 0, true).unwrap();
    owner.unlock(mutex).unwrap();
    assert_eq!(owner.release_sync(mutex), Err(Error::Busy));
    owner.unlock(mutex).unwrap();
    for id in [event, manual, semaphore, mutex] {
        owner.release_sync(id).unwrap();
    }
}
#[test]
fn wait_any_uses_caller_order_and_invalid_later_entries_cannot_consume() {
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let first = owner.semaphore(2, 2).unwrap();
    let second = owner.event(false, true).unwrap();
    let objects = [wait_object(second, 1), wait_object(first, 3)];
    assert_eq!(owner.wait_many(&objects, false, 0, true).unwrap().index, 0);
    assert_eq!(owner.semaphore_state(first).unwrap().count, 2);
    assert_eq!(owner.wait_many(&objects, false, 0, true).unwrap().index, 1);
    assert_eq!(owner.semaphore_state(first).unwrap().count, 1);
    owner.signal(second, true).unwrap();
    for bad in [
        wait_object(first, 2),
        wait_object(u64::MAX, 3),
        wait_object(first, 4),
    ] {
        assert!(owner.wait_many(&[objects[0], bad], false, 0, true).is_err());
        assert!(owner.event_state(second).unwrap().signaled);
        assert_eq!(owner.semaphore_state(first).unwrap().count, 1);
    }
    assert_eq!(owner.wait_many(&[], false, 0, true), Err(Error::Invalid));
    assert_eq!(
        owner.wait_many(&[objects[0]; 65], true, 0, true),
        Err(Error::Invalid)
    );
    for all in [false, true] {
        assert_eq!(
            owner.wait_many(&[objects[0]; 2], all, 0, true),
            Err(Error::Invalid)
        );
        assert_eq!(
            owner.wait_many(&objects, all, 1001, true),
            Err(Error::Invalid)
        );
        for kind in [0, 5, u32::MAX] {
            assert_eq!(
                owner.wait_many(&[wait_object(second, kind)], all, 0, true),
                Err(Error::Invalid)
            );
        }
    }
    let other = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    assert_eq!(other.wait_many(&objects, true, 0, true), Err(Error::Denied));
    owner.release_sync(first).unwrap();
    assert_eq!(owner.wait_many(&objects, true, 0, true), Err(Error::Denied));
    assert!(owner.event_state(second).unwrap().signaled);
    owner.release_sync(second).unwrap();
}
#[test]
fn reversed_concurrent_wait_all_sets_do_not_deadlock_or_partially_consume() {
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let first = owner.semaphore(5, 5).unwrap();
    let second = owner.semaphore(5, 5).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(17));
    let mut jobs = Vec::new();
    for index in 0..16 {
        let copy = owner.clone();
        let gate = barrier.clone();
        jobs.push(thread::spawn(move || {
            let mut objects = [wait_object(first, 3), wait_object(second, 3)];
            if index % 2 != 0 {
                objects.reverse();
            }
            gate.wait();
            copy.wait_many(&objects, true, 0, true)
        }));
    }
    barrier.wait();
    let results = jobs
        .into_iter()
        .map(|job| job.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 5);
    assert_eq!(
        results.iter().filter(|r| **r == Err(Error::Busy)).count(),
        11
    );
    assert_eq!(owner.semaphore_state(first).unwrap().count, 0);
    assert_eq!(owner.semaphore_state(second).unwrap().count, 0);
    owner.release_sync(first).unwrap();
    owner.release_sync(second).unwrap();
}
#[test]
fn wait_sets_wake_on_signal_release_and_revocation_and_accept_64_distinct_ids() {
    for action in 0..4 {
        let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
        let event = owner.event(false, true).unwrap();
        let semaphore = owner.semaphore(0, 1).unwrap();
        let copy = owner.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let job = thread::spawn(move || {
            ready_tx.send(()).unwrap();
            tx.send(copy.wait_many(
                &[wait_object(event, 1), wait_object(semaphore, 3)],
                true,
                1000,
                true,
            ))
            .unwrap();
        });
        ready_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        thread::sleep(Duration::from_millis(20));
        match action {
            0 => {
                owner.semaphore_release(semaphore, 1).unwrap();
            }
            1 => {
                owner.release_sync(semaphore).unwrap();
            }
            2 => {
                owner.close().unwrap();
            }
            _ => {
                owner.signal(event, false).unwrap();
                owner.semaphore_release(semaphore, 1).unwrap();
                owner.signal(event, true).unwrap();
            }
        }
        let expected = if action == 0 || action == 3 {
            Ok(runtime::WaitResult {
                index: 0,
                abandoned: false,
            })
        } else {
            Err(Error::Denied)
        };
        assert_eq!(
            rx.recv_timeout(Duration::from_millis(700)).unwrap(),
            expected
        );
        job.join().unwrap();
        if action != 2 {
            if action == 1 {
                assert!(owner.event_state(event).unwrap().signaled);
            } else {
                owner.release_sync(semaphore).unwrap();
            }
            owner.release_sync(event).unwrap();
        }
    }
    let owner = Runtime::new(&[], vec![], 4 * 1024 * 1024, 0, 65536).unwrap();
    let mut objects = Vec::new();
    for index in 0..64 {
        objects.push(wait_object(owner.event(index % 2 == 0, true).unwrap(), 1));
    }
    objects.reverse();
    assert_eq!(owner.wait_many(&objects, true, 0, true).unwrap().index, 0);
    for object in &objects {
        assert_eq!(
            owner.event_state(object.id).unwrap().signaled,
            owner.event_state(object.id).unwrap().manual
        );
    }
    for object in objects {
        owner.release_sync(object.id).unwrap();
    }
}
fn abandoned_mutex_task(context: TaskContext, mutex: u64) -> u64 {
    context.runtime().mutex_lock(mutex, 0, true).unwrap();
    context.runtime().mutex_lock(mutex, 0, true).unwrap();
    88 // Deliberate normal exit with recursive ownership still held.
}
#[test]
fn task_completion_wait_does_not_join_and_abandoned_mutexes_are_explicit() {
    let owner = Runtime::new(&[], vec![abandoned_mutex_task], 4 * 1024 * 1024, 2, 65536).unwrap();
    let mutex = owner.mutex().unwrap();
    let semaphore = owner.semaphore(0, 1).unwrap();
    let task = owner.spawn(0, mutex, true).unwrap();
    assert_eq!(
        owner.wait_many(&[wait_object(task, 4)], true, 0, true),
        Err(Error::Busy)
    );
    owner.resume(task).unwrap();
    assert_eq!(
        owner
            .wait_many(&[wait_object(task, 4)], false, 1000, true)
            .unwrap()
            .index,
        0
    );
    assert!(owner.task_finished(task).unwrap());
    assert_eq!(
        owner
            .wait_many(&[wait_object(task, 4)], true, 0, true)
            .unwrap()
            .index,
        0
    );
    assert_eq!(owner.mutex_lock(mutex, 0, true), Err(Error::Denied));
    let objects = [
        wait_object(semaphore, 3),
        wait_object(mutex, 2),
        wait_object(task, 4),
    ];
    assert_eq!(owner.wait_many(&objects, true, 0, true), Err(Error::Busy));
    owner.semaphore_release(semaphore, 1).unwrap();
    assert_eq!(
        owner.wait_many(&objects, true, 0, true),
        Ok(runtime::WaitResult {
            index: 1,
            abandoned: true
        })
    );
    assert_eq!(
        owner.wait_many(&[wait_object(mutex, 2)], false, 0, true),
        Ok(runtime::WaitResult {
            index: 0,
            abandoned: false
        })
    );
    owner.unlock(mutex).unwrap();
    owner.unlock(mutex).unwrap();
    assert_eq!(owner.join(task), Ok(88));
    owner.semaphore_release(semaphore, 1).unwrap();
    assert_eq!(owner.wait_many(&objects, true, 0, true), Err(Error::Denied));
    assert_eq!(owner.semaphore_state(semaphore).unwrap().count, 1);
    owner.release_sync(mutex).unwrap();
    owner.release_sync(semaphore).unwrap();
}
fn waitset_cancel_task(context: TaskContext, request: u64) -> u64 {
    let bytes = context
        .runtime()
        .mailbox_receive(request, 16, 1000)
        .unwrap();
    let semaphore = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let ready = u64::from_le_bytes(bytes[8..].try_into().unwrap());
    context.runtime().signal(ready, true).unwrap();
    assert_eq!(
        context
            .runtime()
            .wait_many(&[wait_object(semaphore, 3)], false, 1000, true),
        Err(Error::Denied)
    );
    73
}
#[test]
fn cancelling_a_reviewed_waitset_worker_wakes_without_consuming_a_later_permit() {
    let owner = Runtime::new(&[], vec![waitset_cancel_task], 4 * 1024 * 1024, 1, 65536).unwrap();
    let semaphore = owner.semaphore(0, 1).unwrap();
    let ready = owner.event(false, false).unwrap();
    let mailbox = owner.mailbox(16, 1).unwrap();
    owner
        .mailbox_send(
            mailbox,
            &[semaphore.to_le_bytes(), ready.to_le_bytes()].concat(),
        )
        .unwrap();
    let task = owner.spawn(0, mailbox, false).unwrap();
    owner.event_wait(ready, 1000, true).unwrap();
    thread::sleep(Duration::from_millis(20));
    owner.cancel(task).unwrap();
    owner.semaphore_release(semaphore, 1).unwrap();
    let end = Instant::now() + Duration::from_millis(700);
    while !owner.task_finished(task).unwrap() {
        assert!(Instant::now() < end);
        thread::yield_now();
    }
    assert_eq!(owner.join(task), Ok(73));
    assert_eq!(owner.semaphore_state(semaphore).unwrap().count, 1);
    owner.release_sync(semaphore).unwrap();
    owner.release_sync(ready).unwrap();
    owner.mailbox_release(mailbox).unwrap();
}

#[test]
fn joining_a_task_revokes_a_pending_mixed_wait_without_consuming_ready_objects() {
    let owner = Runtime::new(&[], vec![immediate], 4 * 1024 * 1024, 1, 65536).unwrap();
    let task = owner.spawn(0, 41, false).unwrap();
    owner
        .wait_many(&[wait_object(task, 4)], false, 1000, true)
        .unwrap();
    let event = owner.event(false, false).unwrap();
    let semaphore = owner.semaphore(1, 1).unwrap();
    let copy = owner.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let job = thread::spawn(move || {
        tx.send(copy.wait_many(
            &[
                wait_object(semaphore, 3),
                wait_object(event, 1),
                wait_object(task, 4),
            ],
            true,
            1000,
            true,
        ))
        .unwrap()
    });
    thread::sleep(Duration::from_millis(20));
    assert_eq!(owner.join(task), Ok(42));
    owner.signal(event, true).unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_millis(700)).unwrap(),
        Err(Error::Denied)
    );
    job.join().unwrap();
    assert!(owner.event_state(event).unwrap().signaled);
    assert_eq!(owner.semaphore_state(semaphore).unwrap().count, 1);
    owner.release_sync(event).unwrap();
    owner.release_sync(semaphore).unwrap();
}
fn abandoned_pair_task(context: TaskContext, argument: u64) -> u64 {
    let bytes = context
        .runtime()
        .mailbox_receive(argument, 16, 1000)
        .unwrap();
    for offset in [0, 8] {
        let id = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        context.runtime().mutex_lock(id, 0, true).unwrap();
    }
    96
}
#[test]
fn wait_all_reports_lowest_abandoned_index_and_does_not_abandon_other_tasks_ownership() {
    let owner = Runtime::new(
        &[],
        vec![abandoned_pair_task, immediate],
        4 * 1024 * 1024,
        2,
        65536,
    )
    .unwrap();
    let first = owner.mutex().unwrap();
    let second = owner.mutex().unwrap();
    let retained = owner.mutex().unwrap();
    owner.mutex_lock(retained, 0, true).unwrap();
    let request = owner.mailbox(16, 1).unwrap();
    owner
        .mailbox_send(
            request,
            &[first.to_le_bytes(), second.to_le_bytes()].concat(),
        )
        .unwrap();
    let task = owner.spawn(0, request, false).unwrap();
    owner
        .wait_many(&[wait_object(task, 4)], true, 1000, true)
        .unwrap();
    assert_eq!(owner.unlock(first), Err(Error::Denied));
    assert_eq!(owner.unlock(second), Err(Error::Denied));
    let semaphore = owner.semaphore(1, 1).unwrap();
    assert_eq!(
        owner.wait_many(
            &[
                wait_object(semaphore, 3),
                wait_object(second, 2),
                wait_object(first, 2)
            ],
            true,
            0,
            true
        ),
        Ok(runtime::WaitResult {
            index: 1,
            abandoned: true
        })
    );
    owner.unlock(first).unwrap();
    owner.unlock(second).unwrap();
    assert_eq!(owner.join(task), Ok(96));
    let task = owner.spawn(1, 0, false).unwrap();
    owner
        .wait_many(&[wait_object(task, 4)], false, 1000, true)
        .unwrap();
    owner.join(task).unwrap();
    let copy = owner.clone();
    assert_eq!(
        thread::spawn(move || copy.wait_many(&[wait_object(retained, 2)], false, 0, true))
            .join()
            .unwrap(),
        Err(Error::Busy)
    );
    owner.unlock(retained).unwrap();
    for id in [first, second, retained, semaphore] {
        owner.release_sync(id).unwrap();
    }
    owner.mailbox_release(request).unwrap();
}
