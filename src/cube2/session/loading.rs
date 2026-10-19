//! Exercise the actual pipe reader and loading deadline without engine timing noise.
use super::*;
use std::os::unix::net::UnixStream;

fn frame() -> Vec<u8> {
    let mut bytes = b"GPF3".to_vec();
    for n in [1u32, 1, 4, 0, 2, 0] {
        bytes.extend(n.to_le_bytes());
    }
    bytes
}
fn delayed(marker: &[u8], limit: Duration) -> (bool, bool) {
    let (mut reader, mut writer) = UnixStream::pair().unwrap();
    reader.set_nonblocking(true).unwrap();
    writer.write_all(marker).unwrap();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        let _ = writer.write_all(&frame());
    });
    let mut loading = false;
    let success = read_tick_update(
        &mut reader,
        Instant::now() + Duration::from_millis(50),
        limit,
        &mut loading,
    )
    .is_ok();
    worker.join().unwrap();
    (success, loading)
}
#[test]
fn resource_marker_grants_one_fixed_allowance_and_cached_frames_keep_normal_deadline() {
    assert_eq!(delayed(b"GPL1", Duration::from_secs(1)), (true, true));
    assert_eq!(delayed(b"", Duration::from_secs(1)), (false, false));
    assert_eq!(delayed(b"GPL1", Duration::from_millis(50)), (false, true));
    let (mut reader, mut writer) = UnixStream::pair().unwrap();
    reader.set_nonblocking(true).unwrap();
    writer.write_all(b"GPL1GPL1").unwrap();
    let mut loading = false;
    let error = read_tick_update(
        &mut reader,
        Instant::now() + Duration::from_secs(1),
        Duration::from_secs(1),
        &mut loading,
    )
    .unwrap_err();
    assert!(error.to_string().contains("Duplicate"));
    assert!(loading);
    // A previous loading tick cannot carry its allowance into the next tick.
    let result = read_tick_update(
        &mut reader,
        Instant::now(),
        Duration::from_secs(1),
        &mut loading,
    );
    assert!(result.is_err());
    assert!(!loading);
}
