use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use egui::ViewportId;
use gecko::HostInput;
use gecko::flipper::si::pad::{self, PadStatus, STICK_CENTER, STICK_MAX, STICK_MIN, TRIGGER_MAX, TRIGGER_MIN};
use gecko::flipper::vi::regs::RefreshRate;
use gecko::gamecube::GameCube;
use gecko::system::{GC, WII};
use gecko::host::{DrawVertex, GxAction, RenderSink};
use gecko::wii::Wii;
use image::{Dol, load_dvd};
use wasm_bindgen::prelude::*;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys};
use winit::window::{Window, WindowId};

#[cfg(feature = "debug")]
mod debug_ui;

const BLIT_SHADER: &str = "
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VsOut;
    out.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let color = textureSample(src_tex, src_samp, in.uv);
    return vec4<f32>(color.rgb, 1.0);
}
";

/// One queued [`GxAction`] alongside the vertices appended to the sink's
/// scratch buffer since the previous action. The main-thread drainer
/// extends the renderer's `scratch_vertices` with `vertices` *before*
/// processing `action`, so each draw's `base_vertex` indexes correctly
/// even when an earlier action (e.g. `CopyXfb`) cleared the renderer's
/// scratch mid-batch.
struct ActionMessage {
    action: GxAction,
    vertices: Vec<DrawVertex>,
}

/// Shared between [`WebSink`] (running on the emulator side) and the main
/// thread that drains the queue. The `epoch` is bumped each time the main
/// thread drains; the sink uses it to know when to reset its mirrored
/// vertex scratch.
struct WebSinkShared {
    messages: Vec<ActionMessage>,
    epoch: u64,
}

type WebSinkQueue = Arc<Mutex<WebSinkShared>>;

/// RenderSink that queues actions for synchronous processing on the main thread.
struct WebSink {
    shared: WebSinkQueue,
    /// Vertex scratch handed to the gecko side via
    /// [`RenderSink::vertex_scratch`]. Cleared when the main thread bumps
    /// `shared.epoch` (frame boundary) and again on any action that resets
    /// the renderer's scratch — keeping the gecko side's `base_vertex` in
    /// step with the per-message deltas we ship to the drainer.
    scratch: Vec<DrawVertex>,
    /// How much of `scratch` has been shipped in a prior `exec` message.
    /// New vertices appended past this index are the delta for the next
    /// message.
    scratch_sent_len: usize,
    last_epoch: u64,
}

impl RenderSink for WebSink {
    fn exec(&mut self, action: GxAction) {
        let mut s = self.shared.lock().unwrap();
        if s.epoch != self.last_epoch {
            self.scratch.clear();
            self.scratch_sent_len = 0;
            self.last_epoch = s.epoch;
        }
        let vertices = if self.scratch.len() > self.scratch_sent_len {
            self.scratch[self.scratch_sent_len..].to_vec()
        } else {
            Vec::new()
        };
        self.scratch_sent_len = self.scratch.len();
        let resets = backend_wgpu::sink::action_resets_vertex_scratch(&action);
        s.messages.push(ActionMessage { action, vertices });
        drop(s);
        if resets {
            self.scratch.clear();
            self.scratch_sent_len = 0;
        }
    }

    fn vertex_scratch(&mut self) -> &mut Vec<DrawVertex> {
        let s = self.shared.lock().unwrap();
        if s.epoch != self.last_epoch {
            self.scratch.clear();
            self.scratch_sent_len = 0;
            self.last_epoch = s.epoch;
        }
        drop(s);
        &mut self.scratch
    }
}

struct State {
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    gx_renderer: backend_wgpu::GxRenderer,
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,
    egui_ctx: egui::Context,
    egui_renderer: egui_wgpu::Renderer,
    egui_winit: egui_winit::State,
    fps_history: VecDeque<[f64; 2]>,
    start_ms: f64,
    last_frame_ms: f64,
}

fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

