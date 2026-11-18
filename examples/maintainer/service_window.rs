//! Maintainer smoke fixture. No game content, user ROM, audio or microphone.
#[allow(dead_code)]
#[path = "../../tests/support/adversarial.rs"]
mod fixture;
use gamepkg::port_host::{
    admission::CheckedSources,
    owner::Grants,
    player::{self, Options},
    preparation::{self, LocalLayout, Plan, Target},
};
const SOURCE: &[u8] = br#"
pub fn fixture_task(value:u64)->u64 { value+1 }
fn save(grant:u64) -> Result<(),i32> {
 let previous=gamepkg::file_create_new(grant,"race.sav")?;
 gamepkg::file_write(previous,0,b"old result")?;
 gamepkg::file_close(previous)?;
 let tmp=gamepkg::file_create_new(grant,"race.tmp")?;
 gamepkg::file_write(tmp,0,b"test result")?;
 gamepkg::file_close(tmp)?;
 gamepkg::file_replace(grant,"race.tmp","race.sav",true)
}
fn task() -> Result<u64,i32> {
 let mut request=[0u8;16];
 request[4..12].copy_from_slice(&u64::to_le_bytes(41));
 let mut reply=[0u8;8];
 if gamepkg::local_call(10,&request,&mut reply)?!=8{return Err(-3);}
 let id=u64::from_le_bytes(reply);
 let start=gamepkg::local_performance_sample()?;
 while !gamepkg::local_task_finished(id)? {
  let now=gamepkg::local_performance_sample()?;
  if now.counter-start.counter>start.frequency{return Err(-2);}
 }
 Ok(gamepkg::local_task_join(id)?.result())
}
pub fn game_main(){
 if gamepkg::local_task_status()!=Ok((0,false)) || task()!=Ok(42){return;}
 let grants=match gamepkg::grants(){Ok(v)=>v,Err(_)=>return};
 let mut valid=false;
 for grant in grants {
  if grant.kind==2 && save(grant.id)!=Ok(()){return;}
  if grant.kind==3 {
   let handle=match gamepkg::file_rom_view(grant.id,0,4){Ok(v)=>v,Err(_)=>return};
   let mut bytes=[0u8;4];
   valid=gamepkg::file_read(handle,0,&mut bytes)==Ok(4)&&bytes==*b"test";
   if gamepkg::file_close(handle)!=Ok(()){return;}
  }
 }
 if !valid || render()!=Ok(()){return;}
 let (start,generation)=match gamepkg::monotonic(){Ok(v)=>v,Err(_)=>return};
 let deadline=start+300000000u64;
 let mut now=start;
 let mut wake=generation;
 while now<deadline {
  match gamepkg::wait_until(deadline,wake){Ok((time,generation,_))=>{now=time;wake=generation;},Err(_)=>return}
 }
}
"#;

const RASTER_SHADER: &[u8] = br#"
[shader("vertex")] float4 vertexMain(float2 xy : POSITION) : SV_Position { return float4(xy,0.5,1.0); }
[shader("fragment")] float4 fragmentMain() : SV_Target { return float4(1,0,0,1); }
"#;

fn exit_before_frame() -> bool {
    std::env::var_os("GAMEPKG_FIXTURE_EXIT_BEFORE_FRAME").is_some()
}

const RASTER_GAME: &[u8] = br#"
fn render() -> Result<(),i32> {
 verify_launch_settings()?;
 let vertices=[-1.0f32,-1.0,3.0,-1.0,-1.0,3.0];
 let mut bytes=[0u8;24];
 for (n, position) in vertices.into_iter().enumerate() {
  bytes[n*4..n*4+4].copy_from_slice(&f32::to_le_bytes(position));
 }
 let vertex=gamepkg::buffer_create(0,&bytes)?;
 let index=gamepkg::buffer_create(1,&[0,0,0,0,1,0,0,0,2,0,0,0])?;
 let uniform=gamepkg::buffer_create(2,&[0u8;16])?;
 let target=gamepkg::target_create(64,64)?;
 let draw=gamepkg::IndexedDraw{pipeline:0,vertex,index,uniform,first_index:0,index_count:3,
    base_vertex:0,scissor:[0,0,64,64],textures:&[]};
 gamepkg::target_pass(target,3,0x000000ff,1.0,&[draw],None)?;
 verify_pixel(target,[255,0,0,255])?;
 gamepkg::target_present(target)
}
"#;

