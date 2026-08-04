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

//! Hybrid rendering pipeline.
//!
//! Combines deferred and forward rendering for optimal quality:
//! - Deferred for opaque geometry (many lights efficiently)
//! - Forward for transparent objects and special materials

use super::deferred::{
    DeferredCameraUniforms, DeferredMaterialUniforms, DeferredModelUniforms, DeferredPipeline,
    LightUniforms,
};
use crate::backend::wgpu_backend::{GpuMesh, RenderContext, RenderError, Vertex};

#[allow(unused_imports)]
use super::forward::ForwardPipeline;

/// Render pass type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPassType {
    /// Deferred pass for opaque objects.
    Deferred,
    /// Forward pass for transparent objects.
    Forward,
    /// Post-processing pass.
    PostProcess,
}

/// Material transparency mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransparencyMode {
    /// Fully opaque (uses deferred).
    #[default]
    Opaque,
    /// Alpha blending (uses forward).
    AlphaBlend,
    /// Alpha test/cutout (can use deferred).
    AlphaTest,
    /// Additive blending (uses forward).
    Additive,
    /// Multiplicative blending (uses forward).
    Multiply,
}

impl TransparencyMode {
    /// Check if this mode requires forward rendering.
    pub fn requires_forward(&self) -> bool {
        matches!(self, Self::AlphaBlend | Self::Additive | Self::Multiply)
    }
}

/// Render object for hybrid pipeline.
pub struct HybridRenderObject {
    /// Mesh to render.
    pub mesh: std::sync::Arc<GpuMesh>,
    /// Model uniforms.
    pub model_uniforms: DeferredModelUniforms,
    /// Material uniforms.
    pub material_uniforms: DeferredMaterialUniforms,
    /// Transparency mode.
    pub transparency: TransparencyMode,
    /// Sort key for ordering (depth for transparent objects).
    pub sort_key: f32,
}

/// HDR render target for intermediate results.
pub struct HdrTarget {
    /// HDR texture.
    texture: wgpu::Texture,
    /// HDR texture view.
    view: wgpu::TextureView,
    /// Depth texture.
    depth_texture: wgpu::Texture,
    /// Depth view.
    depth_view: wgpu::TextureView,
    /// Dimensions.
    width: u32,
    height: u32,
}

impl HdrTarget {
    /// Create a new HDR render target.
    pub fn new(ctx: &RenderContext, width: u32, height: u32) -> Self {
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR Target"),
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
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR Depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            depth_texture,
            depth_view,
            width,
            height,
        }
    }

    /// Resize the HDR target.
    pub fn resize(&mut self, ctx: &RenderContext, width: u32, height: u32) {
        if width == 0 || height == 0 || (width == self.width && height == self.height) {
            return;
        }
        *self = Self::new(ctx, width, height);
    }

    /// Get the color view.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get the depth view.
    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }
}

/// Hybrid rendering pipeline.
pub struct HybridPipeline {
    /// Deferred pipeline for opaque objects.
    deferred: DeferredPipeline,
    /// Forward pipeline for transparent objects.
    forward_transparent: wgpu::RenderPipeline,
    /// HDR render target.
    hdr_target: HdrTarget,
    /// Composite pipeline.
    composite_pipeline: wgpu::RenderPipeline,
    /// Composite bind group layout.
    composite_bind_group_layout: wgpu::BindGroupLayout,
    /// Composite bind group.
    composite_bind_group: wgpu::BindGroup,
    /// Sampler.
    sampler: wgpu::Sampler,
}

