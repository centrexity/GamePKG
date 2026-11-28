//! Exercise the shared audio SDK over the real service pipe, without an audio device.
#![cfg(any(unix, windows))]
use gamepkg::port_host::{
    audio_streams::{Layout, Mixer},
    serve, Backend, Error, Frame, Result, Session, WindowCommand,
};
#[cfg(unix)]
use std::os::fd::OwnedFd as OwnedPipe;
#[cfg(windows)]
use std::os::windows::io::OwnedHandle as OwnedPipe;
use std::{fs::File, io};

#[path = "../engines/native-services/client.rs"]
#[allow(dead_code)]
mod client;

struct Output(Mixer);

impl Backend for Output {
    fn audio_streams(&self) -> Option<Mixer> {
        Some(self.0.clone())
    }

    fn window(&mut self, _: WindowCommand) -> Result<()> {
        Err(Error::Unsupported)
    }

    fn present(&mut self, _: Frame) -> Result<()> {
        Err(Error::Unsupported)
    }

    fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
        Err(Error::Unsupported)
    }

    fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
        Err(Error::Unsupported)
    }
}

#[test]
fn audio_streams_create_and_play_through_native_service_pipes() {
    let _exclusive = client::TEST_BINDING_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let (host_in, client_out) = io::pipe().unwrap();
    let (client_in, host_out) = io::pipe().unwrap();
    let as_file = |pipe: OwnedPipe| File::from(pipe);
    client::connect(as_file(client_in.into()), as_file(client_out.into())).unwrap();

    let mixer = Mixer::new(Layout::Stereo, 65536);
    let output = mixer.clone();
    let host = std::thread::spawn(move || {
        let mut permissions =
            gamepkg::manifest::Manifest::parse(include_bytes!("../examples/minimal/manifest.toml"))
                .unwrap()
                .permissions;
        permissions.audio = true;
        serve(
            &mut Session::new(permissions, Output(output)),
            host_in,
            host_out,
        )
        .unwrap();
    });

    assert_eq!(
        client::audio_stream_create_with_rate(96_000, 1, 2),
        Err(Error::Unsupported as i32)
    );
    for rate in [22_050, 24_000, 44_100, 48_000] {
        let id = client::audio_stream_create_with_rate(rate, 1, 2).unwrap();
        client::audio_stream_release(id).unwrap();
    }

    let slow = client::audio_stream_create_with_rate(32_000, 1, 2).unwrap();
    let default = client::audio_stream_create(1, 2).unwrap();
    client::audio_stream_submit(slow, &[10, -10, 20, -20]).unwrap();
    client::audio_stream_submit(default, &[1, -1, 2, -2]).unwrap();
    assert_eq!(mixer.frame(), [0; 6]);
    client::audio_stream_control(slow, 1).unwrap();
    client::audio_stream_control(default, 1).unwrap();
    assert_eq!(mixer.frame(), [11, -11, 0, 0, 0, 0]);
    assert_eq!(mixer.frame(), [12, -12, 0, 0, 0, 0]);
    assert_eq!(client::audio_stream_status(slow), Ok([2, 1, 0, 1]));
    assert_eq!(client::audio_stream_status(default), Ok([2, 2, 0, 0]));
    assert_eq!(mixer.frame(), [20, -20, 0, 0, 0, 0]);
    assert_eq!(mixer.frame(), [0; 6]);
    client::audio_stream_release(slow).unwrap();
    client::audio_stream_release(default).unwrap();

    client::disconnect();
    host.join().unwrap();
}
