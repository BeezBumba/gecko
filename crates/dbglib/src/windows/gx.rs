use egui::{Context, Grid, ScrollArea};
use gecko::flipper::gx::GraphicsProcessor;
use gecko::flipper::gx::draw::TextureDescriptor;
use gecko::mmio::Mmio;

fn texture_preview(ui: &mut egui::Ui, tex: &TextureDescriptor, ram: &[u8]) {
    let rgba = gecko::flipper::gx::texture::decode_to_rgba(ram, tex);
    let size = [tex.width as usize, tex.height as usize];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
    let handle = ui.ctx().load_texture(
        format!("tex_preview_{:08x}", tex.ram_addr),
        image,
        egui::TextureOptions::NEAREST,
    );
    let scale = 256.0 / tex.width.max(tex.height) as f32;
    let display = egui::vec2(tex.width as f32 * scale, tex.height as f32 * scale);
    ui.image(egui::load::SizedTexture::new(handle.id(), display));
    ui.separator();
    ui.label(format!("RAM: 0x{:08X}", tex.ram_addr));
    ui.label(format!("Wrap S: {:?}  T: {:?}", tex.wrap_s, tex.wrap_t));
    ui.label(format!("Mag: {:?}  Min: {:?}", tex.mag_filter, tex.min_filter));
}

pub fn show_gx(ctx: &Context, open: &mut bool, gx: &GraphicsProcessor, mmio: &Mmio) {
    egui::Window::new("GX").open(open).show(ctx, |ui| {
        ScrollArea::vertical().show(ui, |ui| {
            // Projection matrix
            ui.collapsing("Projection", |ui| {
                Grid::new("proj").num_columns(4).show(ui, |ui| {
                    for row in 0..4 {
                        for col in 0..4 {
                            ui.monospace(format!("{:+9.4}", gx.projection.0[col][row]));
                        }
                        ui.end_row();
                    }
                });
            });

            // Viewport / Scissor
            ui.collapsing("Viewport / Scissor", |ui| {
                Grid::new("vp_sc").num_columns(2).striped(true).show(ui, |ui| {
                    let vp = &gx.cur_viewport;
                    ui.label("Viewport");
                    ui.monospace(format!(
                        "({:.0}, {:.0}) {}x{} depth [{:.2}, {:.2}]",
                        vp.x, vp.y, vp.w, vp.h, vp.min_depth, vp.max_depth
                    ));
                    ui.end_row();

                    let sc = &gx.cur_scissor;
                    ui.label("Scissor");
                    ui.monospace(format!("({}, {}) {}x{}", sc.x, sc.y, sc.w, sc.h));
                    ui.end_row();

                    ui.label("Scissor Offset");
                    ui.monospace(format!("({}, {})", gx.cur_scissor_offset_x, gx.cur_scissor_offset_y));
                    ui.end_row();
                });
            });

            // Blend / Depth
            ui.collapsing("Output (PE)", |ui| {
                Grid::new("pe_state").num_columns(2).striped(true).show(ui, |ui| {
                    let bm = gx.cur_blend_mode;
                    ui.label("Blend");
                    if bm.blend_enable() {
                        ui.monospace(format!(
                            "{:?} -> {:?}{}",
                            bm.src_factor(),
                            bm.dst_factor(),
                            if bm.subtract() { " (sub)" } else { "" },
                        ));
                    } else {
                        ui.label("disabled");
                    }
                    ui.end_row();

                    let zm = gx.cur_zmode;
                    ui.label("Depth");
                    if zm.enable() {
                        ui.monospace(format!("{:?} write={}", zm.func(), zm.update_enable()));
                    } else {
                        ui.label("disabled");
                    }
                    ui.end_row();

                    let ac = gx.cur_alpha_compare;
                    ui.label("Alpha Compare");
                    ui.monospace(format!("{:?} / {:?}", ac.comp0(), ac.comp1()));
                    ui.end_row();
                });
            });

            // TEV configuration
            ui.collapsing(format!("TEV ({} stages)", gx.cur_num_tev_stages), |ui| {
                for stage in 0..gx.cur_num_tev_stages as usize {
                    let color = gx.cur_tev_color_env[stage];
                    let alpha = gx.cur_tev_alpha_env[stage];

                    ui.collapsing(format!("Stage {stage}"), |ui| {
                        Grid::new(format!("tev_{stage}"))
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("Color A");
                                ui.monospace(format!("{}", color.a()));
                                ui.end_row();
                                ui.label("Color B");
                                ui.monospace(format!("{}", color.b()));
                                ui.end_row();
                                ui.label("Color C");
                                ui.monospace(format!("{}", color.c()));
                                ui.end_row();
                                ui.label("Color D");
                                ui.monospace(format!("{}", color.d()));
                                ui.end_row();
                                ui.label("Color Dest");
                                ui.monospace(format!("{}", color.dest()));
                                ui.end_row();

                                ui.label("Alpha A");
                                ui.monospace(format!("{}", alpha.a()));
                                ui.end_row();
                                ui.label("Alpha B");
                                ui.monospace(format!("{}", alpha.b()));
                                ui.end_row();
                                ui.label("Alpha C");
                                ui.monospace(format!("{}", alpha.c()));
                                ui.end_row();
                                ui.label("Alpha D");
                                ui.monospace(format!("{}", alpha.d()));
                                ui.end_row();
                                ui.label("Alpha Dest");
                                ui.monospace(format!("{}", alpha.dest()));
                                ui.end_row();
                            });
                    });
                }
            });

            // Bound textures
            ui.collapsing("Textures", |ui| {
                for (slot, tex) in gx.cur_textures.iter().enumerate() {
                    if let Some(tex) = tex {
                        let heading = format!(
                            "Slot {slot}: {:?} {}x{} @ 0x{:08X}",
                            tex.format, tex.width, tex.height, tex.ram_addr
                        );
                        ui.collapsing(heading, |ui| {
                            texture_preview(ui, tex, &mmio.ram);
                        });
                    }
                }
            });
        });
    });
}
