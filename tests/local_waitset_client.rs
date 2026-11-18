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
fn typed_waitset_client_rejects_malformed_replies_and_never_falls_back() {
    let objects = [
        client::LocalWaitObject {
            id: 0xfedcba9876543210,
            kind: 1,
        },
        client::LocalWaitObject {
            id: 0x123456789abcdef0,
            kind: 2,
        },
    ];
    for all in [false, true] {
        set(-5, &[]);
        for bad in [
            vec![],
            vec![objects[0]; 65],
            vec![objects[0]; 2],
            vec![client::LocalWaitObject { id: 0, kind: 1 }],
            vec![client::LocalWaitObject { id: 1, kind: 0 }],
            vec![client::LocalWaitObject { id: 1, kind: 5 }],
        ] {
            assert_eq!(client::local_wait_many(&bad, all, 0), Err(-1));
        }
        assert_eq!(client::local_wait_many(&objects, all, 1001), Err(-1));
        assert!(REPLY.lock().unwrap().requests.is_empty());
        let mut expected = [
            2u32.to_le_bytes(),
            u32::from(all).to_le_bytes(),
            7u32.to_le_bytes(),
        ]
        .concat();
        for object in objects {
            expected.extend(object.id.to_le_bytes());
            expected.extend(object.kind.to_le_bytes());
        }
        for code in [-1, -2, -3, -4, -5] {
            set(code, &[]);
            assert_eq!(client::local_wait_many(&objects, all, 7), Err(code));
            request(47, &expected, 8);
        }
        for length in [0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 32] {
            set(length, &[0; 8]);
            assert_eq!(client::local_wait_many(&objects, all, 7), Err(-3));
            request(47, &expected, 8);
        }
        for (index, reason) in [(2u32, 0u32), (u32::MAX, 0), (0, 2), (0, u32::MAX), (0, 1)] {
            set(8, &[index.to_le_bytes(), reason.to_le_bytes()].concat());
            assert_eq!(client::local_wait_many(&objects, all, 7), Err(-3));
        }
        set(8, &[1u32.to_le_bytes(), 1u32.to_le_bytes()].concat());
        assert_eq!(
            client::local_wait_many(&objects, all, 7),
            Ok(client::LocalWaitResult {
                index: 1,
                abandoned: true
            })
        );
        request(47, &expected, 8);
        set(8, &[0u32.to_le_bytes(), 0u32.to_le_bytes()].concat());
        assert_eq!(
            client::local_wait_many(&objects, all, 7),
            Ok(client::LocalWaitResult {
                index: 0,
                abandoned: false
            })
        );
        set(8, &[1u32.to_le_bytes(), 0u32.to_le_bytes()].concat());
        assert_eq!(
            client::local_wait_many(&objects, all, 7),
            if all {
                Err(-3)
            } else {
                Ok(client::LocalWaitResult {
                    index: 1,
                    abandoned: false,
                })
            }
        );
    }
}