impl HybridPipeline {
    /// Create a new hybrid pipeline.
    pub fn new(
        ctx: &RenderContext,
        output_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let deferred = DeferredPipeline::new(ctx, wgpu::TextureFormat::Rgba16Float, width, height)?;
        let hdr_target = HdrTarget::new(ctx, width, height);

        // Create sampler
        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Hybrid Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create forward transparent pipeline
        let forward_shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Forward Transparent Shader"),
                source: wgpu::ShaderSource::Wgsl(FORWARD_TRANSPARENT_SHADER.into()),
            });

        let forward_layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Forward Transparent Layout"),
                bind_group_layouts: &[
                    deferred.camera_bind_group_layout(),
                    deferred.model_bind_group_layout(),
                    deferred.material_bind_group_layout(),
                    deferred.lights_bind_group_layout(),
                ],
                push_constant_ranges: &[],
            });

        let forward_transparent =
            ctx.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Forward Transparent Pipeline"),
                    layout: Some(&forward_layout),
                    vertex: wgpu::VertexState {
                        module: &forward_shader,
                        entry_point: Some("vs_main"),
                        buffers: &[Vertex::layout()],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &forward_shader,
                        entry_point: Some("fs_main"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: Some(wgpu::BlendState {
                                color: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::SrcAlpha,
                                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                    operation: wgpu::BlendOperation::Add,
                                },
                                alpha: wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::One,
                                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                    operation: wgpu::BlendOperation::Add,
                                },
                            }),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None, // Disable culling for transparent
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: false, // Don't write depth for transparent
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });

        // Create composite pipeline
        let composite_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Composite Bind Group Layout"),
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

        let composite_shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Composite Shader"),
                source: wgpu::ShaderSource::Wgsl(COMPOSITE_SHADER.into()),
            });

        let composite_layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Composite Layout"),
                bind_group_layouts: &[&composite_bind_group_layout],
                push_constant_ranges: &[],
            });

        let composite_pipeline =
            ctx.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Composite Pipeline"),
                    layout: Some(&composite_layout),
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
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });

        let composite_bind_group = Self::create_composite_bind_group(
            ctx,
            &composite_bind_group_layout,
            &hdr_target,
            &sampler,
        );

        Ok(Self {
            deferred,
            forward_transparent,
            hdr_target,
            composite_pipeline,
            composite_bind_group_layout,
            composite_bind_group,
            sampler,
        })
    }

    fn create_composite_bind_group(
        ctx: &RenderContext,
        layout: &wgpu::BindGroupLayout,
        hdr_target: &HdrTarget,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Composite Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_target.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Resize the pipeline.
    pub fn resize(&mut self, ctx: &RenderContext, width: u32, height: u32) {
        self.deferred.resize(ctx, width, height);
        self.hdr_target.resize(ctx, width, height);
        self.composite_bind_group = Self::create_composite_bind_group(
            ctx,
            &self.composite_bind_group_layout,
            &self.hdr_target,
            &self.sampler,
        );
    }

    /// Get the deferred pipeline.
    pub fn deferred(&self) -> &DeferredPipeline {
        &self.deferred
    }

    /// Get the HDR target view.
    pub fn hdr_view(&self) -> &wgpu::TextureView {
        self.hdr_target.view()
    }

    /// Create a camera bind group.
    pub fn create_camera_bind_group(
        &self,
        ctx: &RenderContext,
        uniforms: &DeferredCameraUniforms,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        self.deferred.create_camera_bind_group(ctx, uniforms)
    }

    /// Create a model bind group.
    pub fn create_model_bind_group(
        &self,
        ctx: &RenderContext,
        uniforms: &DeferredModelUniforms,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        self.deferred.create_model_bind_group(ctx, uniforms)
    }

    /// Create a material bind group.
    pub fn create_material_bind_group(
        &self,
        ctx: &RenderContext,
        uniforms: &DeferredMaterialUniforms,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        self.deferred.create_material_bind_group(ctx, uniforms)
    }

    /// Create a lights bind group.
    pub fn create_lights_bind_group(
        &self,
        ctx: &RenderContext,
        uniforms: &LightUniforms,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        self.deferred.create_lights_bind_group(ctx, uniforms)
    }

    /// Render a frame.
    pub fn render(
        &self,
        _ctx: &RenderContext,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        lights_bind_group: &wgpu::BindGroup,
        opaque_objects: &[(
            &wgpu::BindGroup, // model
            &wgpu::BindGroup, // material
            &GpuMesh,
        )],
        transparent_objects: &[(
            &wgpu::BindGroup, // model
            &wgpu::BindGroup, // material
            &GpuMesh,
        )],
    ) {
        // 1. Deferred geometry pass for opaque objects
        {
            let mut geometry_pass = self.deferred.begin_geometry_pass(encoder);
            for (model_bind_group, material_bind_group, mesh) in opaque_objects {
                self.deferred.render_geometry(
                    &mut geometry_pass,
                    camera_bind_group,
                    model_bind_group,
                    material_bind_group,
                    mesh,
                );
            }
        }

        // 2. Deferred lighting pass -> HDR target
        self.deferred.render_lighting(
            encoder,
            self.hdr_target.view(),
            camera_bind_group,
            lights_bind_group,
        );

        // 3. Forward pass for transparent objects (sorted back-to-front)
        if !transparent_objects.is_empty() {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Forward Transparent Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.hdr_target.view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Keep deferred result
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.hdr_target.depth_view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Keep deferred depth
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.forward_transparent);
            render_pass.set_bind_group(0, camera_bind_group, &[]);
            render_pass.set_bind_group(3, lights_bind_group, &[]);

            for (model_bind_group, material_bind_group, mesh) in transparent_objects {
                render_pass.set_bind_group(1, *model_bind_group, &[]);
                render_pass.set_bind_group(2, *material_bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        // 4. Composite to final target with tone mapping
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Composite Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.composite_pipeline);
            render_pass.set_bind_group(0, &self.composite_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
    }
}

/// Forward transparent shader with lighting.
const FORWARD_TRANSPARENT_SHADER: &str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    near_far: vec4<f32>,
}

struct ModelUniforms {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
}

struct MaterialUniforms {
    base_color: vec4<f32>,
    properties: vec4<f32>,
}

struct PointLight {
    position_radius: vec4<f32>,
    color_intensity: vec4<f32>,
}

struct LightUniforms {
    directional_dir: vec4<f32>,
    directional_color: vec4<f32>,
    ambient: vec4<f32>,
    light_counts: vec4<u32>,
    point_lights: array<PointLight, 64>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(1) @binding(0)
var<uniform> model: ModelUniforms;

@group(2) @binding(0)
var<uniform> material: MaterialUniforms;

@group(3) @binding(0)
var<uniform> lights: LightUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = model.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.world_normal = normalize((model.normal_matrix * vec4<f32>(in.normal, 0.0)).xyz);
    out.tex_coords = in.tex_coords;
    out.color = in.color;

    return out;
}

const PI: f32 = 3.14159265359;

fn calculate_lighting(
    normal: vec3<f32>,
    view_dir: vec3<f32>,
    world_pos: vec3<f32>,
    albedo: vec3<f32>,
) -> vec3<f32> {
    var lo = vec3<f32>(0.0);

    // Directional light (simplified Blinn-Phong for transparent)
    let dir_l = normalize(lights.directional_dir.xyz);
    let dir_h = normalize(view_dir + dir_l);
    let dir_diff = max(dot(normal, dir_l), 0.0);
    let dir_spec = pow(max(dot(normal, dir_h), 0.0), 32.0);
    lo += (albedo * dir_diff + vec3<f32>(0.3) * dir_spec) *
          lights.directional_color.rgb * lights.directional_dir.w;

    // Point lights
    let num_point_lights = lights.light_counts.x;
    for (var i = 0u; i < num_point_lights; i++) {
        let light = lights.point_lights[i];
        let light_pos = light.position_radius.xyz;
        let light_radius = light.position_radius.w;
        let light_color = light.color_intensity.rgb;
        let light_intensity = light.color_intensity.a;

        let to_light = light_pos - world_pos;
        let distance = length(to_light);

        if distance < light_radius {
            let l = normalize(to_light);
            let h = normalize(view_dir + l);
            let attenuation = 1.0 - smoothstep(0.0, light_radius, distance);

            let diff = max(dot(normal, l), 0.0);
            let spec = pow(max(dot(normal, h), 0.0), 32.0);

            lo += (albedo * diff + vec3<f32>(0.3) * spec) *
                  light_color * light_intensity * attenuation * attenuation;
        }
    }

    // Ambient
    let ambient = lights.ambient.rgb * lights.ambient.a * albedo;

    return ambient + lo;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = material.base_color.rgb * in.color.rgb;
    let alpha = material.base_color.a * in.color.a;

    let normal = normalize(in.world_normal);
    let view_dir = normalize(camera.camera_pos.xyz - in.world_position);

    let color = calculate_lighting(normal, view_dir, in.world_position, albedo);

    return vec4<f32>(color, alpha);
}
"#;

/// Composite shader with tone mapping.
const COMPOSITE_SHADER: &str = r#"
@group(0) @binding(0)
var hdr_texture: texture_2d<f32>;
@group(0) @binding(1)
var hdr_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
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

// ACES tone mapping
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return saturate((x * (a * x + b)) / (x * (c * x + d) + e));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(hdr_texture, hdr_sampler, in.uv).rgb;

    // Exposure
    let exposure = 1.0;
    color *= exposure;

    // ACES tone mapping
    color = aces_tonemap(color);

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transparency_mode() {
        assert!(!TransparencyMode::Opaque.requires_forward());
        assert!(!TransparencyMode::AlphaTest.requires_forward());
        assert!(TransparencyMode::AlphaBlend.requires_forward());
        assert!(TransparencyMode::Additive.requires_forward());
        assert!(TransparencyMode::Multiply.requires_forward());
    }
}
