//! Maintainer-only malformed-reply provider. Never admitted as game source.
#[allow(dead_code)]
#[path = "../engines/native-services/client.rs"]
mod client;
use std::sync::Mutex;
struct Reply {
    code: i32,
    bytes: Vec<u8>,
    requests: Vec<(u32, Vec<u8>, u32)>,
}
static REPLY: Mutex<Reply> = Mutex::new(Reply {
    code: -5,
    bytes: Vec::new(),
    requests: Vec::new(),
});
#[no_mangle]
unsafe extern "C" fn gamepkg_local_call(
    operation: u32,
    request: *const u8,
    length: u32,
    output: *mut u8,
    capacity: u32,
) -> i32 {
    let mut reply = REPLY.lock().unwrap();
    let request = if length == 0 {
        vec![]
    } else {
        unsafe { std::slice::from_raw_parts(request, length as usize) }.to_vec()
    };
    reply.requests.push((operation, request, capacity));
    if reply.code >= 0 {
        let copied = reply.bytes.len().min(capacity as usize);
        if copied != 0 {
            unsafe {
                std::ptr::copy_nonoverlapping(reply.bytes.as_ptr(), output, copied);
            }
        }
    }
    reply.code
}
fn set(code: i32, bytes: &[u8]) {
    let mut reply = REPLY.lock().unwrap();
    reply.code = code;
    reply.bytes = bytes.to_vec();
    reply.requests.clear();
}
fn request(op: u32, bytes: &[u8], capacity: u32) {
    assert_eq!(
        REPLY.lock().unwrap().requests,
        [(op, bytes.to_vec(), capacity)]
    );
}
#[test]
fn typed_semaphore_client_rejects_malformed_replies_and_never_falls_back() {
    set(-5, &[]);
    assert_eq!(client::local_semaphore_create(2, 1), Err(-1));
    assert_eq!(client::local_semaphore_create(0, 0), Err(-1));
    assert_eq!(client::local_semaphore_create(0, 0x80000000), Err(-1));
    assert_eq!(client::local_semaphore_release(1, 0), Err(-1));
    assert_eq!(client::local_semaphore_release(1, 0x80000000), Err(-1));
    assert_eq!(client::local_semaphore_wait(1, 1001), Err(-1));
    assert!(REPLY.lock().unwrap().requests.is_empty());
    let id = 0xfedcba9876543210u64;
    let creation = [1u32.to_le_bytes(), 2u32.to_le_bytes()].concat();
    for code in [-1, -2, -3, -4, -5] {
        set(code, &[]);
        assert_eq!(client::local_semaphore_create(1, 2), Err(code));
        request(43, &creation, 8);
        set(code, &[]);
        assert_eq!(client::local_semaphore_release(id, 1), Err(code));
        request(
            44,
            &[id.to_le_bytes().as_slice(), &1u32.to_le_bytes()].concat(),
            4,
        );
        set(code, &[]);
        assert_eq!(client::local_semaphore_wait(id, 1000), Err(code));
        request(
            45,
            &[id.to_le_bytes().as_slice(), &1000u32.to_le_bytes()].concat(),
            0,
        );
        set(code, &[]);
        assert_eq!(client::local_semaphore_state(id), Err(code));
        request(46, &id.to_le_bytes(), 8);
    }
    for length in [0, 1, 7, 9] {
        set(length, &id.to_le_bytes());
        assert_eq!(client::local_semaphore_create(1, 2), Err(-3));
    }
    set(8, &[0; 8]);
    assert_eq!(client::local_semaphore_create(1, 2), Err(-3));
    set(8, &id.to_le_bytes());
    assert_eq!(client::local_semaphore_create(1, 2), Ok(id));
    request(43, &creation, 8);
    for length in [0, 1, 3, 5] {
        set(length, &1u32.to_le_bytes());
        assert_eq!(client::local_semaphore_release(id, 1), Err(-3));
    }
    for previous in [0x80000000u32, u32::MAX] {
        set(4, &previous.to_le_bytes());
        assert_eq!(client::local_semaphore_release(id, 1), Err(-3));
    }
    set(4, &0x7fffffffu32.to_le_bytes());
    assert_eq!(client::local_semaphore_release(id, 1), Ok(0x7fffffff));
    set(0, &[]);
    assert_eq!(client::local_semaphore_wait(id, 0), Ok(()));
    set(1, &[]);
    assert_eq!(client::local_semaphore_wait(id, 0), Err(-3));
    for length in [0, 1, 7, 9] {
        set(length, &[0; 8]);
        assert_eq!(client::local_semaphore_state(id), Err(-3));
    }
    for (count, maximum) in [(0u32, 0u32), (2, 1), (0, 0x80000000), (0, u32::MAX)] {
        set(8, &[count.to_le_bytes(), maximum.to_le_bytes()].concat());
        assert_eq!(client::local_semaphore_state(id), Err(-3));
    }
    set(
        8,
        &[0x7fffffffu32.to_le_bytes(), 0x7fffffffu32.to_le_bytes()].concat(),
    );
    assert_eq!(
        client::local_semaphore_state(id),
        Ok(client::LocalSemaphoreState {
            count: 0x7fffffff,
            maximum: 0x7fffffff
        })
    );
    request(46, &id.to_le_bytes(), 8);
}
