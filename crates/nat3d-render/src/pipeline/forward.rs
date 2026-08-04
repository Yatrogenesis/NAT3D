// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! Forward rendering pipeline with shadow mapping.

use crate::backend::wgpu_backend::{
    CameraUniforms, GpuMesh, MaterialUniforms, RenderContext, RenderError, Vertex,
};
use bytemuck;
use nalgebra;
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Maximum number of lights supported.
pub const MAX_LIGHTS: usize = 8;

/// Forward rendering pipeline.
pub struct ForwardPipeline {
    /// Render pipeline for opaque objects.
    pub opaque_pipeline: wgpu::RenderPipeline,
    /// Render pipeline for shadow pass.
    pub shadow_pipeline: wgpu::RenderPipeline,
    /// Pipeline for wireframe overlay.
    pub wireframe_pipeline: Option<wgpu::RenderPipeline>,
    /// Bind group layout for camera uniforms.
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for model uniforms.
    pub model_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for material uniforms.
    pub material_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for light data.
    pub lights_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for shadow mapping resources.
    pub shadow_bind_group_layout: wgpu::BindGroupLayout,
    depth_texture: Option<wgpu::Texture>,
    depth_view: Option<wgpu::TextureView>,
    dimensions: (u32, u32),
}

impl ForwardPipeline {
    /// Creates a new forward rendering pipeline.
    pub fn new(
        ctx: &RenderContext,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let camera_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Camera Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let model_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Model Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let material_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Material Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let lights_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Lights Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let shadow_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Shadow Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Depth,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                            count: None,
                        },
                    ],
                });

        let pipeline_layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Forward Pipeline Layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    &model_bind_group_layout,
                    &material_bind_group_layout,
                    &shadow_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Forward Shader"),
                source: wgpu::ShaderSource::Wgsl(FORWARD_SHADER.into()),
            });

        let shadow_shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shadow Shader"),
                source: wgpu::ShaderSource::Wgsl(SHADOW_SHADER.into()),
            });

        let shadow_pipeline_layout =
            ctx.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Shadow Pipeline Layout"),
                    bind_group_layouts: &[&shadow_bind_group_layout, &model_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let shadow_pipeline = ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Shadow Pipeline"),
                layout: Some(&shadow_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shadow_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::layout()],
                    compilation_options: Default::default(),
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let opaque_pipeline = ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Forward Opaque Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let (depth_texture, depth_view) = ctx.create_depth_texture(width, height);

        Ok(Self {
            opaque_pipeline,
            shadow_pipeline,
            wireframe_pipeline: None,
            camera_bind_group_layout,
            model_bind_group_layout,
            material_bind_group_layout,
            lights_bind_group_layout,
            shadow_bind_group_layout,
            depth_texture: Some(depth_texture),
            depth_view: Some(depth_view),
            dimensions: (width, height),
        })
    }

    /// Resize the forward render pipeline to new dimensions.
    pub fn resize(&mut self, ctx: &RenderContext, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let (depth_texture, depth_view) = ctx.create_depth_texture(width, height);
        self.depth_texture = Some(depth_texture);
        self.depth_view = Some(depth_view);
        self.dimensions = (width, height);
    }

    /// Returns the depth texture view if it exists.
    pub fn depth_view(&self) -> Option<&wgpu::TextureView> {
        self.depth_view.as_ref()
    }
}

/// A command to render a single mesh with associated bind groups.
pub struct RenderCommand {
    /// The mesh to be rendered.
    pub mesh: Arc<GpuMesh>,
    /// Bind group for model uniforms.
    pub model_bind_group: wgpu::BindGroup,
    /// Bind group for material uniforms.
    pub material_bind_group: wgpu::BindGroup,
}