const READBACK: &[u8] = br#"
fn verify_launch_settings() -> Result<(),i32> {
 let launch=gamepkg::launch_settings()?;
 if launch.fullscreen!=Some(false) || launch.widescreen!=Some(false)
     || launch.render_scale!=Some(3) || launch.antialiasing!=Some(2)
     || launch.texture_lod!=Some(1) || launch.sharpening_percent!=Some(10)
     || launch.widescreen_hud!=Some(true) || launch.menu_hud_scale!=Some(2) {
  return Err(-3);
 }
 Ok(())
}
fn verify_pixel(target:u64, expected:[u8;4]) -> Result<(),i32> {
 let ticket=gamepkg::surface_readback(target,[32,32,1,1])?;
 let start=gamepkg::local_performance_sample()?;
 let mut rgba=[0u8;4];
 loop {
  match gamepkg::readback_poll(ticket,&mut rgba) {
   Ok(4)=>break,
   Err(-2)=>{
    let now=gamepkg::local_performance_sample()?;
    if now.counter-start.counter>2*start.frequency{return Err(-2);}
   },
   _=>return Err(-3)
  }
 }
 if rgba!=expected{return Err(-3);}
 Ok(())
}
"#;

fn checked(
    root: &std::path::Path,
    raster: bool,
    templates: Option<&std::path::Path>,
) -> anyhow::Result<(CheckedSources, Vec<u8>)> {
    let mut files = fixture::fixture("examples/minimal");
    let mut manifest = gamepkg::manifest::Manifest::parse(&files["manifest.toml"])?;
    manifest.limits.hard_memory_bytes = 512 * 1024 * 1024;
    manifest.requires_rom = true;
    manifest.permissions.save = raster;
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest)?.into_bytes(),
    );
    let mut source = SOURCE.to_vec();
    if let Some(templates) = templates {
        for mask in 0..16 {
            use std::io::Read;
            let name = format!("template_{mask:02}.slang");
            let mut bytes = Vec::new();
            std::fs::File::open(templates.join(&name))?
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(
                bytes.len() <= 1024 * 1024,
                "template fixture exceeds input budget"
            );
            files.insert(format!("shaders/{name}"), bytes);
        }
        source.extend_from_slice(include_bytes!(
            "../../tests/fixtures/service_takedawn_templates.rs"
        ));
        source.extend_from_slice(READBACK);
    } else if raster {
        files.insert("shaders/native.slang".into(), RASTER_SHADER.to_vec());
        source.extend_from_slice(if exit_before_frame() {
            b"fn render() -> Result<(), i32> { Ok(()) }"
        } else {
            RASTER_GAME
        });
        source.extend_from_slice(READBACK);
    } else {
        source.extend_from_slice(
            b"fn render() -> Result<(), i32> { gamepkg::present(0x2070a0ff,&[]) }",
        );
    }
    files.insert("source/src/lib.rs".into(), source);
    let path = fixture::sign(&files, root);
    let sources = CheckedSources::verify(&mut gamepkg::archive::Package::open(&path)?)?;
    Ok((sources, std::fs::read(path)?))
}

fn grants(root: &std::path::Path) -> anyhow::Result<Grants> {
    let path = root.join("image");
    std::fs::write(&path, b"test")?;
    Ok(Grants {
        roms: vec![std::fs::File::open(&path)?],
        ..Grants::default()
    })
}

fn options() -> Options {
    Options {
        title: "Shared-service smoke test".into(),
        visible: std::env::args().any(|arg| arg == "--visible"),
        mute_audio: true,
        ..Options::default()
    }
}

