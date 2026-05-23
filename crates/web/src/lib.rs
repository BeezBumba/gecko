use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use egui::ViewportId;
use gecko::HostInput;
use gecko::flipper::si::pad::{self, PadStatus, STICK_CENTER, STICK_MAX, STICK_MIN, TRIGGER_MAX, TRIGGER_MIN};
use gecko::flipper::vi::regs::RefreshRate;
use gecko::{GC, GameCube, WII, Wii};
use gecko::host::{DrawVertex, GxAction, RenderSink};
use image::Dol;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
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

fn web_log(message: impl AsRef<str>) {
    web_sys::console::log_1(&JsValue::from_str(message.as_ref()));
}

fn web_warn(message: impl AsRef<str>) {
    web_sys::console::warn_1(&JsValue::from_str(message.as_ref()));
}

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

struct WebSinkShared {
    messages: Vec<ActionMessage>,
}

type WebSinkQueue = Arc<Mutex<WebSinkShared>>;

/// RenderSink that queues actions for synchronous processing on the main thread.
struct WebSink {
    shared: WebSinkQueue,
    /// Vertex scratch handed to the gecko side via
    /// [`RenderSink::vertex_scratch`]. This mirrors the native sink: it
    /// grows across queued actions and is only cleared when an action
    /// resets the renderer's scratch.
    scratch: Vec<DrawVertex>,
    /// How much of `scratch` has been shipped in a prior `exec` message.
    /// New vertices appended past this index are the delta for the next
    /// message.
    scratch_sent_len: usize,
}

impl RenderSink for WebSink {
    fn exec(&mut self, action: GxAction) {
        let mut s = self.shared.lock().unwrap();
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
        &mut self.scratch
    }
}

enum EmulatorInstance {
    Gc(GameCube),
    Wii(Wii),
}

impl EmulatorInstance {
    fn refresh_rate(&self) -> RefreshRate {
        match self {
            Self::Gc(emulator) => emulator.vi.dcr.video_format().refresh_rate(),
            Self::Wii(emulator) => emulator.vi.dcr.video_format().refresh_rate(),
        }
    }

    fn apply_host_input(&mut self, input: &HostInput) {
        match self {
            Self::Gc(emulator) => emulator.apply_host_input(input),
            Self::Wii(emulator) => emulator.apply_host_input(input),
        }
    }

    fn run_until_vsync(&mut self) {
        match self {
            Self::Gc(emulator) => emulator.run_until_vsync(),
            Self::Wii(emulator) => emulator.run_until_vsync(),
        }
    }

    fn load_dsp_irom(&mut self, irom: &[u8]) {
        match self {
            Self::Gc(emulator) => emulator.dsp.load_irom(irom),
            Self::Wii(emulator) => emulator.dsp.load_irom(irom),
        }
    }

    fn load_dsp_coef(&mut self, coef: &[u8]) {
        match self {
            Self::Gc(emulator) => emulator.dsp.load_coef(coef),
            Self::Wii(emulator) => emulator.dsp.load_coef(coef),
        }
    }

    fn neutral_input(&self) -> HostInput {
        match self {
            Self::Gc(_) => HostInput::neutral_for(GC),
            Self::Wii(_) => HostInput::neutral_for(WII),
        }
    }

    fn set_render_sink(&mut self, sink: Box<dyn RenderSink>) {
        match self {
            Self::Gc(emulator) => emulator.render_sink = sink,
            Self::Wii(emulator) => emulator.render_sink = sink,
        }
    }

