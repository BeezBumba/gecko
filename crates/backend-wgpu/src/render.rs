use crate::{FrameUniforms, GxRenderer};
use crate::{GpuVertex, align_up};
use encase::ShaderType as _;
use gecko::flipper::gx::draw::EfbCopyCmd;
use gecko::host::{TextureId, XfbPart};

impl GxRenderer {
    pub(crate) fn upload_buffers(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame_uniform_bytes: &[u8]) {
        let num_draws = self.scratch_draws.len();
        self.ensure_draw_capacity(device, num_draws);

        if self.scratch_vertices.len() > self.vertex_capacity {
            self.vertex_capacity = self.scratch_vertices.len().next_power_of_two();
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gx_vertices"),
                size: (self.vertex_capacity * std::mem::size_of::<GpuVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        let frame_stride = align_up(
            FrameUniforms::min_size().get(),
            device.limits().min_uniform_buffer_offset_alignment as u64,
        ) as usize;
        let needed_frame_size = (num_draws * frame_stride) as u64;
        if needed_frame_size > self.frame_uniform_buffer.size() {
            self.frame_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gx_frame_uniforms"),
                size: needed_frame_size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.bind_group_cache.clear();
        }

        queue.write_buffer(&self.frame_uniform_buffer, 0, frame_uniform_bytes);
        queue.write_buffer(&self.draw_uniform_buffer, 0, &self.scratch_uniform_bytes);
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.scratch_vertices));
    }

    pub(crate) fn execute_efb_copy(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, copy: &EfbCopyCmd) {
        // Snapshot the EFB region into texture_cache so that a later
        // SetTexture referencing dest_addr finds the GPU-side copy
        // instead of stale/unwritten RAM data.
        let width = copy.src_w.min(crate::EFB_WIDTH.saturating_sub(copy.src_x));
        let height = copy.src_h.min(crate::EFB_HEIGHT.saturating_sub(copy.src_y));
        if width > 0 && height > 0 {
            let id = TextureId(copy.dest_addr);
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("efb_copy_tex"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.efb_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: copy.src_x,
                        y: copy.src_y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::default(),
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::default(),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit([encoder.finish()]);

            let view = tex.create_view(&Default::default());
            self.bind_group_cache
                .retain(|key, _| !key.tex_keys.iter().any(|k| *k == Some(id)));
            self.texture_cache.insert(id, (tex, view));
        }

        if copy.clear {
            self.efb_clear.clear_region(
                device,
                queue,
                &self.efb_msaa_view,
                &self.efb_view,
                &self.efb_depth_view,
                crate::EFB_WIDTH,
                crate::EFB_HEIGHT,
                copy.src_x,
                copy.src_y,
                copy.src_w,
                copy.src_h,
                copy.clear_color,
                copy.clear_z,
            );
        }
    }

    pub(crate) fn execute_copy_xfb(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: u32,
        src_x: u32,
        src_y: u32,
        src_w: u32,
        src_h: u32,
        clear: bool,
        clear_color: [f32; 4],
        clear_z: f32,
    ) {
        let width = src_w.min(crate::EFB_WIDTH.saturating_sub(src_x));
        let height = src_h.min(crate::EFB_HEIGHT.saturating_sub(src_y));
        if width == 0 || height == 0 {
            tracing::warn!(src_x, src_y, src_w, src_h, "efb_copy: zero-area region after clamping, skipping");
            return;
        }
        let entry = self.xfb_copies.entry(id).or_insert_with(|| {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("xfb_copy_tmp"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            (tex, view)
        });

        // Recreate if size changed.
        let existing_size = entry.0.size();
        if existing_size.width != width || existing_size.height != height {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("xfb_copy_tmp"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            let view = tex.create_view(&Default::default());
            *entry = (tex, view);
        }

        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.efb_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: src_x,
                    y: src_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::default(),
            },
            wgpu::TexelCopyTextureInfo {
                texture: &entry.0,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::default(),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);

        // Region-scoped EFB clear after copy (if requested).
        if clear {
            self.efb_clear.clear_region(
                device,
                queue,
                &self.efb_msaa_view,
                &self.efb_view,
                &self.efb_depth_view,
                crate::EFB_WIDTH,
                crate::EFB_HEIGHT,
                src_x,
                src_y,
                src_w,
                src_h,
                clear_color,
                clear_z,
            );
        }
    }

    pub(crate) fn execute_present_xfb(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        parts: &[XfbPart],
    ) {
        let width = width.max(1);
        let height = height.max(1);

        // Resize the XFB output texture if the frame dimensions changed.
        let cur = self.xfb_texture.size();
        if cur.width != width || cur.height != height {
            self.xfb_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("xfb_accum"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.xfb_view = self.xfb_texture.create_view(&Default::default());
        }

        let mut encoder = device.create_command_encoder(&Default::default());

        // Don't clear the XFB: let previous content persist so partial
        // frames show the last valid content instead of a black flash.

        let xfb_size = self.xfb_texture.size();

        for part in parts {
            let Some((tex, _)) = self.xfb_copies.get(&part.id) else {
                tracing::warn!(id = part.id, "present_xfb: XFB copy not found in cache, skipping part");
                continue;
            };
            let src_size = tex.size();
            let width = src_size.width.min(xfb_size.width.saturating_sub(part.offset_x));
            let height = src_size.height.min(xfb_size.height.saturating_sub(part.offset_y));
            if width == 0 || height == 0 {
                tracing::warn!(id = part.id, "present_xfb: zero-area XFB part after clamping, skipping");
                continue;
            }
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::default(),
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &self.xfb_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: part.offset_x,
                        y: part.offset_y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::default(),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }

        queue.submit([encoder.finish()]);
        self.xfb_has_content = true;
    }

    fn ensure_draw_capacity(&mut self, device: &wgpu::Device, count: usize) {
        if count <= self.draw_uniform_capacity {
            return;
        }

        self.draw_uniform_capacity = count.next_power_of_two();
        self.draw_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gx_draw_uniforms"),
            size: self.draw_uniform_stride * self.draw_uniform_capacity as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.bind_group_cache.clear();
    }
}
