#[allow(dead_code)]
#[path = "../engines/native-services/client.rs"]
mod client;
#[path = "../engines/native-services/local_bindings.rs"]
mod local;
use local::runtime::{Error, Runtime, TaskContext, Window};

static TEST_RUNTIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn task(context: TaskContext, arg: u64) -> u64 {
    assert!(!context.cancelled());
    assert!(context.runtime().windows().is_ok());
    let status = local::call(14, &[], 12).unwrap();
    assert_ne!(&status[..8], &0u64.to_le_bytes());
    assert_eq!(&status[8..], &0u32.to_le_bytes());
    arg + 1
}
#[test]
fn child_local_abi_is_bounded_typed_revocable_and_not_a_broker_syscall() {
    let _exclusive = TEST_RUNTIME.lock().unwrap();
    local::shutdown();
    assert_eq!(local::call(0, &[], 4), Err(Error::Denied));
    assert_eq!(client::local_performance_sample(), Err(-4));
    assert_eq!(client::local_clock_state(), Err(-4));
    assert_eq!(client::local_clock_wake(), Err(-4));
    assert_eq!(client::local_mutex_lock(0, 0), Err(-4));
    assert_eq!(client::local_event_wait(0, 0), Err(-4));
    assert_eq!(client::local_event_state(0), Err(-4));
    assert_eq!(client::local_task_join(0).err(), Some(-4));
    local::install(
        Runtime::new(
            &[Window {
                address: 0x80000000,
                bytes: 65536,
            }],
            vec![task, semaphore_cancelled_abi],
            4 * 1024 * 1024,
            2,
            65536,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(local::call(0, &[], 4).unwrap(), 1u32.to_le_bytes());
    assert_eq!(local::call(0, &[1], 4), Err(Error::Invalid));
    assert_eq!(local::call(0, &[], 3), Err(Error::Invalid));
    assert_eq!(local::call(999, &[], 0), Err(Error::Unsupported));
    assert_eq!(local::call(14, &[], 11), Err(Error::Invalid));
    assert_eq!(local::call(14, &[1], 12), Err(Error::Invalid));
    assert_eq!(local::call(14, &[], 12).unwrap(), [0; 12]);
    let create = [4u32.to_le_bytes(), 2u32.to_le_bytes()].concat();
    assert_eq!(local::call(30, &create, 7), Err(Error::Invalid));
    assert_eq!(local::call(30, &[0; 9], 8), Err(Error::Invalid));
    let mailbox = local::call(30, &create, 8).unwrap();
    let mut message = mailbox.clone();
    message.extend([1, 2, 3, 4]);
    local::call(31, &message, 0).unwrap();
    let mut receive = mailbox.clone();
    receive.extend(0u32.to_le_bytes());
    assert_eq!(local::call(32, &receive, 3), Err(Error::Invalid));
    assert_eq!(local::call(32, &receive, 4).unwrap(), [1, 2, 3, 4]);
    assert_eq!(local::call(32, &receive, 4), Err(Error::Busy));
    let mut trailing = mailbox.clone();
    trailing.push(1);
    assert_eq!(local::call(33, &trailing, 0), Err(Error::Invalid));
    local::call(33, &mailbox, 0).unwrap();
    assert_eq!(local::call(31, &message, 0), Err(Error::Denied));
    let windows = local::call(1, &[], 16).unwrap();
    let id = u64::from_le_bytes(windows[..8].try_into().unwrap());
    let mut commit = id.to_le_bytes().to_vec();
    commit.extend(0u64.to_le_bytes());
    commit.extend(4096u64.to_le_bytes());
    commit.extend(2u32.to_le_bytes());
    local::call(2, &commit, 0).unwrap();
    let mut write = id.to_le_bytes().to_vec();
    write.extend(7u64.to_le_bytes());
    write.extend([1, 2, 3]);
    local::call(6, &write, 0).unwrap();
    let mut read = id.to_le_bytes().to_vec();
    read.extend(7u64.to_le_bytes());
    read.extend(3u32.to_le_bytes());
    assert_eq!(local::call(5, &read, 2), Err(Error::Invalid));
    assert_eq!(local::call(5, &read, 3).unwrap(), [1, 2, 3]);
    let exchange = [
        id.to_le_bytes().as_slice(),
        &7u64.to_le_bytes(),
        &3u32.to_le_bytes(),
        &[1, 2, 3],
        &[9, 8, 7],
    ]
    .concat();
    for length in 0..exchange.len() {
        assert_eq!(local::call(42, &exchange[..length], 4), Err(Error::Invalid));
    }
    let mut trailing = exchange.clone();
    trailing.push(0);
    assert_eq!(local::call(42, &trailing, 4), Err(Error::Invalid));
    assert_eq!(local::call(42, &exchange, 3), Err(Error::Invalid));
    assert_eq!(
        client::local_memory_compare_exchange(id, 7, &[1, 2, 3], &[9, 8, 7]),
        Ok(true)
    );
    assert_eq!(
        client::local_memory_compare_exchange(id, 7, &[1, 2, 3], &[0, 0, 0]),
        Ok(false)
    );
    assert_eq!(
        client::local_memory_compare_exchange(id, 7, &[9, 8, 7], &[1, 2, 3]),
        Ok(true)
    );
    assert_eq!(
        client::local_memory_compare_exchange(id, 7, &[], &[]),
        Err(-1)
    );
    assert_eq!(
        client::local_memory_compare_exchange(id, 7, &[0], &[0, 0]),
        Err(-1)
    );
    assert_eq!(
        client::local_memory_compare_exchange(id | 0x100000000, 7, &[1], &[0]),
        Err(-4)
    );
    let bulk = [id, 7, id, 20, 3]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    local::call(7, &bulk, 0).unwrap();
    let copied = [
        id.to_le_bytes().as_slice(),
        &20u64.to_le_bytes(),
        &3u32.to_le_bytes(),
    ]
    .concat();
    assert_eq!(local::call(5, &copied, 3).unwrap(), [1, 2, 3]);
    let fill = [id, 30, 3, 0xabu64]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    local::call(8, &fill, 0).unwrap();
    let filled = [
        id.to_le_bytes().as_slice(),
        &30u64.to_le_bytes(),
        &3u32.to_le_bytes(),
    ]
    .concat();
    assert_eq!(local::call(5, &filled, 3).unwrap(), [0xab; 3]);
    for length in 0..32 {
        assert_eq!(local::call(8, &fill[..length], 0), Err(Error::Invalid));
    }
    let mut trailing_fill = fill.clone();
    trailing_fill.push(0);
    assert_eq!(local::call(8, &trailing_fill, 0), Err(Error::Invalid));
    for (window, offset, bytes, value, error) in [
        (id, 30, 3, 256, Error::Invalid),
        (id, 30, 3, u64::MAX, Error::Invalid),
        (id, 30, 64 * 1024 * 1024 + 1, 0, Error::Invalid),
        (id, u64::MAX, 2, 0, Error::Invalid),
        (0, 0, 0, 0, Error::Denied),
    ] {
        let request = [window, offset, bytes, value]
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(local::call(8, &request, 0), Err(error));
        assert_eq!(local::call(5, &filled, 3).unwrap(), [0xab; 3]);
    }
    assert_eq!(local::call(8, &fill, 1024 * 1024 + 1), Err(Error::Invalid));
    assert_eq!(
        unsafe { local::gamepkg_local_call(8, fill.as_ptr(), 32, std::ptr::null_mut(), 1) },
        -1
    );
    assert_eq!(local::call(5, &filled, 3).unwrap(), [0xab; 3]);
    assert_eq!(local::call(7, &bulk[..39], 0), Err(Error::Invalid));
    let mut extra = bulk.clone();
    extra.push(1);
    assert_eq!(local::call(7, &extra, 0), Err(Error::Invalid));
    let bad = [id, 7, id, 20, 64 * 1024 * 1024 + 1]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(local::call(7, &bad, 0), Err(Error::Invalid));
    assert_eq!(local::call(5, &copied, 3).unwrap(), [1, 2, 3]);
    let mut alias = read.clone();
    let length = unsafe {
        local::gamepkg_local_call(5, alias.as_ptr(), alias.len() as u32, alias.as_mut_ptr(), 3)
    };
    assert_eq!(length, 3);
    assert_eq!(&alias[..3], &[1, 2, 3]);
    assert_eq!(
        unsafe { local::gamepkg_local_call(0, std::ptr::null(), 1, std::ptr::null_mut(), 0) },
        -1
    );
    let mut spawn = 0u32.to_le_bytes().to_vec();
    spawn.extend(41u64.to_le_bytes());
    spawn.extend(1u32.to_le_bytes());
    let job = local::call(10, &spawn, 8).unwrap();
    assert_eq!(local::call(11, &job, 8), Err(Error::Busy));
    assert_eq!(local::call(16, &job, 4).unwrap(), 0u32.to_le_bytes());
    for length in 0..8 {
        assert_eq!(local::call(16, &job[..length], 4), Err(Error::Invalid));
    }
    assert_eq!(
        local::call(16, &[job.clone(), vec![0]].concat(), 4),
        Err(Error::Invalid)
    );
    assert_eq!(local::call(16, &job, 3), Err(Error::Invalid));
    assert_eq!(local::call(16, &0u64.to_le_bytes(), 4), Err(Error::Denied));

    assert_eq!(local::call(13, &job, 3), Err(Error::Invalid));
    assert_eq!(local::call(13, &job, 4).unwrap(), 1u32.to_le_bytes());
    let end = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match local::call(11, &job, 8) {
            Ok(bytes) => {
                assert_eq!(bytes, 42u64.to_le_bytes());
                break;
            }
            Err(Error::Busy) => {
                assert!(std::time::Instant::now() < end);
                std::thread::yield_now()
            }
            value => panic!("{value:?}"),
        }
    }
    assert_eq!(local::call(11, &job, 8), Err(Error::Denied));
    assert_eq!(local::call(16, &job, 4), Err(Error::Denied));
    assert_eq!(
        local::call(20, &[2u32.to_le_bytes(), 0u32.to_le_bytes()].concat(), 8),
        Err(Error::Invalid)
    );
    let event = local::call(20, &[0; 8], 8).unwrap();
    let mut wait = event.clone();
    wait.extend(1000u32.to_le_bytes());
    let worker = std::thread::spawn(move || local::call(23, &wait, 0));
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert_eq!(local::call(34, &[], 8), Err(Error::Invalid));
    assert_eq!(local::call(34, &[0; 4097], 8), Err(Error::Invalid));
    assert_eq!(local::call(34, &[1], 7), Err(Error::Invalid));
    let record = local::call(34, &[1, 2, 3, 4], 8).unwrap();
    assert_eq!(local::call(35, &record, 11), Err(Error::Invalid));
    assert_eq!(
        local::call(35, &[record.clone(), vec![0]].concat(), 12),
        Err(Error::Invalid)
    );
    assert_eq!(
        local::call(35, &record, 12).unwrap(),
        [vec![0; 8], vec![1, 2, 3, 4]].concat()
    );
    let request = [
        record.clone(),
        0u64.to_le_bytes().to_vec(),
        vec![4, 3, 2, 1],
    ]
    .concat();
    assert_eq!(local::call(36, &request, 7), Err(Error::Invalid));
    assert_eq!(local::call(36, &request, 8).unwrap(), 1u64.to_le_bytes());
    assert_eq!(local::call(36, &request, 8), Err(Error::Busy));
    assert_eq!(local::call(36, &request[..19], 8), Err(Error::Invalid));
    assert_eq!(
        local::call(37, &[record.clone(), vec![0]].concat(), 0),
        Err(Error::Invalid)
    );
    assert_eq!(
        local::call(35, &record, 12).unwrap(),
        [1u64.to_le_bytes().to_vec(), vec![4, 3, 2, 1]].concat()
    );
    local::call(37, &record, 0).unwrap();
    assert_eq!(local::call(35, &record, 12), Err(Error::Denied));
    performance_clock_abi();
    typed_mutex_abi();
    typed_event_abi();
    typed_semaphore_abi();
    typed_join_abi();
    local_deadline_abi();
    segmented_abi(id);
    page_states_abi(id);
    local::shutdown();
    assert_eq!(client::local_performance_sample(), Err(-4));
    assert_eq!(client::local_clock_state(), Err(-4));
    assert_eq!(client::local_clock_wake(), Err(-4));
    assert_eq!(client::local_memory_page_states(id, 0, 1), Err(-4));
    assert_eq!(local::call(8, &fill, 0), Err(Error::Denied));
    assert_eq!(worker.join().unwrap(), Err(Error::Denied));
    assert_eq!(local::call(1, &[], 16), Err(Error::Denied));
}

fn segmented_abi(id: u64) {
    fn request(segments: &[[u64; 3]]) -> Vec<u8> {
        let mut bytes = (segments.len() as u32).to_le_bytes().to_vec();
        for segment in segments {
            for value in segment {
                bytes.extend(value.to_le_bytes());
            }
        }
        bytes
    }
    let spans = [[id, 40, 2], [id, 44, 2]];
    let read = request(&spans);
    let write = [read.clone(), vec![1, 2, 3, 4]].concat();
    local::call(15, &write, 0).unwrap();
    assert_eq!(local::call(9, &read, 4).unwrap(), [1, 2, 3, 4]);
    assert_eq!(local::call(9, &read, 3), Err(Error::Invalid));
    assert_eq!(local::call(15, &write, 1048577), Err(Error::Invalid));
    for len in 0..read.len() {
        assert_eq!(local::call(9, &read[..len], 4), Err(Error::Invalid));
    }
    for len in 0..write.len() {
        assert_eq!(local::call(15, &write[..len], 0), Err(Error::Invalid));
    }
    let mut extra = read.clone();
    extra.push(0);
    assert_eq!(local::call(9, &extra, 4), Err(Error::Invalid));
    let mut extra = write.clone();
    extra.push(0);
    assert_eq!(local::call(15, &extra, 0), Err(Error::Invalid));
    for bad in [
        vec![],
        vec![[id, 0, 1]; 33],
        vec![[id, 0, 0]],
        vec![[id, 0, 1048577]],
        vec![[id, 0, u64::MAX]],
        vec![[id, 0, 1], [id, 1, u64::MAX]],
    ] {
        let bytes = request(&bad);
        assert_eq!(local::call(9, &bytes, 4), Err(Error::Invalid));
        assert_eq!(local::call(15, &bytes, 0), Err(Error::Invalid));
    }
    for (bad, error) in [
        ([[id, 40, 2], [0, 0, 2]], Error::Denied),
        ([[id, 40, 2], [id, u64::MAX, 2]], Error::Invalid),
        ([[id, 40, 2], [id, 4096, 2]], Error::Denied),
    ] {
        let read_bad = request(&bad);
        let write_bad = [read_bad.clone(), vec![9; 4]].concat();
        assert_eq!(local::call(15, &write_bad, 0), Err(error));
        let mut output = [0x5a; 4];
        assert_eq!(
            unsafe {
                local::gamepkg_local_call(
                    9,
                    read_bad.as_ptr(),
                    read_bad.len() as u32,
                    output.as_mut_ptr(),
                    4,
                )
            },
            error as i32
        );
        assert_eq!(output, [0x5a; 4]);
        assert_eq!(local::call(9, &read, 4).unwrap(), [1, 2, 3, 4]);
    }
    // Request/output aliases are copied by the FFI boundary before parsing.
    let mut alias = read.clone();
    assert_eq!(
        unsafe {
            local::gamepkg_local_call(9, alias.as_ptr(), alias.len() as u32, alias.as_mut_ptr(), 4)
        },
        4
    );
    assert_eq!(&alias[..4], &[1, 2, 3, 4]);
    let commit = [
        id.to_le_bytes().as_slice(),
        &4096u64.to_le_bytes(),
        &61440u64.to_le_bytes(),
        &2u32.to_le_bytes(),
    ]
    .concat();
    local::call(2, &commit, 0).unwrap();
    let max_read = request(&[[id, 0, 32768]; 32]);
    assert_eq!(local::call(9, &max_read, 1048576).unwrap().len(), 1048576);
    // At 32 descriptors the write payload cap is 1 MiB minus its 772-byte header.
    let mut spans = [[id, 0, 32768]; 32];
    spans[31][2] -= 772;
    let mut max_write = request(&spans);
    max_write.resize(1048576, 0x5a);
    local::call(15, &max_write, 0).unwrap();
    assert!(local::call(9, &request(&spans), 1047804)
        .unwrap()
        .iter()
        .all(|b| *b == 0x5a));
    spans[31][2] += 1;
    let mut overflow = request(&spans);
    overflow.resize(1048577, 1);
    assert_eq!(local::call(15, &overflow, 0), Err(Error::Invalid));
}

fn page_states_abi(id: u64) {
    fn request(id: u64, offset: u64, count: u32, reserved: u32) -> Vec<u8> {
        [
            id.to_le_bytes().as_slice(),
            &offset.to_le_bytes(),
            &count.to_le_bytes(),
            &reserved.to_le_bytes(),
        ]
        .concat()
    }
    let all = [id.to_le_bytes(), 0u64.to_le_bytes(), 65536u64.to_le_bytes()].concat();
    local::call(4, &all, 0).unwrap();
    for (offset, mode) in [(0u64, 1u32), (4096, 2), (8192, 2)] {
        let commit = [
            id.to_le_bytes().as_slice(),
            &offset.to_le_bytes(),
            &4096u64.to_le_bytes(),
            &mode.to_le_bytes(),
        ]
        .concat();
        local::call(2, &commit, 0).unwrap();
    }
    let protect = [
        id.to_le_bytes().as_slice(),
        &8192u64.to_le_bytes(),
        &4096u64.to_le_bytes(),
        &0u32.to_le_bytes(),
    ]
    .concat();
    local::call(3, &protect, 0).unwrap();
    let req = request(id, 0, 16, 0);
    let expected = [vec![2, 3, 1], vec![0; 13]].concat();
    assert_eq!(local::call(17, &req, 16).unwrap(), expected);
    assert_eq!(
        client::local_memory_page_states(id, 0, 16),
        Ok(expected.clone())
    );
    for len in 0..req.len() {
        assert_eq!(local::call(17, &req[..len], 16), Err(Error::Invalid));
    }
    assert_eq!(
        local::call(17, &[req.clone(), vec![0]].concat(), 16),
        Err(Error::Invalid)
    );
    assert_eq!(
        local::call(17, &request(id, 0, 16, 1), 16),
        Err(Error::Invalid)
    );
    assert_eq!(
        local::call(17, &request(id, 0, 16, u32::MAX), 16),
        Err(Error::Invalid)
    );
    for count in [0, 257, u32::MAX] {
        assert_eq!(
            local::call(17, &request(id, 0, count, 0), 256),
            Err(Error::Invalid)
        );
    }
    for capacity in 0..16 {
        assert_eq!(local::call(17, &req, capacity), Err(Error::Invalid));
    }
    for offset in [1, 65536, u64::MAX - 4095] {
        assert_eq!(
            local::call(17, &request(id, offset, 1, 0), 1),
            Err(Error::Invalid)
        );
    }
    for (bad, error) in [
        (request(0, 0, 1, 0), Error::Denied),
        (request(u64::MAX, 0, 1, 0), Error::Denied),
        (request(id, 0, 1, 1), Error::Invalid),
        (request(id, 1, 1, 0), Error::Invalid),
    ] {
        let mut output = [0xa5u8; 8];
        assert_eq!(
            unsafe {
                local::gamepkg_local_call(
                    17,
                    bad.as_ptr(),
                    bad.len() as u32,
                    output.as_mut_ptr(),
                    8,
                )
            },
            error as i32
        );
        assert_eq!(output, [0xa5; 8]);
    }
    let mut alias = req.clone();
    assert_eq!(
        unsafe {
            local::gamepkg_local_call(
                17,
                alias.as_ptr(),
                alias.len() as u32,
                alias.as_mut_ptr(),
                16,
            )
        },
        16
    );
    assert_eq!(&alias[..16], &expected);
    assert_eq!(&alias[16..], &req[16..]);
    assert_eq!(client::local_memory_page_states(id, 1, 1), Err(-1));
    assert_eq!(client::local_memory_page_states(id, 0, 0), Err(-1));
    assert_eq!(client::local_memory_page_states(id, 0, 257), Err(-1));
    let invalid_mode = [
        id.to_le_bytes().as_slice(),
        &4096u64.to_le_bytes(),
        &4096u64.to_le_bytes(),
        &3u32.to_le_bytes(),
    ]
    .concat();
    assert_eq!(local::call(3, &invalid_mode, 0), Err(Error::Invalid));
    // Querying metadata grants no access and does not change the described page.
    let write = [id.to_le_bytes().as_slice(), &0u64.to_le_bytes(), &[1]].concat();
    assert_eq!(local::call(6, &write, 0), Err(Error::Denied));
    let read = [
        id.to_le_bytes().as_slice(),
        &8192u64.to_le_bytes(),
        &1u32.to_le_bytes(),
    ]
    .concat();
    assert_eq!(local::call(5, &read, 1), Err(Error::Denied));
    assert_eq!(client::local_memory_page_states(id, 0, 16), Ok(expected));
}

#[test]
fn secure_clear_is_slice_relative_and_rejects_oversize_before_writing() {
    let mut guarded = vec![0xa5; 1024 * 1024 + 2];
    client::zeroize(&mut guarded[1..1024 * 1024 + 1]).unwrap();
    assert_eq!(guarded[0], 0xa5);
    assert_eq!(guarded[1024 * 1024 + 1], 0xa5);
    assert!(guarded[1..1024 * 1024 + 1].iter().all(|&b| b == 0));
    guarded.fill(0x9d);
    assert_eq!(client::zeroize(&mut guarded), Err(-1));
    assert!(guarded.iter().all(|&b| b == 0x9d));
    client::zeroize(&mut []).unwrap();
}

fn performance_clock_abi() {
    for length in 0..24 {
        assert_eq!(local::call(38, &[], length), Err(Error::Invalid));
    }
    assert_eq!(local::call(38, &[0], 24), Err(Error::Invalid));
    assert_eq!(local::call(38, &[0; 24], 24), Err(Error::Invalid));
    let first = client::local_performance_sample().unwrap();
    let second = client::local_performance_sample().unwrap();
    assert!(first.frequency > 0 && first.frequency <= i64::MAX as u64);
    assert_eq!(first.frequency, second.frequency);
    assert!(second.counter >= first.counter && second.uptime_ms >= first.uptime_ms);
    let mut untouched = [0xa5; 24];
    assert_eq!(
        unsafe { local::gamepkg_local_call(38, std::ptr::null(), 0, untouched.as_mut_ptr(), 23) },
        -1
    );
    assert_eq!(untouched, [0xa5; 24]);
    #[cfg(unix)]
    performance_clock_while_broker_is_blocked();
}

#[cfg(unix)]
fn performance_clock_while_broker_is_blocked() {
    use std::{
        fs::File,
        io::{Read, Write},
        os::{fd::OwnedFd, unix::net::UnixStream},
        sync::mpsc,
        time::Duration,
    };
    let (socket, mut host) = UnixStream::pair().unwrap();
    client::connect(
        File::from(OwnedFd::from(socket.try_clone().unwrap())),
        File::from(OwnedFd::from(socket)),
    )
    .unwrap();
    let (sample_tx, sample_rx) = mpsc::sync_channel(1);
    let owner_request = std::thread::spawn(|| client::fullscreen(true));
    let mut header = [0; 16];
    host.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    host.read_exact(&mut header).unwrap();
    assert_eq!(&header[..4], b"GPH1");
    assert_eq!(u32::from_le_bytes(header[4..8].try_into().unwrap()), 10);
    assert_eq!(u32::from_le_bytes(header[8..12].try_into().unwrap()), 8);
    let mut payload = [0; 8];
    host.read_exact(&mut payload).unwrap();
    assert_eq!(
        payload,
        [0u32.to_le_bytes(), 1u32.to_le_bytes()].concat().as_slice()
    );
    // Header received: the main caller demonstrably holds the broker mutex,
    // awaiting this response. The second thread must finish before we reply.
    let sampler = std::thread::spawn(move || {
        let first = client::local_performance_sample().unwrap();
        let clock = client::local_clock_state().unwrap();
        let waited = client::local_clock_wait(
            clock.sample.counter + clock.sample.frequency.div_ceil(1000),
            clock.sample.frequency,
            clock.generation,
        )
        .unwrap();
        assert_eq!(waited.reason, 0);
        assert_eq!(client::local_clock_wake(), Ok(clock.generation + 1));
        assert_eq!(
            client::local_clock_wait(0, clock.sample.frequency, clock.generation)
                .unwrap()
                .reason,
            1
        );
        std::thread::sleep(Duration::from_millis(20));
        let second = client::local_performance_sample().unwrap();
        sample_tx.send((first, second)).unwrap();
    });
    let sampled = sample_rx.recv_timeout(Duration::from_secs(5));
    // Always unblock the owner, even on regression, rather than hang the suite.
    host.write_all(&[0; 8]).unwrap();
    drop(host); // A regression using the broker must fail, not hang after timeout.
    assert_eq!(owner_request.join().unwrap(), Ok(()));
    sampler.join().unwrap();
    client::disconnect();
    let (first, second) = sampled.expect("clock blocked behind the owner RPC");
    assert_eq!(first.frequency, second.frequency);
    assert!(second.counter > first.counter);
    assert!(second.uptime_ms >= first.uptime_ms);
}

#[test]
fn local_performance_clock_denies_cancelled_tasks_and_closed_runtime() {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    fn cancelled_task(context: TaskContext, _: u64) -> u64 {
        STARTED.store(true, std::sync::atomic::Ordering::Release);
        while !context.cancelled() {
            std::thread::yield_now();
        }
        assert_eq!(context.runtime().performance_sample(), Err(Error::Denied));
        17
    }
    let runtime = Runtime::new(&[], vec![cancelled_task], 4 * 1024 * 1024, 2, 65536).unwrap();
    let task = runtime.spawn(0, 0, false).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !STARTED.load(std::sync::atomic::Ordering::Acquire) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    runtime.cancel(task).unwrap();
    while !runtime.task_finished(task).unwrap() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(runtime.join(task).unwrap(), 17);
    let clone = runtime.clone();
    runtime.close().unwrap();
    assert_eq!(clone.performance_sample(), Err(Error::Denied));
}

fn local_deadline_abi() {
    for capacity in 0..32 {
        assert_eq!(local::call(39, &[], capacity), Err(Error::Invalid));
    }
    assert_eq!(local::call(39, &[0], 32), Err(Error::Invalid));
    for capacity in 0..8 {
        assert_eq!(local::call(41, &[], capacity), Err(Error::Invalid));
    }
    assert_eq!(local::call(41, &[0], 8), Err(Error::Invalid));
    let initial = client::local_clock_state().unwrap();
    let request = [
        0u64.to_le_bytes(),
        initial.sample.frequency.to_le_bytes(),
        initial.generation.to_le_bytes(),
    ]
    .concat();
    for length in 0..24 {
        assert_eq!(local::call(40, &request[..length], 36), Err(Error::Invalid));
    }
    for capacity in 0..36 {
        assert_eq!(local::call(40, &request, capacity), Err(Error::Invalid));
    }
    assert_eq!(
        local::call(40, &[request.clone(), vec![0]].concat(), 36),
        Err(Error::Invalid)
    );
    assert_eq!(
        client::local_clock_state().unwrap().generation,
        initial.generation
    );
    assert_eq!(client::local_clock_wait(0, 0, 0), Err(-1));
    assert_eq!(client::local_clock_wake(), Ok(initial.generation + 1));
    let wake = client::local_clock_wait(0, initial.sample.frequency, initial.generation).unwrap();
    assert_eq!(wake.reason, 1);
    assert_eq!(wake.state.generation, initial.generation + 1);
    let state = client::local_clock_state().unwrap();
    let result = client::local_clock_wait(
        state.sample.counter + state.sample.frequency.div_ceil(1000),
        state.sample.frequency,
        state.generation,
    )
    .unwrap();
    assert_eq!(result.reason, 0);
    assert!(
        result.state.sample.counter >= state.sample.counter + state.sample.frequency.div_ceil(1000)
    );
}

fn typed_mutex_abi() {
    let event = local::call(20, &[0u32.to_le_bytes(), 1u32.to_le_bytes()].concat(), 8).unwrap();
    let request = [event.as_slice(), &0u32.to_le_bytes()].concat();
    for length in 0..12 {
        assert_eq!(local::call(26, &request[..length], 0), Err(Error::Invalid));
    }
    assert_eq!(
        local::call(26, &[request.clone(), vec![0]].concat(), 0),
        Err(Error::Invalid)
    );
    assert_eq!(
        local::call(26, &[event.as_slice(), &1001u32.to_le_bytes()].concat(), 0),
        Err(Error::Invalid)
    );
    assert_eq!(local::call(26, &[0u8; 12], 0), Err(Error::Denied));
    let mut untouched = [0xa5u8; 8];
    assert_eq!(
        unsafe { local::gamepkg_local_call(26, request.as_ptr(), 12, untouched.as_mut_ptr(), 8) },
        -1
    );
    assert_eq!(untouched, [0xa5; 8]);
    // An incorrectly typed guard must never consume an auto-reset event.
    assert_eq!(
        client::local_mutex_lock(u64::from_le_bytes(event.clone().try_into().unwrap()), 0),
        Err(-1)
    );
    local::call(23, &request, 0).unwrap();
    assert_eq!(local::call(23, &request, 0), Err(Error::Busy));
    local::call(25, &event, 0).unwrap();
    let mutex = local::call(21, &[], 8).unwrap();
    let id = u64::from_le_bytes(mutex.clone().try_into().unwrap());
    let request = [mutex.as_slice(), &0u32.to_le_bytes()].concat();
    assert_eq!(
        unsafe { local::gamepkg_local_call(26, request.as_ptr(), 12, untouched.as_mut_ptr(), 8) },
        0
    );
    assert_eq!(untouched, [0xa5; 8]);
    assert_eq!(client::local_mutex_lock(id, 1001), Err(-1));
    client::local_mutex_lock(id, 0).unwrap();
    assert_eq!(local::call(25, &mutex, 0), Err(Error::Busy));
    assert_eq!(
        std::thread::spawn(move || client::local_mutex_lock(id, 0))
            .join()
            .unwrap(),
        Err(-2)
    );
    local::call(24, &mutex, 0).unwrap();
    local::call(24, &mutex, 0).unwrap();
    local::call(25, &mutex, 0).unwrap();
    assert_eq!(client::local_mutex_lock(id, 0), Err(-4));
}
fn typed_event_abi() {
    let event = local::call(20, &[0u32.to_le_bytes(), 1u32.to_le_bytes()].concat(), 8).unwrap();
    let id = u64::from_le_bytes(event.clone().try_into().unwrap());
    let wait = [event.as_slice(), &0u32.to_le_bytes()].concat();
    for length in 0..12 {
        assert_eq!(local::call(27, &wait[..length], 0), Err(Error::Invalid));
    }
    assert_eq!(
        local::call(27, &[wait.clone(), vec![0]].concat(), 0),
        Err(Error::Invalid)
    );
    assert_eq!(
        local::call(27, &[event.as_slice(), &1001u32.to_le_bytes()].concat(), 0),
        Err(Error::Invalid)
    );
    for length in 0..8 {
        assert_eq!(local::call(28, &event[..length], 8), Err(Error::Invalid));
    }
    assert_eq!(
        local::call(28, &[event.clone(), vec![0]].concat(), 8),
        Err(Error::Invalid)
    );
    for capacity in 0..8 {
        assert_eq!(local::call(28, &event, capacity), Err(Error::Invalid));
    }
    assert_eq!(
        local::call(28, &event, 8).unwrap(),
        [0u32.to_le_bytes(), 1u32.to_le_bytes()].concat()
    );
    let expected = client::LocalEventState {
        manual: false,
        signaled: true,
    };
    assert_eq!(client::local_event_state(id), Ok(expected));
    assert_eq!(client::local_event_state(id), Ok(expected));
    assert_eq!(client::local_event_wait(id, 1001), Err(-1));
    let mut untouched = [0xa5u8; 16];
    assert_eq!(
        unsafe { local::gamepkg_local_call(28, event.as_ptr(), 8, untouched.as_mut_ptr(), 7) },
        -1
    );
    assert_eq!(untouched, [0xa5; 16]);
    assert_eq!(
        unsafe { local::gamepkg_local_call(28, event.as_ptr(), 8, untouched.as_mut_ptr(), 16) },
        8
    );
    assert_eq!(
        &untouched[..8],
        &[0u32.to_le_bytes(), 1u32.to_le_bytes()].concat()
    );
    assert_eq!(&untouched[8..], &[0xa5; 8]);
    untouched = [0xa5; 16];
    assert_eq!(
        unsafe { local::gamepkg_local_call(27, wait.as_ptr(), 12, untouched.as_mut_ptr(), 16) },
        0
    );
    assert_eq!(untouched, [0xa5; 16]);
    assert_eq!(
        client::local_event_state(id),
        Ok(client::LocalEventState {
            manual: false,
            signaled: false
        })
    );
    assert_eq!(client::local_event_wait(id, 0), Err(-2));
    local::call(25, &event, 0).unwrap();
    assert_eq!(client::local_event_state(id), Err(-4));
    assert_eq!(client::local_event_wait(id, 0), Err(-4));
    let mutex = local::call(21, &[], 8).unwrap();
    let id = u64::from_le_bytes(mutex.clone().try_into().unwrap());
    let wait = [mutex.as_slice(), &0u32.to_le_bytes()].concat();
    untouched = [0xa5; 16];
    assert_eq!(
        unsafe { local::gamepkg_local_call(27, wait.as_ptr(), 12, untouched.as_mut_ptr(), 16) },
        -1
    );
    assert_eq!(untouched, [0xa5; 16]);
    assert_eq!(client::local_event_state(id), Err(-1));
    std::thread::spawn(move || {
        client::local_mutex_lock(id, 0).unwrap();
        local::call(24, &id.to_le_bytes(), 0).unwrap();
    })
    .join()
    .unwrap();
    local::call(25, &mutex, 0).unwrap();
}

fn typed_join_abi() {
    assert_eq!(client::local_task_join(0).err(), Some(-4));
    for value in [41u64, 0x100000000u64, 0xfedcba9876543200u64] {
        let request = [
            0u32.to_le_bytes().as_slice(),
            &value.to_le_bytes(),
            &1u32.to_le_bytes(),
        ]
        .concat();
        let bytes = local::call(10, &request, 8).unwrap();
        let id = u64::from_le_bytes(bytes.try_into().unwrap());
        assert_eq!(client::local_task_join(id).err(), Some(-2));
        assert_eq!(client::local_task_finished(id), Ok(false));
        local::call(13, &id.to_le_bytes(), 4).unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !client::local_task_finished(id).unwrap() {
            assert!(std::time::Instant::now() < end);
            std::thread::yield_now();
        }
        let joined = client::local_task_join(id).unwrap();
        assert_eq!(joined.task_id(), id);
        assert_eq!(joined.result(), value + 1);
        assert_eq!(client::local_task_join(id).err(), Some(-4));
        assert_eq!(client::local_task_finished(id), Err(-4));
        // Evidence survives resource revocation; it is a copied result of a
        // completed join, never a new/native handle or a live status query.
        assert_eq!(joined.task_id(), id);
        assert_eq!(joined.result(), value + 1);
    }
    let event = local::call(20, &[0; 8], 8).unwrap();
    let event = u64::from_le_bytes(event.try_into().unwrap());
    assert_eq!(client::local_task_join(event).err(), Some(-4));
    assert!(!client::local_event_state(event).unwrap().signaled);
    local::call(25, &event.to_le_bytes(), 0).unwrap();
}

fn semaphore_cancelled_abi(_: TaskContext, request: u64) -> u64 {
    let mut bytes = [0u8; 16];
    assert_eq!(
        client::local_mailbox_receive(request, 1000, &mut bytes),
        Ok(16)
    );
    let semaphore = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let ready = u64::from_le_bytes(bytes[8..].try_into().unwrap());
    client::local_mailbox_send(ready, &[1]).unwrap();
    assert_eq!(client::local_semaphore_wait(semaphore, 1000), Err(-4));
    assert_eq!(client::local_semaphore_state(semaphore), Err(-4));
    assert_eq!(client::local_semaphore_release(semaphore, 1), Err(-4));
    assert_eq!(client::local_semaphore_create(0, 1), Err(-4));
    55
}
fn typed_semaphore_abi() {
    let create = [1u32.to_le_bytes(), 2u32.to_le_bytes()].concat();
    for length in 0..8 {
        assert_eq!(local::call(43, &create[..length], 8), Err(Error::Invalid));
    }
    assert_eq!(
        local::call(43, &[create.clone(), vec![0]].concat(), 8),
        Err(Error::Invalid)
    );
    assert_eq!(local::call(43, &create, 7), Err(Error::Invalid));
    for (initial, maximum) in [(0u32, 0u32), (0, 0x80000000), (2, 1), (u32::MAX, 1)] {
        let request = [initial.to_le_bytes(), maximum.to_le_bytes()].concat();
        assert_eq!(local::call(43, &request, 8), Err(Error::Invalid));
        assert_eq!(client::local_semaphore_create(initial, maximum), Err(-1));
    }
    let semaphore = client::local_semaphore_create(1, 2).unwrap();
    let state = client::LocalSemaphoreState {
        count: 1,
        maximum: 2,
    };
    assert_eq!(client::local_semaphore_state(semaphore), Ok(state));
    let release = [semaphore.to_le_bytes().as_slice(), &1u32.to_le_bytes()].concat();
    for length in 0..12 {
        assert_eq!(local::call(44, &release[..length], 4), Err(Error::Invalid));
        assert_eq!(local::call(45, &release[..length], 0), Err(Error::Invalid));
    }
    for op in [44, 45] {
        assert_eq!(
            local::call(op, &[release.clone(), vec![0]].concat(), 8),
            Err(Error::Invalid)
        );
    }
    assert_eq!(local::call(44, &release, 3), Err(Error::Invalid));
    assert_eq!(
        local::call(46, &semaphore.to_le_bytes(), 7),
        Err(Error::Invalid)
    );
    assert_eq!(local::call(46, &release, 8), Err(Error::Invalid));
    for length in 0..8 {
        assert_eq!(
            local::call(46, &semaphore.to_le_bytes()[..length], 8),
            Err(Error::Invalid)
        );
    }
    for permits in [0u32, 2, 0x80000000, u32::MAX] {
        assert_eq!(
            local::call(
                44,
                &[semaphore.to_le_bytes().as_slice(), &permits.to_le_bytes()].concat(),
                4
            ),
            Err(Error::Invalid)
        );
        assert_eq!(client::local_semaphore_release(semaphore, permits), Err(-1));
    }
    assert_eq!(client::local_semaphore_wait(semaphore, 1001), Err(-1));
    assert_eq!(
        local::call(
            45,
            &[semaphore.to_le_bytes().as_slice(), &1001u32.to_le_bytes()].concat(),
            0
        ),
        Err(Error::Invalid)
    );
    assert_eq!(client::local_mutex_lock(semaphore, 0), Err(-1));
    assert_eq!(client::local_event_wait(semaphore, 0), Err(-1));
    assert_eq!(client::local_event_state(semaphore), Err(-1));
    assert_eq!(
        local::call(
            23,
            &[semaphore.to_le_bytes().as_slice(), &0u32.to_le_bytes()].concat(),
            0
        ),
        Err(Error::Invalid)
    );
    assert_eq!(client::local_semaphore_state(semaphore), Ok(state));
    assert_eq!(client::local_semaphore_release(semaphore, 1), Ok(1));
    for _ in 0..2 {
        client::local_semaphore_wait(semaphore, 0).unwrap();
    }
    assert_eq!(client::local_semaphore_wait(semaphore, 0), Err(-2));
    for op in [20u32, 21] {
        let request = if op == 20 {
            vec![1u8, 0, 0, 0, 1, 0, 0, 0]
        } else {
            vec![]
        };
        let bytes = local::call(op, &request, 8).unwrap();
        let id = u64::from_le_bytes(bytes.clone().try_into().unwrap());
        assert_eq!(client::local_semaphore_state(id), Err(-1));
        assert_eq!(client::local_semaphore_release(id, 1), Err(-1));
        assert_eq!(client::local_semaphore_wait(id, 0), Err(-1));
        if op == 20 {
            assert!(client::local_event_state(id).unwrap().signaled);
        } else {
            client::local_mutex_lock(id, 0).unwrap();
            local::call(24, &bytes, 0).unwrap();
        }
        local::call(25, &bytes, 0).unwrap();
    }
    let request = client::local_mailbox_create(16, 1).unwrap();
    let ready = client::local_mailbox_create(1, 1).unwrap();
    client::local_mailbox_send(
        request,
        &[semaphore.to_le_bytes(), ready.to_le_bytes()].concat(),
    )
    .unwrap();
    let task = u64::from_le_bytes(
        local::call(
            10,
            &[
                1u32.to_le_bytes().as_slice(),
                &request.to_le_bytes(),
                &0u32.to_le_bytes(),
            ]
            .concat(),
            8,
        )
        .unwrap()
        .try_into()
        .unwrap(),
    );
    let mut signal = [0u8; 1];
    assert_eq!(
        client::local_mailbox_receive(ready, 1000, &mut signal),
        Ok(1)
    );
    assert_eq!(signal, [1]);
    local::call(12, &task.to_le_bytes(), 0).unwrap();
    assert_eq!(client::local_semaphore_release(semaphore, 1), Ok(0));
    let end = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !client::local_task_finished(task).unwrap() {
        assert!(std::time::Instant::now() < end);
        std::thread::yield_now();
    }
    assert_eq!(client::local_task_join(task).unwrap().result(), 55);
    assert_eq!(client::local_semaphore_state(semaphore).unwrap().count, 1);
    client::local_mailbox_release(request).unwrap();
    client::local_mailbox_release(ready).unwrap();
    local::call(25, &semaphore.to_le_bytes(), 0).unwrap();
    assert_eq!(client::local_semaphore_state(semaphore), Err(-4));
    assert_eq!(client::local_semaphore_wait(semaphore, 0), Err(-4));
    assert_eq!(client::local_semaphore_release(semaphore, 1), Err(-4));
    let mut objects = Vec::new();
    for _ in 0..128 {
        match client::local_semaphore_create(0, 1) {
            Ok(id) => objects.push(id),
            Err(-2) => break,
            Err(code) => panic!("unexpected semaphore creation error {code}"),
        }
    }
    assert!(!objects.is_empty());
    assert_eq!(client::local_semaphore_create(0, 1), Err(-2));
    for id in objects {
        local::call(25, &id.to_le_bytes(), 0).unwrap();
    }
}
fn waitset_request(objects: &[client::LocalWaitObject], all: u32, milliseconds: u32) -> Vec<u8> {
    let mut bytes = [
        (objects.len() as u32).to_le_bytes(),
        all.to_le_bytes(),
        milliseconds.to_le_bytes(),
    ]
    .concat();
    for object in objects {
        bytes.extend(object.id.to_le_bytes());
        bytes.extend(object.kind.to_le_bytes());
    }
    bytes
}
fn waitset_abandon(context: TaskContext, argument: u64) -> u64 {
    context.runtime().mutex_lock(argument, 0, true).unwrap();
    123
}
#[test]
fn exact_waitset_abi_rejects_every_truncation_and_short_reply_before_acquisition() {
    let _exclusive = TEST_RUNTIME.lock().unwrap();
    local::shutdown();
    local::install(Runtime::new(&[], vec![waitset_abandon], 4 * 1024 * 1024, 1, 65536).unwrap())
        .unwrap();
    let event = u64::from_le_bytes(
        local::call(20, &[0u32.to_le_bytes(), 1u32.to_le_bytes()].concat(), 8)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let semaphore = client::local_semaphore_create(1, 2).unwrap();
    let objects = [
        client::LocalWaitObject { id: event, kind: 1 },
        client::LocalWaitObject {
            id: semaphore,
            kind: 3,
        },
    ];
    let request = waitset_request(&objects, 1, 0);
    for length in 0..request.len() {
        assert_eq!(local::call(47, &request[..length], 8), Err(Error::Invalid));
    }
    let mut trailing = request.clone();
    trailing.push(0);
    assert_eq!(local::call(47, &trailing, 8), Err(Error::Invalid));
    for capacity in 0..8 {
        assert_eq!(local::call(47, &request, capacity), Err(Error::Invalid));
    }
    for malformed in [
        waitset_request(&objects, 2, 0),
        waitset_request(&objects, 0, 1001),
        waitset_request(&[objects[0]; 2], 0, 0),
        waitset_request(&[], 0, 0),
        waitset_request(&[objects[0]; 65], 0, 0),
        waitset_request(&[client::LocalWaitObject { id: event, kind: 5 }], 0, 0),
    ] {
        assert_eq!(local::call(47, &malformed, 8), Err(Error::Invalid));
    }
    assert!(client::local_event_state(event).unwrap().signaled);
    assert_eq!(client::local_semaphore_state(semaphore).unwrap().count, 1);
    assert_eq!(
        client::local_wait_many(&objects, true, 0),
        Ok(client::LocalWaitResult {
            index: 0,
            abandoned: false
        })
    );
    assert!(!client::local_event_state(event).unwrap().signaled);
    assert_eq!(client::local_semaphore_state(semaphore).unwrap().count, 0);
    let mutex = u64::from_le_bytes(local::call(21, &[], 8).unwrap().try_into().unwrap());
    let task = u64::from_le_bytes(
        local::call(
            10,
            &[
                0u32.to_le_bytes().as_slice(),
                &mutex.to_le_bytes(),
                &1u32.to_le_bytes(),
            ]
            .concat(),
            8,
        )
        .unwrap()
        .try_into()
        .unwrap(),
    );
    let completion = [client::LocalWaitObject { id: task, kind: 4 }];
    assert_eq!(client::local_wait_many(&completion, false, 0), Err(-2));
    local::call(13, &task.to_le_bytes(), 4).unwrap();
    client::local_wait_many(&completion, true, 1000).unwrap();
    assert!(client::local_task_finished(task).unwrap());
    assert_eq!(
        client::local_wait_many(&[client::LocalWaitObject { id: mutex, kind: 2 }], false, 0),
        Ok(client::LocalWaitResult {
            index: 0,
            abandoned: true
        })
    );
    local::call(24, &mutex.to_le_bytes(), 0).unwrap();
    assert_eq!(client::local_task_join(task).unwrap().result(), 123);
    assert_eq!(client::local_wait_many(&completion, false, 0), Err(-4));
    for id in [event, semaphore, mutex] {
        local::call(25, &id.to_le_bytes(), 0).unwrap();
    }
    local::shutdown();
}