impl State {
    async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("failed to find a suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("failed to create device");

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let gx_renderer = backend_wgpu::GxRenderer::new(&device, &queue, surface_format);

        // Blit pipeline (same as sink::Renderer)
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit_shader"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });
        let blit_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&blit_bind_group_layout)],
            immediate_size: 0,
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit_pipeline"),
            layout: Some(&blit_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let egui_ctx = egui::Context::default();
        #[cfg(feature = "debug")]
        {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            fonts.font_data.insert(
                "phosphor-fill".into(),
                egui_phosphor::Variant::Fill.font_data().into(),
            );
            fonts.families.insert(
                egui::FontFamily::Name("phosphor-fill".into()),
                vec!["phosphor-fill".into()],
            );
            egui_ctx.set_fonts(fonts);
        }
        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, egui_wgpu::RendererOptions::default());
        let egui_winit = egui_winit::State::new(egui_ctx.clone(), ViewportId::ROOT, window.as_ref(), None, None, None);

        let now = now_ms();

        State {
            surface,
            surface_config,
            device,
            queue,
            gx_renderer,
            blit_pipeline,
            blit_bind_group_layout,
            blit_sampler,
            egui_ctx,
            egui_renderer,
            egui_winit,
            fps_history: VecDeque::new(),
            start_ms: now,
            last_frame_ms: now,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    fn render(
        &mut self,
        emulator: &mut EmulatorVariant,
        action_queue: &WebSinkQueue,
        #[cfg(feature = "debug")] debug_state: &mut debug_ui::DebugState,
        window: &Window,
    ) {
        let now = now_ms();
        let delta = (now - self.last_frame_ms) / 1000.0;
        self.last_frame_ms = now;
        let fps = if delta > 0.0 { 1.0 / delta } else { 0.0 };
        let elapsed = (now - self.start_ms) / 1000.0;
        self.fps_history.push_back([elapsed, fps]);
        while self.fps_history.front().is_some_and(|e| elapsed - e[0] > 5.0) {
            self.fps_history.pop_front();
        }
        let native_hz = match emulator.refresh_rate() {
            RefreshRate::Hz60 => 60.0_f64,
            RefreshRate::Hz50 => 50.0_f64,
        };
        let native_pct = (fps / native_hz) * 100.0;

        // Run emulation (queues GxActions into the WebSink).
        #[cfg(feature = "debug")]
        emulator.tick_with_debug(debug_state);
        #[cfg(not(feature = "debug"))]
        emulator.run_until_vsync();

        // Drain queued action messages.
        let messages: Vec<ActionMessage> = {
            let mut s = action_queue.lock().unwrap();
            s.epoch = s.epoch.wrapping_add(1);
            std::mem::take(&mut s.messages)
        };
        // Start the frame with an empty external scratch; each message
        // contributes its vertex delta to it before its action runs, and
        // actions that flush the renderer's scratch truncate the external
        // one back to zero in lockstep.
        let mut external_scratch = self.gx_renderer.replace_vertex_scratch(Vec::new());
        external_scratch.clear();
        for msg in messages {
            external_scratch.extend_from_slice(&msg.vertices);
            self.gx_renderer.process_action_with_external_scratch(
                &self.device,
                &self.queue,
                &msg.action,
                &mut external_scratch,
            );
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());

        // Blit the GxRenderer's XFB output to the swapchain.
        self.blit_xfb(&view);

        // egui overlay
        let raw_input = self.egui_winit.take_egui_input(window);
        let full_output = self.egui_ctx.run_ui(raw_input, |ui| {
            let ctx = ui.ctx().clone();
            let frame_style =
                egui::Frame::window(&ctx.global_style()).fill(egui::Color32::from_rgba_unmultiplied(20, 20, 20, 180));
            egui::Window::new("perf_hud")
                .title_bar(false)
                .resizable(false)
                .movable(false)
                .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
                .frame(frame_style)
                .show(&ctx, |ui| {
                    ui.label(egui::RichText::new(format!("{fps:.1} FPS  {native_pct:.1}%")).monospace());
                });

            #[cfg(feature = "debug")]
            emulator.show_debug(debug_state, &ctx);
        });

        self.egui_winit
            .handle_platform_output(window, full_output.platform_output);

        let screen_desc = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: window.scale_factor() as f32,
        };
        let tris = self
            .egui_ctx
            .tessellate(full_output.shapes, screen_desc.pixels_per_point);

        for (id, delta) in full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, id, &delta);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.egui_renderer
            .update_buffers(&self.device, &self.queue, &mut encoder, &tris, &screen_desc);
        {
            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.egui_renderer
                .render(&mut rpass.forget_lifetime(), &tris, &screen_desc);
        }
        self.queue.submit([encoder.finish()]);

        for id in full_output.textures_delta.free {
            self.egui_renderer.free_texture(&id);
        }

        frame.present();
    }

    fn blit_xfb(&self, target: &wgpu::TextureView) {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit_bg"),
            layout: &self.blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.gx_renderer.xfb_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                },
            ],
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("xfb_blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            rpass.set_pipeline(&self.blit_pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
    }
}

// shared between async wgpu init and the winit event loop
type SharedState = Rc<RefCell<Option<State>>>;

