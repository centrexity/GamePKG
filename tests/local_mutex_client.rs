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
fn typed_mutex_observation_validates_exact_replies_without_retry_or_fallback() {
    let id = 0xfedcba9876543210u64;
    for code in [-1, -2, -3, -4, -5] {
        set(code, &[]);
        assert_eq!(client::local_mutex_state(id), Err(code));
        request(48, &id.to_le_bytes(), 16);
    }
    for length in [0, 1, 7, 8, 15, 17, i32::MAX] {
        set(length, &[0; 16]);
        assert_eq!(client::local_mutex_state(id), Err(-3));
        request(48, &id.to_le_bytes(), 16);
    }
    for kind in [0u32, 4, u32::MAX] {
        set(-5, &[]);
        assert_eq!(client::local_sync_release(id, kind), Err(-1));
        assert!(REPLY.lock().unwrap().requests.is_empty());
    }
    for kind in 1..=3u32 {
        let expected = [id.to_le_bytes().as_slice(), &kind.to_le_bytes()].concat();
        for code in [-1, -2, -3, -4, -5] {
            set(code, &[]);
            assert_eq!(client::local_sync_release(id, kind), Err(code));
            request(49, &expected, 0);
        }
        for code in [1, 4, 16, i32::MAX] {
            set(code, &[]);
            assert_eq!(client::local_sync_release(id, kind), Err(-3));
            request(49, &expected, 0);
        }
        set(0, &[]);
        assert_eq!(client::local_sync_release(id, kind), Ok(()));
        request(49, &expected, 0);
    }
    // Exhaust all Boolean relationships and depth-zero/nonzero combinations.
    for locked in [0u32, 1, 2, u32::MAX] {
        for owned in [0u32, 1, 2, u32::MAX] {
            for abandoned in [0u32, 1, 2, u32::MAX] {
                for depth in [0u32, 1, 2, u32::MAX] {
                    let bytes = [
                        locked.to_le_bytes(),
                        owned.to_le_bytes(),
                        abandoned.to_le_bytes(),
                        depth.to_le_bytes(),
                    ]
                    .concat();
                    set(16, &bytes);
                    let valid = locked <= 1
                        && owned <= 1
                        && abandoned <= 1
                        && !(owned != 0 && locked == 0)
                        && ((locked != 0) == (depth != 0))
                        && !(abandoned != 0 && locked != 0);
                    assert_eq!(
                        client::local_mutex_state(id),
                        if valid {
                            Ok(client::LocalMutexState {
                                locked: locked != 0,
                                owned_by_caller: owned != 0,
                                abandoned: abandoned != 0,
                                depth,
                            })
                        } else {
                            Err(-3)
                        }
                    );
                    request(48, &id.to_le_bytes(), 16);
                }
            }
        }
    }
}
