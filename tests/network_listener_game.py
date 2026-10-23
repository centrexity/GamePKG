"""Signed native/core/component TCP listener, partial I/O and handle cleanup."""
from pathlib import Path
import os, shutil, socket, subprocess, tempfile, threading, time

project = Path(__file__).resolve().parents[1]
cli = Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR', project/'target/release'))/'gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-listener-') as temporary:
    root = Path(temporary)
    (root/'key').write_text('49'*32)
    (root/'settings.toml').write_text('automatic_update_checks=false\n')
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args):
        result = subprocess.run([str(cli), *map(str, args)], env=env, capture_output=True, text=True, timeout=120)
        assert result.returncode == 0, result.stdout+result.stderr
        return result.stdout
    for profile in ['native', 'core-v1', 'component-v1']:
        with socket.socket() as reservation:
            reservation.bind(('127.0.0.1', 0))
            port = reservation.getsockname()[1]
        recipe = root/profile
        shutil.copytree(project/'examples/minimal', recipe)
        game_id = 'com.gamepkg.listener-'+profile
        manifest = recipe/'manifest.toml'
        text = manifest.read_text().replace('com.gamepkg.minimal', game_id).replace('network = "none"', 'network = true')
        if profile != 'native': text = f'runtime_profile="{profile}"\n'+text
        manifest.write_text(text)
        (recipe/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,state:u64)->u64 {
            if state>>32==42 || state>>32==999 {return state;}
            let listener=(state&65535) as u32;
            let connection=((state>>16)&65535) as u32;
            if listener==0 {
                return match gamepkg::tcp_listen("127.0.0.1:PORT") {
                    Ok(handle)=>u64::from(handle),
                    Err(gamepkg::NetworkError::Pending)=>state,
                    _=>999u64<<32,
                };
            }
            if connection==0 {
                return match gamepkg::tcp_accept(listener) {
                    Ok(handle)=>state|(u64::from(handle)<<16),
                    Err(gamepkg::NetworkError::Pending)=>state,
                    _=>999u64<<32,
                };
            }
            let offset=(state>>32) as usize;
            let mut next=offset;
            if offset<4 {
                let mut bytes=[0u8;4];
                match gamepkg::network_receive(connection,&mut bytes[..1]) {
                    Ok(count)=>{
                        if count==0 {return 999u64<<32;}
                        for index in 0..count as usize {
                            if bytes[index]!=b"ping"[offset+index] {return 999u64<<32;}
                        }
                        next+=count as usize;
                    }
                    Err(gamepkg::NetworkError::Pending)=>return state,
                    _=>return 999u64<<32,
                }
            } else if offset<8 {
                match gamepkg::network_send(connection,&b"pong"[offset-4..offset-3]) {
                    Ok(count)=>next+=count as usize,
                    Err(gamepkg::NetworkError::Pending)=>return state,
                    _=>return 999u64<<32,
                }
            } else {
                let closed=gamepkg::network_close(connection);
                let listener_closed=gamepkg::network_close(listener);
                let stale_rejected=match gamepkg::network_receive(connection,&mut [0u8;1]) {Err(gamepkg::NetworkError::PermissionDenied)=>true,_=>false};
                return if closed && listener_closed && stale_rejected {42u64<<32} else {999u64<<32};
            }
            (state&4294967295)|(next as u64)<<32
        }'''.replace('PORT', str(port)))
        archive = root/(profile+'.gamepkg')
        run('pack', recipe, '-o', archive, '--key', root/'key')
        run('install', archive, '--trust-creator')
        run('build', game_id, '--target', 'native' if profile == 'native' else 'wasm')
        errors = []
        stop = threading.Event()
        def client():
            try:
                deadline = time.monotonic()+10
                while not stop.is_set():
                    try:
                        stream = socket.create_connection(('127.0.0.1', port), timeout=.1)
                        break
                    except OSError:
                        assert time.monotonic()<deadline, 'Listener never became available'
                        stop.wait(.001)
                else:
                    raise AssertionError('Game ended before a connection')
                with stream:
                    stream.settimeout(5)
                    # Deliver separate writes; the guest retains every partial read.
                    for byte in b'ping': stream.sendall(bytes([byte]))
                    reply = bytearray()
                    while len(reply)<4:
                        chunk = stream.recv(4-len(reply))
                        assert chunk, reply
                        reply.extend(chunk)
                    assert reply == b'pong', reply
                    assert stream.recv(1) == b'', 'Accepted socket was not closed'
            except BaseException as error:
                errors.append(error)
        worker = threading.Thread(target=client)
        worker.start()
        try:
            output = run('run', game_id, '--frames', '10000')
            assert 'score: 42' in output, output
        finally:
            stop.set()
            worker.join(7)
        assert not worker.is_alive(), 'Client did not stop'
        assert not errors, errors
        print(profile+': signed TCP listener/accept, partial ping/pong, close and stale-handle rejection passed.')
