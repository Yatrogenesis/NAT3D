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

//! wgpu rendering backend.
//!
//! Provides GPU-accelerated rendering using the wgpu library.

use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Rendering context holding wgpu resources.
pub struct RenderContext {
    /// wgpu instance.
    pub instance: Arc<wgpu::Instance>,
    /// GPU adapter.
    pub adapter: Arc<wgpu::Adapter>,
    /// GPU device.
    pub device: Arc<wgpu::Device>,
    /// Command queue.
    pub queue: Arc<wgpu::Queue>,
    /// Surface configuration (if rendering to window).
    pub surface_config: Option<wgpu::SurfaceConfiguration>,
}

impl RenderContext {
    /// Create a new render context.
    pub async fn new() -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or(RenderError::NoAdapter)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("NAT3D Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .map_err(|e| RenderError::DeviceCreation(e.to_string()))?;

        Ok(Self {
            instance: Arc::new(instance),
            adapter: Arc::new(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
            surface_config: None,
        })
    }

    /// Get adapter info.
    pub fn adapter_info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }

    /// Create a buffer.
    pub fn create_buffer(
        &self,
        label: &str,
        contents: &[u8],
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
    }

    /// Create an empty buffer.
    pub fn create_empty_buffer(
        &self,
        label: &str,
        size: u64,
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
    }

    /// Create a safe texture with automatic hardware limit clamping.
    pub fn create_safe_texture(&self, mut desc: wgpu::TextureDescriptor) -> wgpu::Texture {
        let max_dim = self.device.limits().max_texture_dimension_2d;

        if desc.size.width > max_dim {
            tracing::warn!(
                "Texture width {} exceeds GPU limit {}, clamping.",
                desc.size.width,
                max_dim
            );
            desc.size.width = max_dim;
        }
        if desc.size.height > max_dim {
            tracing::warn!(
                "Texture height {} exceeds GPU limit {}, clamping.",
                desc.size.height,
                max_dim
            );
            desc.size.height = max_dim;
        }

        self.device.create_texture(&desc)
    }

    /// Create a texture.
    pub fn create_texture(&self, desc: &wgpu::TextureDescriptor) -> wgpu::Texture {
        self.device.create_texture(desc)
    }

    /// Create a depth texture.
    pub fn create_depth_texture(
        &self,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
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

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    /// Create a shader module from WGSL source.
    pub fn create_shader(&self, label: &str, source: &str) -> wgpu::ShaderModule {
        self.device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            })
    }

    /// Create a bind group layout.
    pub fn create_bind_group_layout(
        &self,
        label: &str,
        entries: &[wgpu::BindGroupLayoutEntry],
    ) -> wgpu::BindGroupLayout {
        self.device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
    }

    /// Create a bind group.
    pub fn create_bind_group(
        &self,
        label: &str,
        layout: &wgpu::BindGroupLayout,
        entries: &[wgpu::BindGroupEntry],
    ) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries,
        })
    }

    /// Create a pipeline layout.
    pub fn create_pipeline_layout(
        &self,
        label: &str,
        bind_group_layouts: &[&wgpu::BindGroupLayout],
    ) -> wgpu::PipelineLayout {
        self.device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts,
                push_constant_ranges: &[],
            })
    }

    /// Submit command buffers.
    pub fn submit(&self, commands: impl IntoIterator<Item = wgpu::CommandBuffer>) {
        self.queue.submit(commands);
    }
}

/// Render error types.
#[derive(Debug, Clone)]
pub enum RenderError {
    /// No suitable GPU adapter found.
    NoAdapter,
    /// Failed to create device.
    DeviceCreation(String),
    /// Surface error.
    Surface(String),
    /// Shader compilation error.
    Shader(String),
    /// Buffer error.
    Buffer(String),
    /// Texture error.
    Texture(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAdapter => write!(f, "No suitable GPU adapter found"),
            Self::DeviceCreation(e) => write!(f, "Failed to create device: {}", e),
            Self::Surface(e) => write!(f, "Surface error: {}", e),
            Self::Shader(e) => write!(f, "Shader error: {}", e),
            Self::Buffer(e) => write!(f, "Buffer error: {}", e),
            Self::Texture(e) => write!(f, "Texture error: {}", e),
        }
    }
}

