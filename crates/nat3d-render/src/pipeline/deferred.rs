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

//! Deferred rendering pipeline.
//!
//! Implements a G-buffer based deferred rendering pipeline for handling
//! many lights efficiently. Geometry is rendered to multiple render targets
//! (G-buffer), then lighting is computed in screen-space.

use crate::backend::wgpu_backend::{GpuMesh, RenderContext, RenderError, Vertex};
use wgpu::util::DeviceExt;

/// G-buffer texture indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GBufferTexture {
    /// Position (RGB) + Depth (A).
    Position = 0,
    /// Normal (RGB) + unused (A).
    Normal = 1,
    /// Albedo (RGB) + Metallic (A).
    Albedo = 2,
    /// Roughness (R) + AO (G) + Emissive (B) + flags (A).
    Material = 3,
}

/// Number of G-buffer textures.
pub const GBUFFER_COUNT: usize = 4;

/// G-buffer for deferred rendering.
pub struct GBuffer {
    /// G-buffer textures.
    textures: [wgpu::Texture; GBUFFER_COUNT],
    /// G-buffer texture views.
    views: [wgpu::TextureView; GBUFFER_COUNT],
    /// Depth texture.
    depth_texture: wgpu::Texture,
    /// Depth view.
    depth_view: wgpu::TextureView,
    /// Current dimensions.
    width: u32,
    height: u32,
}

impl GBuffer {
    /// Create a new G-buffer.
    pub fn new(ctx: &RenderContext, width: u32, height: u32) -> Self {
        let formats = [
            wgpu::TextureFormat::Rgba32Float, // Position + depth
            wgpu::TextureFormat::Rgba16Float, // Normal
            wgpu::TextureFormat::Rgba8Unorm,  // Albedo + metallic
            wgpu::TextureFormat::Rgba8Unorm,  // Material properties
        ];

        let mut textures = Vec::with_capacity(GBUFFER_COUNT);
        let mut views = Vec::with_capacity(GBUFFER_COUNT);

        for (i, &format) in formats.iter().enumerate() {
            let texture = ctx.create_safe_texture(wgpu::TextureDescriptor {
                label: Some(&format!("GBuffer Texture {}", i)),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            textures.push(texture);
            views.push(view);
        }

        let depth_texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GBuffer Depth"),
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
            textures: textures.try_into().unwrap(),
            views: views.try_into().unwrap(),
            depth_texture,
            depth_view,
            width,
            height,
        }
    }

    /// Resize the G-buffer.
    pub fn resize(&mut self, ctx: &RenderContext, width: u32, height: u32) {
        if width == 0 || height == 0 || (width == self.width && height == self.height) {
            return;
        }

        *self = Self::new(ctx, width, height);
    }

    /// Get a texture view.
    pub fn view(&self, texture: GBufferTexture) -> &wgpu::TextureView {
        &self.views[texture as usize]
    }

    /// Get all views.
    pub fn views(&self) -> &[wgpu::TextureView; GBUFFER_COUNT] {
        &self.views
    }

    /// Get the depth view.
    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    /// Get dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Camera uniforms for deferred rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DeferredCameraUniforms {
    /// View-projection matrix.
    pub view_proj: [[f32; 4]; 4],
    /// Inverse view-projection matrix.
    pub inv_view_proj: [[f32; 4]; 4],
    /// View matrix.
    pub view: [[f32; 4]; 4],
    /// Projection matrix.
    pub proj: [[f32; 4]; 4],
    /// Camera position.
    pub camera_pos: [f32; 4],
    /// Near and far planes.
    pub near_far: [f32; 4],
}

impl Default for DeferredCameraUniforms {
    fn default() -> Self {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        Self {
            view_proj: identity,
            inv_view_proj: identity,
            view: identity,
            proj: identity,
            camera_pos: [0.0, 0.0, 5.0, 1.0],
            near_far: [0.1, 1000.0, 0.0, 0.0],
        }
    }
}

/// Model uniforms for deferred rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DeferredModelUniforms {
    /// Model matrix.
    pub model: [[f32; 4]; 4],
    /// Normal matrix (transpose of inverse model matrix).
    pub normal_matrix: [[f32; 4]; 4],
}

impl Default for DeferredModelUniforms {
    fn default() -> Self {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        Self {
            model: identity,
            normal_matrix: identity,
        }
    }
}

/// Material uniforms for deferred rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DeferredMaterialUniforms {
    /// Base color (RGB) + alpha (A).
    pub base_color: [f32; 4],
    /// Metallic, roughness, AO, emissive.
    pub properties: [f32; 4],
}