enum EmulatorVariant {
    Gc(GameCube),
    Wii(Wii),
}

impl EmulatorVariant {
    fn system_id(&self) -> gecko::system::SystemId {
        match self {
            Self::Gc(_) => GC,
            Self::Wii(_) => WII,
        }
    }

    fn apply_host_input(&mut self, input: &HostInput) {
        match self {
            Self::Gc(emu) => emu.apply_host_input(input),
            Self::Wii(emu) => emu.apply_host_input(input),
        }
    }

    fn run_until_vsync(&mut self) {
        match self {
            Self::Gc(emu) => emu.run_until_vsync(),
            Self::Wii(emu) => emu.run_until_vsync(),
        }
    }

    fn refresh_rate(&self) -> RefreshRate {
        match self {
            Self::Gc(emu) => emu.vi.dcr.video_format().refresh_rate(),
            Self::Wii(emu) => emu.vi.dcr.video_format().refresh_rate(),
        }
    }

    fn render_sink_mut(&mut self) -> &mut Box<dyn RenderSink> {
        match self {
            Self::Gc(emu) => &mut emu.render_sink,
            Self::Wii(emu) => &mut emu.render_sink,
        }
    }

    fn load_dsp_irom(&mut self, irom: &[u8]) {
        match self {
            Self::Gc(emu) => emu.dsp.load_irom(irom),
            Self::Wii(emu) => emu.dsp.load_irom(irom),
        }
    }

    #[cfg(feature = "debug")]
    fn tick_with_debug(&mut self, debug_state: &mut debug_ui::DebugState) {
        if let Self::Gc(emu) = self {
            debug_state.tick(emu);
        } else {
            self.run_until_vsync();
        }
    }

    #[cfg(feature = "debug")]
    fn show_debug(&self, debug_state: &mut debug_ui::DebugState, ctx: &egui::Context) {
        if let Self::Gc(emu) = self {
            debug_state.show(ctx, emu);
        }
    }
}

