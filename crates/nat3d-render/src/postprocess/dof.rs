/*
 * NAT3D - Next-generation Advanced Technology for 3D
 * Professional 3D Modeling, CAD, Physics Simulation and Rendering Suite
 *
 * Copyright (C) 2023-2026 Francisco Molina <pako.molina@gmail.com>
 *
 * This software is dual-licensed:
 * 1. Open Source: GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)
 * 2. Commercial: For commercial use, please contact <fmolina@avermex.com>
 *
 * For research information, visit: https://research.avermex.com
 * For collaborations, contact: <pako.molina@gmail.com>
 *
 * DOI: [PENDING]
 */

//! Depth of Field post-processing.
//!
//! Simulates camera lens blur based on depth.

use crate::backend::wgpu_backend::{RenderContext, RenderError};

/// Depth of field parameters.
#[derive(Debug, Clone, Copy)]
pub struct DofParams {
    /// Focus distance.
    pub focus_distance: f32,
    /// Aperture (f-stop).
    pub aperture: f32,
    /// Focal length in mm.
    pub focal_length: f32,
    /// Near blur amount.
    pub near_blur: f32,
    /// Far blur amount.
    pub far_blur: f32,
    /// Bokeh shape (number of blades, 0 = circular).
    pub bokeh_blades: u32,
}

impl Default for DofParams {
    fn default() -> Self {
        Self {
            focus_distance: 5.0,
            aperture: 2.8,
            focal_length: 50.0,
            near_blur: 1.0,
            far_blur: 1.0,
            bokeh_blades: 6,
        }
    }
}

/// Depth of field post-process pass.
pub struct DofPass {
    /// CoC (Circle of Confusion) compute pipeline.
    coc_pipeline: wgpu::RenderPipeline,
    /// Blur pipeline.
    blur_pipeline: wgpu::RenderPipeline,
    /// Composite pipeline.
    composite_pipeline: wgpu::RenderPipeline,
    /// Bind group layout for CoC.
    coc_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for blur.
    blur_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for composite.
    composite_bind_group_layout: wgpu::BindGroupLayout,
    /// Parameters buffer.
    params_buffer: wgpu::Buffer,
    /// Sampler.
    sampler: wgpu::Sampler,
    /// CoC texture.
    coc_texture: Option<wgpu::Texture>,
    /// Blurred texture.
    blur_texture: Option<wgpu::Texture>,
    /// Current dimensions.
    dimensions: (u32, u32),
}

impl DofPass {
    /// Create a new depth of field pass.
    pub fn new(
        ctx: &RenderContext,
        width: u32,
        height: u32,
        output_format: wgpu::TextureFormat,
    ) -> Result<Self, RenderError> {
        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("DoF Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Bind group layouts
        let coc_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("DoF CoC Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Depth,
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
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        let blur_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("DoF Blur Bind Group Layout"),
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
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        let composite_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("DoF Composite Bind Group Layout"),
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
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        // Create shaders
        let coc_shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("DoF CoC Shader"),
                source: wgpu::ShaderSource::Wgsl(DOF_COC_SHADER.into()),
            });

        let blur_shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("DoF Blur Shader"),
                source: wgpu::ShaderSource::Wgsl(DOF_BLUR_SHADER.into()),
            });

        let composite_shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("DoF Composite Shader"),
                source: wgpu::ShaderSource::Wgsl(DOF_COMPOSITE_SHADER.into()),
            });

        // Create pipelines
        let coc_pipeline_layout =
            ctx.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("DoF CoC Pipeline Layout"),
                    bind_group_layouts: &[&coc_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let coc_pipeline = ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("DoF CoC Pipeline"),
                layout: Some(&coc_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &coc_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &coc_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::R16Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let blur_pipeline_layout =
            ctx.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("DoF Blur Pipeline Layout"),
                    bind_group_layouts: &[&blur_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let blur_pipeline = ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("DoF Blur Pipeline"),
                layout: Some(&blur_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &blur_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &blur_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: output_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let composite_pipeline_layout =
            ctx.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("DoF Composite Pipeline Layout"),
                    bind_group_layouts: &[&composite_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let composite_pipeline =
            ctx.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("DoF Composite Pipeline"),
                    layout: Some(&composite_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &composite_shader,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &composite_shader,
                        entry_point: Some("fs_main"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: output_format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });

        let params_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DoF Params Buffer"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            coc_pipeline,
            blur_pipeline,
            composite_pipeline,
            coc_bind_group_layout,
            blur_bind_group_layout,
            composite_bind_group_layout,
            params_buffer,
            sampler,
            coc_texture: None,
            blur_texture: None,
            dimensions: (width, height),
        })
    }

    /// Update DoF parameters.
    pub fn update_params(&self, ctx: &RenderContext, params: &DofParams) {
        let data = DofUniforms {
            focus_distance: params.focus_distance,
            aperture: params.aperture,
            focal_length: params.focal_length,
            near_blur: params.near_blur,
            far_blur: params.far_blur,
            bokeh_blades: params.bokeh_blades,
            _padding: [0; 2],
        };
        ctx.queue
            .write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&data));
    }

    /// Resize internal textures.
    pub fn resize(&mut self, ctx: &RenderContext, width: u32, height: u32) {
        self.dimensions = (width, height);

        self.coc_texture = Some(ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DoF CoC Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        }));

        self.blur_texture = Some(ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DoF Blur Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        }));
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DofUniforms {
    focus_distance: f32,
    aperture: f32,
    focal_length: f32,
    near_blur: f32,
    far_blur: f32,
    bokeh_blades: u32,
    _padding: [u32; 2],
}