/// High-level renderer using the forward pipeline.
pub struct ForwardRenderer {
    /// The underlying forward pipeline.
    pub pipeline: ForwardPipeline,
    camera_buffer: wgpu::Buffer,
    /// Bind group for camera uniforms.
    pub camera_bind_group: wgpu::BindGroup,
    default_material_bind_group: wgpu::BindGroup,
    default_material_buffer: wgpu::Buffer,
    /// The background color for clearing the render target.
    pub clear_color: wgpu::Color,
    shadow_texture: wgpu::Texture,
    shadow_view: wgpu::TextureView,
    shadow_sampler: wgpu::Sampler,
    shadow_buffer: wgpu::Buffer,
    shadow_bind_group: wgpu::BindGroup,
}

impl ForwardRenderer {
    /// Creates a new forward renderer.
    pub fn new(
        ctx: &RenderContext,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let pipeline = ForwardPipeline::new(ctx, surface_format, width, height)?;

        let camera_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: std::mem::size_of::<CameraUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &pipeline.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let default_material_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Default Material Buffer"),
            size: std::mem::size_of::<MaterialUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let default_material_bind_group =
            ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Default Material Bind Group"),
                layout: &pipeline.material_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: default_material_buffer.as_entire_binding(),
                }],
            });

        let shadow_texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Map"),
            size: wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Buffer"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Bind Group"),
            layout: &pipeline.shadow_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });

        Ok(Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            default_material_bind_group,
            default_material_buffer,
            clear_color: wgpu::Color {
                r: 0.1,
                g: 0.1,
                b: 0.15,
                a: 1.0,
            },
            shadow_texture,
            shadow_view,
            shadow_sampler,
            shadow_buffer,
            shadow_bind_group,
        })
    }

    /// Updates the camera uniforms on the GPU.
    pub fn update_camera(&self, ctx: &RenderContext, uniforms: &CameraUniforms) {
        ctx.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Renders the scene using the provided commands.
    pub fn render(
        &self,
        ctx: &RenderContext,
        target: &wgpu::TextureView,
        commands: &[RenderCommand],
    ) {
        let depth_view = match self.pipeline.depth_view() {
            Some(v) => v,
            None => return,
        };

        // Update shadow matrix
        let light_view = nalgebra::Matrix4::look_at_rh(
            &nalgebra::Point3::new(10.0, 20.0, 10.0),
            &nalgebra::Point3::origin(),
            &nalgebra::Vector3::y(),
        );
        let light_proj = nalgebra::Matrix4::new_orthographic(-20.0, 20.0, -20.0, 20.0, 0.1, 100.0);
        let light_view_proj = light_proj * light_view;
        let light_vp_raw: [[f32; 4]; 4] = light_view_proj.into();
        ctx.queue.write_buffer(
            &self.shadow_buffer,
            0,
            bytemuck::cast_slice(&[light_vp_raw]),
        );

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            shadow_pass.set_pipeline(&self.pipeline.shadow_pipeline);
            shadow_pass.set_bind_group(0, &self.shadow_bind_group, &[]);
            for cmd in commands {
                shadow_pass.set_bind_group(1, &cmd.model_bind_group, &[]);
                shadow_pass.set_vertex_buffer(0, cmd.mesh.vertex_buffer.slice(..));
                shadow_pass
                    .set_index_buffer(cmd.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                shadow_pass.draw_indexed(0..cmd.mesh.index_count, 0, 0..1);
            }
        }
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_pipeline(&self.pipeline.opaque_pipeline);
            rp.set_bind_group(0, &self.camera_bind_group, &[]);
            rp.set_bind_group(3, &self.shadow_bind_group, &[]);
            for cmd in commands {
                rp.set_bind_group(1, &cmd.model_bind_group, &[]);
                rp.set_bind_group(2, &cmd.material_bind_group, &[]);
                rp.set_vertex_buffer(0, cmd.mesh.vertex_buffer.slice(..));
                rp.set_index_buffer(cmd.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..cmd.mesh.index_count, 0, 0..1);
            }
        }
        ctx.queue.submit(Some(encoder.finish()));
    }
}