struct App {
    emulator: EmulatorVariant,
    input: HostInput,
    action_queue: WebSinkQueue,
    window: Option<Arc<Window>>,
    state: SharedState,
    #[cfg(feature = "debug")]
    debug_state: debug_ui::DebugState,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Gecko").with_append(true))
                .unwrap(),
        );

        let shared = self.state.clone();
        let win = window.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let state = State::new(win.clone()).await;
            *shared.borrow_mut() = Some(state);
            win.request_redraw();
        });

        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        {
            if let Some(state) = self.state.borrow_mut().as_mut() {
                if let Some(window) = self.window.as_ref() {
                    let _ = state.egui_winit.on_window_event(window, &event);
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(state) = self.state.borrow_mut().as_mut() {
                    state.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state.is_pressed();
                    if let PhysicalKey::Code(key) = event.physical_key {
                        if let HostInput::Gc(pad) = &mut self.input {
                            update_pad(pad, key, pressed);
                        }
                        self.emulator.apply_host_input(&self.input);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(window) = self.window.clone() {
                    if let Some(state) = self.state.borrow_mut().as_mut() {
                        state.render(
                            &mut self.emulator,
                            &self.action_queue,
                            #[cfg(feature = "debug")]
                            &mut self.debug_state,
                            &window,
                        );
                    }
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

#[wasm_bindgen]
pub fn start_emulator(
    boot_mode: String,
    primary_data: &[u8],
    primary_name: String,
    disc_data: Option<Vec<u8>>,
    disc_name: Option<String>,
    dsp_irom: Option<Vec<u8>>,
) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    // NOTE: Baseline web profiling points:
    // - frame loop: `State::render`
    // - action queue drain: `State::render` message drain block
    // - boot dispatch: `start_emulator`
    //
    // Browser hard blocker for native JIT:
    // gecko web builds with `default-features = false`, so Cranelift native JIT
    // backends are disabled; `cranelift_jit` in PPC/DSP JIT paths targets host ISA
    // and executable memory and is not browser-compatible.

    let mode = boot_mode.to_ascii_lowercase();
    let primary_name_lc = primary_name.to_ascii_lowercase();
    let mut emulator = match mode.as_str() {
        "dol" => {
            let dol = Dol::parse(primary_data.to_vec());
            EmulatorVariant::Gc(GameCube::with_image(&dol))
        }
        "ipl" => {
            let mut emu = GameCube::with_ipl(primary_data, false);
            if let Some(disc_bytes) = disc_data {
                emu.insert_dvd(load_dvd(disc_bytes));
            }
            EmulatorVariant::Gc(emu)
        }
        "disc" => {
            let dvd = load_dvd(primary_data.to_vec());
            if dvd.header().is_wii() {
                web_sys::console::warn_1(
                    &"Wii disc detected. Browser Wii support requires a wasm-compatible NAND FS backend and may not boot yet."
                        .into(),
                );
                EmulatorVariant::Wii(Wii::apploader_hle(dvd).build())
            } else {
                EmulatorVariant::Gc(GameCube::with_ipl_hle(dvd))
            }
        }
        _ => {
            // Backward-compatible fallback for older pages passing only filename semantics.
            if primary_name_lc.ends_with(".bin") || primary_name_lc.ends_with(".ipl") {
                EmulatorVariant::Gc(GameCube::with_ipl(primary_data, false))
            } else if primary_name_lc.ends_with(".iso")
                || primary_name_lc.ends_with(".rvz")
                || primary_name_lc.ends_with(".zip")
            {
                let dvd = load_dvd(primary_data.to_vec());
                if dvd.header().is_wii() {
                    EmulatorVariant::Wii(Wii::apploader_hle(dvd).build())
                } else {
                    EmulatorVariant::Gc(GameCube::with_ipl_hle(dvd))
                }
            } else {
                let dol = Dol::parse(primary_data.to_vec());
                EmulatorVariant::Gc(GameCube::with_image(&dol))
            }
        }
    };

    if let Some(irom) = dsp_irom {
        emulator.load_dsp_irom(&irom);
    }

    if let Some(name) = disc_name {
        web_sys::console::log_1(&format!("Loaded disc: {name}").into());
    }

    let input = HostInput::neutral_for(emulator.system_id());
    emulator.apply_host_input(&input);

    // Install the WebSink as the emulator's render sink.
    let action_queue: WebSinkQueue = Arc::new(Mutex::new(WebSinkShared {
        messages: Vec::new(),
        epoch: 0,
    }));
    *emulator.render_sink_mut() = Box::new(WebSink {
        shared: action_queue.clone(),
        scratch: Vec::new(),
        scratch_sent_len: 0,
        last_epoch: 0,
    });

    let event_loop = EventLoop::new().unwrap();
    let app = App {
        emulator,
        input,
        action_queue,
        window: None,
        state: Rc::new(RefCell::new(None)),
        #[cfg(feature = "debug")]
        debug_state: debug_ui::DebugState::default(),
    };

    event_loop.spawn_app(app);
    Ok(())
}

fn update_pad(pad: &mut PadStatus, key: KeyCode, pressed: bool) {
    let set_button = |buttons: &mut u16, mask: u16, on: bool| {
        if on {
            *buttons |= mask;
        } else {
            *buttons &= !mask;
        }
    };

    match key {
        KeyCode::ArrowUp => pad.stick_y = if pressed { STICK_MAX } else { STICK_CENTER },
        KeyCode::ArrowDown => pad.stick_y = if pressed { STICK_MIN } else { STICK_CENTER },
        KeyCode::ArrowLeft => pad.stick_x = if pressed { STICK_MIN } else { STICK_CENTER },
        KeyCode::ArrowRight => pad.stick_x = if pressed { STICK_MAX } else { STICK_CENTER },
        KeyCode::KeyX => set_button(&mut pad.buttons, pad::A, pressed),
        KeyCode::KeyZ => set_button(&mut pad.buttons, pad::B, pressed),
        KeyCode::KeyC => set_button(&mut pad.buttons, pad::X, pressed),
        KeyCode::KeyV => set_button(&mut pad.buttons, pad::Y, pressed),
        KeyCode::Enter => set_button(&mut pad.buttons, pad::START, pressed),
        KeyCode::KeyA => {
            set_button(&mut pad.buttons, pad::L, pressed);
            pad.trigger_left = if pressed { TRIGGER_MAX } else { TRIGGER_MIN };
        }
        KeyCode::KeyS => {
            set_button(&mut pad.buttons, pad::R, pressed);
            pad.trigger_right = if pressed { TRIGGER_MAX } else { TRIGGER_MIN };
        }
        KeyCode::KeyD => set_button(&mut pad.buttons, pad::Z, pressed),
        KeyCode::KeyI => set_button(&mut pad.buttons, pad::DPAD_UP, pressed),
        KeyCode::KeyK => set_button(&mut pad.buttons, pad::DPAD_DOWN, pressed),
        KeyCode::KeyJ => set_button(&mut pad.buttons, pad::DPAD_LEFT, pressed),
        KeyCode::KeyL => set_button(&mut pad.buttons, pad::DPAD_RIGHT, pressed),
        _ => {}
    }
}
