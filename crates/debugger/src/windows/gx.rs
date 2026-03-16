use egui::{Context, Grid, RichText, ScrollArea};
use egui_material_icons::icons;
use gekko::flipper::gx::Gx;
use gekko::flipper::gx::draw::TextureDescriptor;
use gekko::mmio::Mmio;

fn texture_preview(ui: &mut egui::Ui, tex: &TextureDescriptor, ram: &[u8]) {
    let rgba = backend_wgpu::texture::decode_to_rgba(ram, tex);
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

fn mono_header(text: &str) -> RichText {
    RichText::new(text).monospace().strong()
}

pub fn show_gx(ctx: &Context, open: &mut bool, gx: &Gx, mmio: &Mmio) {
    egui::Window::new("GX").open(open).show(ctx, |ui| {
        let dc = &gx.draw_commands;

        ScrollArea::vertical().show(ui, |ui| {
            // Summary
            ui.horizontal(|ui| {
                ui.strong(format!("{} draw calls", dc.commands.len()));
                ui.separator();
                ui.strong(format!("{} TEV stages", dc.num_tev_stages));
                ui.separator();
                let n_tex = dc.textures.iter().filter(|t| t.is_some()).count();
                ui.strong(format!("{n_tex} textures bound"));
            });
            ui.separator();

            // Transform
            ui.collapsing("Transform", |ui| {
                ui.label("Projection Matrix");
                Grid::new("proj").num_columns(4).show(ui, |ui| {
                    for row in 0..4 {
                        for col in 0..4 {
                            ui.monospace(format!("{:+9.4}", dc.projection.0[col][row]));
                        }
                        ui.end_row();
                    }
                });
            });

            // TEV Stages
            ui.collapsing(format!("TEV Stages ({})", dc.num_tev_stages), |ui| {
                for stage in 0..dc.num_tev_stages as usize {
                    let color = dc.tev_color_env[stage];
                    let alpha = dc.tev_alpha_env[stage];
                    let order = dc.tev_orders[stage / 2];
                    let (texmap, texcoord, chan, tex_en) = if stage % 2 == 0 {
                        (
                            order.texmap0(),
                            order.texcoord0(),
                            order.channel0(),
                            order.tex_enable0(),
                        )
                    } else {
                        (
                            order.texmap1(),
                            order.texcoord1(),
                            order.channel1(),
                            order.tex_enable1(),
                        )
                    };

                    ui.collapsing(format!("Stage {stage}"), |ui| {
                        Grid::new(format!("tev_{stage}"))
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                // Texture / raster inputs
                                ui.label("Texture");
                                if tex_en {
                                    let primary = if let Some(tex) = dc.textures[texmap as usize] {
                                        format!(
                                            "tex{texmap} / coord{texcoord} — {:?} {}x{}",
                                            tex.format, tex.width, tex.height
                                        )
                                    } else {
                                        format!("tex{texmap} / coord{texcoord} — (unbound)")
                                    };
                                    let resp = ui.label(primary);
                                    if let Some(tex) = dc.textures[texmap as usize] {
                                        resp.on_hover_ui(|ui| texture_preview(ui, &tex, &mmio.ram));
                                    }
                                } else {
                                    ui.monospace("disabled");
                                }
                                ui.end_row();

                                ui.label("Raster Channel");
                                ui.monospace(format!("{chan:?}"));
                                ui.end_row();

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
                                ui.label("Color Destination");
                                ui.monospace(format!("{}", color.dest())).on_hover_text(format!(
                                    "bias={:?}  scale={:?}  sub={}  clamp={}",
                                    color.bias(),
                                    color.scale(),
                                    color.sub(),
                                    color.clamp(),
                                ));
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
                                ui.label("Alpha Destination");
                                ui.monospace(format!("{}", alpha.dest())).on_hover_text(format!(
                                    "bias={:?}  scale={:?}  sub={}  clamp={}",
                                    alpha.bias(),
                                    alpha.scale(),
                                    alpha.sub(),
                                    alpha.clamp(),
                                ));
                                ui.end_row();
                            });
                    });
                }
            });

            // Textures
            let bound_count = dc.textures.iter().filter(|t| t.is_some()).count();
            ui.collapsing(format!("Textures ({bound_count})"), |ui| {
                if bound_count == 0 {
                    ui.label("none bound");
                } else {
                    Grid::new("textures").num_columns(2).striped(true).show(ui, |ui| {
                        for (slot, tex_opt) in dc.textures.iter().enumerate() {
                            if let Some(tex) = tex_opt {
                                ui.label(format!("tex{slot}"));
                                ui.monospace(format!(
                                    "{:?} {}x{} @ 0x{:08X}",
                                    tex.format, tex.width, tex.height, tex.ram_addr
                                ))
                                .on_hover_ui(|ui| texture_preview(ui, tex, &mmio.ram));
                                ui.end_row();
                            }
                        }
                    });
                }
            });

            // PE
            ui.collapsing("Output (PE)", |ui| {
                Grid::new("output").num_columns(2).striped(true).show(ui, |ui| {
                    let bm = dc.bp_blend_mode;

                    ui.label("Blend");
                    if bm.blend_enable() {
                        ui.monospace(format!(
                            "{:?}  ->  {:?}{}",
                            bm.src_factor(),
                            bm.dst_factor(),
                            if bm.subtract() { "  (subtract)" } else { "" },
                        ))
                        .on_hover_text(format!(
                            "Color Write: {}  Alpha Write: {}  Dither: {}",
                            bm.color_update(),
                            bm.alpha_update(),
                            bm.dither_enable(),
                        ));
                    } else {
                        ui.label("disabled");
                    }
                    ui.end_row();

                    if bm.logic_op_enable() {
                        ui.label("Logic Operation");
                        ui.monospace(format!("{:?}", bm.logic_op()));
                        ui.end_row();
                    }

                    ui.label("Color Write");
                    ui.monospace(bm.color_update().to_string());
                    ui.end_row();

                    ui.label("Alpha Write");
                    ui.monospace(bm.alpha_update().to_string());
                    ui.end_row();

                    let zm = dc.bp_zmode;
                    ui.label("Depth Test");
                    if zm.enable() {
                        ui.monospace(format!("{:?}  write={}", zm.func(), zm.update_enable(),));
                    } else {
                        ui.label("disabled");
                    }
                    ui.end_row();

                    let ac = dc.bp_alpha_compare;
                    ui.label("Alpha Compare");
                    ui.monospace(format!(
                        "{:?}({})  {:?}  {:?}({})",
                        ac.comp0(),
                        ac.ref0(),
                        ac.op(),
                        ac.comp1(),
                        ac.ref1(),
                    ));
                    ui.end_row();
                });
            });

            // TEV Registers
            ui.collapsing("TEV Registers", |ui| {
                Grid::new("tev_regs").num_columns(5).striped(true).show(ui, |ui| {
                    ui.label(mono_header("Reg "));
                    ui.label(mono_header("   R"));
                    ui.label(mono_header("   G"));
                    ui.label(mono_header("   B"));
                    ui.label(mono_header("   A"));
                    ui.end_row();

                    for (i, name) in ["Prev", "Reg0", "Reg1", "Reg2"].iter().enumerate() {
                        let lo = dc.tev_color_regs_lo[i];
                        let hi = dc.tev_color_regs_hi[i];
                        ui.monospace(*name);
                        ui.monospace(format!("{:4}", lo.r()));
                        ui.monospace(format!("{:4}", hi.g()));
                        ui.monospace(format!("{:4}", hi.b()));
                        ui.monospace(format!("{:4}", lo.a()));
                        ui.end_row();
                    }
                });
            });

            // Draw Calls
            ui.collapsing(format!("Draw Calls ({})", dc.commands.len()), |ui| {
                ScrollArea::vertical()
                    .id_salt("draw_calls")
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for (i, call) in dc.commands.iter().enumerate() {
                            let heading = RichText::new(format!(
                                "[{i:>3}]  {:?}  x  {} vertices",
                                call.primitive,
                                call.vertices.len(),
                            ))
                            .monospace();

                            ui.collapsing(heading, |ui| {
                                ui.label("Modelview matrix");
                                Grid::new(format!("mv_{i}")).num_columns(4).show(ui, |ui| {
                                    for row in 0..4 {
                                        for col in 0..4 {
                                            ui.monospace(format!("{:+8.3}", call.modelview.0[col][row]));
                                        }
                                        ui.end_row();
                                    }
                                });

                                ui.add_space(2.0);
                                let preview = call.vertices.len().min(4);
                                for (vi, v) in call.vertices.iter().take(preview).enumerate() {
                                    ui.monospace(format!(
                                        "v{vi}  position ({:+.3}, {:+.3}, {:+.3})",
                                        v.position[0], v.position[1], v.position[2],
                                    ));
                                    ui.monospace(format!(
                                        "    color    ({:.2}, {:.2}, {:.2}, {:.2})",
                                        v.color0[0], v.color0[1], v.color0[2], v.color0[3],
                                    ));
                                    if let Some(uv) = v.tex0 {
                                        ui.monospace(format!("    texcoord ({:+.4}, {:+.4})", uv[0], uv[1],));
                                    }
                                }
                                if call.vertices.len() > preview {
                                    ui.label(format!(
                                        "{} {} more vertices",
                                        icons::ICON_MORE_HORIZ,
                                        call.vertices.len() - preview,
                                    ));
                                }
                            });
                        }
                    });
            });
        });
    });
}