impl std::error::Error for RenderError {}

/// Vertex format for mesh rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    /// Position (x, y, z).
    pub position: [f32; 3],
    /// Normal (x, y, z).
    pub normal: [f32; 3],
    /// Texture coordinates (u, v).
    pub tex_coords: [f32; 2],
    /// Vertex color (r, g, b, a).
    pub color: [f32; 4],
}

impl Vertex {
    /// Vertex buffer layout.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Tex coords
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }

    /// Create a new vertex.
    pub fn new(
        position: [f32; 3],
        normal: [f32; 3],
        tex_coords: [f32; 2],
        color: [f32; 4],
    ) -> Self {
        Self {
            position,
            normal,
            tex_coords,
            color,
        }
    }

    /// Create a vertex with position only.
    pub fn from_position(position: [f32; 3]) -> Self {
        Self {
            position,
            normal: [0.0, 1.0, 0.0],
            tex_coords: [0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// Camera uniforms for shaders.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniforms {
    /// View-projection matrix.
    pub view_proj: [[f32; 4]; 4],
    /// View matrix.
    pub view: [[f32; 4]; 4],
    /// Projection matrix.
    pub proj: [[f32; 4]; 4],
    /// Camera position.
    pub camera_pos: [f32; 4],
}

impl Default for CameraUniforms {
    fn default() -> Self {
        Self {
            view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            view: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            camera_pos: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

/// Model uniforms for shaders.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelUniforms {
    /// Model matrix.
    pub model: [[f32; 4]; 4],
    /// Normal matrix (inverse transpose of model).
    pub normal: [[f32; 4]; 4],
}

impl Default for ModelUniforms {
    fn default() -> Self {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        Self {
            model: identity,
            normal: identity,
        }
    }
}

/// Material uniforms for shaders.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniforms {
    /// Base color (albedo).
    pub base_color: [f32; 4],
    /// Metallic factor.
    pub metallic: f32,
    /// Roughness factor.
    pub roughness: f32,
    /// Ambient occlusion factor.
    pub ao: f32,
    /// Emissive strength.
    pub emissive: f32,
}

impl Default for MaterialUniforms {
    fn default() -> Self {
        Self {
            base_color: [0.8, 0.8, 0.8, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            ao: 1.0,
            emissive: 0.0,
        }
    }
}

/// Light uniforms for shaders.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniforms {
    /// Light position (w=1 for point, w=0 for directional).
    pub position: [f32; 4],
    /// Light color.
    pub color: [f32; 4],
    /// Light direction (for spot/directional).
    pub direction: [f32; 4],
    /// Light parameters (intensity, range, inner_cone, outer_cone).
    pub params: [f32; 4],
}

impl Default for LightUniforms {
    fn default() -> Self {
        Self {
            position: [0.0, 10.0, 0.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            direction: [0.0, -1.0, 0.0, 0.0],
            params: [1.0, 100.0, 0.9, 0.8],
        }
    }
}

/// GPU mesh data.
pub struct GpuMesh {
    /// Vertex buffer.
    pub vertex_buffer: wgpu::Buffer,
    /// Index buffer.
    pub index_buffer: wgpu::Buffer,
    /// Number of indices.
    pub index_count: u32,
}

impl GpuMesh {
    /// Create a new GPU mesh.
    pub fn new(ctx: &RenderContext, vertices: &[Vertex], indices: &[u32]) -> Self {
        let vertex_buffer = ctx.create_buffer(
            "Mesh Vertices",
            bytemuck::cast_slice(vertices),
            wgpu::BufferUsages::VERTEX,
        );

        let index_buffer = ctx.create_buffer(
            "Mesh Indices",
            bytemuck::cast_slice(indices),
            wgpu::BufferUsages::INDEX,
        );

        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }

    /// Create a simple cube mesh.
    pub fn cube(ctx: &RenderContext, size: f32) -> Self {
        let s = size / 2.0;

        let vertices = vec![
            // Front face
            Vertex::new(
                [-s, -s, s],
                [0.0, 0.0, 1.0],
                [0.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [s, -s, s],
                [0.0, 0.0, 1.0],
                [1.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new([s, s, s], [0.0, 0.0, 1.0], [1.0, 1.0], [1.0, 1.0, 1.0, 1.0]),
            Vertex::new(
                [-s, s, s],
                [0.0, 0.0, 1.0],
                [0.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            // Back face
            Vertex::new(
                [s, -s, -s],
                [0.0, 0.0, -1.0],
                [0.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [-s, -s, -s],
                [0.0, 0.0, -1.0],
                [1.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [-s, s, -s],
                [0.0, 0.0, -1.0],
                [1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [s, s, -s],
                [0.0, 0.0, -1.0],
                [0.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            // Top face
            Vertex::new(
                [-s, s, s],
                [0.0, 1.0, 0.0],
                [0.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new([s, s, s], [0.0, 1.0, 0.0], [1.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
            Vertex::new(
                [s, s, -s],
                [0.0, 1.0, 0.0],
                [1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [-s, s, -s],
                [0.0, 1.0, 0.0],
                [0.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            // Bottom face
            Vertex::new(
                [-s, -s, -s],
                [0.0, -1.0, 0.0],
                [0.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [s, -s, -s],
                [0.0, -1.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [s, -s, s],
                [0.0, -1.0, 0.0],
                [1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [-s, -s, s],
                [0.0, -1.0, 0.0],
                [0.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            // Right face
            Vertex::new(
                [s, -s, s],
                [1.0, 0.0, 0.0],
                [0.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [s, -s, -s],
                [1.0, 0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [s, s, -s],
                [1.0, 0.0, 0.0],
                [1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new([s, s, s], [1.0, 0.0, 0.0], [0.0, 1.0], [1.0, 1.0, 1.0, 1.0]),
            // Left face
            Vertex::new(
                [-s, -s, -s],
                [-1.0, 0.0, 0.0],
                [0.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [-s, -s, s],
                [-1.0, 0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [-s, s, s],
                [-1.0, 0.0, 0.0],
                [1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [-s, s, -s],
                [-1.0, 0.0, 0.0],
                [0.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
        ];

        let indices: Vec<u32> = vec![
            0, 1, 2, 0, 2, 3, // Front
            4, 5, 6, 4, 6, 7, // Back
            8, 9, 10, 8, 10, 11, // Top
            12, 13, 14, 12, 14, 15, // Bottom
            16, 17, 18, 16, 18, 19, // Right
            20, 21, 22, 20, 22, 23, // Left
        ];

        Self::new(ctx, &vertices, &indices)
    }
}

/// Default shaders for basic rendering.
pub mod shaders {
    /// Basic vertex shader.
    pub const BASIC_VERTEX: &str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
}

struct ModelUniforms {
    model: mat4x4<f32>,
    normal: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(1) @binding(0)
var<uniform> model: ModelUniforms;

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
    out.world_normal = normalize((model.normal * vec4<f32>(in.normal, 0.0)).xyz);
    out.tex_coords = in.tex_coords;
    out.color = in.color;

    return out;
}
"#;

    /// Basic fragment shader with simple lighting.
    pub const BASIC_FRAGMENT: &str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
}

struct MaterialUniforms {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    ao: f32,
    emissive: f32,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(2) @binding(0)
var<uniform> material: MaterialUniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) color: vec4<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let view_dir = normalize(camera.camera_pos.xyz - in.world_position);
    let half_dir = normalize(light_dir + view_dir);

    let ambient = 0.1;
    let diffuse = max(dot(in.world_normal, light_dir), 0.0);
    let specular = pow(max(dot(in.world_normal, half_dir), 0.0), 32.0) * (1.0 - material.roughness);

    let lighting = ambient + diffuse + specular * 0.5;
    let color = material.base_color.rgb * in.color.rgb * lighting;

    return vec4<f32>(color, material.base_color.a * in.color.a);
}
"#;
}