/// Renderer for the ground grid overlay.
pub struct GridRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl GridRenderer {
    /// Creates a new grid renderer.
    pub fn new(ctx: &RenderContext, format: wgpu::TextureFormat) -> Self {
        let shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Grid Shader"),
                source: wgpu::ShaderSource::Wgsl(GRID_SHADER.into()),
            });
        let layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&ctx.device.create_bind_group_layout(
                    &wgpu::BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }],
                    },
                )],
                push_constant_ranges: &[],
            });
        let pipeline = ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Grid Pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 12,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        }],
                    }],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::LineList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });
        let mut verts = Vec::new();
        let mut inds = Vec::new();
        for i in -20..=20 {
            let p = i as f32;
            let idx = verts.len() as u32;
            verts.push([-20.0, 0.0, p]);
            verts.push([20.0, 0.0, p]);
            inds.push(idx);
            inds.push(idx + 1);
            let idx = verts.len() as u32;
            verts.push([p, 0.0, -20.0]);
            verts.push([p, 0.0, 20.0]);
            inds.push(idx);
            inds.push(idx + 1);
        }
        let vertex_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&inds),
                usage: wgpu::BufferUsages::INDEX,
            });
        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count: inds.len() as u32,
        }
    }

    /// Renders the grid overlay.
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Grid Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}
const FORWARD_SHADER: &str = r#"
struct CameraUniforms { view_proj: mat4x4<f32>, view: mat4x4<f32>, proj: mat4x4<f32>, camera_pos: vec4<f32> }
struct ModelUniforms { model: mat4x4<f32>, normal: mat4x4<f32> }
struct MaterialUniforms { base_color: vec4<f32>, metallic: f32, roughness: f32, ao: f32, emissive: f32 }
struct ShadowUniforms { light_view_proj: mat4x4<f32> }
@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(1) @binding(0) var<uniform> model: ModelUniforms;
@group(2) @binding(0) var<uniform> material: MaterialUniforms;
@group(3) @binding(0) var<uniform> shadow_uni: ShadowUniforms;
@group(3) @binding(1) var t_shadow: texture_depth_2d;
@group(3) @binding(2) var s_shadow: sampler_comparison;
struct VertexInput { @location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) tex_coords: vec2<f32>, @location(3) color: vec4<f32> }
struct VertexOutput { @builtin(position) clip_pos: vec4<f32>, @location(0) world_pos: vec3<f32>, @location(1) world_normal: vec3<f32>, @location(2) shadow_pos: vec4<f32> }
@vertex fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let wpos = model.model * vec4<f32>(in.position, 1.0);
    out.world_pos = wpos.xyz;
    out.clip_pos = camera.view_proj * wpos;
    out.world_normal = (model.normal * vec4<f32>(in.normal, 0.0)).xyz;
    let spos = shadow_uni.light_view_proj * wpos;
    out.shadow_pos = vec4<f32>(spos.x*0.5+0.5, 1.0-(spos.y*0.5+0.5), spos.z, spos.w);
    return out;
}