const DOF_COC_SHADER: &str = r#"
struct DofParams {
    focus_distance: f32,
    aperture: f32,
    focal_length: f32,
    near_blur: f32,
    far_blur: f32,
    bokeh_blades: u32,
}

@group(0) @binding(0) var depth_texture: texture_depth_2d;
@group(0) @binding(1) var depth_sampler: sampler;
@group(0) @binding(2) var<uniform> params: DofParams;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var pos = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var uv = array<vec2<f32>, 3>(vec2(0.0, 1.0), vec2(2.0, 1.0), vec2(0.0, -1.0));
    var out: VertexOutput;
    out.position = vec4(pos[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}

fn linearize_depth(d: f32, near: f32, far: f32) -> f32 {
    return near * far / (far - d * (far - near));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) f32 {
    let depth = textureSample(depth_texture, depth_sampler, in.uv);
    let linear_depth = linearize_depth(depth, 0.1, 1000.0);
    let f = params.focal_length / 1000.0;
    let N = params.aperture;
    let S1 = params.focus_distance;
    let S2 = linear_depth;
    let coc = abs(f * (S1 - S2) / (S2 * (S1 - f))) * (f / N);
    let sign_coc = select(params.far_blur, -params.near_blur, S2 < S1);
    return clamp(coc * sign_coc * 100.0, -1.0, 1.0);
}
"#;

const DOF_BLUR_SHADER: &str = r#"
@group(0) @binding(0) var color_texture: texture_2d<f32>;
@group(0) @binding(1) var coc_texture: texture_2d<f32>;
@group(0) @binding(2) var tex_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var pos = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var uv = array<vec2<f32>, 3>(vec2(0.0, 1.0), vec2(2.0, 1.0), vec2(0.0, -1.0));
    var out: VertexOutput;
    out.position = vec4(pos[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let coc = abs(textureSample(coc_texture, tex_sampler, in.uv).r);
    let texel_size = 1.0 / vec2<f32>(textureDimensions(color_texture));
    var color = vec3<f32>(0.0);
    var weight = 0.0;
    let kernel_size = i32(coc * 16.0) + 1;
    for (var y = -kernel_size; y <= kernel_size; y++) {
        for (var x = -kernel_size; x <= kernel_size; x++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size * coc;
            let sample_uv = in.uv + offset;
            if (sample_uv.x >= 0.0 && sample_uv.x <= 1.0 && sample_uv.y >= 0.0 && sample_uv.y <= 1.0) {
                let dist = length(vec2<f32>(f32(x), f32(y)));
                if (dist <= f32(kernel_size)) {
                    let w = 1.0 - dist / f32(kernel_size + 1);
                    color += textureSample(color_texture, tex_sampler, sample_uv).rgb * w;
                    weight += w;
                }
            }
        }
    }
    if (weight > 0.0) { color /= weight; } else { color = textureSample(color_texture, tex_sampler, in.uv).rgb; }
    return vec4<f32>(color, 1.0);
}
"#;

const DOF_COMPOSITE_SHADER: &str = r#"
@group(0) @binding(0) var sharp_texture: texture_2d<f32>;
@group(0) @binding(1) var blur_texture: texture_2d<f32>;
@group(0) @binding(2) var coc_texture: texture_2d<f32>;
@group(0) @binding(3) var tex_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var pos = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var uv = array<vec2<f32>, 3>(vec2(0.0, 1.0), vec2(2.0, 1.0), vec2(0.0, -1.0));
    var out: VertexOutput;
    out.position = vec4(pos[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sharp = textureSample(sharp_texture, tex_sampler, in.uv).rgb;
    let blur = textureSample(blur_texture, tex_sampler, in.uv).rgb;
    let coc = abs(textureSample(coc_texture, tex_sampler, in.uv).r);
    let result = mix(sharp, blur, smoothstep(0.0, 0.5, coc));
    return vec4<f32>(result, 1.0);
}
"#;