    fn telemetry_line(&self) -> String {
        match self {
            Self::Gc(emulator) => format!(
                "sys=GC pc={:08X} nia={:08X} cycles={} ee={} vsync_pending={} pi_pending={} pi_intsr={:08X} pi_intmr={:08X} di_irq_active={} di_tstart={} di_status={:08X} di_cover={:08X} di_cmd0={:08X} di_cmd1={:08X} di_cmd2={:08X} di_dma_addr={:08X} di_dma_len={:08X} dvd_present={}",
                emulator.gekko.pc,
                emulator.gekko.nia,
                emulator.scheduler.cycles,
                emulator.gekko.msr.external_interrupt_enable(),
                emulator.vsync_pending,
                emulator.pi.interrupt_pending(),
                emulator.pi.intsr.raw(),
                emulator.pi.intmr.raw(),
                (emulator.di.status.break_complete() && emulator.di.status.break_complete_mask())
                    || (emulator.di.status.device_error() && emulator.di.status.device_error_mask())
                    || (emulator.di.status.transfer_complete() && emulator.di.status.transfer_complete_mask())
                    || (emulator.di.cover.cover_interrupt() && emulator.di.cover.cover_interrupt_mask()),
                emulator.di.control.tstart(),
                emulator.di.status.raw(),
                emulator.di.cover.raw(),
                emulator.di.cmdbuf0,
                emulator.di.cmdbuf1,
                emulator.di.cmdbuf2,
                emulator.di.dma_address.raw(),
                emulator.di.dma_length.raw(),
                emulator.di.dvd.is_some()
            ),
            Self::Wii(emulator) => format!(
                "sys=WII pc={:08X} nia={:08X} cycles={} ee={} vsync_pending={} pi_pending={} pi_intsr={:08X} pi_intmr={:08X} di_irq_active={} di_tstart={} di_status={:08X} di_cover={:08X} di_cmd0={:08X} di_cmd1={:08X} di_cmd2={:08X} di_dma_addr={:08X} di_dma_len={:08X} dvd_present={}",
                emulator.gekko.pc,
                emulator.gekko.nia,
                emulator.scheduler.cycles,
                emulator.gekko.msr.external_interrupt_enable(),
                emulator.vsync_pending,
                emulator.pi.interrupt_pending(),
                emulator.pi.intsr.raw(),
                emulator.pi.intmr.raw(),
                (emulator.di.status.break_complete() && emulator.di.status.break_complete_mask())
                    || (emulator.di.status.device_error() && emulator.di.status.device_error_mask())
                    || (emulator.di.status.transfer_complete() && emulator.di.status.transfer_complete_mask())
                    || (emulator.di.cover.cover_interrupt() && emulator.di.cover.cover_interrupt_mask()),
                emulator.di.control.tstart(),
                emulator.di.status.raw(),
                emulator.di.cover.raw(),
                emulator.di.cmdbuf0,
                emulator.di.cmdbuf1,
                emulator.di.cmdbuf2,
                emulator.di.dma_address.raw(),
                emulator.di.dma_length.raw(),
                emulator.di.dvd.is_some()
            ),
        }
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
    frame_index: u64,
    empty_action_streak: u64,
    queued_vertices: Vec<DrawVertex>,
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
            frame_index: 0,
            empty_action_streak: 0,
            queued_vertices: Vec::new(),
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
        emulator: &mut EmulatorInstance,
        action_queue: &WebSinkQueue,
        #[cfg(feature = "debug")] debug_state: &mut debug_ui::DebugState,
        window: &Window,
    ) {
        self.frame_index = self.frame_index.wrapping_add(1);

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
        match emulator {
            EmulatorInstance::Gc(emulator) => debug_state.tick(emulator),
            EmulatorInstance::Wii(emulator) => emulator.run_until_vsync(),
        }
        #[cfg(not(feature = "debug"))]
        emulator.run_until_vsync();

        // Drain queued action messages.
        let messages: Vec<ActionMessage> = {
            let mut s = action_queue.lock().unwrap();
            std::mem::take(&mut s.messages)
        };
        let action_count = messages.len();
        if action_count == 0 {
            self.empty_action_streak = self.empty_action_streak.saturating_add(1);
        } else {
            self.empty_action_streak = 0;
        }
        if self.frame_index == 1 || self.frame_index % 120 == 0 {
            web_log(format!(
                "[web] frame={} fps={:.1} queued_actions={} empty_action_streak={}",
                self.frame_index, fps, action_count, self.empty_action_streak
            ));
            web_log(format!("[web] {}", emulator.telemetry_line()));
        }
        for msg in messages {
            self.queued_vertices.extend_from_slice(&msg.vertices);
            self.gx_renderer.process_action_with_external_scratch(
                &self.device,
                &self.queue,
                &msg.action,
                &mut self.queued_vertices,
            );
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                web_warn("[web] failed to acquire swapchain texture");
                return;
            }
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
            if let EmulatorInstance::Gc(emulator) = emulator {
                debug_state.show(&ctx, emulator);
            }
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

struct App {
    emulator: EmulatorInstance,
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
    rom_data: &[u8],
    filename: String,
    dsp_irom: Option<Vec<u8>>,
    dsp_coef: Option<Vec<u8>>,
    gc_ipl: Option<Vec<u8>>,
) {
    console_error_panic_hook::set_once();
    web_log(format!(
        "[web] start_emulator file='{}' bytes={} dsp_irom={} dsp_coef={} gc_ipl={}",
        filename,
        rom_data.len(),
        dsp_irom.as_ref().map(|v| v.len()).unwrap_or(0),
        dsp_coef.as_ref().map(|v| v.len()).unwrap_or(0),
        gc_ipl.as_ref().map(|v| v.len()).unwrap_or(0)
    ));

    let name = filename.to_lowercase();
    let mut emulator = if name.ends_with(".bin") || name.ends_with(".ipl") {
        web_log("[web] boot path: GameCube IPL");
        EmulatorInstance::Gc(GameCube::with_ipl(rom_data, false))
    } else if name.ends_with(".dol") {
        web_log("[web] boot path: GameCube DOL");
        let dol = Dol::parse(rom_data.to_vec());
        EmulatorInstance::Gc(GameCube::with_image(&dol))
    } else if name.ends_with(".iso") || name.ends_with(".rvz") || name.ends_with(".zip") {
        web_log("[web] boot path: Disc image");
        let dvd = image::load_dvd(rom_data.to_vec());
        let header = dvd.header();
        let game_name = String::from_utf8_lossy(&header.game_name)
            .trim_end_matches('\0')
            .to_string();
        web_log(format!(
            "[web] disc header game_id={} is_wii={} is_gc={} disk_id={} version={} name='{}'",
            header.game_id(),
            header.is_wii(),
            header.is_gc(),
            header.disk_id,
            header.version,
            game_name
        ));
        if dvd.header().is_wii() {
            web_log("[web] selected Wii apploader HLE");
            EmulatorInstance::Wii(Wii::apploader_hle(dvd).build())
        } else {
            match gc_ipl.as_ref() {
                Some(ipl) => {
                    web_log("[web] selected GameCube real IPL boot (skip enabled)");
                    let mut emulator = GameCube::with_ipl(ipl, true);
                    emulator.insert_dvd(dvd);
                    EmulatorInstance::Gc(emulator)
                }
                None => {
                    web_log("[web] selected GameCube IPL HLE (no GC IPL provided)");
                    EmulatorInstance::Gc(GameCube::with_ipl_hle(dvd))
                }
            }
        }
    } else {
        panic!("unsupported file extension; expected .dol/.bin/.ipl/.iso/.rvz/.zip")
    };

    if let Some(irom) = dsp_irom {
        emulator.load_dsp_irom(&irom);
        web_log("[web] loaded DSP IROM");
    }

    if let Some(coef) = dsp_coef {
        emulator.load_dsp_coef(&coef);
        web_log("[web] loaded DSP COEF");
    }

    let input = emulator.neutral_input();
    emulator.apply_host_input(&input);
    web_log("[web] host input initialized");

    // Install the WebSink as the emulator's render sink.
    let action_queue: WebSinkQueue = Arc::new(Mutex::new(WebSinkShared {
        messages: Vec::new(),
    }));
    emulator.set_render_sink(Box::new(WebSink {
        shared: action_queue.clone(),
        scratch: Vec::new(),
        scratch_sent_len: 0,
    }));

    let event_loop = EventLoop::new().unwrap();
    web_log("[web] event loop created; spawning app");
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