fn fixture_plan(target: Target) -> anyhow::Result<Plan> {
    Plan::new(
        target,
        "game_main",
        vec!["fixture_task".into()],
        LocalLayout::None,
        true,
    )
}

fn main() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    if std::env::args().any(|arg| arg == "--takedawn-templates") {
        let path = std::env::var_os("GAMEPKG_TAKEDAWN_TEMPLATE_ROOT")
            .map(std::path::PathBuf::from)
            .ok_or_else(|| {
                anyhow::anyhow!("select the reviewed service-renderer/shaders directory")
            })?;
        let (sources, archive) = checked(root.path(), true, Some(&path))?;
        return link_and_run(root.path(), &sources, &archive, true);
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    if std::env::args().any(|arg| arg == "--local") {
        let (sources, archive) = checked(root.path(), true, None)?;
        return link_and_run(root.path(), &sources, &archive, false);
    }
    let (sources, _) = checked(root.path(), false, None)?;
    let target = if cfg!(windows) {
        Target::Windows64
    } else {
        Target::Linux64
    };
    let plan = fixture_plan(target)?;
    let artifact = preparation::build(root.path(), &sources, &plan, vec![], vec![])?;
    let report = player::run(artifact, vec![], grants(root.path())?, options())?;
    anyhow::ensure!(
        report.presented_frames > 0,
        "no hardware surface frames presented"
    );
    println!("{report:?}");
    println!("Synthetic game-only window/pipe lifecycle completed; no audio, controls or gameplay qualification.");
    Ok(())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn link_and_run(
    root: &std::path::Path,
    sources: &CheckedSources,
    archive: &[u8],
    takedawn: bool,
) -> anyhow::Result<()> {
    use anyhow::ensure;
    use eframe::wgpu;
    use gamepkg::{
        launch_options::{AspectRatio, LaunchOptions},
        port_host::process::NativeLaunch,
    };
    use gamepkg::{
        manifest::Shader,
        port_host::graphics::{takedawn_nvi1_recipes, PipelineRecipe, VerifiedPipelines},
    };
    use preparation::Bootstrap;
    let plan = if takedawn {
        Plan::new(
            Target::Linux64,
            "game_main",
            vec!["fixture_task".into()],
            LocalLayout::TakedawnServices,
            true,
        )?
    } else {
        fixture_plan(Target::Linux64)?
    };
    let launch = NativeLaunch {
        graphics: LaunchOptions {
            aspect_ratio: AspectRatio::Standard,
            render_scale: 3,
            ..Default::default()
        }
        .into(),
        visible: options().visible,
        mute_audio: true,
        volume: 50,
    };
    let recipe = || PipelineRecipe {
        vertex: Shader {
            source: "shaders/native.slang".into(),
            entry: "vertexMain".into(),
            stage: "vertex".into(),
        },
        fragment: Shader {
            source: "shaders/native.slang".into(),
            entry: "fragmentMain".into(),
            stage: "fragment".into(),
        },
        source_hashes: None,
        stride: 8,
        attributes: vec![wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        }],
        uniform_bytes: 16,
        textures: vec![],
        blend: None,
        cull: None,
        depth_write: false,
        depth_compare: wgpu::CompareFunction::Always,
        raster: Default::default(),
        uniform_policy: Default::default(),
    };
    let settings = gamepkg::settings::Settings {
        shader_backend: std::env::var("GAMEPKG_FIXTURE_SHADER_BACKEND")
            .unwrap_or_else(|_| "wgsl".into()),
        ..Default::default()
    };
    let recipes = || {
        if takedawn {
            takedawn_nvi1_recipes()
        } else {
            vec![recipe()]
        }
    };
    let (different, _) = checked(tempfile::tempdir()?.path(), false, None)?;
    let error = VerifiedPipelines::prepare(root, &different, archive, &settings, recipes(), 1)
        .err()
        .expect("other signed source kit must be refused");
    ensure!(
        error
            .to_string()
            .contains("shader package differs from checked game source kit"),
        "unexpected mismatched shader-package error: {error}"
    );
    if takedawn {
        let mut wrong_pin = takedawn_nvi1_recipes();
        wrong_pin[0].source_hashes = wrong_pin[1].source_hashes;
        let error = VerifiedPipelines::prepare(root, sources, archive, &settings, wrong_pin, 1)
            .err()
            .expect("signed source must match reviewed Slang pin");
        ensure!(
            error
                .to_string()
                .contains("shader source differs from reviewed template"),
            "unexpected template hash error: {error}"
        );
    }
    let pipelines = VerifiedPipelines::prepare(root, sources, archive, &settings, recipes(), 1)?;
    let build_cache = std::env::var_os("GAMEPKG_FIXTURE_BUILD_CACHE").map(std::path::PathBuf::from);
    let artifact = preparation::build_in_process(
        build_cache.as_deref().unwrap_or(root),
        sources,
        &plan,
        Some(pipelines),
    )?;
    let receipt = artifact.receipt();
    ensure!(
        receipt.bootstrap == Bootstrap::InProcess,
        "wrong linked runtime"
    );
    ensure!(
        receipt.game.source_kit == sources.identity(),
        "wrong source-kit identity"
    );
    ensure!(
        receipt.game.objects.len() == 1 && receipt.raw_objects == receipt.game.objects,
        "missing exact game object provenance"
    );
    let pipeline_count = if takedawn { 16 } else { 1 };
    ensure!(
        receipt.game.shaders.len() == pipeline_count && receipt.shader_preparation.len() == 1,
        "missing verified pipeline and Slang identities"
    );
    let published = artifact.publish(&root.join("cache"))?;
    ensure!(
        gamepkg::verify::digest(&std::fs::read(&published)?) == receipt.binary,
        "published executable differs from receipt"
    );
    let rom = grants(root)?.roms.pop().expect("one synthetic ROM");
    let library = root.join("library");
    std::fs::create_dir(&library)?;
    let save = root.join("saves");
    std::fs::create_dir(&save)?;
    let writable = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join("image"))?;
    let denied = gamepkg::port_host::process::Process::start_in_process(
        &artifact,
        &library,
        Some(writable),
        None,
        launch,
    )
    .err()
    .expect("writable ROM descriptor must be refused");
    ensure!(
        denied.to_string().contains("read-only regular file"),
        "unexpected writable ROM error: {denied}"
    );
    let mut process = gamepkg::port_host::process::Process::start_in_process(
        &artifact,
        &library,
        Some(rom),
        Some((&save, 1024 * 1024)),
        launch,
    )?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        if let Some(status) = process.poll()? {
            ensure!(
                status == 0,
                "linked window fixture exited {status}: {}",
                process.diagnostics()
            );
            break;
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "linked window fixture timed out: {}",
            process.diagnostics()
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    println!("{}", process.diagnostics());
    let expects_frame = takedawn || !exit_before_frame();
    ensure!(
        process.first_frame_presented() == expects_frame,
        "unexpected native frame readiness"
    );
    let history = gamepkg::progress::load(
        &library,
        &sources.manifest().id,
        &sources.manifest().creator_key,
    )?;
    ensure!(
        (history.played_ms > 0) == expects_frame && history.played_ms < 2000,
        "expected play time for the presented fixture: {} ms",
        history.played_ms
    );
    let mut scopes = std::fs::read_dir(save.join("port-state"))?;
    let scope = scopes
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing private save scope"))??;
    ensure!(
        scopes.next().is_none(),
        "save was mounted outside the title scope"
    );
    let files = scope.path().join("files");
    ensure!(
        std::fs::read(files.join("race.sav"))? == b"test result"
            && !files.join("race.tmp").exists(),
        "atomic save did not publish the expected contents"
    );
    if expects_frame {
        println!("Audited fixture verified {pipeline_count} signed pipeline(s) and recorded {} ms of play.", history.played_ms);
    } else {
        println!("Audited fixture exited before its first frame and recorded 0 ms of play.");
    }
    Ok(())
}
