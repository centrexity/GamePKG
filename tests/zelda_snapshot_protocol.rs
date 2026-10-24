//! Actual trusted worker framing with a synthetic engine; no ROM or game assets.
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
const SIZE: u32 = 275497;
fn function(source: &str, signature: &str) -> String {
    assert_eq!(source.matches(signature).count(), 1);
    let start = source.find(signature).unwrap();
    let brace = start + source[start..].find('{').unwrap();
    let mut depth = 1;
    for (offset, byte) in source.as_bytes().iter().enumerate().skip(brace + 1) {
        depth += i32::from(*byte == b'{') - i32::from(*byte == b'}');
        if depth == 0 {
            return source[start..=offset].replace("pub(super)", "pub(crate)");
        }
    }
    panic!("unclosed trusted function");
}
struct Worker(Child);
impl Worker {
    fn start(binary: &std::path::Path) -> Self {
        let child = Command::new(binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut worker = Self(child);
        assert_eq!(worker.receive(), [255]);
        worker
    }
    fn receive(&mut self) -> Vec<u8> {
        let output = self.0.stdout.as_mut().unwrap();
        let mut length = [0; 4];
        output.read_exact(&mut length).unwrap();
        let n = u32::from_le_bytes(length) as usize;
        assert!((1..=65538).contains(&n));
        let mut bytes = vec![0; n];
        output.read_exact(&mut bytes).unwrap();
        bytes
    }
    fn send(&mut self, control: bool, bytes: &[u8]) {
        let length = bytes.len() as u32 | if control { 0x80000000 } else { 0 };
        let input = self.0.stdin.as_mut().unwrap();
        input.write_all(&length.to_le_bytes()).unwrap();
        input.write_all(bytes).unwrap();
        input.flush().unwrap();
    }
    fn control(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.send(true, bytes);
        self.receive()
    }
    fn normal(&mut self, state: u64) {
        let mut bytes = vec![0; 4];
        bytes.extend_from_slice(&state.to_le_bytes());
        self.send(false, &bytes);
        let mut expected = vec![0];
        expected.extend_from_slice(&state.wrapping_add(1).to_le_bytes());
        assert_eq!(self.receive(), expected);
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn numbers(op: u8, values: &[u32]) -> Vec<u8> {
    let mut b = vec![op];
    for value in values {
        b.extend_from_slice(&value.to_le_bytes());
    }
    b
}
#[test]
fn trusted_snapshot_protocol_bounds_rejections_and_full_width_tick_state() {
    let guest = include_str!("../src/native/guest.rs.txt");
    let engine = include_str!("../src/native/engine.rs.txt");
    let source = include_str!("fixtures/zelda-snapshot-protocol.rs")
        .replace(
            "// ACTUAL_PACKET",
            &function(guest, "pub(super) fn __gamepkg_packet"),
        )
        .replace(
            "// ACTUAL_NEXT_TICK",
            &function(guest, "pub(super) fn __gamepkg_next_tick"),
        )
        .replace(
            "// ACTUAL_SNAPSHOT_CONTROL",
            &function(engine, "pub(super) fn snapshot_control"),
        );
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("worker.rs");
    std::fs::write(&path, source).unwrap();
    for zelda in [true, false] {
        let binary = temporary.path().join(format!(
            "{}{}",
            if zelda { "zelda" } else { "ordinary" },
            std::env::consts::EXE_SUFFIX
        ));
        let mut command = Command::new("rustc");
        command
            .args(["--edition=2021", "-Dwarnings"])
            .arg(&path)
            .arg("-o")
            .arg(&binary);
        if zelda {
            command.args(["--cfg", "gamepkg_zelda"]);
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let mut worker = Worker::start(&binary);
        worker.normal(u64::MAX);
        worker.normal(0);
        worker.normal(1 << 63);
        if !zelda {
            worker.send(true, &[0]);
            assert!(
                !worker.0.wait().unwrap().success(),
                "ordinary game accepted snapshot control"
            );
            continue;
        }
        assert_eq!(
            worker.control(&[0]),
            numbers(0x82, &[])
                .into_iter()
                .chain([0])
                .chain(SIZE.to_le_bytes())
                .collect::<Vec<_>>()
        );
        for request in [
            vec![9],
            vec![0, 0],
            vec![1],
            vec![2],
            vec![3],
            numbers(1, &[0, 0]),
            numbers(1, &[0, 65537]),
            numbers(1, &[SIZE, 1]),
            numbers(1, &[u32::MAX, u32::MAX]),
            numbers(2, &[SIZE, 42]),
            numbers(3, &[SIZE - 1]),
            numbers(3, &[u32::MAX]),
        ] {
            assert_eq!(worker.control(&request), [0x82, 1], "{request:?}");
        }
        let mut request = numbers(2, &[SIZE - 1]);
        request.push(42);
        assert_eq!(worker.control(&request), [0x82, 0]);
        assert_eq!(worker.control(&numbers(1, &[SIZE - 1, 1])), [0x82, 0, 42]);
        let mut request = numbers(2, &[0]);
        request.extend((0..65536).map(|i| (i % 251) as u8));
        assert_eq!(worker.control(&request), [0x82, 0]);
        let response = worker.control(&numbers(1, &[0, 65536]));
        assert_eq!(&response[..2], &[0x82, 0]);
        assert_eq!(&response[2..], &request[5..]);
        let expected = [
            vec![0x82, 0],
            [0x11223344u32, 0x55667788, 0x99aabbcc]
                .iter()
                .flat_map(|n| n.to_le_bytes())
                .collect(),
        ]
        .concat();
        assert_eq!(worker.control(&numbers(3, &[SIZE])), expected);
        let mut request = numbers(2, &[0]);
        request.push(0xee);
        assert_eq!(worker.control(&request), [0x82, 0]);
        assert_eq!(worker.control(&numbers(3, &[SIZE])), [0x82, 1]);
        worker.normal(u64::MAX);
        worker.normal(0);
        worker.send(true, &[]);
        assert!(
            !worker.0.wait().unwrap().success(),
            "empty control frame was accepted"
        );
    }
}
