use backend_wgpu::{GxRenderer, capture};
use gecko::HostInput;
use gecko::flipper::si::pad;
use gecko::flipper::vi::regs::RefreshRate;
use gecko::gamecube::GameCube;
use gecko::host::{GxAction, RenderSink};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const IPL: &[u8] = include_bytes!("../../../../private/IPL.decoded.bin");
const DSP: &[u8] = include_bytes!("../../../../private/dsp_rom.bin");
const COEF: &[u8] = include_bytes!("../../../../private/dsp_coef.bin");

struct SyncSink {
    gx: Arc<Mutex<GxRenderer>>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    scratch: Vec<gecko::host::DrawVertex>,
}

impl RenderSink for SyncSink {
    fn exec(&mut self, action: GxAction) {
        self.gx.lock().unwrap().process_action_with_external_scratch(
            &self.device,
            &self.queue,
            &action,
            &mut self.scratch,
        );
    }

    fn vertex_scratch(&mut self) -> &mut Vec<gecko::host::DrawVertex> {
        &mut self.scratch
    }
}

fn take_screenshot(device: &wgpu::Device, queue: &wgpu::Queue, gx: &GxRenderer, code: &str, frame: u32) {
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    let mut captured = capture::capture_texture(device, queue, &gx.xfb_texture).expect("capture_texture returned None");

    for px in captured.rgba.chunks_exact_mut(4) {
        px[3] = 255;
    }

    let path = format!("screenshotdb/{}/{}.png", code, frame);
    let file = std::fs::File::create(&path).expect("Failed to create PNG file");

    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), captured.width, captured.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header().expect("Failed to write PNG header");
    writer
        .write_image_data(&captured.rgba)
        .expect("Failed to write PNG data");
}

fn main() {
    let file = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("worker requires a path to a single ISO/RVZ/ZIP"),
    );

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("no compatible wgpu adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .expect("failed to acquire wgpu device");

    let surface_format = wgpu::TextureFormat::Rgba8Unorm;

    run_one(&device, &queue, surface_format, &file);
}

fn run_one(device: &wgpu::Device, queue: &wgpu::Queue, surface_format: wgpu::TextureFormat, file: &std::path::Path) {
    let buffer = std::fs::read(file).expect("Failed to read the file");
    let image = image::load_dvd(buffer);

    let name = String::from_utf8_lossy(&image.header().game_name);
    let name = name.trim_end_matches('\0').to_owned();
    let code = String::from_utf8_lossy(&image.header().game_code);
    let code = code.trim_end_matches('\0').to_owned();
    println!("Running: {} ({})", name, code);

    let out_dir = format!("screenshotdb/{}", code);
    std::fs::create_dir_all(&out_dir).expect("Failed to create screenshotdb directory");

    let gx = Arc::new(Mutex::new(GxRenderer::new(device, queue, surface_format)));

    let mut gamecube = GameCube::with_ipl(IPL, true);
    gamecube.dsp.load_irom(DSP);
    gamecube.dsp.load_coef(COEF);

    let mut input = HostInput::gc_connected();
    gamecube.apply_host_input(&input);

    gamecube.render_sink = Box::new(SyncSink {
        gx: gx.clone(),
        device: device.clone(),
        queue: queue.clone(),
        scratch: Vec::new(),
    });

    gamecube.insert_dvd(image);

    let framerate = match gamecube.vi.dcr.video_format().refresh_rate() {
        RefreshRate::Hz50 => 50,
        RefreshRate::Hz60 => 60,
    };

    // Preliminary for IPL skip
    for _ in 0..(framerate * 1) {
        gamecube.run_until_vsync();
    }

    let mut frame: u32 = framerate * 2;
    {
        let g = gx.lock().unwrap();
        take_screenshot(device, queue, &g, &code, frame);
    }

    for idx in 0..20 {
        if let HostInput::Gc(pad) = &mut input {
            pad.stick_y = pad::STICK_CENTER;
            pad.buttons = 0;

            if idx == 3 {
                pad.stick_y = 255;
            } else if idx > 3 && idx % 5 == 0 {
                pad.buttons = pad::A | pad::START;
            }
        }
        gamecube.apply_host_input(&input);

        for _ in 0..(framerate * 2) {
            gamecube.run_until_vsync();
            frame += 1;
        }

        let g = gx.lock().unwrap();
        take_screenshot(device, queue, &g, &code, frame);
    }
}