impl Default for DeferredMaterialUniforms {
    fn default() -> Self {
        Self {
            base_color: [0.8, 0.8, 0.8, 1.0],
            properties: [0.0, 0.5, 1.0, 0.0], // metallic, roughness, ao, emissive
        }
    }
}

/// Point light data.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLightData {
    /// Position (XYZ) + radius (W).
    pub position_radius: [f32; 4],
    /// Color (RGB) + intensity (A).
    pub color_intensity: [f32; 4],
}

/// Maximum number of point lights.
pub const MAX_POINT_LIGHTS: usize = 64;

/// Light uniforms for deferred rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniforms {
    /// Directional light direction (XYZ) + intensity (W).
    pub directional_dir: [f32; 4],
    /// Directional light color (RGB) + unused (A).
    pub directional_color: [f32; 4],
    /// Ambient color (RGB) + intensity (A).
    pub ambient: [f32; 4],
    /// Number of point lights (X) + unused (YZW).
    pub light_counts: [u32; 4],
    /// Point lights.
    pub point_lights: [PointLightData; MAX_POINT_LIGHTS],
}

impl Default for LightUniforms {
    fn default() -> Self {
        Self {
            directional_dir: [0.5, 1.0, 0.3, 1.0],
            directional_color: [1.0, 0.98, 0.95, 1.0],
            ambient: [0.1, 0.1, 0.12, 0.3],
            light_counts: [0, 0, 0, 0],
            point_lights: [PointLightData {
                position_radius: [0.0; 4],
                color_intensity: [0.0; 4],
            }; MAX_POINT_LIGHTS],
        }
    }
}

/// Deferred rendering pipeline.
pub struct DeferredPipeline {
    /// G-buffer.
    gbuffer: GBuffer,
    /// Geometry pass pipeline.
    geometry_pipeline: wgpu::RenderPipeline,
    /// Lighting pass pipeline.
    lighting_pipeline: wgpu::RenderPipeline,
    /// Bind group layouts.
    camera_bind_group_layout: wgpu::BindGroupLayout,
    model_bind_group_layout: wgpu::BindGroupLayout,
    material_bind_group_layout: wgpu::BindGroupLayout,
    gbuffer_bind_group_layout: wgpu::BindGroupLayout,
    lights_bind_group_layout: wgpu::BindGroupLayout,
    /// G-buffer bind group.
    gbuffer_bind_group: wgpu::BindGroup,
    /// G-buffer sampler.
    gbuffer_sampler: wgpu::Sampler,
}

