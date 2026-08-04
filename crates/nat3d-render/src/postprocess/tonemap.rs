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

//! Tone mapping post-processing.
//!
//! Converts HDR values to LDR for display. Supports multiple tone mapping
//! operators including ACES, Reinhard, and Filmic.

use crate::backend::wgpu_backend::{RenderContext, RenderError};

/// Tone mapping operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToneMapOperator {
    /// No tone mapping (clamp).
    None,
    /// Reinhard tone mapping.
    Reinhard,
    /// ACES filmic tone mapping (default).
    #[default]
    Aces,
    /// Uncharted 2 filmic.
    Uncharted2,
    /// Khronos PBR neutral.
    KhronosPbrNeutral,
}

/// Tone mapping parameters.
#[derive(Debug, Clone, Copy)]
pub struct ToneMapParams {
    /// Exposure value.
    pub exposure: f32,
    /// Gamma correction value.
    pub gamma: f32,
    /// Tone mapping operator.
    pub operator: ToneMapOperator,
    /// White point (for Reinhard).
    pub white_point: f32,
}

impl Default for ToneMapParams {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            gamma: 2.2,
            operator: ToneMapOperator::Aces,
            white_point: 4.0,
        }
    }
}

/// Tone mapping post-process pass.
pub struct ToneMapPass {
    /// Render pipeline.
    pipeline: wgpu::RenderPipeline,
    /// Bind group layout.
    bind_group_layout: wgpu::BindGroupLayout,
    /// Parameters uniform buffer.
    params_buffer: wgpu::Buffer,
    /// Sampler.
    sampler: wgpu::Sampler,
}

impl ToneMapPass {
    /// Create a new tone mapping pass.
    pub fn new(
        ctx: &RenderContext,
        output_format: wgpu::TextureFormat,
    ) -> Result<Self, RenderError> {
        // Create sampler
        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ToneMap Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("ToneMap Bind Group Layout"),
                    entries: &[
                        // HDR input texture
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
                        // Sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        // Parameters
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

        // Create pipeline layout
        let pipeline_layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ToneMap Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        // Create shader
        let shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("ToneMap Shader"),
                source: wgpu::ShaderSource::Wgsl(TONEMAP_SHADER.into()),
            });

        // Create render pipeline
        let pipeline = ctx
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ToneMap Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: output_format,
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
                multiview: None,
                cache: None,
            });

        // Create params buffer
        let params_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ToneMap Params Buffer"),
            size: 32, // 4 floats + padding
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            sampler,
        })
    }

    /// Create bind group for rendering.
    pub fn create_bind_group(
        &self,
        ctx: &RenderContext,
        hdr_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ToneMap Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Update tone mapping parameters.
    pub fn update_params(&self, ctx: &RenderContext, params: &ToneMapParams) {
        let data = ToneMapUniforms {
            exposure: params.exposure,
            gamma: params.gamma,
            operator: params.operator as u32,
            white_point: params.white_point,
        };
        ctx.queue
            .write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&data));
    }

    /// Render tone mapping pass.
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..3, 0..1); // Full-screen triangle
    }
}

/// Tone mapping uniform data.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ToneMapUniforms {
    exposure: f32,
    gamma: f32,
    operator: u32,
    white_point: f32,
}

/// Tone mapping WGSL shader.
const TONEMAP_SHADER: &str = r#"
struct ToneMapParams {
    exposure: f32,
    gamma: f32,
    operator: u32,
    white_point: f32,
}

@group(0) @binding(0)
var hdr_texture: texture_2d<f32>;
@group(0) @binding(1)
var hdr_sampler: sampler;
@group(0) @binding(2)
var<uniform> params: ToneMapParams;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full-screen triangle
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

// Reinhard tone mapping
fn tonemap_reinhard(color: vec3<f32>) -> vec3<f32> {
    return color / (color + vec3<f32>(1.0));
}

// Reinhard with white point
fn tonemap_reinhard_white(color: vec3<f32>, white: f32) -> vec3<f32> {
    let white_sq = white * white;
    let numerator = color * (1.0 + color / white_sq);
    return numerator / (1.0 + color);
}

// ACES filmic tone mapping
fn tonemap_aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return saturate((x * (a * x + b)) / (x * (c * x + d) + e));
}

// Uncharted 2 helper
fn uncharted2_tonemap_partial(x: vec3<f32>) -> vec3<f32> {
    let A = 0.15;
    let B = 0.50;
    let C = 0.10;
    let D = 0.20;
    let E = 0.02;
    let F = 0.30;
    return ((x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F)) - E / F;
}

// Uncharted 2 tone mapping
fn tonemap_uncharted2(color: vec3<f32>) -> vec3<f32> {
    let exposure_bias = 2.0;
    let curr = uncharted2_tonemap_partial(color * exposure_bias);
    let white_scale = vec3<f32>(1.0) / uncharted2_tonemap_partial(vec3<f32>(11.2));
    return curr * white_scale;
}

// Khronos PBR Neutral tone mapping
fn tonemap_pbr_neutral(color: vec3<f32>) -> vec3<f32> {
    let start_compression = 0.8 - 0.04;
    let desaturation = 0.15;

    var x = min(color, vec3<f32>(1.0));
    let peak = max(max(x.r, x.g), x.b);

    if (peak < start_compression) {
        return x;
    }

    let d = 1.0 - start_compression;
    let new_peak = 1.0 - d * d / (peak + d - start_compression);
    x *= new_peak / peak;

    let g = 1.0 - 1.0 / (desaturation * (peak - new_peak) + 1.0);
    return mix(x, vec3<f32>(new_peak), g);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var hdr_color = textureSample(hdr_texture, hdr_sampler, in.uv).rgb;

    // Apply exposure
    hdr_color *= params.exposure;

    // Apply tone mapping
    var ldr_color: vec3<f32>;
    switch (params.operator) {
        case 0u: { // None
            ldr_color = saturate(hdr_color);
        }
        case 1u: { // Reinhard
            ldr_color = tonemap_reinhard_white(hdr_color, params.white_point);
        }
        case 2u: { // ACES
            ldr_color = tonemap_aces(hdr_color);
        }
        case 3u: { // Uncharted 2
            ldr_color = tonemap_uncharted2(hdr_color);
        }
        case 4u: { // Khronos PBR Neutral
            ldr_color = tonemap_pbr_neutral(hdr_color);
        }
        default: {
            ldr_color = tonemap_aces(hdr_color);
        }
    }

    // Gamma correction
    let gamma_inv = 1.0 / params.gamma;
    ldr_color = pow(ldr_color, vec3<f32>(gamma_inv));

    return vec4<f32>(ldr_color, 1.0);
}
"#;