@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = material.base_color.rgb;
    let n = normalize(in.world_normal);
    let l = normalize(vec3<f32>(0.5, 1.0, 0.3));
    
    // Shadow map sampling
    let shadow = textureSampleCompare(t_shadow, s_shadow, in.shadow_pos.xy/in.shadow_pos.w, in.shadow_pos.z/in.shadow_pos.w - 0.005);
    
    let diffuse = max(dot(n, l), 0.0) * shadow;
    var final_color = albedo * (diffuse + 0.1);

    // BATCH 24: Procedural Infinite Grid (P1.3)
    // REF: [Acerola, 2023] "Infinite Grid Shader"
    // Only draw grid on world Y=0 (approx)
    if abs(in.world_pos.y) < 0.01 {
        let coord = in.world_pos.xz;
        let derivative = fwidth(coord);
        let grid = abs(fract(coord - 0.5) - 0.5) / derivative;
        let line = min(grid.x, grid.y);
        let color = 1.0 - min(line, 1.0);
        
        // Fading based on distance
        let dist = length(camera.camera_pos.xyz - in.world_pos);
        let fade = 1.0 - smoothstep(10.0, 100.0, dist);
        
        if color > 0.1 {
            final_color = mix(final_color, vec3<f32>(0.5, 0.5, 0.5), color * fade);
        }
        
        // Axes
        if abs(in.world_pos.x) < 0.1 { final_color = mix(final_color, vec3<f32>(0.2, 0.8, 0.2), fade); } // Z-axis (Green in many 3D apps, but XYZ standard is RGB)
        if abs(in.world_pos.z) < 0.1 { final_color = mix(final_color, vec3<f32>(0.8, 0.2, 0.2), fade); } // X-axis (Red)
    }

    return vec4<f32>(final_color, material.base_color.a);
}

"#;

const SHADOW_SHADER: &str = r#"
struct ShadowUniforms { light_view_proj: mat4x4<f32> }
struct ModelUniforms { model: mat4x4<f32>, normal: mat4x4<f32> }
@group(0) @binding(0) var<uniform> shadow_uni: ShadowUniforms;
@group(1) @binding(0) var<uniform> model: ModelUniforms;
@vertex fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    return shadow_uni.light_view_proj * model.model * vec4<f32>(pos, 1.0);
}
"#;

const GRID_SHADER: &str = r#"
struct CameraUniforms { view_proj: mat4x4<f32>, view: mat4x4<f32>, proj: mat4x4<f32>, camera_pos: vec4<f32> }
@group(0) @binding(0) var<uniform> camera: CameraUniforms;
struct VertexOutput { @builtin(position) clip_pos: vec4<f32>, @location(0) world_pos: vec3<f32> }
@vertex fn vs_main(@location(0) pos: vec3<f32>) -> VertexOutput {
    var out: VertexOutput; out.world_pos = pos; out.clip_pos = camera.view_proj * vec4<f32>(pos, 1.0); return out;
}

@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = material.base_color.rgb;
    let n = normalize(in.world_normal);
    let l = normalize(vec3<f32>(0.5, 1.0, 0.3));
    
    // Shadow map sampling
    let shadow = textureSampleCompare(t_shadow, s_shadow, in.shadow_pos.xy/in.shadow_pos.w, in.shadow_pos.z/in.shadow_pos.w - 0.005);
    
    let diffuse = max(dot(n, l), 0.0) * shadow;
    var final_color = albedo * (diffuse + 0.1);

    // BATCH 24: Procedural Infinite Grid (P1.3)
    // REF: [Acerola, 2023] "Infinite Grid Shader"
    // Only draw grid on world Y=0 (approx)
    if abs(in.world_pos.y) < 0.01 {
        let coord = in.world_pos.xz;
        let derivative = fwidth(coord);
        let grid = abs(fract(coord - 0.5) - 0.5) / derivative;
        let line = min(grid.x, grid.y);
        let color = 1.0 - min(line, 1.0);
        
        // Fading based on distance
        let dist = length(camera.camera_pos.xyz - in.world_pos);
        let fade = 1.0 - smoothstep(10.0, 100.0, dist);
        
        if color > 0.1 {
            final_color = mix(final_color, vec3<f32>(0.5, 0.5, 0.5), color * fade);
        }
        
        // Axes
        if abs(in.world_pos.x) < 0.1 { final_color = mix(final_color, vec3<f32>(0.2, 0.8, 0.2), fade); } // Z-axis (Green in many 3D apps, but XYZ standard is RGB)
        if abs(in.world_pos.z) < 0.1 { final_color = mix(final_color, vec3<f32>(0.8, 0.2, 0.2), fade); } // X-axis (Red)
    }

    return vec4<f32>(final_color, material.base_color.a);
}

"#;