impl DeferredPipeline {
    /// Create a new deferred pipeline.
    pub fn new(
        ctx: &RenderContext,
        output_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let gbuffer = GBuffer::new(ctx, width, height);

        // Create sampler
        let gbuffer_sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("GBuffer Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layouts
        let camera_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Deferred Camera Layout"),
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
                    label: Some("Deferred Model Layout"),
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
                    label: Some("Deferred Material Layout"),
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

        let gbuffer_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("GBuffer Read Layout"),
                    entries: &[
                        // Position texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        // Normal texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        // Albedo texture
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
                        // Material texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        // Depth texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Depth,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        // Sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 5,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                            count: None,
                        },
                    ],
                });

        let lights_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Lights Layout"),
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

        // Create geometry pass pipeline
        let geometry_shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Deferred Geometry Shader"),
                source: wgpu::ShaderSource::Wgsl(GEOMETRY_SHADER.into()),
            });

        let geometry_layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Deferred Geometry Layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    &model_bind_group_layout,
                    &material_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let geometry_pipeline =
            ctx.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Deferred Geometry Pipeline"),
                    layout: Some(&geometry_layout),
                    vertex: wgpu::VertexState {
                        module: &geometry_shader,
                        entry_point: Some("vs_main"),
                        buffers: &[Vertex::layout()],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &geometry_shader,
                        entry_point: Some("fs_main"),
                        targets: &[
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba32Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba8Unorm,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba8Unorm,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                        ],
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
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });

        // Create lighting pass pipeline
        let lighting_shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Deferred Lighting Shader"),
                source: wgpu::ShaderSource::Wgsl(LIGHTING_SHADER.into()),
            });

        let lighting_layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Deferred Lighting Layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    &gbuffer_bind_group_layout,
                    &lights_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let lighting_pipeline =
            ctx.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Deferred Lighting Pipeline"),
                    layout: Some(&lighting_layout),
                    vertex: wgpu::VertexState {
                        module: &lighting_shader,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &lighting_shader,
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

        // Create G-buffer bind group
        let gbuffer_bind_group = Self::create_gbuffer_bind_group(
            ctx,
            &gbuffer_bind_group_layout,
            &gbuffer,
            &gbuffer_sampler,
        );

        Ok(Self {
            gbuffer,
            geometry_pipeline,
            lighting_pipeline,
            camera_bind_group_layout,
            model_bind_group_layout,
            material_bind_group_layout,
            gbuffer_bind_group_layout,
            lights_bind_group_layout,
            gbuffer_bind_group,
            gbuffer_sampler,
        })
    }

    fn create_gbuffer_bind_group(
        ctx: &RenderContext,
        layout: &wgpu::BindGroupLayout,
        gbuffer: &GBuffer,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GBuffer Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.view(GBufferTexture::Position),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.view(GBufferTexture::Normal),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.view(GBufferTexture::Albedo),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        gbuffer.view(GBufferTexture::Material),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(gbuffer.depth_view()),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Resize the pipeline.
    pub fn resize(&mut self, ctx: &RenderContext, width: u32, height: u32) {
        self.gbuffer.resize(ctx, width, height);
        self.gbuffer_bind_group = Self::create_gbuffer_bind_group(
            ctx,
            &self.gbuffer_bind_group_layout,
            &self.gbuffer,
            &self.gbuffer_sampler,
        );
    }

    /// Get the camera bind group layout.
    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bind_group_layout
    }

    /// Get the model bind group layout.
    pub fn model_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.model_bind_group_layout
    }

    /// Get the material bind group layout.
    pub fn material_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.material_bind_group_layout
    }

    /// Get the lights bind group layout.
    pub fn lights_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.lights_bind_group_layout
    }

    /// Create a camera bind group.
    pub fn create_camera_bind_group(
        &self,
        ctx: &RenderContext,
        uniforms: &DeferredCameraUniforms,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Deferred Camera Buffer"),
                contents: bytemuck::bytes_of(uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Deferred Camera Bind Group"),
            layout: &self.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        (buffer, bind_group)
    }

    /// Create a model bind group.
    pub fn create_model_bind_group(
        &self,
        ctx: &RenderContext,
        uniforms: &DeferredModelUniforms,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Deferred Model Buffer"),
                contents: bytemuck::bytes_of(uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Deferred Model Bind Group"),
            layout: &self.model_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        (buffer, bind_group)
    }

    /// Create a material bind group.
    pub fn create_material_bind_group(
        &self,
        ctx: &RenderContext,
        uniforms: &DeferredMaterialUniforms,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Deferred Material Buffer"),
                contents: bytemuck::bytes_of(uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Deferred Material Bind Group"),
            layout: &self.material_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        (buffer, bind_group)
    }

    /// Create a lights bind group.
    pub fn create_lights_bind_group(
        &self,
        ctx: &RenderContext,
        uniforms: &LightUniforms,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Lights Buffer"),
                contents: bytemuck::bytes_of(uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lights Bind Group"),
            layout: &self.lights_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        (buffer, bind_group)
    }

    /// Begin geometry pass.
    pub fn begin_geometry_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Deferred Geometry Pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: self.gbuffer.view(GBufferTexture::Position),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: self.gbuffer.view(GBufferTexture::Normal),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: self.gbuffer.view(GBufferTexture::Albedo),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: self.gbuffer.view(GBufferTexture::Material),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: self.gbuffer.depth_view(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }

    /// Render geometry.
    pub fn render_geometry<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
        model_bind_group: &'a wgpu::BindGroup,
        material_bind_group: &'a wgpu::BindGroup,
        mesh: &'a GpuMesh,
    ) {
        render_pass.set_pipeline(&self.geometry_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, model_bind_group, &[]);
        render_pass.set_bind_group(2, material_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }

    /// Render lighting pass.
    pub fn render_lighting<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        target: &'a wgpu::TextureView,
        camera_bind_group: &'a wgpu::BindGroup,
        lights_bind_group: &'a wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Deferred Lighting Pass"),
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

        render_pass.set_pipeline(&self.lighting_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.gbuffer_bind_group, &[]);
        render_pass.set_bind_group(2, lights_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

/// Geometry pass shader.
const GEOMETRY_SHADER: &str = r#"
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
    properties: vec4<f32>, // metallic, roughness, ao, emissive
}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(1) @binding(0)
var<uniform> model: ModelUniforms;

@group(2) @binding(0)
var<uniform> material: MaterialUniforms;

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

struct GBufferOutput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) albedo: vec4<f32>,
    @location(3) material: vec4<f32>,
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

@fragment
fn fs_main(in: VertexOutput) -> GBufferOutput {
    var out: GBufferOutput;

    // Position + linear depth
    let view_pos = camera.view * vec4<f32>(in.world_position, 1.0);
    let linear_depth = -view_pos.z;
    out.position = vec4<f32>(in.world_position, linear_depth);

    // Normal (encoded as 0-1 range)
    out.normal = vec4<f32>(in.world_normal * 0.5 + 0.5, 1.0);

    // Albedo + metallic
    let albedo = material.base_color.rgb * in.color.rgb;
    out.albedo = vec4<f32>(albedo, material.properties.x); // metallic in alpha

    // Material properties
    out.material = vec4<f32>(
        material.properties.y, // roughness
        material.properties.z, // ao
        material.properties.w, // emissive
        1.0
    );

    return out;
}
"#;

/// Lighting pass shader with PBR.
const LIGHTING_SHADER: &str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    near_far: vec4<f32>,
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
var gbuffer_position: texture_2d<f32>;
@group(1) @binding(1)
var gbuffer_normal: texture_2d<f32>;
@group(1) @binding(2)
var gbuffer_albedo: texture_2d<f32>;
@group(1) @binding(3)
var gbuffer_material: texture_2d<f32>;
@group(1) @binding(4)
var gbuffer_depth: texture_depth_2d;
@group(1) @binding(5)
var gbuffer_sampler: sampler;

@group(2) @binding(0)
var<uniform> lights: LightUniforms;

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

const PI: f32 = 3.14159265359;

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(n, h), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;
    let denom = n_dot_h2 * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn calculate_pbr_light(
    n: vec3<f32>,
    v: vec3<f32>,
    l: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    light_color: vec3<f32>,
    light_intensity: f32,
) -> vec3<f32> {
    let h = normalize(v + l);
    let radiance = light_color * light_intensity;

    var f0 = vec3<f32>(0.04);
    f0 = mix(f0, albedo, metallic);

    let ndf = distribution_ggx(n, h, roughness);
    let g = geometry_smith(n, v, l, roughness);
    let f = fresnel_schlick(max(dot(h, v), 0.0), f0);

    let ks = f;
    var kd = vec3<f32>(1.0) - ks;
    kd *= 1.0 - metallic;

    let numerator = ndf * g * f;
    let denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001;
    let specular = numerator / denominator;

    let n_dot_l = max(dot(n, l), 0.0);
    return (kd * albedo / PI + specular) * radiance * n_dot_l;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(textureDimensions(gbuffer_position));
    let tex_coord = vec2<i32>(in.uv * tex_size);

    // Sample G-buffer
    let position_data = textureLoad(gbuffer_position, tex_coord, 0);
    let normal_data = textureLoad(gbuffer_normal, tex_coord, 0);
    let albedo_data = textureLoad(gbuffer_albedo, tex_coord, 0);
    let material_data = textureLoad(gbuffer_material, tex_coord, 0);

    // Early out for background
    if position_data.w == 0.0 {
        return vec4<f32>(0.05, 0.05, 0.08, 1.0);
    }

    let world_pos = position_data.xyz;
    let normal = normalize(normal_data.xyz * 2.0 - 1.0);
    let albedo = albedo_data.rgb;
    let metallic = albedo_data.a;
    let roughness = max(material_data.r, 0.04);
    let ao = material_data.g;
    let emissive = material_data.b;

    let v = normalize(camera.camera_pos.xyz - world_pos);

    var lo = vec3<f32>(0.0);

    // Directional light
    let dir_l = normalize(lights.directional_dir.xyz);
    lo += calculate_pbr_light(
        normal, v, dir_l,
        albedo, metallic, roughness,
        lights.directional_color.rgb,
        lights.directional_dir.w
    );

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
            let attenuation = 1.0 - smoothstep(0.0, light_radius, distance);

            lo += calculate_pbr_light(
                normal, v, l,
                albedo, metallic, roughness,
                light_color,
                light_intensity * attenuation * attenuation
            );
        }
    }

    // Ambient
    let ambient = lights.ambient.rgb * lights.ambient.a * albedo * ao;
    var color = ambient + lo;

    // Emissive
    color += albedo * emissive;

    return vec4<f32>(color, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deferred_uniforms_size() {
        assert_eq!(std::mem::size_of::<DeferredCameraUniforms>(), 288);
        assert_eq!(std::mem::size_of::<DeferredModelUniforms>(), 128);
        assert_eq!(std::mem::size_of::<DeferredMaterialUniforms>(), 32);
        assert_eq!(std::mem::size_of::<PointLightData>(), 32);
    }

    #[test]
    fn test_gbuffer_texture_indices() {
        assert_eq!(GBufferTexture::Position as usize, 0);
        assert_eq!(GBufferTexture::Normal as usize, 1);
        assert_eq!(GBufferTexture::Albedo as usize, 2);
        assert_eq!(GBufferTexture::Material as usize, 3);
    }
}
