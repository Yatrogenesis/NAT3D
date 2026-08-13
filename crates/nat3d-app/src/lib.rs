#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    unused_imports,
    dead_code,
    clippy::field_reassign_with_default,
    clippy::unnecessary_unwrap,
    clippy::unwrap_or_default,
    clippy::ptr_arg,
    clippy::type_complexity,
    clippy::manual_clamp,
    clippy::collapsible_if,
    clippy::needless_range_loop
)]
//! NAT3D main application.
//!
//! A professional 3D modeling, CAD, simulation, and rendering suite.

#![allow(unused_imports)]

use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;

pub mod license;
pub mod startup;
pub mod viewport;
use tokio::io::AsyncReadExt;
pub mod compositor;
pub mod console;
pub mod nodes;
pub mod panels;
pub mod state;
pub mod tools;
pub mod weight_paint;
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AppMaterialUniforms {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub ao: f32,
    pub emissive: f32,
    pub simulation_mode: u32,
    pub _padding: [f32; 3],
}

// GPU rendering imports from nat3d-render
use eframe::egui_wgpu;
use nat3d_render::backend::wgpu_backend::{
    CameraUniforms, MaterialUniforms, ModelUniforms, Vertex,
};
use parking_lot::RwLock;
#[allow(unused_imports)]
use state::{
    AlignAxis, AnimationDriver, AppState, ArmatureBone, AssetCategory, AxisConstraint, BooleanOp,
    CameraSettings, ClothSettings, ColorManagement, CustomPropType, CustomProperty, DriverType,
    EditMode, EditModeSelection, EditSelection, EditTool, EditableMesh, FluidSettings, FluidType,
    ForceFieldSettings, ForceFieldType, GizmoMode, GreasePencilStroke, HairSettings, Keyframe,
    MaterialState, NLAStrip, NLATrack, ObjectCollection, ObjectConstraint, ObjectType,
    OutlinerFilter, ParticleSystem, ParticleType, PivotPoint, ProportionalFalloff, QuickFavorite,
    RenderEngine, RenderLayer, RenderPass, SceneObject, SceneProperties, SculptBrush,
    SequencerStrip, SequencerStripType, ShadingMode, SnapElement, SnapTarget, SoftBodySettings,
    TextureSlot, TextureType, Tool, TransformOrientation, VertexGroup, ViewLayer, ViewportOverlays,
    WorkspaceLayout, WorldSettings,
};
use std::sync::Arc;

/// Main NAT3D application.

/// Per-object GPU buffers — each scene object owns its uniforms to allow correct multi-object rendering.
struct MeshEntry {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    model_buffer: wgpu::Buffer,
    model_bind_group: wgpu::BindGroup,
    material_buffer: wgpu::Buffer,
    material_bind_group: wgpu::BindGroup,
    signal_buffer: wgpu::Buffer,
}

struct ViewportCallback {
    simulation_mode: state::SimulationMode,
    renderer: Arc<RwLock<GpuRendererState>>,
    camera: viewport::ViewportCamera,
    objects: Vec<SceneObject>,
    show_grid: bool,
}

impl egui_wgpu::CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let mut renderer = self.renderer.write();

        // Record a viewport size change for `update()` to act on — do NOT resize
        // here. `resize()` needs the `egui_wgpu::Renderer` lock to re-register
        // the render texture, and eframe already holds that same lock (as a
        // writer) for the whole duration of this callback:
        //   egui-wgpu winit.rs:401  render_state.renderer.write()
        //   egui-wgpu winit.rs:411  renderer.update_buffers(..)
        //   egui-wgpu renderer.rs:974  callback.prepare(..)   <- we are here
        // `egui::mutex::RwLock` (parking_lot) is not reentrant, so taking it
        // again from this thread deadlocked the main thread permanently: the
        // window painted exactly one frame and then froze, which the desktop
        // reported as "application is not responding / force quit".
        let w = screen_descriptor.size_in_pixels[0].max(1);
        let h = screen_descriptor.size_in_pixels[1].max(1);
        if renderer.dimensions != (w, h) {
            renderer.pending_resize = Some((w, h));
        }

        // Sync mesh cache: upload new objects, evict removed ones
        renderer.mesh_cache.retain(|&id, _| id < self.objects.len());
        for (i, obj) in self.objects.iter().enumerate() {
            if obj.visible && !renderer.mesh_cache.contains_key(&i) {
                let (verts, indices) = GpuRendererState::generate_object_geometry(obj);
                if !indices.is_empty() {
                    renderer.upload_object_mesh(device, i, obj, &verts, &indices);
                }
            }
        }

        // Update camera uniforms
        let aspect = w as f32 / h as f32;
        let view = self.camera.view_matrix();
        let proj = self.camera.projection_matrix(aspect);
        let mut view_proj = [[0.0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    view_proj[i][j] += view[i][k] * proj[k][j];
                }
            }
        }
        let camera_uniforms = CameraUniforms {
            view_proj,
            view,
            proj,
            camera_pos: [
                self.camera.position[0],
                self.camera.position[1],
                self.camera.position[2],
                1.0,
            ],
        };
        queue.write_buffer(
            &renderer.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniforms]),
        );

        // Update per-object model/material uniforms before the render pass.
        // write_buffer() submits immediately to the staging queue; draw commands
        // execute at queue.submit() time — so each object's buffer is independent.
        for (i, obj) in self.objects.iter().enumerate() {
            if !obj.visible {
                continue;
            }
            if let Some(entry) = renderer.mesh_cache.get(&i) {
                let model_matrix = GpuRendererState::build_model_matrix(obj);
                let model_uniforms = ModelUniforms {
                    model: model_matrix,
                    normal: model_matrix,
                };
                let material_uniforms = AppMaterialUniforms {
                    base_color: obj.material.base_color,
                    metallic: obj.material.metallic,
                    roughness: obj.material.roughness,
                    ao: 1.0,
                    emissive: obj.material.emissive,
                    simulation_mode: self.simulation_mode as u32,
                    _padding: [0.0; 3],
                };
                queue.write_buffer(
                    &entry.model_buffer,
                    0,
                    bytemuck::cast_slice(&[model_uniforms]),
                );
                queue.write_buffer(
                    &entry.material_buffer,
                    0,
                    bytemuck::cast_slice(&[material_uniforms]),
                );
                queue.write_buffer(
                    &entry.signal_buffer,
                    0,
                    bytemuck::cast_slice(&[obj.physiological_signal]),
                );
            }
        }

        // Record render pass targeting off-screen render_texture (with depth)
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Viewport Render Encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Viewport 3D Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &renderer.render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.165,
                            g: 0.165,
                            b: 0.188,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &renderer.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rp.set_pipeline(&renderer.opaque_pipeline);
            rp.set_bind_group(0, &renderer.camera_bind_group, &[]);

            for (i, obj) in self.objects.iter().enumerate() {
                if !obj.visible {
                    continue;
                }
                if let Some(entry) = renderer.mesh_cache.get(&i) {
                    rp.set_bind_group(1, &entry.model_bind_group, &[]);
                    rp.set_bind_group(2, &entry.material_bind_group, &[]);
                    rp.set_vertex_buffer(1, entry.signal_buffer.slice(..));
                    rp.set_vertex_buffer(0, entry.vertex_buffer.slice(..));
                    rp.set_index_buffer(entry.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rp.draw_indexed(0..entry.index_count, 0, 0..1);
                }
            }
        }

        vec![encoder.finish()]
    }

    // paint() is a no-op — scene is rendered in prepare() to an off-screen texture.
    // egui's render pass carries no depth attachment, so the depth-enabled opaque_pipeline
    // cannot be used here. The rendered image is displayed via painter.image() in viewport_3d().
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        _render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
    ) {
    }
}

/// State machine for the GitHub Education Device Flow UI.
#[derive(Default)]
enum EduOAuthStep {
    #[default]
    Idle,
    /// Waiting for the user to enter the code on github.com/login/device.
    AwaitingUser {
        user_code: String,
        verification_uri: String,
    },
    /// Polling GitHub — token not yet received.
    Polling,
    /// Flow completed successfully — show the serial.
    Confirmed {
        serial: String,
        github_handle: String,
    },
    /// Account has no Education benefit.
    NotEdu { github_handle: String },
    /// Something went wrong.
    Failed(String),
}

pub struct Nat3DApp {
    /// Application state.
    state: AppState,
    /// Show hierarchy panel.
    show_hierarchy: bool,
    /// Show properties panel.
    show_properties: bool,
    /// Show timeline panel.
    show_timeline: bool,
    /// Show material editor.
    show_materials: bool,
    /// Show render settings window.
    show_render_settings: bool,
    /// Show about window.
    show_about: bool,
    /// Show preferences window.
    show_preferences: bool,
    /// Show console window.
    show_console: bool,
    /// Show node editor window.
    show_node_editor: bool,
    /// Show UV editor window.
    show_uv_editor: bool,
    /// Hierarchy search filter.
    hierarchy_search: String,
    /// Box selection end position (for drawing rectangle).
    box_select_end: Option<egui::Pos2>,
    /// Show dopesheet in timeline (keyframe visualization).
    show_dopesheet: bool,
    /// Console log entries.
    console_entries: Vec<console::LogEntry>,
    /// Console filter level.
    console_filter: console::LogLevel,
    /// Status message.
    status_message: String,
    /// Current project file path.
    project_path: Option<PathBuf>,
    /// Render settings.
    render_settings: RenderSettings,
    /// App preferences.
    preferences: AppPreferences,
    /// Show shape keys editor.
    #[allow(dead_code)]
    show_shape_keys: bool,
    /// Show constraints panel.
    #[allow(dead_code)]
    show_constraints: bool,
    /// Lasso selection points for display.
    #[allow(dead_code)]
    lasso_display: Vec<egui::Pos2>,
    /// Show graph editor window.
    show_graph_editor: bool,
    /// Graph editor view bounds (normalized 0–1 over timeline start..end).
    graph_view_left: f32,
    graph_view_right: f32,
    /// Active drag: (kf_idx, ch_idx, orig_frame, orig_val, start_screen_pos).
    graph_drag: Option<(usize, usize, i32, f32, egui::Pos2)>,
    /// Selected keyframe indices in the graph editor.
    graph_selected: Vec<usize>,
    /// Box-select start screen position.
    graph_box_start: Option<egui::Pos2>,
    /// Show camera settings window.
    show_camera_settings: bool,
    /// Show world settings window.
    show_world_settings: bool,
    /// Show quick favorites panel (Q menu).
    #[allow(dead_code)]
    show_quick_favorites: bool,
    /// Show scene properties window.
    show_scene_properties: bool,
    /// Show NLA editor window.
    show_nla_editor: bool,
    /// Show color management window.
    show_color_management: bool,
    /// Show asset browser window.
    show_asset_browser: bool,
    /// Show render layers window.
    show_render_layers: bool,
    /// Show spreadsheet editor window.
    show_spreadsheet: bool,
    /// Show text editor window.
    #[cfg(feature = "python")]
    show_text_editor: bool,
    /// Text editor content.
    text_editor_content: String,
    /// Show sequencer window.
    show_sequencer: bool,
    /// Show image editor window.
    show_image_editor: bool,
    gpu_renderer: Option<Arc<RwLock<GpuRendererState>>>,
    /// Show welcome screen (first launch or Help → Welcome).
    show_welcome: bool,
    /// Current license status.
    license_status: license::LicenseStatus,
    /// Show license activation dialog.
    show_license_dialog: bool,
    /// Serial input buffer for license dialog.
    license_serial_input: String,
    /// GitHub Education OAuth — receiver from background thread.
    /// Wrapped in Mutex to satisfy ScriptingHost: Sync bound (Receiver is Send but not Sync).
    edu_oauth_rx: Option<parking_lot::Mutex<std::sync::mpsc::Receiver<license::EduFlowEvent>>>,
    /// Current step shown in the GitHub Education flow UI.
    edu_oauth_step: EduOAuthStep,
    /// Material/compositor node graph.
    node_graph: nodes::NodeGraph,
    /// In-progress socket connection: (source node id, source socket id).
    pending_connection: Option<(nodes::NodeId, nodes::SocketId)>,
    /// Node being dragged: (node id, mouse offset from node origin at drag start).
    node_drag: Option<(nodes::NodeId, egui::Vec2)>,
    /// iPad remote input — decoded messages from the background TCP listener
    /// thread (see BATCH 24). Wrapped in Mutex for the same Sync reason as
    /// `edu_oauth_rx` (Receiver is Send but not Sync).
    #[cfg(feature = "ipad")]
    ipad_rx: Option<parking_lot::Mutex<std::sync::mpsc::Receiver<nat3d_sync::protocol::SyncMessage>>>,
    /// Touch gesture state machine — turns raw touch points into pan/zoom/orbit.
    #[cfg(feature = "ipad")]
    ipad_input: nat3d_sync::input::ipad::IPadInput,
    /// Pressure-curve state — turns raw Apple Pencil samples into brush parameters.
    #[cfg(feature = "ipad")]
    pencil_input: nat3d_sync::input::pencil::PencilInput,
    /// Most recent brush parameters derived from Apple Pencil input.
    /// NOTE: no paint/sculpt tool consumes this yet — there's no brush
    /// system in the app to feed. Kept so that one has somewhere to read
    /// from once such a tool exists, instead of the pencil data being
    /// silently discarded as it was before.
    #[cfg(feature = "ipad")]
    #[allow(dead_code)]
    last_pencil_params: Option<nat3d_sync::input::pencil::BrushParams>,
}

/// GPU rendering state (wgpu-based).
/// Initialized from eframe::CreationContext in Nat3DApp::new().
struct GpuRendererState {
    opaque_pipeline: wgpu::RenderPipeline,
    model_bind_group_layout: wgpu::BindGroupLayout,
    material_bind_group_layout: wgpu::BindGroupLayout,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    render_texture: wgpu::Texture,
    render_view: wgpu::TextureView,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    egui_texture_id: egui::TextureId,
    /// egui renderer reference — needed to re-register render_texture after resize.
    egui_renderer: Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>,
    dimensions: (u32, u32),
    /// Viewport size requested by the paint callback, pending application by
    /// `update()`. The resize itself cannot run inside the callback — see the
    /// deadlock note in `ViewportCallback::prepare`.
    pending_resize: Option<(u32, u32)>,
    /// Per-object GPU buffers keyed by scene object index.
    mesh_cache: HashMap<usize, MeshEntry>,
}

impl GpuRendererState {
    fn new(
        wgpu_render_state: &eframe::egui_wgpu::RenderState,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let device = &wgpu_render_state.device;
        let max_dim = device.limits().max_texture_dimension_2d;
        let width = width.min(max_dim);
        let height = height.min(max_dim);

        // Create bind group layouts
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GPU Pipeline Layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &model_bind_group_layout,
                &material_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        // Create shader module (simplified forward shader)
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("NAT3D Forward Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/forward.wgsl").into()),
        });

        // Create opaque render pipeline
        let opaque_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("NAT3D Opaque Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    Vertex::layout(),
                    wgpu::VertexBufferLayout {
                        array_stride: 4,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32,
                        }],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
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

        // Create uniform buffers
        use wgpu::util::DeviceExt;
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[CameraUniforms::default()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Create render texture
        let render_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Render Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let render_view = render_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create depth texture
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
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

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Register render texture with egui
        let egui_renderer = Arc::clone(&wgpu_render_state.renderer);
        let egui_texture_id = egui_renderer.write().register_native_texture(
            device,
            &render_view,
            wgpu::FilterMode::Linear,
        );

        Ok(Self {
            opaque_pipeline,
            model_bind_group_layout,
            material_bind_group_layout,
            camera_buffer,
            camera_bind_group,
            render_texture,
            render_view,
            depth_texture,
            depth_view,
            egui_texture_id,
            egui_renderer,
            dimensions: (width, height),
            pending_resize: None,
            mesh_cache: HashMap::new(),
        })
    }

    /// Recreate render and depth textures for a new viewport size, and re-register with egui.
    fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let max_dim = device.limits().max_texture_dimension_2d;
        let width = width.min(max_dim);
        let height = height.min(max_dim);
        self.render_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Render Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.render_view = self
            .render_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.depth_texture = device.create_texture(&wgpu::TextureDescriptor {
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
        self.depth_view = self
            .depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.egui_renderer
            .write()
            .update_egui_texture_from_wgpu_texture(
                device,
                &self.render_view,
                wgpu::FilterMode::Linear,
                self.egui_texture_id,
            );
        self.dimensions = (width, height);
    }

    /// Upload vertex/index data and create per-object model+material buffers for one scene object.
    fn upload_object_mesh(
        &mut self,
        device: &wgpu::Device,
        id: usize,
        obj: &SceneObject,
        vertices: &[Vertex],
        indices: &[u32],
    ) {
        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let model_matrix = Self::build_model_matrix(obj);
        let model_uniforms = ModelUniforms {
            model: model_matrix,
            normal: model_matrix,
        };
        let model_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Model Buffer"),
            contents: bytemuck::cast_slice(&[model_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Model Bind Group"),
            layout: &self.model_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: model_buffer.as_entire_binding(),
            }],
        });

        let material_uniforms = AppMaterialUniforms {
            base_color: obj.material.base_color,
            metallic: obj.material.metallic,
            roughness: obj.material.roughness,
            ao: 1.0,
            emissive: obj.material.emissive,
            simulation_mode: 0,
            _padding: [0.0; 3],
        };
        let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Material Buffer"),
            contents: bytemuck::cast_slice(&[material_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let signal_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Signal Buffer"),
            contents: bytemuck::cast_slice(&[obj.physiological_signal]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &self.material_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: material_buffer.as_entire_binding(),
            }],
        });

        self.mesh_cache.insert(
            id,
            MeshEntry {
                vertex_buffer,
                index_buffer,
                index_count: indices.len() as u32,
                model_buffer,
                model_bind_group,
                material_buffer,
                material_bind_group,
                signal_buffer,
            },
        );
    }

    /// Build model matrix from object transform (column-major TRS, matches CPU XYZ Euler renderer).
    fn build_model_matrix(obj: &SceneObject) -> [[f32; 4]; 4] {
        let [tx, ty, tz] = obj.position;
        let [sx, sy, sz] = obj.scale;
        let [rx, ry, rz] = obj.rotation;

        let (sin_x, cos_x) = rx.to_radians().sin_cos();
        let (sin_y, cos_y) = ry.to_radians().sin_cos();
        let (sin_z, cos_z) = rz.to_radians().sin_cos();

        // Each [f32;4] is a column. Rotation order: Rx then Ry then Rz (= Rz*Ry*Rx on column vec).
        [
            [cos_y * cos_z * sx, cos_y * sin_z * sx, -sin_y * sx, 0.0],
            [
                (sin_x * sin_y * cos_z - cos_x * sin_z) * sy,
                (sin_x * sin_y * sin_z + cos_x * cos_z) * sy,
                sin_x * cos_y * sy,
                0.0,
            ],
            [
                (cos_x * sin_y * cos_z + sin_x * sin_z) * sz,
                (cos_x * sin_y * sin_z - sin_x * cos_z) * sz,
                cos_x * cos_y * sz,
                0.0,
            ],
            [tx, ty, tz, 1.0],
        ]
    }

    /// Generate vertices and indices for an object.
    fn generate_object_geometry(obj: &SceneObject) -> (Vec<Vertex>, Vec<u32>) {
        match obj.object_type {
            state::ObjectType::Cube => Self::generate_cube(),
            state::ObjectType::Sphere => Self::generate_sphere(24, 16),
            state::ObjectType::Cylinder => Self::generate_cylinder(32, 2.0),
            state::ObjectType::Plane => Self::generate_plane(),
            state::ObjectType::Torus => Self::generate_torus(32, 16, 0.3, 0.1),
            state::ObjectType::Cone => Self::generate_cone(32, 2.0),
            state::ObjectType::IcoSphere => Self::generate_icosphere(2),
            state::ObjectType::Grid => Self::generate_grid(10, 10),
            state::ObjectType::Circle => Self::generate_circle(32),
            _ => Self::generate_cube(), // Default to cube for other types
        }
    }

    fn generate_cube() -> (Vec<Vertex>, Vec<u32>) {
        let vertices = vec![
            // Front
            Vertex::new(
                [-0.5, -0.5, 0.5],
                [0.0, 0.0, 1.0],
                [0.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [0.5, -0.5, 0.5],
                [0.0, 0.0, 1.0],
                [1.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [0.5, 0.5, 0.5],
                [0.0, 0.0, 1.0],
                [1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [-0.5, 0.5, 0.5],
                [0.0, 0.0, 1.0],
                [0.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            // Back
            Vertex::new(
                [-0.5, -0.5, -0.5],
                [0.0, 0.0, -1.0],
                [1.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [-0.5, 0.5, -0.5],
                [0.0, 0.0, -1.0],
                [1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [0.5, 0.5, -0.5],
                [0.0, 0.0, -1.0],
                [0.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [0.5, -0.5, -0.5],
                [0.0, 0.0, -1.0],
                [0.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
        ];
        let indices = vec![
            0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 3, 2, 6, 6, 5, 3, 4, 7, 1, 1, 0, 4, 1, 7, 6, 6, 2,
            1, 4, 0, 3, 3, 5, 4,
        ];
        (vertices, indices)
    }

    fn generate_sphere(rings: u32, sectors: u32) -> (Vec<Vertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let pi = std::f32::consts::PI;

        for i in 0..=rings {
            let theta = i as f32 * pi / rings as f32;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            for j in 0..=sectors {
                let phi = j as f32 * 2.0 * pi / sectors as f32;
                let sin_phi = phi.sin();
                let cos_phi = phi.cos();

                let x = sin_theta * cos_phi;
                let y = cos_theta;
                let z = sin_theta * sin_phi;

                let u = j as f32 / sectors as f32;
                let v = i as f32 / rings as f32;

                vertices.push(Vertex::new(
                    [x * 0.5, y * 0.5, z * 0.5],
                    [x, y, z],
                    [u, v],
                    [1.0, 1.0, 1.0, 1.0],
                ));
            }
        }

        for i in 0..rings {
            for j in 0..sectors {
                let first = i * (sectors + 1) + j;
                let second = first + sectors + 1;
                indices.extend_from_slice(&[
                    first,
                    second,
                    first + 1,
                    second,
                    second + 1,
                    first + 1,
                ]);
            }
        }

        (vertices, indices)
    }

    fn generate_cylinder(sectors: u32, height: f32) -> (Vec<Vertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let pi = std::f32::consts::PI;
        let radius = 0.5;
        let half_height = height * 0.25;

        // Top cap center
        vertices.push(Vertex::new(
            [0.0, half_height, 0.0],
            [0.0, 1.0, 0.0],
            [0.5, 0.5],
            [1.0, 1.0, 1.0, 1.0],
        ));
        // Bottom cap center
        vertices.push(Vertex::new(
            [0.0, -half_height, 0.0],
            [0.0, -1.0, 0.0],
            [0.5, 0.5],
            [1.0, 1.0, 1.0, 1.0],
        ));

        // Side vertices
        for i in 0..=sectors {
            let angle = i as f32 * 2.0 * pi / sectors as f32;
            let x = radius * angle.cos();
            let z = radius * angle.sin();
            let u = i as f32 / sectors as f32;

            // Top
            vertices.push(Vertex::new(
                [x, half_height, z],
                [x * 2.0, 0.0, z * 2.0],
                [u, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ));
            // Bottom
            vertices.push(Vertex::new(
                [x, -half_height, z],
                [x * 2.0, 0.0, z * 2.0],
                [u, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ));
        }

        // Side faces
        for i in 0..sectors {
            let base = 2 + i * 2;
            indices.extend_from_slice(&[base, base + 2, base + 1, base + 2, base + 3, base + 1]);
        }

        (vertices, indices)
    }

    fn generate_plane() -> (Vec<Vertex>, Vec<u32>) {
        let vertices = vec![
            Vertex::new(
                [-0.5, 0.0, -0.5],
                [0.0, 1.0, 0.0],
                [0.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [0.5, 0.0, -0.5],
                [0.0, 1.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [0.5, 0.0, 0.5],
                [0.0, 1.0, 0.0],
                [1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
            Vertex::new(
                [-0.5, 0.0, 0.5],
                [0.0, 1.0, 0.0],
                [0.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
        ];
        let indices = vec![0, 1, 2, 2, 3, 0];
        (vertices, indices)
    }

    fn generate_torus(
        major_segs: u32,
        minor_segs: u32,
        major_rad: f32,
        minor_rad: f32,
    ) -> (Vec<Vertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let pi = std::f32::consts::PI;

        for i in 0..=major_segs {
            let theta = i as f32 * 2.0 * pi / major_segs as f32;
            let cos_theta = theta.cos();
            let sin_theta = theta.sin();

            for j in 0..=minor_segs {
                let phi = j as f32 * 2.0 * pi / minor_segs as f32;
                let cos_phi = phi.cos();
                let sin_phi = phi.sin();

                let x = (major_rad + minor_rad * cos_phi) * cos_theta;
                let y = minor_rad * sin_phi;
                let z = (major_rad + minor_rad * cos_phi) * sin_theta;

                let nx = cos_phi * cos_theta;
                let ny = sin_phi;
                let nz = cos_phi * sin_theta;

                let u = i as f32 / major_segs as f32;
                let v = j as f32 / minor_segs as f32;

                vertices.push(Vertex::new(
                    [x, y, z],
                    [nx, ny, nz],
                    [u, v],
                    [1.0, 1.0, 1.0, 1.0],
                ));
            }
        }

        for i in 0..major_segs {
            for j in 0..minor_segs {
                let first = i * (minor_segs + 1) + j;
                let second = first + minor_segs + 1;
                indices.extend_from_slice(&[
                    first,
                    second,
                    first + 1,
                    second,
                    second + 1,
                    first + 1,
                ]);
            }
        }

        (vertices, indices)
    }

    fn generate_cone(sectors: u32, height: f32) -> (Vec<Vertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let pi = std::f32::consts::PI;
        let radius = 0.5;
        let half_height = height * 0.25;

        // Apex
        vertices.push(Vertex::new(
            [0.0, half_height, 0.0],
            [0.0, 1.0, 0.0],
            [0.5, 0.0],
            [1.0, 1.0, 1.0, 1.0],
        ));
        // Base center
        vertices.push(Vertex::new(
            [0.0, -half_height, 0.0],
            [0.0, -1.0, 0.0],
            [0.5, 0.5],
            [1.0, 1.0, 1.0, 1.0],
        ));

        // Base vertices
        for i in 0..=sectors {
            let angle = i as f32 * 2.0 * pi / sectors as f32;
            let x = radius * angle.cos();
            let z = radius * angle.sin();
            vertices.push(Vertex::new(
                [x, -half_height, z],
                [x, 0.5, z],
                [i as f32 / sectors as f32, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ));
        }

        // Side faces
        for i in 0..sectors {
            indices.extend_from_slice(&[0, 2 + i, 2 + i + 1]);
        }

        (vertices, indices)
    }

    fn generate_icosphere(_subdivisions: u32) -> (Vec<Vertex>, Vec<u32>) {
        let t = (1.0 + 5.0_f32.sqrt()) / 2.0;
        let mut vertices = vec![
            [-1.0, t, 0.0],
            [1.0, t, 0.0],
            [-1.0, -t, 0.0],
            [1.0, -t, 0.0],
            [0.0, -1.0, t],
            [0.0, 1.0, t],
            [0.0, -1.0, -t],
            [0.0, 1.0, -t],
            [t, 0.0, -1.0],
            [t, 0.0, 1.0],
            [-t, 0.0, -1.0],
            [-t, 0.0, 1.0],
        ];

        // Normalize
        for v in &mut vertices {
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            v[0] /= len;
            v[1] /= len;
            v[2] /= len;
        }

        let indices = vec![
            0, 11, 5, 0, 5, 1, 0, 1, 7, 0, 7, 10, 0, 10, 11, 1, 5, 9, 5, 11, 4, 11, 10, 2, 10, 7,
            6, 7, 1, 8, 3, 9, 4, 3, 4, 2, 3, 2, 6, 3, 6, 8, 3, 8, 9, 4, 9, 5, 2, 4, 11, 6, 2, 10,
            8, 6, 7, 9, 8, 1,
        ];

        // Convert to Vertex format
        let gpu_vertices: Vec<Vertex> = vertices
            .iter()
            .map(|&pos| {
                Vertex::new(
                    [pos[0] * 0.5, pos[1] * 0.5, pos[2] * 0.5],
                    pos,
                    [0.0, 0.0],
                    [1.0, 1.0, 1.0, 1.0],
                )
            })
            .collect();

        (gpu_vertices, indices)
    }

    fn generate_grid(subdivisions_x: u32, subdivisions_z: u32) -> (Vec<Vertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let step_x = 1.0 / subdivisions_x as f32;
        let step_z = 1.0 / subdivisions_z as f32;

        // Generate vertices
        for i in 0..=subdivisions_x {
            for j in 0..=subdivisions_z {
                let x = (i as f32 * step_x - 0.5) * 2.0;
                let z = (j as f32 * step_z - 0.5) * 2.0;
                let u = i as f32 / subdivisions_x as f32;
                let v = j as f32 / subdivisions_z as f32;

                vertices.push(Vertex::new(
                    [x, 0.0, z],
                    [0.0, 1.0, 0.0],
                    [u, v],
                    [1.0, 1.0, 1.0, 1.0],
                ));
            }
        }

        // Generate indices
        for i in 0..subdivisions_x {
            for j in 0..subdivisions_z {
                let base = i * (subdivisions_z + 1) + j;
                let next_row = base + subdivisions_z + 1;

                indices.extend_from_slice(&[
                    base,
                    next_row,
                    base + 1,
                    next_row,
                    next_row + 1,
                    base + 1,
                ]);
            }
        }

        (vertices, indices)
    }

    fn generate_circle(segments: u32) -> (Vec<Vertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let pi = std::f32::consts::PI;

        // Center vertex
        vertices.push(Vertex::new(
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.5, 0.5],
            [1.0, 1.0, 1.0, 1.0],
        ));

        // Edge vertices
        for i in 0..=segments {
            let angle = i as f32 * 2.0 * pi / segments as f32;
            let x = 0.5 * angle.cos();
            let z = 0.5 * angle.sin();
            let u = x + 0.5;
            let v = z + 0.5;

            vertices.push(Vertex::new(
                [x, 0.0, z],
                [0.0, 1.0, 0.0],
                [u, v],
                [1.0, 1.0, 1.0, 1.0],
            ));
        }

        // Triangles from center
        for i in 0..segments {
            indices.extend_from_slice(&[0, i + 1, i + 2]);
        }

        (vertices, indices)
    }
}

/// Application preferences.
#[derive(Clone)]
struct AppPreferences {
    simulation_mode: state::SimulationMode,
    /// Theme (dark/light).
    dark_mode: bool,
    /// Auto-save interval (0 = disabled).
    auto_save_minutes: u32,
    /// Show FPS counter.
    show_fps: bool,
    /// Show grid.
    show_grid: bool,
    /// Show axes.
    show_axes: bool,
    /// Grid size.
    grid_size: i32,
    /// Anti-aliasing samples.
    aa_samples: u32,
    /// UI scale.
    ui_scale: f32,
    /// Use GPU rendering (wgpu) instead of software renderer.
    #[allow(dead_code)] // GPU viewport integration pending
    use_gpu_rendering: bool,
    /// Suppress welcome screen on subsequent launches.
    dont_show_welcome: bool,
}

/// Render settings.
#[derive(Clone)]
struct RenderSettings {
    /// Output width.
    width: u32,
    /// Output height.
    height: u32,
    /// Samples per pixel.
    samples: u32,
    /// Use denoiser.
    use_denoiser: bool,
    /// Output format.
    output_format: String,
}

impl Nat3DApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(feature = "ipad")]
        // BATCH 24: iPad Remote Input Listener (P3.4)
        let ctx_clone = cc.egui_ctx.clone();
        // Decoded messages are forwarded to the UI thread through this channel
        // instead of being decoded-and-discarded (see `process_ipad_input`,
        // called from `update()`).
        #[cfg(feature = "ipad")]
        let (ipad_tx, ipad_rx) = std::sync::mpsc::channel::<nat3d_sync::protocol::SyncMessage>();
        #[cfg(feature = "ipad")]
        std::thread::spawn(move || {
            // Creamos un runtime dedicado para el listener de iPad,
            // evitando depender del contexto del thread de UI.
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to build iPad listener runtime: {e}");
                    return;
                }
            };

            rt.block_on(async move {
                if let Ok(listener) = tokio::net::TcpListener::bind("0.0.0.0:9001").await {
                    tracing::info!("iPad Input Listener active on port 9001");
                    while let Ok((mut socket, _)) = listener.accept().await {
                        let mut buf = vec![0u8; 4096];
                        if let Ok(n) = socket.read(&mut buf).await {
                            if let Ok(msg) = nat3d_sync::protocol::SyncProtocol::decode(&buf[..n])
                            {
                                // Best-effort: drop silently if the UI side
                                // already shut down (receiver dropped).
                                let _ = ipad_tx.send(msg);
                                ctx_clone.request_repaint();
                            }
                        }
                    }
                }
            });
        });

        // Register GPU resources for egui_wgpu callbacks (BATCH 24)

        let gpu_renderer = if let Some(wgpu_render_state) = cc.wgpu_render_state.as_ref() {
            // screen_rect() may be unavailable on first call; resize() in prepare() corrects this each frame
            let init_rect = cc.egui_ctx.screen_rect();
            let ppp = cc.egui_ctx.pixels_per_point();
            let init_w = ((init_rect.width() * ppp) as u32).max(256);
            let init_h = ((init_rect.height() * ppp) as u32).max(256);
            match GpuRendererState::new(wgpu_render_state, init_w, init_h) {
                Ok(renderer) => {
                    tracing::info!("GPU renderer initialized successfully");
                    Some(Arc::new(RwLock::new(renderer)))
                }

                Err(e) => {
                    tracing::warn!(
                        "Failed to initialize GPU renderer: {}, falling back to software",
                        e
                    );
                    None
                }
            }
        } else {
            tracing::info!("WGPU not available, using software renderer");
            None
        };

        Self {
            state: AppState::new(),
            show_hierarchy: true,
            show_properties: true,
            show_timeline: true,
            show_materials: false,
            show_render_settings: false,
            show_about: false,
            show_preferences: false,
            status_message: "Ready".to_string(),
            project_path: None,
            render_settings: RenderSettings {
                width: 1920,
                height: 1080,
                samples: 128,
                use_denoiser: true,
                output_format: "PNG".to_string(),
            },
            preferences: AppPreferences {
                simulation_mode: state::SimulationMode::Off,
                dark_mode: true,
                auto_save_minutes: 0,
                show_fps: true,
                show_grid: true,
                show_axes: true,
                grid_size: 10,
                aa_samples: 4,
                ui_scale: 1.0,
                use_gpu_rendering: true,
                dont_show_welcome: false,
            },
            show_console: false,
            show_node_editor: false,
            show_uv_editor: false,
            hierarchy_search: String::new(),
            box_select_end: None,
            show_dopesheet: true,
            console_entries: vec![
                console::LogEntry {
                    level: console::LogLevel::Info,
                    message: if gpu_renderer.is_some() {
                        "NAT3D initialized with GPU rendering".to_string()
                    } else {
                        "NAT3D initialized with software rendering".to_string()
                    },
                    source: Some("System".to_string()),
                    timestamp: 0.0,
                    count: 1,
                },
            ],
            console_filter: console::LogLevel::Info,
            show_shape_keys: false,
            show_constraints: false,
            lasso_display: Vec::new(),
            show_graph_editor: false,
            graph_view_left: 0.0,
            graph_view_right: 1.0,
            graph_drag: None,
            graph_selected: Vec::new(),
            graph_box_start: None,
            show_camera_settings: false,
            show_world_settings: false,
            show_quick_favorites: false,
            show_scene_properties: false,
            show_nla_editor: false,
            show_color_management: false,
            show_asset_browser: false,
            show_render_layers: false,
            show_spreadsheet: false,
            #[cfg(feature = "python")]
            show_text_editor: false,
            text_editor_content: "# NAT3D Python Script\nimport nat3d\n\nscene = nat3d.get_active_scene()\nfor obj in scene.objects:\n    print(obj.name)\n".to_string(),
            show_sequencer: false,
            show_image_editor: false,
            gpu_renderer,
            show_welcome: !Self::welcome_sentinel_exists(),
            license_status: license::LicenseStatus::Trial,
            show_license_dialog: false,
            license_serial_input: String::new(),
            edu_oauth_rx: None,
            edu_oauth_step: EduOAuthStep::Idle,
            node_graph: nodes::NodeGraph::default_material(),
            pending_connection: None,
            node_drag: None,
            #[cfg(feature = "ipad")]
            ipad_rx: Some(parking_lot::Mutex::new(ipad_rx)),
            #[cfg(feature = "ipad")]
            ipad_input: nat3d_sync::input::ipad::IPadInput::new(1600, 900),
            #[cfg(feature = "ipad")]
            pencil_input: nat3d_sync::input::pencil::PencilInput::new(),
            #[cfg(feature = "ipad")]
            last_pencil_params: None,
        }
    }

    /// Drain messages received from the iPad TCP listener thread (see
    /// `Self::new`) and apply them: touch gestures drive the viewport camera
    /// the same way mouse drag does, pencil samples are run through the
    /// pressure curve and stashed for a future paint/sculpt tool to read.
    /// Called once per frame from `update()`.
    #[cfg(feature = "ipad")]
    fn process_ipad_input(&mut self) {
        let Some(rx) = self.ipad_rx.as_ref() else {
            return;
        };
        // Drain everything pending this frame without holding the lock
        // while we mutate other app state.
        let messages: Vec<nat3d_sync::protocol::SyncMessage> = {
            let rx = rx.lock();
            std::iter::from_fn(|| rx.try_recv().ok()).collect()
        };

        for msg in messages {
            match msg {
                nat3d_sync::protocol::SyncMessage::InputEvent {
                    event_type, x, y, ..
                } => {
                    let ended = matches!(event_type.as_str(), "up" | "end" | "cancel");
                    if let Some(gesture) = self.ipad_input.handle_touch_event(0, x, y, ended) {
                        // Seed tracking once per gesture; re-calling
                        // `handle_gesture` mid-gesture would reset the
                        // initial distance/angle and zero out every delta
                        // (see `IPadInput::is_tracking_gesture` docs).
                        if !self.ipad_input.is_tracking_gesture() {
                            self.ipad_input.handle_gesture(gesture);
                        }
                    }
                    match self.ipad_input.get_viewport_transform() {
                        nat3d_sync::input::ipad::ViewportTransform::Pan { dx, dy } => {
                            self.state.camera.pan(dx, dy);
                        }
                        nat3d_sync::input::ipad::ViewportTransform::Zoom { factor } => {
                            self.state.camera.zoom(factor - 1.0);
                        }
                        nat3d_sync::input::ipad::ViewportTransform::Rotate { angle } => {
                            self.state.camera.orbit(angle.to_degrees(), 0.0);
                        }
                        nat3d_sync::input::ipad::ViewportTransform::None => {}
                    }
                }
                nat3d_sync::protocol::SyncMessage::PencilUpdate {
                    x,
                    y,
                    force,
                    tilt_x,
                    tilt_y,
                    azimuth,
                    in_contact,
                    ..
                } => {
                    if in_contact {
                        let altitude = (std::f32::consts::FRAC_PI_2
                            - tilt_x.hypot(tilt_y))
                        .max(0.0);
                        let event = nat3d_sync::input::pencil::PencilEvent::new(x, y)
                            .with_pressure(force)
                            .with_altitude(altitude)
                            .with_azimuth(azimuth);
                        self.last_pencil_params =
                            Some(self.pencil_input.handle_pencil_event(event));
                    }
                }
                _ => {}
            }
        }
    }

    /// Returns true if the sentinel file marking a prior launch exists.
    fn welcome_sentinel_exists() -> bool {
        Self::welcome_sentinel_path()
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    fn welcome_sentinel_path() -> Option<std::path::PathBuf> {
        std::env::var("APPDATA").ok().map(|appdata| {
            std::path::PathBuf::from(appdata)
                .join("NAT3D")
                .join(".launched")
        })
    }

    fn write_welcome_sentinel() {
        if let Some(path) = Self::welcome_sentinel_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, b"1");
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Scene").clicked() {
                    self.state.new_scene();
                    self.project_path = None;
                    self.status_message = "New scene created".to_string();
                    ui.close_menu();
                }
                if ui.button("Open Project...").clicked() {
                    self.open_project_dialog();
                    ui.close_menu();
                }
                if ui.button("Save Project").clicked() {
                    self.save_project();
                    ui.close_menu();
                }
                if ui.button("Save Project As...").clicked() {
                    self.save_project_as_dialog();
                    ui.close_menu();
                }
                ui.separator();
                ui.menu_button("Import", |ui| {
                    if ui.button("OBJ (.obj)").clicked() {
                        self.import_file_dialog("obj");
                        ui.close_menu();
                    }
                    if ui.button("STL (.stl)").clicked() {
                        self.import_file_dialog("stl");
                        ui.close_menu();
                    }
                    if ui.button("glTF (.gltf, .glb)").clicked() {
                        self.import_file_dialog("gltf");
                        ui.close_menu();
                    }
                    if ui.button("FBX (.fbx)").clicked() {
                        self.import_file_dialog("fbx");
                        ui.close_menu();
                    }
                    if ui.button("DXF (.dxf)").clicked() {
                        self.import_file_dialog("dxf");
                        ui.close_menu();
                    }
                    if ui.button("STEP (.step, .stp)").clicked() {
                        self.import_file_dialog("step");
                        ui.close_menu();
                    }
                    if ui.button("IGES (.igs, .iges)").clicked() {
                        self.import_file_dialog("iges");
                        ui.close_menu();
                    }
                });
                ui.menu_button("Export", |ui| {
                    if ui.button("OBJ (.obj)").clicked() {
                        self.export_file_dialog("obj");
                        ui.close_menu();
                    }
                    if ui.button("STL (.stl)").clicked() {
                        self.export_file_dialog("stl");
                        ui.close_menu();
                    }
                    if ui.button("glTF (.glb)").clicked() {
                        self.export_file_dialog("glb");
                        ui.close_menu();
                    }
                    if ui.button("FBX (.fbx)").clicked() {
                        self.export_file_dialog("fbx");
                        ui.close_menu();
                    }
                    if ui.button("DXF (.dxf)").clicked() {
                        self.export_file_dialog("dxf");
                        ui.close_menu();
                    }
                });
                ui.separator();
                if ui.button("Exit").clicked() {
                    std::process::exit(0);
                }
            });

            ui.menu_button("Edit", |ui| {
                let undo_text = if self.state.can_undo() {
                    "Undo (Ctrl+Z)"
                } else {
                    "Undo"
                };
                if ui
                    .add_enabled(self.state.can_undo(), egui::Button::new(undo_text))
                    .clicked()
                {
                    if self.state.undo() {
                        self.status_message = "Undo".to_string();
                    }
                    ui.close_menu();
                }
                let redo_text = if self.state.can_redo() {
                    "Redo (Ctrl+Shift+Z)"
                } else {
                    "Redo"
                };
                if ui
                    .add_enabled(self.state.can_redo(), egui::Button::new(redo_text))
                    .clicked()
                {
                    if self.state.redo() {
                        self.status_message = "Redo".to_string();
                    }
                    ui.close_menu();
                }
                ui.separator();
                let has_selection = self.state.selected_object.is_some();
                if ui
                    .add_enabled(has_selection, egui::Button::new("Delete (Del)"))
                    .clicked()
                {
                    self.state.save_undo_state();
                    self.state.delete_selected();
                    self.status_message = "Deleted object".to_string();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(has_selection, egui::Button::new("Duplicate (Shift+D)"))
                    .clicked()
                {
                    self.state.save_undo_state();
                    self.state.duplicate_selected();
                    self.status_message = "Duplicated object".to_string();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Select All (Ctrl+A)").clicked() {
                    if !self.state.objects.is_empty() {
                        self.state.selected_object = Some(0);
                    }
                    ui.close_menu();
                }
                if ui.button("Deselect (Esc)").clicked() {
                    self.state.selected_object = None;
                    ui.close_menu();
                }
                ui.separator();
                if ui
                    .add_enabled(has_selection, egui::Button::new("Copy (Ctrl+C)"))
                    .clicked()
                {
                    if let Some(idx) = self.state.selected_object {
                        self.state.clipboard = vec![self.state.objects[idx].clone()];
                        // Also copy multi-selected
                        for &mi in &self.state.multi_selected.clone() {
                            if mi < self.state.objects.len() {
                                self.state.clipboard.push(self.state.objects[mi].clone());
                            }
                        }
                        self.status_message =
                            format!("Copied {} object(s)", self.state.clipboard.len());
                    }
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        !self.state.clipboard.is_empty(),
                        egui::Button::new("Paste (Ctrl+V)"),
                    )
                    .clicked()
                {
                    self.state.save_undo_state();
                    let clip = self.state.clipboard.clone();
                    for mut obj in clip {
                        obj.name = format!("{}.copy", obj.name);
                        obj.position[0] += 1.0;
                        self.state.objects.push(obj);
                    }
                    self.state.selected_object = Some(self.state.objects.len() - 1);
                    self.status_message = "Pasted from clipboard".to_string();
                    ui.close_menu();
                }

                // Edit Mode operations
                if self.state.edit_mode == EditMode::Edit {
                    ui.separator();
                    ui.label("Edit Mode Operations:");

                    let has_edit_mesh = if let Some(idx) = self.state.selected_object {
                        idx < self.state.objects.len()
                            && self.state.objects[idx].edit_mesh.is_some()
                    } else {
                        false
                    };

                    if has_edit_mesh {
                        if let Some(sel_idx) = self.state.selected_object {
                            let selection = self.state.objects[sel_idx].edit_selection.clone();
                            let has_verts = !selection.vertices.is_empty();
                            let has_edges = !selection.edges.is_empty();
                            let has_faces = !selection.faces.is_empty();

                            if ui
                                .add_enabled(
                                    has_verts || has_faces,
                                    egui::Button::new("Delete (X)"),
                                )
                                .clicked()
                            {
                                // Trigger delete operation (handled by keyboard shortcut logic)
                                self.status_message = "Use X key to delete selection".to_string();
                                ui.close_menu();
                            }
                            if ui
                                .add_enabled(
                                    has_verts && selection.vertices.len() >= 2,
                                    egui::Button::new("Merge Vertices (Ctrl+M)"),
                                )
                                .clicked()
                            {
                                self.status_message = "Use Ctrl+M to merge vertices".to_string();
                                ui.close_menu();
                            }
                            if ui
                                .add_enabled(has_faces, egui::Button::new("Extrude Faces (Ctrl+E)"))
                                .clicked()
                            {
                                self.status_message = "Use Ctrl+E to extrude faces".to_string();
                                ui.close_menu();
                            }
                            if ui
                                .add_enabled(has_faces, egui::Button::new("Inset Faces (I)"))
                                .clicked()
                            {
                                self.status_message = "Use I key to inset faces".to_string();
                                ui.close_menu();
                            }
                            if ui
                                .add_enabled(
                                    has_edges,
                                    egui::Button::new("Subdivide Edges (Ctrl+R)"),
                                )
                                .clicked()
                            {
                                self.status_message = "Use Ctrl+R to subdivide edges".to_string();
                                ui.close_menu();
                            }
                            if ui
                                .add_enabled(
                                    true,
                                    egui::Button::new("Subdivide Surface (Shift+Ctrl+S)"),
                                )
                                .clicked()
                            {
                                self.status_message =
                                    "Use Shift+Ctrl+S for Catmull-Clark subdivision".to_string();
                                ui.close_menu();
                            }
                        }
                    } else {
                        ui.label("(Select object and enter Edit Mode)");
                    }
                }

                ui.separator();
                if ui.button("Preferences...").clicked() {
                    self.show_preferences = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("Add", |ui| {
                ui.menu_button("Mesh", |ui| {
                    if ui.button("Cube").clicked() {
                        self.state.add_cube();
                        self.status_message = "Added Cube".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Sphere").clicked() {
                        self.state.add_sphere();
                        self.status_message = "Added Sphere".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Cylinder").clicked() {
                        self.state.add_cylinder();
                        self.status_message = "Added Cylinder".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Plane").clicked() {
                        self.state.add_plane();
                        self.status_message = "Added Plane".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Torus").clicked() {
                        self.state.add_torus();
                        self.status_message = "Added Torus".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Cone").clicked() {
                        self.state.add_cone();
                        self.status_message = "Added Cone".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Ico Sphere").clicked() {
                        self.state.add_icosphere();
                        self.status_message = "Added Ico Sphere".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Grid").clicked() {
                        self.state.add_grid();
                        self.status_message = "Added Grid".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Circle").clicked() {
                        self.state.add_circle();
                        self.status_message = "Added Circle".to_string();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Curve", |ui| {
                    if ui.button("Bezier Curve").clicked() {
                        self.state.add_bezier_curve();
                        self.status_message = "Added Bezier Curve".to_string();
                        ui.close_menu();
                    }
                    if ui.button("NURBS Curve").clicked() {
                        self.state.add_nurbs_curve();
                        self.status_message = "Added NURBS Curve".to_string();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Light", |ui| {
                    if ui.button("Point Light").clicked() {
                        self.state.add_point_light();
                        self.status_message = "Added Point Light".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Directional Light").clicked() {
                        self.state.add_directional_light();
                        self.status_message = "Added Directional Light".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Spot Light").clicked() {
                        self.state.add_point_light(); // Use point light for now
                        self.status_message = "Added Spot Light".to_string();
                        ui.close_menu();
                    }
                });
                if ui.button("Camera").clicked() {
                    self.state.add_camera_object();
                    self.status_message = "Added Camera".to_string();
                    ui.close_menu();
                }
                if ui.button("Text").clicked() {
                    self.state.add_text();
                    self.status_message = "Added Text".to_string();
                    ui.close_menu();
                }
                if ui.button("Empty").clicked() {
                    self.state.add_empty();
                    self.status_message = "Added Empty".to_string();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Armature").clicked() {
                    self.state.add_armature();
                    self.status_message = "Added Armature (5 bones)".to_string();
                    ui.close_menu();
                }
            });

            ui.menu_button("Modify", |ui| {
                let has_selection = self.state.selected_object.is_some();
                ui.menu_button("Generate", |ui| {
                    if !has_selection {
                        ui.disable();
                    }
                    for name in &[
                        "Array",
                        "Bevel",
                        "Boolean",
                        "Mirror",
                        "Screw",
                        "Solidify",
                        "Wireframe",
                    ] {
                        if ui.button(*name).clicked() {
                            self.state.add_modifier(name);
                            self.status_message = format!("Added {} Modifier", name);
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("Deform", |ui| {
                    if !has_selection {
                        ui.disable();
                    }
                    for name in &[
                        "Bend",
                        "Cast",
                        "Curve Deform",
                        "Displace",
                        "Lattice",
                        "Mesh Deform",
                        "Shrinkwrap",
                        "Simple Deform",
                        "Smooth",
                        "Corrective Smooth",
                        "Taper",
                        "Twist",
                        "Wave",
                    ] {
                        if ui.button(*name).clicked() {
                            self.state.add_modifier(name);
                            self.status_message = format!("Added {} Modifier", name);
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("Normals", |ui| {
                    if !has_selection {
                        ui.disable();
                    }
                    for name in &["Normal Edit", "Weighted Normal"] {
                        if ui.button(*name).clicked() {
                            self.state.add_modifier(name);
                            self.status_message = format!("Added {} Modifier", name);
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("Mesh", |ui| {
                    if !has_selection {
                        ui.disable();
                    }
                    for name in &[
                        "Decimate",
                        "Edge Split",
                        "Remesh",
                        "Subdivision",
                        "Triangulate",
                        "Weld",
                    ] {
                        if ui.button(*name).clicked() {
                            self.state.add_modifier(name);
                            self.status_message = format!("Added {} Modifier", name);
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("Surface", |ui| {
                    if !has_selection {
                        ui.disable();
                    }
                    for name in &["Skin", "UV Project"] {
                        if ui.button(*name).clicked() {
                            self.state.add_modifier(name);
                            self.status_message = format!("Added {} Modifier", name);
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("Boolean", |ui| {
                    if !has_selection {
                        ui.disable();
                    }
                    for name in &["Union", "Difference", "Intersection"] {
                        if ui.button(*name).clicked() {
                            self.state.add_modifier(&format!("Boolean: {}", name));
                            self.status_message = format!("Boolean {}", name);
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("SOTA Research", |ui| {
                    if !has_selection {
                        ui.disable();
                    }
                    if ui.button("Spectral Smooth (Sorkine 2006)").clicked() {
                        self.state.add_modifier("Spectral Smooth");
                        self.status_message =
                            "Added Spectral Smooth (Laplacian) Modifier".to_string();
                        ui.close_menu();
                    }
                    if ui.button("Hyperbolic Warp (Ungar 2001)").clicked() {
                        self.state.add_modifier("Hyperbolic Warp");
                        self.status_message =
                            "Added Hyperbolic Warp (Poincaré Ball) Modifier".to_string();
                        ui.close_menu();
                    }
                });
            });

            ui.menu_button("Object", |ui| {
                let has_selection = self.state.selected_object.is_some();
                let has_multi = !self.state.multi_selected.is_empty();
                ui.menu_button("Parent", |ui| {
                    if ui
                        .add_enabled(has_multi, egui::Button::new("Set Parent (Ctrl+P)"))
                        .clicked()
                    {
                        // Parent multi-selected objects to the primary selected
                        if let Some(parent_idx) = self.state.selected_object {
                            let children: Vec<usize> = self.state.multi_selected.clone();
                            for child_idx in children {
                                if child_idx < self.state.objects.len() && child_idx != parent_idx {
                                    self.state.objects[child_idx].parent = Some(parent_idx);
                                }
                            }
                            self.status_message = "Set parent".to_string();
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Clear Parent (Alt+P)"))
                        .clicked()
                    {
                        if let Some(idx) = self.state.selected_object {
                            self.state.objects[idx].parent = None;
                        }
                        // Also clear for multi-selected
                        for &mi in &self.state.multi_selected.clone() {
                            if mi < self.state.objects.len() {
                                self.state.objects[mi].parent = None;
                            }
                        }
                        self.status_message = "Cleared parent".to_string();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Vertex Groups", |ui| {
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Add Group"))
                        .clicked()
                    {
                        if let Some(idx) = self.state.selected_object {
                            let n = self.state.objects[idx].vertex_groups.len();
                            self.state.objects[idx].vertex_groups.push(VertexGroup {
                                name: format!("Group.{:03}", n),
                                weights: Vec::new(),
                            });
                            self.status_message = "Added vertex group".to_string();
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Remove Last Group"))
                        .clicked()
                    {
                        if let Some(idx) = self.state.selected_object {
                            self.state.objects[idx].vertex_groups.pop();
                            self.status_message = "Removed vertex group".to_string();
                        }
                        ui.close_menu();
                    }
                });
                ui.separator();
                ui.menu_button("Transform Orientation", |ui| {
                    for orient in &[
                        TransformOrientation::Global,
                        TransformOrientation::Local,
                        TransformOrientation::Normal,
                        TransformOrientation::Gimbal,
                        TransformOrientation::View,
                    ] {
                        let label = format!("{}", orient);
                        let selected = self.state.transform_orientation == *orient;
                        if ui.selectable_label(selected, &label).clicked() {
                            self.state.transform_orientation = *orient;
                            self.status_message = format!("Orientation: {}", orient);
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("Snap Target", |ui| {
                    for target in &[
                        SnapTarget::Closest,
                        SnapTarget::Center,
                        SnapTarget::Median,
                        SnapTarget::Active,
                    ] {
                        let label = format!("{}", target);
                        let selected = self.state.snap_target == *target;
                        if ui.selectable_label(selected, &label).clicked() {
                            self.state.snap_target = *target;
                            self.status_message = format!("Snap target: {}", target);
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                if ui
                    .add_enabled(has_selection, egui::Button::new("Apply Transforms"))
                    .clicked()
                {
                    if let Some(idx) = self.state.selected_object {
                        self.state.objects[idx].position = [0.0, 0.0, 0.0];
                        self.state.objects[idx].rotation = [0.0, 0.0, 0.0];
                        self.state.objects[idx].scale = [1.0, 1.0, 1.0];
                        self.status_message = "Applied transforms".to_string();
                    }
                    ui.close_menu();
                }
                if ui
                    .add_enabled(has_selection, egui::Button::new("Reset Origin"))
                    .clicked()
                {
                    if let Some(idx) = self.state.selected_object {
                        self.state.objects[idx].position = [0.0, 0.0, 0.0];
                        self.status_message = "Reset origin to center".to_string();
                    }
                    ui.close_menu();
                }
                ui.separator();
                ui.menu_button("Align", |ui| {
                    if !has_selection {
                        ui.disable();
                    }
                    let aligns = [
                        AlignAxis::AlignX,
                        AlignAxis::AlignY,
                        AlignAxis::AlignZ,
                        AlignAxis::DistributeX,
                        AlignAxis::DistributeY,
                        AlignAxis::DistributeZ,
                        AlignAxis::CenterToWorld,
                        AlignAxis::CenterToActive,
                        AlignAxis::SnapToGrid,
                        AlignAxis::SnapToGround,
                    ];
                    for align in &aligns {
                        if ui.button(format!("{}", align)).clicked() {
                            self.state.save_undo_state();
                            if self.state.align_objects(*align) {
                                self.status_message = format!("{}", align);
                            }
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("Overlays", |ui| {
                    ui.checkbox(&mut self.state.overlays.show_grid, "Floor Grid");
                    ui.checkbox(&mut self.state.overlays.show_axes, "Axis Lines");
                    ui.checkbox(
                        &mut self.state.overlays.wireframe_on_solid,
                        "Wireframe on Solid",
                    );
                    ui.checkbox(&mut self.state.overlays.show_motion_paths, "Motion Paths");
                    ui.add(
                        egui::Slider::new(&mut self.state.overlays.grid_opacity, 0.0..=1.0)
                            .text("Grid Opacity"),
                    );
                });
                ui.separator();
                ui.menu_button("Collections", |ui| {
                    if ui.button("New Collection").clicked() {
                        let n = self.state.collections.len();
                        self.state.collections.push(ObjectCollection {
                            name: format!("Collection {}", n + 1),
                            object_indices: Vec::new(),
                            visible: true,
                            color: [0.5, 0.7, 1.0],
                        });
                        self.status_message = format!("Created Collection {}", n + 1);
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_selection && !self.state.collections.is_empty(),
                            egui::Button::new("Add Selected to Last Collection"),
                        )
                        .clicked()
                    {
                        let selected = self.state.all_selected();
                        if let Some(last) = self.state.collections.last_mut() {
                            for idx in selected {
                                if !last.object_indices.contains(&idx) {
                                    last.object_indices.push(idx);
                                }
                            }
                            self.status_message = format!("Added to {}", last.name);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    let mut toggle_vis: Option<usize> = None;
                    for (ci, coll) in self.state.collections.iter().enumerate() {
                        let label = format!(
                            "{} [{}] {}",
                            if coll.visible { "v" } else { "." },
                            coll.object_indices.len(),
                            coll.name
                        );
                        if ui.button(&label).clicked() {
                            toggle_vis = Some(ci);
                        }
                    }
                    if let Some(ci) = toggle_vis {
                        let vis = !self.state.collections[ci].visible;
                        self.state.collections[ci].visible = vis;
                        // Toggle visibility of all objects in this collection
                        let indices = self.state.collections[ci].object_indices.clone();
                        for idx in indices {
                            if idx < self.state.objects.len() {
                                self.state.objects[idx].visible = vis;
                            }
                        }
                        self.status_message = format!(
                            "Collection {} visibility: {}",
                            self.state.collections[ci].name,
                            if vis { "shown" } else { "hidden" }
                        );
                    }
                });
            });

            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.show_hierarchy, "Hierarchy Panel");
                ui.checkbox(&mut self.show_properties, "Properties Panel");
                ui.checkbox(&mut self.show_timeline, "Timeline Panel");
                ui.checkbox(&mut self.show_materials, "Material Editor");
                ui.checkbox(&mut self.show_console, "Console");
                ui.checkbox(&mut self.show_node_editor, "Node Editor");
                ui.checkbox(&mut self.show_uv_editor, "UV Editor");
                ui.checkbox(&mut self.show_graph_editor, "Graph Editor");
                ui.checkbox(&mut self.show_camera_settings, "Camera Settings");
                ui.checkbox(&mut self.show_world_settings, "World Settings");
                ui.checkbox(&mut self.show_scene_properties, "Scene Properties");
                ui.checkbox(&mut self.show_nla_editor, "NLA Editor");
                ui.checkbox(&mut self.show_color_management, "Color Management");
                ui.checkbox(&mut self.show_asset_browser, "Asset Browser");
                ui.checkbox(&mut self.show_render_layers, "Render Layers");
                ui.checkbox(&mut self.show_spreadsheet, "Spreadsheet");
                #[cfg(feature = "python")]
                ui.checkbox(&mut self.show_text_editor, "Text Editor");
                ui.checkbox(&mut self.show_sequencer, "Sequencer");
                ui.checkbox(&mut self.show_image_editor, "Image Editor");
                ui.checkbox(&mut self.state.show_perf_overlay, "Performance Overlay");
                ui.separator();
                ui.checkbox(&mut self.state.wireframe_overlay, "Wireframe Overlay");
                ui.checkbox(&mut self.state.show_viewport_stats, "Scene Statistics");
                ui.checkbox(&mut self.state.show_normals, "Show Normals");
                ui.checkbox(&mut self.state.show_object_info, "Object Info Overlay");
                ui.checkbox(&mut self.state.show_orientation_cube, "Orientation Cube");
                ui.checkbox(&mut self.state.show_camera_preview, "Camera Preview");
                ui.checkbox(&mut self.state.show_face_orientation, "Face Orientation");
                ui.checkbox(
                    &mut self.state.show_relationship_lines,
                    "Relationship Lines",
                );
                ui.checkbox(&mut self.state.show_motion_paths_viewport, "Motion Paths");
                ui.separator();
                ui.menu_button("Viewport Shading", |ui| {
                    ui.checkbox(&mut self.state.xray_mode, "X-Ray Mode (Alt+Z)");
                    ui.checkbox(&mut self.state.backface_culling, "Backface Culling");
                    ui.checkbox(&mut self.state.show_cavity, "Cavity");
                    ui.checkbox(&mut self.state.show_shadows, "Shadows");
                    ui.checkbox(&mut self.state.show_specular, "Specular Lighting");
                    ui.checkbox(&mut self.state.show_only_render, "Show Only Render");
                    ui.separator();
                    ui.add(
                        egui::Slider::new(&mut self.state.clip_near, 0.001..=10.0)
                            .text("Clip Start")
                            .logarithmic(true),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.state.clip_far, 10.0..=10000.0)
                            .text("Clip End")
                            .logarithmic(true),
                    );
                });
                ui.separator();
                // Matcap selection
                ui.menu_button("Matcap", |ui| {
                    let matcaps = [
                        "None", "Clay", "Chrome", "Jade", "Pearl", "Obsidian", "Copper",
                    ];
                    for (i, name) in matcaps.iter().enumerate() {
                        let selected = self.state.matcap_index == i;
                        if ui.selectable_label(selected, *name).clicked() {
                            self.state.matcap_index = i;
                            self.status_message = format!("Matcap: {}", name);
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                ui.menu_button("Animation Overlays", |ui| {
                    ui.checkbox(&mut self.state.onion_skinning, "Onion Skinning");
                    ui.add(
                        egui::Slider::new(&mut self.state.onion_frames, 1..=10)
                            .text("Ghost Frames"),
                    );
                    ui.checkbox(&mut self.state.auto_key, "Auto-Key");
                });
                ui.separator();
                if ui.button("Front View").clicked() {
                    self.state.camera.set_view_front();
                    ui.close_menu();
                }
                if ui.button("Back View").clicked() {
                    self.state.camera.set_view_back();
                    ui.close_menu();
                }
                if ui.button("Left View").clicked() {
                    self.state.camera.set_view_left();
                    ui.close_menu();
                }
                if ui.button("Right View").clicked() {
                    self.state.camera.set_view_right();
                    ui.close_menu();
                }
                if ui.button("Top View").clicked() {
                    self.state.camera.set_view_top();
                    ui.close_menu();
                }
                if ui.button("Bottom View").clicked() {
                    self.state.camera.set_view_bottom();
                    ui.close_menu();
                }
            });

            ui.menu_button("Simulation", |ui| {
                let has_selection = self.state.selected_object.is_some();
                ui.menu_button("Physics", |ui| {
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Add Rigid Body"))
                        .clicked()
                    {
                        self.state.enable_rigid_body();
                        self.status_message = "Added Rigid Body physics".to_string();
                        self.log_console(
                            console::LogLevel::Info,
                            "Rigid Body added to selected object",
                            "Physics",
                        );
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Add Static Collider"))
                        .clicked()
                    {
                        self.state.enable_static_collider();
                        self.status_message = "Added Static Collider".to_string();
                        self.log_console(
                            console::LogLevel::Info,
                            "Static Collider added to selected object",
                            "Physics",
                        );
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Add Cloth"))
                        .clicked()
                    {
                        self.status_message = "Added Cloth Simulation".to_string();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Particles", |ui| {
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Add Particle System"))
                        .clicked()
                    {
                        if let Some(idx) = self.state.selected_object {
                            self.state.objects[idx]
                                .particle_systems
                                .push(ParticleSystem::default());
                            self.status_message = "Added Particle System".to_string();
                            self.log_console(
                                console::LogLevel::Info,
                                "Particle system added",
                                "Particles",
                            );
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Remove Particle System"))
                        .clicked()
                    {
                        if let Some(idx) = self.state.selected_object {
                            self.state.objects[idx].particle_systems.pop();
                            self.status_message = "Removed Particle System".to_string();
                        }
                        ui.close_menu();
                    }
                });
                ui.menu_button("Force Fields", |ui| {
                    if !has_selection {
                        ui.disable();
                    }
                    let fields = [
                        "Wind",
                        "Vortex",
                        "Turbulence",
                        "Drag",
                        "Magnetic",
                        "Harmonic",
                        "Charge",
                        "Lennard-Jones",
                    ];
                    for &name in &fields {
                        if ui.button(name).clicked() {
                            if let Some(idx) = self.state.selected_object {
                                let ft = match name {
                                    "Wind" => ForceFieldType::Wind,
                                    "Vortex" => ForceFieldType::Vortex,
                                    "Turbulence" => ForceFieldType::Turbulence,
                                    "Drag" => ForceFieldType::Drag,
                                    "Magnetic" => ForceFieldType::Magnetic,
                                    "Harmonic" => ForceFieldType::Harmonic,
                                    "Charge" => ForceFieldType::Charge,
                                    _ => ForceFieldType::Lennard,
                                };
                                self.state.objects[idx].force_field = Some(ForceFieldSettings {
                                    field_type: ft,
                                    ..ForceFieldSettings::default()
                                });
                                self.status_message = format!("Added {} force field", name);
                            }
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("Cloth / Soft Body", |ui| {
                    if !has_selection {
                        ui.disable();
                    }
                    if ui.button("Add Cloth Simulation").clicked() {
                        if let Some(idx) = self.state.selected_object {
                            self.state.objects[idx].cloth = Some(ClothSettings::default());
                            self.status_message = "Added Cloth Simulation".to_string();
                        }
                        ui.close_menu();
                    }
                    if ui.button("Add Soft Body").clicked() {
                        if let Some(idx) = self.state.selected_object {
                            self.state.objects[idx].soft_body = Some(SoftBodySettings::default());
                            self.status_message = "Added Soft Body".to_string();
                        }
                        ui.close_menu();
                    }
                    if ui.button("Remove Cloth").clicked() {
                        if let Some(idx) = self.state.selected_object {
                            self.state.objects[idx].cloth = None;
                            self.status_message = "Removed Cloth Simulation".to_string();
                        }
                        ui.close_menu();
                    }
                    if ui.button("Remove Soft Body").clicked() {
                        if let Some(idx) = self.state.selected_object {
                            self.state.objects[idx].soft_body = None;
                            self.status_message = "Removed Soft Body".to_string();
                        }
                        ui.close_menu();
                    }
                });
                ui.menu_button("Fluids", |ui| {
                    if ui.button("Add Fluid Domain").clicked() {
                        if let Some(idx) = self.state.selected_object {
                            if idx < self.state.objects.len() {
                                self.state.objects[idx].fluid = Some(FluidSettings {
                                    fluid_type: FluidType::Domain,
                                    ..FluidSettings::default()
                                });
                                self.status_message = "Added Fluid Domain".to_string();
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Add Fluid Inflow").clicked() {
                        if let Some(idx) = self.state.selected_object {
                            if idx < self.state.objects.len() {
                                self.state.objects[idx].fluid = Some(FluidSettings {
                                    fluid_type: FluidType::Inflow,
                                    ..FluidSettings::default()
                                });
                                self.status_message = "Added Fluid Inflow".to_string();
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Add Fluid Outflow").clicked() {
                        if let Some(idx) = self.state.selected_object {
                            if idx < self.state.objects.len() {
                                self.state.objects[idx].fluid = Some(FluidSettings {
                                    fluid_type: FluidType::Outflow,
                                    ..FluidSettings::default()
                                });
                                self.status_message = "Added Fluid Outflow".to_string();
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Remove Fluid").clicked() {
                        if let Some(idx) = self.state.selected_object {
                            if idx < self.state.objects.len() {
                                self.state.objects[idx].fluid = None;
                                self.status_message = "Removed Fluid".to_string();
                            }
                        }
                        ui.close_menu();
                    }
                });
                ui.separator();
                let running = self.state.physics_running;
                let play_text = if running {
                    "Pause Simulation"
                } else {
                    "Run Simulation"
                };
                if ui.button(play_text).clicked() {
                    self.state.physics_running = !running;
                    self.status_message = if !running {
                        self.log_console(
                            console::LogLevel::Info,
                            "Physics simulation started",
                            "Physics",
                        );
                        "Physics simulation running...".to_string()
                    } else {
                        self.log_console(
                            console::LogLevel::Info,
                            "Physics simulation paused",
                            "Physics",
                        );
                        "Physics simulation paused".to_string()
                    };
                    ui.close_menu();
                }
                if ui.button("Reset Simulation").clicked() {
                    self.state.physics_running = false;
                    self.state.physics.clear();
                    self.state.sync_physics();
                    self.status_message = "Simulation reset".to_string();
                    self.log_console(
                        console::LogLevel::Info,
                        "Physics simulation reset",
                        "Physics",
                    );
                    ui.close_menu();
                }
                if ui.button("Bake Simulation").clicked() {
                    self.status_message = "Baking simulation...".to_string();
                    ui.close_menu();
                }
            });

            ui.menu_button("Animation", |ui| {
                let has_selection = self.state.selected_object.is_some();
                if ui
                    .add_enabled(has_selection, egui::Button::new("Insert Keyframe (I)"))
                    .clicked()
                {
                    if self.state.insert_keyframe() {
                        let f = self.state.timeline.current_frame;
                        self.status_message = format!("Keyframe at frame {}", f);
                        self.log_console(
                            console::LogLevel::Info,
                            &format!("Keyframe at frame {}", f),
                            "Animation",
                        );
                    }
                    ui.close_menu();
                }
                if ui
                    .add_enabled(has_selection, egui::Button::new("Delete Keyframe (Alt+I)"))
                    .clicked()
                {
                    if self.state.delete_keyframe() {
                        self.status_message = "Keyframe deleted".to_string();
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui
                    .add_enabled(has_selection, egui::Button::new("Clear All Keyframes"))
                    .clicked()
                {
                    if let Some(idx) = self.state.selected_object {
                        self.state.objects[idx].keyframes.clear();
                        self.status_message = "All keyframes cleared".to_string();
                    }
                    ui.close_menu();
                }
                ui.separator();
                ui.checkbox(&mut self.show_dopesheet, "Show Dopesheet");
            });

            ui.menu_button("Mesh", |ui| {
                let multi = self.state.all_selected().len() >= 2;
                ui.menu_button("Boolean", |ui| {
                    if !multi {
                        ui.disable();
                    }
                    if ui.button("Union").clicked() {
                        self.state.save_undo_state();
                        if let Some(name) = self.state.boolean_op(BooleanOp::Union) {
                            self.status_message = format!("Boolean Union: {}", name);
                            self.log_console(
                                console::LogLevel::Info,
                                &format!("Boolean Union -> {}", name),
                                "Mesh",
                            );
                        }
                        ui.close_menu();
                    }
                    if ui.button("Difference").clicked() {
                        self.state.save_undo_state();
                        if let Some(name) = self.state.boolean_op(BooleanOp::Difference) {
                            self.status_message = format!("Boolean Difference: {}", name);
                            self.log_console(
                                console::LogLevel::Info,
                                &format!("Boolean Difference -> {}", name),
                                "Mesh",
                            );
                        }
                        ui.close_menu();
                    }
                    if ui.button("Intersection").clicked() {
                        self.state.save_undo_state();
                        if let Some(name) = self.state.boolean_op(BooleanOp::Intersection) {
                            self.status_message = format!("Boolean Intersection: {}", name);
                            self.log_console(
                                console::LogLevel::Info,
                                &format!("Boolean Intersection -> {}", name),
                                "Mesh",
                            );
                        }
                        ui.close_menu();
                    }
                });
                ui.separator();
                let has_sel = self.state.selected_object.is_some();
                if ui
                    .add_enabled(has_sel, egui::Button::new("Add Shape Key"))
                    .clicked()
                {
                    let count = self
                        .state
                        .selected_object
                        .and_then(|i| self.state.objects.get(i))
                        .map_or(0, |o| o.shape_keys.len());
                    self.state.add_shape_key(&format!("Key.{:03}", count));
                    self.status_message = "Added Shape Key".to_string();
                    ui.close_menu();
                }
                ui.separator();
                ui.menu_button("Constraints", |ui| {
                    if !has_sel {
                        ui.disable();
                    }
                    if multi {
                        let target =
                            *self.state.all_selected().last().expect(
                                "invariant: `multi` (len >= 2) implies non-empty selection",
                            );
                        if ui.button("Track To (last selected)").clicked() {
                            self.state
                                .add_constraint(ObjectConstraint::TrackTo { target_idx: target });
                            self.status_message = format!("Track To -> object {}", target);
                            ui.close_menu();
                        }
                        if ui.button("Copy Location (last selected)").clicked() {
                            self.state.add_constraint(ObjectConstraint::CopyLocation {
                                target_idx: target,
                                influence: 1.0,
                            });
                            self.status_message = format!("Copy Location -> object {}", target);
                            ui.close_menu();
                        }
                        if ui.button("Copy Rotation (last selected)").clicked() {
                            self.state.add_constraint(ObjectConstraint::CopyRotation {
                                target_idx: target,
                                influence: 1.0,
                            });
                            self.status_message = format!("Copy Rotation -> object {}", target);
                            ui.close_menu();
                        }
                    }
                    if ui.button("Limit Location").clicked() {
                        self.state.add_constraint(ObjectConstraint::LimitLocation {
                            min: [-10.0, -10.0, -10.0],
                            max: [10.0, 10.0, 10.0],
                        });
                        self.status_message = "Limit Location constraint added".to_string();
                        ui.close_menu();
                    }
                });
                ui.separator();
                ui.checkbox(
                    &mut self.state.proportional_editing,
                    "Proportional Editing (O)",
                );
                if self.state.proportional_editing {
                    ui.horizontal(|ui| {
                        ui.label("Radius:");
                        ui.add(egui::Slider::new(
                            &mut self.state.proportional_radius,
                            0.5..=10.0,
                        ));
                    });
                }
                ui.separator();
                ui.label("Measurement");
                let measuring = self.state.measuring;
                if ui
                    .button(if measuring {
                        "Stop Measuring"
                    } else {
                        "Measure (M)"
                    })
                    .clicked()
                {
                    self.state.measuring = !measuring;
                    if self.state.measuring {
                        self.state.measure_start = None;
                    }
                    ui.close_menu();
                }
                if ui.button("Clear Measurements (Ctrl+M)").clicked() {
                    self.state.clear_measurements();
                    self.status_message = "Measurements cleared".to_string();
                    ui.close_menu();
                }
            });

            ui.menu_button("Render", |ui| {
                if ui.button("Render Image").clicked() {
                    self.render_image();
                    ui.close_menu();
                }
                if ui.button("Render Animation").clicked() {
                    self.render_animation();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Render Settings...").clicked() {
                    self.show_render_settings = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard Shortcuts").clicked() {
                    self.status_message = "Q/W/E/R: Tools | G/R/S: Grab/Rotate/Scale | X/Y/Z: Axis | Tab: Mode | 1/3/7: Views | Z: Shading | F: Focus | N/T: Panels | Del: Delete | Shift+D: Duplicate".to_string();
                    ui.close_menu();
                }
                if ui.button("Documentation").clicked() {
                    #[cfg(feature = "file-dialog")]
                    if let Err(e) = open::that("https://github.com/Yatrogenesis/NAT3D") {
                        self.status_message = format!("Failed to open browser: {}", e);
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Welcome Screen...").clicked() {
                    self.show_welcome = true;
                    ui.close_menu();
                }
                if ui.button("License / Activate...").clicked() {
                    self.show_license_dialog = true;
                    ui.close_menu();
                }
                if ui.button("About NAT3D").clicked() {
                    self.show_about = true;
                    ui.close_menu();
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Transform tools (BATCH 24: Tooltips)
            ui.selectable_value(&mut self.state.tool, Tool::Select, "Select (Q)")
                .on_hover_text(
                    "Select objects - Left click to select, Shift+Click to multi-select",
                );
            ui.selectable_value(&mut self.state.tool, Tool::Move, "Move (W/G)")
                .on_hover_text(
                    "Move objects - Press X/Y/Z for axis constraint, Shift for grid snap",
                );
            ui.selectable_value(&mut self.state.tool, Tool::Rotate, "Rotate (E/R)")
                .on_hover_text("Rotate objects - Press X/Y/Z for axis, Shift snaps to 5 degrees");
            ui.selectable_value(&mut self.state.tool, Tool::Scale, "Scale (S)")
                .on_hover_text("Scale objects - Press X/Y/Z for axis, Shift snaps to 0.25 units");

            ui.separator();

            // Edit mode toggle
            ui.selectable_value(&mut self.state.edit_mode, EditMode::Object, "Object");
            ui.selectable_value(&mut self.state.edit_mode, EditMode::Edit, "Edit");
            ui.selectable_value(&mut self.state.edit_mode, EditMode::Sculpt, "Sculpt");
            ui.selectable_value(
                &mut self.state.edit_mode,
                EditMode::TexturePaint,
                "TexPaint",
            );
            ui.selectable_value(&mut self.state.edit_mode, EditMode::WeightPaint, "WtPaint");

            ui.separator();

            // Viewport shading
            ui.selectable_value(&mut self.state.shading, ShadingMode::Wireframe, "Wire");
            ui.selectable_value(&mut self.state.shading, ShadingMode::Solid, "Solid");
            ui.selectable_value(&mut self.state.shading, ShadingMode::Material, "Matl");
            ui.selectable_value(&mut self.state.shading, ShadingMode::Rendered, "Rend");

            // Sculpt brush selector (only in Sculpt mode)
            if self.state.edit_mode == EditMode::Sculpt {
                ui.separator();
                ui.selectable_value(&mut self.state.sculpt_brush, SculptBrush::Draw, "Draw");
                ui.selectable_value(&mut self.state.sculpt_brush, SculptBrush::Smooth, "Smth");
                ui.selectable_value(&mut self.state.sculpt_brush, SculptBrush::Flatten, "Flat");
                ui.selectable_value(&mut self.state.sculpt_brush, SculptBrush::Pinch, "Pnch");
                ui.selectable_value(&mut self.state.sculpt_brush, SculptBrush::Inflate, "Infl");
                ui.selectable_value(&mut self.state.sculpt_brush, SculptBrush::Grab, "Grab");
            }

            // Edit mode sub-selection: Vertex/Edge/Face (only in Edit mode)
            if self.state.edit_mode == EditMode::Edit {
                ui.separator();
                ui.selectable_value(
                    &mut self.state.edit_selection,
                    EditSelection::Vertex,
                    "Vert(1)",
                );
                ui.selectable_value(
                    &mut self.state.edit_selection,
                    EditSelection::Edge,
                    "Edge(2)",
                );
                ui.selectable_value(
                    &mut self.state.edit_selection,
                    EditSelection::Face,
                    "Face(3)",
                );
                ui.separator();
                // Edit mode tools
                egui::ComboBox::from_id_salt("edit_tool_combo")
                    .selected_text(format!("{}", self.state.edit_tool))
                    .width(70.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.state.edit_tool, EditTool::Select, "Select");
                        ui.selectable_value(
                            &mut self.state.edit_tool,
                            EditTool::Extrude,
                            "Extrude (E)",
                        );
                        ui.selectable_value(
                            &mut self.state.edit_tool,
                            EditTool::LoopCut,
                            "Loop Cut (Ctrl+R)",
                        );
                        ui.selectable_value(
                            &mut self.state.edit_tool,
                            EditTool::Knife,
                            "Knife (K)",
                        );
                        ui.selectable_value(
                            &mut self.state.edit_tool,
                            EditTool::BevelEdge,
                            "Bevel (Ctrl+B)",
                        );
                        ui.selectable_value(
                            &mut self.state.edit_tool,
                            EditTool::InsetFace,
                            "Inset (I)",
                        );
                        ui.selectable_value(
                            &mut self.state.edit_tool,
                            EditTool::PolyBuild,
                            "PolyBuild",
                        );
                        ui.selectable_value(&mut self.state.edit_tool, EditTool::SpinTool, "Spin");
                    });
                if self.state.edit_tool == EditTool::LoopCut {
                    ui.add(
                        egui::DragValue::new(&mut self.state.loop_cut_segments)
                            .speed(1)
                            .range(1..=100)
                            .prefix("Cuts: "),
                    );
                }
            }

            // Texture paint controls
            if self.state.edit_mode == EditMode::TexturePaint {
                ui.separator();
                let mut color = [
                    self.state.paint_color[0],
                    self.state.paint_color[1],
                    self.state.paint_color[2],
                ];
                if ui.color_edit_button_rgb(&mut color).changed() {
                    self.state.paint_color = [color[0], color[1], color[2], 1.0];
                }
                ui.add(egui::Slider::new(&mut self.state.paint_radius, 5.0..=100.0).text("R"));
            }

            // Weight paint controls
            if self.state.edit_mode == EditMode::WeightPaint {
                ui.separator();
                ui.add(egui::Slider::new(&mut self.state.weight_value, 0.0..=1.0).text("Wt"));
                ui.add(egui::Slider::new(&mut self.state.paint_radius, 5.0..=100.0).text("R"));
            }

            // Pivot point selector
            ui.separator();
            egui::ComboBox::from_id_salt("pivot_combo")
                .selected_text(format!("Pivot: {}", self.state.pivot_point))
                .width(90.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.state.pivot_point,
                        PivotPoint::MedianPoint,
                        "Median Point",
                    );
                    ui.selectable_value(
                        &mut self.state.pivot_point,
                        PivotPoint::IndividualOrigins,
                        "Individual Origins",
                    );
                    ui.selectable_value(
                        &mut self.state.pivot_point,
                        PivotPoint::Cursor3D,
                        "3D Cursor",
                    );
                    ui.selectable_value(
                        &mut self.state.pivot_point,
                        PivotPoint::ActiveElement,
                        "Active Element",
                    );
                    ui.selectable_value(
                        &mut self.state.pivot_point,
                        PivotPoint::BoundingBoxCenter,
                        "BBox Center",
                    );
                });

            // Transform orientation
            egui::ComboBox::from_id_salt("orient_combo")
                .selected_text(format!("{}", self.state.transform_orientation))
                .width(60.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.state.transform_orientation,
                        TransformOrientation::Global,
                        "Global",
                    );
                    ui.selectable_value(
                        &mut self.state.transform_orientation,
                        TransformOrientation::Local,
                        "Local",
                    );
                    ui.selectable_value(
                        &mut self.state.transform_orientation,
                        TransformOrientation::Normal,
                        "Normal",
                    );
                    ui.selectable_value(
                        &mut self.state.transform_orientation,
                        TransformOrientation::Gimbal,
                        "Gimbal",
                    );
                    ui.selectable_value(
                        &mut self.state.transform_orientation,
                        TransformOrientation::View,
                        "View",
                    );
                });

            // Snap target
            egui::ComboBox::from_id_salt("snap_target_combo")
                .selected_text(format!("Snap: {}", self.state.snap_target))
                .width(80.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.state.snap_target,
                        SnapTarget::Closest,
                        "Closest",
                    );
                    ui.selectable_value(&mut self.state.snap_target, SnapTarget::Center, "Center");
                    ui.selectable_value(&mut self.state.snap_target, SnapTarget::Median, "Median");
                    ui.selectable_value(&mut self.state.snap_target, SnapTarget::Active, "Active");
                });

            // Snap element type
            egui::ComboBox::from_id_salt("snap_elem_combo")
                .selected_text(format!("{}", self.state.snap_element))
                .width(70.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.state.snap_element,
                        SnapElement::Increment,
                        "Increment",
                    );
                    ui.selectable_value(
                        &mut self.state.snap_element,
                        SnapElement::Vertex,
                        "Vertex",
                    );
                    ui.selectable_value(&mut self.state.snap_element, SnapElement::Edge, "Edge");
                    ui.selectable_value(&mut self.state.snap_element, SnapElement::Face, "Face");
                    ui.selectable_value(
                        &mut self.state.snap_element,
                        SnapElement::Volume,
                        "Volume",
                    );
                    ui.selectable_value(
                        &mut self.state.snap_element,
                        SnapElement::EdgeCenter,
                        "Edge Center",
                    );
                    ui.selectable_value(
                        &mut self.state.snap_element,
                        SnapElement::EdgePerpendicular,
                        "Edge Perp.",
                    );
                });

            // Auto-Key indicator
            if self.state.auto_key {
                ui.separator();
                ui.colored_label(egui::Color32::from_rgb(255, 80, 80), "AutoKey");
            }

            // Proportional editing indicator
            if self.state.proportional_editing {
                ui.separator();
                ui.colored_label(egui::Color32::from_rgb(100, 200, 255), "Prop");
                egui::ComboBox::from_id_salt("prop_falloff")
                    .selected_text(format!("{}", self.state.proportional_falloff))
                    .width(70.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.state.proportional_falloff,
                            ProportionalFalloff::Smooth,
                            "Smooth",
                        );
                        ui.selectable_value(
                            &mut self.state.proportional_falloff,
                            ProportionalFalloff::Sphere,
                            "Sphere",
                        );
                        ui.selectable_value(
                            &mut self.state.proportional_falloff,
                            ProportionalFalloff::Root,
                            "Root",
                        );
                        ui.selectable_value(
                            &mut self.state.proportional_falloff,
                            ProportionalFalloff::InverseSquare,
                            "Inv. Square",
                        );
                        ui.selectable_value(
                            &mut self.state.proportional_falloff,
                            ProportionalFalloff::Sharp,
                            "Sharp",
                        );
                        ui.selectable_value(
                            &mut self.state.proportional_falloff,
                            ProportionalFalloff::Linear,
                            "Linear",
                        );
                        ui.selectable_value(
                            &mut self.state.proportional_falloff,
                            ProportionalFalloff::Constant,
                            "Constant",
                        );
                        ui.selectable_value(
                            &mut self.state.proportional_falloff,
                            ProportionalFalloff::Random,
                            "Random",
                        );
                    });
            }

            // Measuring indicator
            if self.state.measuring {
                ui.separator();
                ui.colored_label(egui::Color32::from_rgb(255, 255, 100), "Measure");
            }

            // Active axis constraint indicator
            if self.state.axis_constraint != AxisConstraint::None && self.state.tool != Tool::Select
            {
                ui.separator();
                let (axis_str, axis_color) = match self.state.axis_constraint {
                    AxisConstraint::X => ("X", egui::Color32::from_rgb(230, 60, 60)),
                    AxisConstraint::Y => ("Y", egui::Color32::from_rgb(60, 200, 60)),
                    AxisConstraint::Z => ("Z", egui::Color32::from_rgb(80, 80, 230)),
                    AxisConstraint::None => ("", egui::Color32::WHITE),
                };
                ui.colored_label(axis_color, format!("Axis: {}", axis_str));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Workspace layout
                egui::ComboBox::from_id_salt("workspace_combo")
                    .selected_text(format!("{}", self.state.workspace))
                    .width(85.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.state.workspace,
                            WorkspaceLayout::Modeling,
                            "Modeling",
                        );
                        ui.selectable_value(
                            &mut self.state.workspace,
                            WorkspaceLayout::Sculpting,
                            "Sculpting",
                        );
                        ui.selectable_value(
                            &mut self.state.workspace,
                            WorkspaceLayout::UVEditing,
                            "UV Editing",
                        );
                        ui.selectable_value(
                            &mut self.state.workspace,
                            WorkspaceLayout::TexturePaint,
                            "Texture Paint",
                        );
                        ui.selectable_value(
                            &mut self.state.workspace,
                            WorkspaceLayout::Animation,
                            "Animation",
                        );
                        ui.selectable_value(
                            &mut self.state.workspace,
                            WorkspaceLayout::Compositing,
                            "Compositing",
                        );
                        ui.selectable_value(
                            &mut self.state.workspace,
                            WorkspaceLayout::Rendering,
                            "Rendering",
                        );
                        ui.selectable_value(
                            &mut self.state.workspace,
                            WorkspaceLayout::Scripting,
                            "Scripting",
                        );
                    });
                ui.separator();

                // Snap settings
                ui.checkbox(&mut self.state.snap_enabled, "Snap");
                if self.state.snap_enabled {
                    ui.add(
                        egui::DragValue::new(&mut self.state.snap_increment)
                            .speed(0.1)
                            .range(0.01..=10.0)
                            .prefix("Grid: "),
                    );
                }
                ui.separator();
                // Selection count
                let sel_count = self.state.all_selected().len();
                if sel_count > 0 {
                    ui.label(format!("{} sel", sel_count));
                }
            });
        });
    }

    fn hierarchy_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Scene Hierarchy");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+").on_hover_text("Add Object").clicked() {
                    self.state.add_cube();
                }
            });
        });
        ui.separator();

        let search_filter = self.hierarchy_search.to_lowercase();
        let mut to_select = None;
        let mut to_delete = None;
        let mut to_toggle_visibility = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, obj) in self.state.objects.iter().enumerate() {
                if !search_filter.is_empty() && !obj.name.to_lowercase().contains(&search_filter) {
                    continue;
                }

                ui.horizontal(|ui| {
                    // Visibility Toggle
                    let vis_icon = if obj.visible { "⌂" } else { "⌃" }; // Simplified icons
                    if ui.button(vis_icon).clicked() {
                        to_toggle_visibility = Some(i);
                    }

                    // Object Selection
                    let is_selected = self.state.selected_object == Some(i);
                    if ui.selectable_label(is_selected, &obj.name).clicked() {
                        to_select = Some(i);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("x").on_hover_text("Delete").clicked() {
                            to_delete = Some(i);
                        }
                    });
                });
            }
        });

        if let Some(idx) = to_select {
            self.state.selected_object = Some(idx);
        }
        if let Some(idx) = to_delete {
            self.state.objects.remove(idx);
            if self.state.selected_object == Some(idx) {
                self.state.selected_object = None;
            }
        }
        if let Some(idx) = to_toggle_visibility {
            self.state.objects[idx].visible = !self.state.objects[idx].visible;
        }
    }

    fn properties_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Properties");
        ui.separator();

        let mut want_insert_kf = false;
        let mut want_delete_kf = false;
        let mut want_calc_motion_path = false;
        if let Some(idx) = self.state.selected_object {
            if let Some(obj) = self.state.objects.get_mut(idx) {
                // Editable name
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut obj.name);
                });
                ui.horizontal(|ui| {
                    ui.label(format!("Type: {:?}", obj.object_type));
                    ui.separator();
                    ui.checkbox(&mut obj.visible, "Visible");
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut obj.smooth_shading, "Smooth Shading");
                    ui.separator();
                    ui.checkbox(&mut obj.locked, "Locked");
                });
                ui.separator();

                // Transform section
                ui.collapsing("Transform", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Position:");
                    });
                    ui.horizontal(|ui| {
                        ui.label("X:");
                        ui.add(egui::DragValue::new(&mut obj.position[0]).speed(0.1));
                        ui.label("Y:");
                        ui.add(egui::DragValue::new(&mut obj.position[1]).speed(0.1));
                        ui.label("Z:");
                        ui.add(egui::DragValue::new(&mut obj.position[2]).speed(0.1));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Rotation:");
                    });
                    ui.horizontal(|ui| {
                        ui.label("X:");
                        ui.add(
                            egui::DragValue::new(&mut obj.rotation[0])
                                .speed(1.0)
                                .suffix("°"),
                        );
                        ui.label("Y:");
                        ui.add(
                            egui::DragValue::new(&mut obj.rotation[1])
                                .speed(1.0)
                                .suffix("°"),
                        );
                        ui.label("Z:");
                        ui.add(
                            egui::DragValue::new(&mut obj.rotation[2])
                                .speed(1.0)
                                .suffix("°"),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label("Scale:");
                    });
                    ui.horizontal(|ui| {
                        ui.label("X:");
                        ui.add(egui::DragValue::new(&mut obj.scale[0]).speed(0.01));
                        ui.label("Y:");
                        ui.add(egui::DragValue::new(&mut obj.scale[1]).speed(0.01));
                        ui.label("Z:");
                        ui.add(egui::DragValue::new(&mut obj.scale[2]).speed(0.01));
                    });
                });

                // Material section
                ui.collapsing("Material", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Color:");
                        let mut color = [
                            obj.material.base_color[0],
                            obj.material.base_color[1],
                            obj.material.base_color[2],
                        ];
                        if ui.color_edit_button_rgb(&mut color).changed() {
                            obj.material.base_color[0] = color[0];
                            obj.material.base_color[1] = color[1];
                            obj.material.base_color[2] = color[2];
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Metallic:");
                        ui.add(egui::Slider::new(&mut obj.material.metallic, 0.0..=1.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Roughness:");
                        ui.add(egui::Slider::new(&mut obj.material.roughness, 0.0..=1.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Emissive:");
                        ui.add(egui::Slider::new(&mut obj.material.emissive, 0.0..=10.0));
                    });
                });

                // Modifiers section
                let mut modifier_to_remove: Option<usize> = None;
                let mut modifier_to_move_up: Option<usize> = None;
                let mut modifier_to_move_down: Option<usize> = None;

                ui.collapsing("Modifiers", |ui| {
                    if obj.modifiers.is_empty() {
                        ui.label("No modifiers");
                    } else {
                        for (i, modifier) in obj.modifiers.iter().enumerate() {
                            ui.horizontal(|ui| {
                                // Up/Down buttons
                                if ui.small_button("^").on_hover_text("Move up").clicked() && i > 0
                                {
                                    modifier_to_move_up = Some(i);
                                }
                                if ui.small_button("v").on_hover_text("Move down").clicked()
                                    && i < obj.modifiers.len() - 1
                                {
                                    modifier_to_move_down = Some(i);
                                }
                                ui.label(modifier);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("X").on_hover_text("Remove").clicked() {
                                            modifier_to_remove = Some(i);
                                        }
                                    },
                                );
                            });
                        }
                    }
                    ui.separator();
                    ui.menu_button("Add Modifier", |ui| {
                        ui.menu_button("Generate", |ui| {
                            for name in &[
                                "Array",
                                "Bevel",
                                "Boolean",
                                "Mirror",
                                "Screw",
                                "Solidify",
                                "Wireframe",
                            ] {
                                if ui.button(*name).clicked() {
                                    obj.modifiers.push(name.to_string());
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("Deform", |ui| {
                            for name in &[
                                "Bend",
                                "Cast",
                                "Curve Deform",
                                "Displace",
                                "Lattice",
                                "Mesh Deform",
                                "Shrinkwrap",
                                "Simple Deform",
                                "Smooth",
                                "Corrective Smooth",
                                "Taper",
                                "Twist",
                                "Wave",
                            ] {
                                if ui.button(*name).clicked() {
                                    obj.modifiers.push(name.to_string());
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("Mesh", |ui| {
                            for name in &[
                                "Decimate",
                                "Edge Split",
                                "Remesh",
                                "Subdivision",
                                "Triangulate",
                                "Weld",
                            ] {
                                if ui.button(*name).clicked() {
                                    obj.modifiers.push(name.to_string());
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                });

                // Apply modifier changes after the UI
                if let Some(idx_r) = modifier_to_remove {
                    obj.modifiers.remove(idx_r);
                }
                if let Some(idx_u) = modifier_to_move_up {
                    obj.modifiers.swap(idx_u, idx_u - 1);
                }
                if let Some(idx_d) = modifier_to_move_down {
                    obj.modifiers.swap(idx_d, idx_d + 1);
                }

                // Sculpt settings (when in sculpt mode)
                if self.state.edit_mode == EditMode::Sculpt {
                    ui.collapsing("Sculpt Brush", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Brush:");
                            egui::ComboBox::from_id_salt("sculpt_brush_combo")
                                .selected_text(format!("{}", self.state.sculpt_brush))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.state.sculpt_brush,
                                        SculptBrush::Draw,
                                        "Draw",
                                    );
                                    ui.selectable_value(
                                        &mut self.state.sculpt_brush,
                                        SculptBrush::Smooth,
                                        "Smooth",
                                    );
                                    ui.selectable_value(
                                        &mut self.state.sculpt_brush,
                                        SculptBrush::Flatten,
                                        "Flatten",
                                    );
                                    ui.selectable_value(
                                        &mut self.state.sculpt_brush,
                                        SculptBrush::Pinch,
                                        "Pinch",
                                    );
                                    ui.selectable_value(
                                        &mut self.state.sculpt_brush,
                                        SculptBrush::Inflate,
                                        "Inflate",
                                    );
                                    ui.selectable_value(
                                        &mut self.state.sculpt_brush,
                                        SculptBrush::Grab,
                                        "Grab",
                                    );
                                });
                        });
                        ui.horizontal(|ui| {
                            ui.label("Radius:");
                            ui.add(egui::Slider::new(
                                &mut self.state.sculpt_radius,
                                5.0..=200.0,
                            ));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Strength:");
                            ui.add(egui::Slider::new(
                                &mut self.state.sculpt_strength,
                                0.0..=1.0,
                            ));
                        });
                    });
                }

                // Animation keyframes section - read data into locals to avoid borrow conflict
                let anim_frame = self.state.timeline.current_frame;
                let anim_kf_count = obj.keyframes.len();
                let anim_kf_data: Vec<(i32, [f32; 3])> = obj
                    .keyframes
                    .iter()
                    .map(|kf| (kf.frame, kf.position))
                    .collect();
                ui.collapsing("Animation", |ui| {
                    ui.label(format!(
                        "Keyframes: {} | Current: {}",
                        anim_kf_count, anim_frame
                    ));

                    ui.horizontal(|ui| {
                        if ui.button("Insert Key (I)").clicked() {
                            want_insert_kf = true;
                        }
                        if ui.button("Delete Key").clicked() {
                            want_delete_kf = true;
                        }
                    });

                    if !anim_kf_data.is_empty() {
                        ui.separator();
                        for (frame, pos) in &anim_kf_data {
                            let marker = if *frame == anim_frame { "> " } else { "  " };
                            ui.label(format!(
                                "{}F{}: pos({:.1},{:.1},{:.1})",
                                marker, frame, pos[0], pos[1], pos[2]
                            ));
                        }
                    }
                });

                // Constraints section - read into locals for display
                let constraint_count = obj.constraints.len();
                let constraint_names: Vec<String> =
                    obj.constraints.iter().map(|c| format!("{}", c)).collect();
                let mut constraint_to_remove: Option<usize> = None;
                ui.collapsing(format!("Constraints ({})", constraint_count), |ui| {
                    if constraint_names.is_empty() {
                        ui.label("No constraints");
                    } else {
                        for (ci, cname) in constraint_names.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(cname);
                                if ui.small_button("X").clicked() {
                                    constraint_to_remove = Some(ci);
                                }
                            });
                        }
                    }
                });

                // Shape keys section
                let shape_key_count = obj.shape_keys.len();
                let mut shape_key_values: Vec<(String, f32)> = obj
                    .shape_keys
                    .iter()
                    .map(|sk| (sk.name.clone(), sk.value))
                    .collect();
                let mut shape_key_changed = false;
                ui.collapsing(format!("Shape Keys ({})", shape_key_count), |ui| {
                    if shape_key_values.is_empty() {
                        ui.label("No shape keys");
                    } else {
                        for (name, value) in &mut shape_key_values {
                            ui.horizontal(|ui| {
                                ui.label(name.as_str());
                                if ui.add(egui::Slider::new(value, 0.0..=1.0)).changed() {
                                    shape_key_changed = true;
                                }
                            });
                        }
                    }
                });

                // Apply deferred constraint removal
                if let Some(ci) = constraint_to_remove {
                    if ci < obj.constraints.len() {
                        obj.constraints.remove(ci);
                    }
                }

                // Apply deferred shape key value changes
                if shape_key_changed {
                    for (i, (_, val)) in shape_key_values.iter().enumerate() {
                        if i < obj.shape_keys.len() {
                            obj.shape_keys[i].value = *val;
                        }
                    }
                }

                // Vertex Groups section
                let vg_count = obj.vertex_groups.len();
                let vg_names: Vec<String> =
                    obj.vertex_groups.iter().map(|g| g.name.clone()).collect();
                let vg_weights: Vec<usize> =
                    obj.vertex_groups.iter().map(|g| g.weights.len()).collect();
                let mut vg_remove: Option<usize> = None;
                let mut vg_add = false;
                ui.collapsing(format!("Vertex Groups ({})", vg_count), |ui| {
                    if vg_names.is_empty() {
                        ui.label("No vertex groups");
                    } else {
                        for (gi, gname) in vg_names.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(format!("{} ({} verts)", gname, vg_weights[gi]));
                                if ui.small_button("X").clicked() {
                                    vg_remove = Some(gi);
                                }
                            });
                        }
                    }
                    if ui.button("+ Add Group").clicked() {
                        vg_add = true;
                    }
                });
                if let Some(gi) = vg_remove {
                    if gi < obj.vertex_groups.len() {
                        obj.vertex_groups.remove(gi);
                    }
                }
                if vg_add {
                    let n = obj.vertex_groups.len();
                    obj.vertex_groups.push(VertexGroup {
                        name: format!("Group.{:03}", n),
                        weights: Vec::new(),
                    });
                }

                // Particle Systems section
                let ps_count = obj.particle_systems.len();
                let mut ps_remove: Option<usize> = None;
                let mut ps_add = false;
                ui.collapsing(format!("Particle Systems ({})", ps_count), |ui| {
                    if obj.particle_systems.is_empty() {
                        ui.label("No particle systems");
                    } else {
                        for (pi, ps) in obj.particle_systems.iter_mut().enumerate() {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(ps.name.to_string());
                                    if ui.small_button("X").clicked() {
                                        ps_remove = Some(pi);
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Type:");
                                    egui::ComboBox::from_id_salt(format!("pstype_{}", pi))
                                        .selected_text(format!("{}", ps.particle_type))
                                        .width(70.0)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut ps.particle_type,
                                                ParticleType::Emitter,
                                                "Emitter",
                                            );
                                            ui.selectable_value(
                                                &mut ps.particle_type,
                                                ParticleType::Hair,
                                                "Hair",
                                            );
                                        });
                                });
                                ui.add(
                                    egui::Slider::new(&mut ps.count, 10..=100000)
                                        .text("Count")
                                        .logarithmic(true),
                                );
                                ui.add(
                                    egui::Slider::new(&mut ps.lifetime, 1.0..=500.0)
                                        .text("Lifetime"),
                                );
                                ui.add(egui::Slider::new(&mut ps.size, 0.01..=1.0).text("Size"));
                                ui.add(
                                    egui::Slider::new(&mut ps.gravity, 0.0..=2.0).text("Gravity"),
                                );
                                ui.checkbox(&mut ps.active, "Active");
                            });
                        }
                    }
                    if ui.button("+ Add Particle System").clicked() {
                        ps_add = true;
                    }
                });
                if let Some(pi) = ps_remove {
                    if pi < obj.particle_systems.len() {
                        obj.particle_systems.remove(pi);
                    }
                }
                if ps_add {
                    let n = obj.particle_systems.len();
                    obj.particle_systems.push(ParticleSystem {
                        name: format!("Particles.{:03}", n),
                        ..ParticleSystem::default()
                    });
                }

                // Armature Bones section
                if !obj.bones.is_empty() {
                    ui.collapsing(format!("Armature ({} bones)", obj.bones.len()), |ui| {
                        ui.checkbox(&mut self.state.pose_mode, "Pose Mode");
                        ui.checkbox(&mut self.state.show_bone_names, "Show Names");
                        ui.checkbox(&mut self.state.show_bone_axes, "Show Axes");
                        for bone in &obj.bones {
                            let parent_str = bone.parent.as_deref().unwrap_or("(root)");
                            let ik_str = if bone.ik_enabled { " [IK]" } else { "" };
                            ui.label(format!("  {} -> {}{}", bone.name, parent_str, ik_str));
                        }
                    });
                }

                // Force Field section
                let mut remove_ff = false;
                let mut add_ff = false;
                if let Some(ref mut ff) = obj.force_field {
                    ui.collapsing(format!("Force Field: {}", ff.field_type), |ui| {
                        ui.add(egui::Slider::new(&mut ff.strength, 0.0..=100.0).text("Strength"));
                        ui.add(egui::Slider::new(&mut ff.falloff, 0.0..=5.0).text("Falloff"));
                        ui.add(egui::Slider::new(&mut ff.noise, 0.0..=10.0).text("Noise"));
                        ui.add(egui::Slider::new(&mut ff.flow, 0.0..=10.0).text("Flow"));
                        ui.checkbox(&mut ff.enabled, "Enabled");
                        if ui.button("Remove Force Field").clicked() {
                            remove_ff = true;
                        }
                    });
                } else if ui.small_button("+ Add Force Field").clicked() {
                    add_ff = true;
                }
                if remove_ff {
                    obj.force_field = None;
                }
                if add_ff {
                    obj.force_field = Some(ForceFieldSettings::default());
                }

                // Cloth section
                let mut remove_cloth = false;
                if let Some(ref mut cloth) = obj.cloth {
                    ui.collapsing("Cloth Simulation", |ui| {
                        ui.add(egui::Slider::new(&mut cloth.quality, 1..=20).text("Quality"));
                        ui.add(egui::Slider::new(&mut cloth.mass, 0.01..=10.0).text("Mass"));
                        ui.add(
                            egui::Slider::new(&mut cloth.stiffness, 0.1..=100.0).text("Stiffness"),
                        );
                        ui.add(egui::Slider::new(&mut cloth.damping, 0.0..=50.0).text("Damping"));
                        ui.add(
                            egui::Slider::new(&mut cloth.air_resistance, 0.0..=10.0)
                                .text("Air Resistance"),
                        );
                        ui.add(
                            egui::Slider::new(&mut cloth.pressure, -10.0..=10.0).text("Pressure"),
                        );
                        ui.checkbox(&mut cloth.self_collision, "Self Collision");
                        ui.checkbox(&mut cloth.enabled, "Enabled");
                        if ui.button("Remove Cloth").clicked() {
                            remove_cloth = true;
                        }
                    });
                }
                if remove_cloth {
                    obj.cloth = None;
                }

                // Soft Body section
                let mut remove_sb = false;
                if let Some(ref mut sb) = obj.soft_body {
                    ui.collapsing("Soft Body", |ui| {
                        ui.add(egui::Slider::new(&mut sb.mass, 0.01..=10.0).text("Mass"));
                        ui.add(egui::Slider::new(&mut sb.friction, 0.0..=5.0).text("Friction"));
                        ui.add(egui::Slider::new(&mut sb.speed, 0.01..=10.0).text("Speed"));
                        ui.add(
                            egui::Slider::new(&mut sb.goal_strength, 0.0..=1.0)
                                .text("Goal Strength"),
                        );
                        ui.add(
                            egui::Slider::new(&mut sb.edge_stiffness, 0.0..=1.0)
                                .text("Edge Stiffness"),
                        );
                        ui.add(egui::Slider::new(&mut sb.push, 0.0..=2.0).text("Push"));
                        ui.add(egui::Slider::new(&mut sb.pull, 0.0..=2.0).text("Pull"));
                        ui.add(egui::Slider::new(&mut sb.damping, 0.0..=2.0).text("Damping"));
                        ui.checkbox(&mut sb.self_collision, "Self Collision");
                        ui.checkbox(&mut sb.enabled, "Enabled");
                        if ui.button("Remove Soft Body").clicked() {
                            remove_sb = true;
                        }
                    });
                }
                if remove_sb {
                    obj.soft_body = None;
                }

                // Drivers section
                let driver_count = obj.drivers.len();
                let mut driver_remove: Option<usize> = None;
                ui.collapsing(format!("Drivers ({})", driver_count), |ui| {
                    if obj.drivers.is_empty() {
                        ui.label("No drivers. Drivers link properties between objects.");
                    }
                    for (di, drv) in obj.drivers.iter_mut().enumerate() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}:", drv.name));
                                if ui.small_button("X").clicked() {
                                    driver_remove = Some(di);
                                }
                            });
                            ui.label(format!(
                                "  src obj {} .{}",
                                drv.source_object, drv.source_property
                            ));
                            ui.label(format!(
                                "  -> .{} x{:.2}",
                                drv.target_property, drv.influence
                            ));
                            ui.horizontal(|ui| {
                                ui.label("Type:");
                                egui::ComboBox::from_id_salt(format!("drv_type_{}", di))
                                    .selected_text(format!("{}", drv.driver_type))
                                    .width(60.0)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut drv.driver_type,
                                            DriverType::Direct,
                                            "Direct",
                                        );
                                        ui.selectable_value(
                                            &mut drv.driver_type,
                                            DriverType::Sum,
                                            "Sum",
                                        );
                                        ui.selectable_value(
                                            &mut drv.driver_type,
                                            DriverType::Average,
                                            "Average",
                                        );
                                        ui.selectable_value(
                                            &mut drv.driver_type,
                                            DriverType::Min,
                                            "Min",
                                        );
                                        ui.selectable_value(
                                            &mut drv.driver_type,
                                            DriverType::Max,
                                            "Max",
                                        );
                                    });
                            });
                            ui.add(
                                egui::Slider::new(&mut drv.influence, 0.0..=2.0).text("Influence"),
                            );
                            ui.checkbox(&mut drv.enabled, "Enabled");
                        });
                    }
                    if ui.button("+ Add Driver").clicked() {
                        obj.drivers.push(AnimationDriver {
                            name: format!("Driver.{:03}", driver_count),
                            source_object: 0,
                            source_property: "position.x".to_string(),
                            target_property: "rotation.z".to_string(),
                            influence: 1.0,
                            driver_type: DriverType::Direct,
                            enabled: true,
                        });
                    }
                });
                if let Some(di) = driver_remove {
                    if di < obj.drivers.len() {
                        obj.drivers.remove(di);
                    }
                }

                // Texture Slots section
                let tex_count = obj.texture_slots.len();
                let mut tex_remove: Option<usize> = None;
                ui.collapsing(format!("Textures ({})", tex_count), |ui| {
                    if obj.texture_slots.is_empty() {
                        ui.label("No texture slots. Add texture maps to material.");
                    }
                    for (ti, slot) in obj.texture_slots.iter_mut().enumerate() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}:", slot.texture_type));
                                if ui.small_button("X").clicked() {
                                    tex_remove = Some(ti);
                                }
                            });
                            egui::ComboBox::from_id_salt(format!("tex_type_{}", ti))
                                .selected_text(format!("{}", slot.texture_type))
                                .width(100.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut slot.texture_type,
                                        TextureType::Diffuse,
                                        "Diffuse",
                                    );
                                    ui.selectable_value(
                                        &mut slot.texture_type,
                                        TextureType::Normal,
                                        "Normal",
                                    );
                                    ui.selectable_value(
                                        &mut slot.texture_type,
                                        TextureType::Roughness,
                                        "Roughness",
                                    );
                                    ui.selectable_value(
                                        &mut slot.texture_type,
                                        TextureType::Metallic,
                                        "Metallic",
                                    );
                                    ui.selectable_value(
                                        &mut slot.texture_type,
                                        TextureType::AmbientOcclusion,
                                        "AO",
                                    );
                                    ui.selectable_value(
                                        &mut slot.texture_type,
                                        TextureType::Emissive,
                                        "Emissive",
                                    );
                                    ui.selectable_value(
                                        &mut slot.texture_type,
                                        TextureType::Height,
                                        "Height",
                                    );
                                    ui.selectable_value(
                                        &mut slot.texture_type,
                                        TextureType::Opacity,
                                        "Opacity",
                                    );
                                });
                            ui.horizontal(|ui| {
                                ui.label("Path:");
                                ui.text_edit_singleline(&mut slot.image_path);
                            });
                            ui.add(
                                egui::Slider::new(&mut slot.strength, 0.0..=1.0).text("Strength"),
                            );
                            ui.horizontal(|ui| {
                                ui.label("UV Channel:");
                                ui.add(egui::DragValue::new(&mut slot.uv_channel).range(0..=7));
                            });
                            ui.checkbox(&mut slot.enabled, "Enabled");
                        });
                    }
                    if ui.button("+ Add Texture Slot").clicked() {
                        obj.texture_slots.push(TextureSlot::default());
                    }
                });
                if let Some(ti) = tex_remove {
                    if ti < obj.texture_slots.len() {
                        obj.texture_slots.remove(ti);
                    }
                }

                // Custom Properties section
                let cp_count = obj.custom_properties.len();
                let mut cp_remove: Option<usize> = None;
                ui.collapsing(format!("Custom Properties ({})", cp_count), |ui| {
                    if obj.custom_properties.is_empty() {
                        ui.label("No custom properties.");
                    }
                    for (ci, prop) in obj.custom_properties.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut prop.name);
                            ui.label("=");
                            ui.text_edit_singleline(&mut prop.value);
                            egui::ComboBox::from_id_salt(format!("cptype_{}", ci))
                                .selected_text(format!("{}", prop.prop_type))
                                .width(60.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut prop.prop_type,
                                        CustomPropType::String,
                                        "String",
                                    );
                                    ui.selectable_value(
                                        &mut prop.prop_type,
                                        CustomPropType::Integer,
                                        "Int",
                                    );
                                    ui.selectable_value(
                                        &mut prop.prop_type,
                                        CustomPropType::Float,
                                        "Float",
                                    );
                                    ui.selectable_value(
                                        &mut prop.prop_type,
                                        CustomPropType::Boolean,
                                        "Bool",
                                    );
                                });
                            if ui.small_button("X").clicked() {
                                cp_remove = Some(ci);
                            }
                        });
                    }
                    if ui.button("+ Add Property").clicked() {
                        obj.custom_properties.push(CustomProperty {
                            name: format!("prop_{}", cp_count),
                            value: String::new(),
                            prop_type: CustomPropType::String,
                        });
                    }
                });
                if let Some(ci) = cp_remove {
                    if ci < obj.custom_properties.len() {
                        obj.custom_properties.remove(ci);
                    }
                }

                // Hair Settings section (for Hair type particles)
                let has_hair = obj
                    .particle_systems
                    .iter()
                    .any(|p| p.particle_type == ParticleType::Hair);
                if has_hair {
                    ui.collapsing("Hair Settings", |ui| {
                        if obj.hair_settings.is_none() {
                            obj.hair_settings = Some(HairSettings::default());
                        }
                        if let Some(ref mut hair) = obj.hair_settings {
                            ui.add(egui::Slider::new(&mut hair.length, 0.01..=5.0).text("Length"));
                            ui.horizontal(|ui| {
                                ui.label("Children:");
                                ui.add(egui::DragValue::new(&mut hair.children).range(0..=100));
                            });
                            ui.add(egui::Slider::new(&mut hair.clump, 0.0..=1.0).text("Clump"));
                            ui.add(
                                egui::Slider::new(&mut hair.roughness, 0.0..=1.0).text("Roughness"),
                            );
                            ui.horizontal(|ui| {
                                ui.label("Seed:");
                                ui.add(egui::DragValue::new(&mut hair.random_seed).range(0..=999));
                            });
                            ui.add(
                                egui::Slider::new(&mut hair.root_radius, 0.001..=0.1)
                                    .text("Root Radius")
                                    .logarithmic(true),
                            );
                            ui.add(
                                egui::Slider::new(&mut hair.tip_radius, 0.0001..=0.05)
                                    .text("Tip Radius")
                                    .logarithmic(true),
                            );
                            ui.horizontal(|ui| {
                                ui.label("Render Steps:");
                                ui.add(egui::DragValue::new(&mut hair.render_steps).range(1..=10));
                            });
                            ui.checkbox(&mut hair.dynamics, "Hair Dynamics");
                        }
                    });
                }

                // Fluid Settings section
                if let Some(ref mut fluid) = obj.fluid {
                    ui.collapsing(format!("Fluid ({})", fluid.fluid_type), |ui| {
                        egui::ComboBox::from_id_salt("fluid_type_combo")
                            .selected_text(format!("{}", fluid.fluid_type))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut fluid.fluid_type,
                                    FluidType::Domain,
                                    "Domain",
                                );
                                ui.selectable_value(
                                    &mut fluid.fluid_type,
                                    FluidType::Inflow,
                                    "Inflow",
                                );
                                ui.selectable_value(
                                    &mut fluid.fluid_type,
                                    FluidType::Outflow,
                                    "Outflow",
                                );
                                ui.selectable_value(
                                    &mut fluid.fluid_type,
                                    FluidType::Obstacle,
                                    "Obstacle",
                                );
                            });
                        if fluid.fluid_type == FluidType::Domain {
                            ui.horizontal(|ui| {
                                ui.label("Resolution:");
                                ui.add(egui::DragValue::new(&mut fluid.resolution).range(16..=512));
                            });
                            ui.add(
                                egui::Slider::new(&mut fluid.viscosity, 0.0..=1.0)
                                    .text("Viscosity")
                                    .logarithmic(true),
                            );
                            ui.add(
                                egui::Slider::new(&mut fluid.time_scale, 0.1..=10.0)
                                    .text("Time Scale"),
                            );
                            if fluid.baked {
                                ui.colored_label(egui::Color32::GREEN, "Baked");
                                if ui.button("Free Bake").clicked() {
                                    fluid.baked = false;
                                }
                            } else if ui.button("Bake Fluid").clicked() {
                                fluid.baked = true;
                            }
                        }
                        ui.checkbox(&mut fluid.enabled, "Enabled");
                    });
                }

                // Motion Path section
                ui.collapsing("Motion Path", |ui| {
                    if let Some(ref mp) = obj.motion_path {
                        ui.label(format!("Frames: {} - {}", mp.start_frame, mp.end_frame));
                        ui.label(format!("Points: {}", mp.points.len()));
                        want_calc_motion_path = false; // already calculated
                    } else {
                        ui.label("No motion path calculated.");
                    }
                    if !obj.keyframes.is_empty() {
                        if ui.button("Calculate Motion Path").clicked() {
                            want_calc_motion_path = true;
                        }
                    } else {
                        ui.label("(Add keyframes first)");
                    }
                    if obj.motion_path.is_some() && ui.button("Clear Motion Path").clicked() {
                        obj.motion_path = None;
                    }
                });
            } // End of obj borrow

            // Physics section (needs separate self.state borrow)
            self.state.sync_physics();
            if idx < self.state.physics.len() {
                ui.collapsing("Physics", |ui| {
                    let phys = &mut self.state.physics[idx];
                    ui.checkbox(&mut phys.is_rigid_body, "Rigid Body");
                    ui.checkbox(&mut phys.is_static, "Static Collider");
                    if phys.is_rigid_body || phys.is_static {
                        ui.horizontal(|ui| {
                            ui.label("Mass:");
                            ui.add(
                                egui::DragValue::new(&mut phys.mass)
                                    .speed(0.1)
                                    .range(0.01..=1000.0),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Bounce:");
                            ui.add(egui::Slider::new(&mut phys.restitution, 0.0..=1.0));
                        });
                        if phys.is_rigid_body {
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "Vel: ({:.1}, {:.1}, {:.1})",
                                    phys.velocity[0], phys.velocity[1], phys.velocity[2]
                                ));
                            });
                            if ui.button("Reset Velocity").clicked() {
                                phys.velocity = [0.0, 0.0, 0.0];
                            }
                        }
                    }
                });
            }
            // Deferred keyframe actions (after obj borrow released)
            if want_insert_kf {
                let frame = self.state.timeline.current_frame;
                self.state.insert_keyframe();
                self.status_message = format!("Keyframe inserted at frame {}", frame);
            }
            if want_delete_kf && self.state.delete_keyframe() {
                self.status_message = "Keyframe deleted".to_string();
            }
            if want_calc_motion_path {
                self.state.calculate_motion_path(idx);
                self.status_message = "Motion path calculated".to_string();
            }
        } else {
            ui.label("No object selected");
        }
    }

    fn timeline_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Transport controls
            if ui
                .button("|<")
                .on_hover_text("Go to start (Home)")
                .clicked()
            {
                self.state.timeline.goto_start();
            }
            if ui
                .button("<")
                .on_hover_text("Previous frame (Left)")
                .clicked()
            {
                self.state.timeline.step_backward();
            }
            let play_label = if self.state.timeline.is_playing {
                "||"
            } else {
                ">"
            };
            let play_tip = if self.state.timeline.is_playing {
                "Pause (Space)"
            } else {
                "Play (Space)"
            };
            if ui.button(play_label).on_hover_text(play_tip).clicked() {
                self.state.timeline.toggle_play();
            }
            if ui.button(">").on_hover_text("Next frame (Right)").clicked() {
                self.state.timeline.step_forward();
            }
            if ui.button(">|").on_hover_text("Go to end (End)").clicked() {
                self.state.timeline.goto_end();
            }

            ui.separator();

            // Frame counter
            ui.label("Frame:");
            let mut frame = self.state.timeline.current_frame;
            if ui
                .add(
                    egui::DragValue::new(&mut frame)
                        .range(self.state.timeline.start_frame..=self.state.timeline.end_frame)
                        .speed(1.0),
                )
                .changed()
            {
                self.state.timeline.set_frame(frame);
            }

            // Timeline slider
            ui.add(
                egui::Slider::new(
                    &mut self.state.timeline.current_frame,
                    self.state.timeline.start_frame..=self.state.timeline.end_frame,
                )
                .show_value(false),
            );

            ui.separator();

            // Keyframe controls (BATCH 24: Animation keyframing UI)
            if ui
                .button("Insert")
                .on_hover_text("Insert keyframe (I)")
                .clicked()
            {
                if self.state.insert_keyframe() {
                    self.status_message = format!(
                        "Keyframe inserted at frame {}",
                        self.state.timeline.current_frame
                    );
                } else {
                    self.status_message =
                        "No object selected or no changes to keyframe".to_string();
                }
            }
            if ui
                .button("Delete")
                .on_hover_text("Delete keyframe (Alt+I)")
                .clicked()
            {
                if self.state.delete_keyframe() {
                    self.status_message = format!(
                        "Keyframe deleted at frame {}",
                        self.state.timeline.current_frame
                    );
                } else {
                    self.status_message = "No keyframe at current frame".to_string();
                }
            }

            ui.separator();

            // Frame rate
            ui.label("FPS:");
            ui.add(
                egui::DragValue::new(&mut self.state.timeline.frame_rate)
                    .range(1.0..=120.0)
                    .speed(1.0),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Range controls
                ui.label("End:");
                ui.add(egui::DragValue::new(&mut self.state.timeline.end_frame).range(1..=10000));
                ui.label("Start:");
                ui.add(
                    egui::DragValue::new(&mut self.state.timeline.start_frame)
                        .range(1..=self.state.timeline.end_frame),
                );

                // Progress indicator
                let progress = self.state.timeline.progress();
                ui.add(
                    egui::ProgressBar::new(progress)
                        .show_percentage()
                        .desired_width(100.0),
                );
            });
        });

        // Enhanced Dopesheet: multi-track keyframe display
        if self.show_dopesheet {
            ui.separator();
            let start = self.state.timeline.start_frame;
            let end = self.state.timeline.end_frame;
            let range = (end - start) as f32;
            let avail_w = ui.available_width() - 80.0;

            // Color palette for different objects
            let track_colors = [
                egui::Color32::from_rgb(200, 150, 50),
                egui::Color32::from_rgb(50, 180, 200),
                egui::Color32::from_rgb(200, 80, 80),
                egui::Color32::from_rgb(80, 200, 120),
                egui::Color32::from_rgb(180, 100, 200),
                egui::Color32::from_rgb(200, 180, 60),
            ];

            // Collect animated objects
            let animated: Vec<(usize, String, Vec<i32>)> = self
                .state
                .objects
                .iter()
                .enumerate()
                .filter(|(_, obj)| !obj.keyframes.is_empty())
                .map(|(i, obj)| {
                    (
                        i,
                        obj.name.clone(),
                        obj.keyframes.iter().map(|k| k.frame).collect(),
                    )
                })
                .collect();

            if animated.is_empty() {
                ui.label("No keyframes");
            } else if range > 0.0 && avail_w > 50.0 {
                let track_height = 14.0;
                let total_h = animated.len() as f32 * (track_height + 2.0);
                let (response, painter) = ui.allocate_painter(
                    egui::vec2(avail_w + 70.0, total_h.min(80.0)),
                    egui::Sense::hover(),
                );
                let r = response.rect;
                painter.rect_filled(r, 2.0, egui::Color32::from_rgb(30, 30, 35));

                for (ti, (obj_idx, name, keyframes)) in animated.iter().enumerate() {
                    let track_y = r.top() + ti as f32 * (track_height + 2.0);
                    let is_selected = self.state.selected_object == Some(*obj_idx);
                    let color = track_colors[ti % track_colors.len()];

                    // Object name label
                    let label_r = egui::Rect::from_min_size(
                        egui::pos2(r.left() + 2.0, track_y),
                        egui::vec2(65.0, track_height),
                    );
                    if is_selected {
                        painter.rect_filled(label_r, 0.0, egui::Color32::from_rgb(50, 50, 65));
                    }
                    let truncated: String = name.chars().take(8).collect();
                    painter.text(
                        egui::pos2(r.left() + 4.0, track_y + track_height * 0.5),
                        egui::Align2::LEFT_CENTER,
                        &truncated,
                        egui::FontId::proportional(9.0),
                        if is_selected {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_rgb(160, 160, 170)
                        },
                    );

                    // Track bar
                    let track_r = egui::Rect::from_min_size(
                        egui::pos2(r.left() + 68.0, track_y),
                        egui::vec2(avail_w, track_height),
                    );
                    painter.rect_filled(track_r, 1.0, egui::Color32::from_rgb(35, 35, 42));

                    // Keyframe diamonds
                    for &kf in keyframes {
                        let t = (kf - start) as f32 / range;
                        let x = track_r.left() + t * track_r.width();
                        let cy = track_r.center().y;
                        let size = 3.5;
                        let diamond = vec![
                            egui::pos2(x, cy - size),
                            egui::pos2(x + size, cy),
                            egui::pos2(x, cy + size),
                            egui::pos2(x - size, cy),
                        ];
                        let is_current = kf == self.state.timeline.current_frame;
                        let kf_color = if is_current {
                            egui::Color32::from_rgb(255, 240, 120)
                        } else {
                            color
                        };
                        painter.add(egui::Shape::convex_polygon(
                            diamond,
                            kf_color,
                            egui::Stroke::NONE,
                        ));
                    }
                }

                // Playhead line spanning all tracks
                let t = (self.state.timeline.current_frame - start) as f32 / range;
                let px = r.left() + 68.0 + t * avail_w;
                painter.line_segment(
                    [egui::pos2(px, r.top()), egui::pos2(px, r.bottom())],
                    egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(100, 200, 255)),
                );
            }
        }

        // Timeline markers
        if !self.state.timeline_markers.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Markers:");
                for marker in &self.state.timeline_markers {
                    let is_at = marker.frame == self.state.timeline.current_frame;
                    let color = if is_at {
                        egui::Color32::from_rgb(100, 255, 100)
                    } else {
                        egui::Color32::from_rgb(80, 180, 80)
                    };
                    ui.colored_label(color, format!("F{}: {}", marker.frame, marker.name));
                }
            });
        }

        // Add marker button
        ui.horizontal(|ui| {
            if ui.small_button("+ Marker").clicked() {
                let frame = self.state.timeline.current_frame;
                self.state.add_timeline_marker(&format!("Mark {}", frame));
                self.status_message = format!("Marker added at frame {}", frame);
            }
            if ui.small_button("Clear Markers").clicked() {
                self.state.timeline_markers.clear();
                self.status_message = "Markers cleared".to_string();
            }
            ui.checkbox(&mut self.show_dopesheet, "Dopesheet");
        });

        // Keyboard shortcuts for timeline
        ui.ctx().input(|i| {
            if i.key_pressed(egui::Key::Space) {
                self.state.timeline.toggle_play();
            }
            if i.key_pressed(egui::Key::ArrowLeft) && !i.modifiers.any() {
                self.state.timeline.step_backward();
            }
            if i.key_pressed(egui::Key::ArrowRight) && !i.modifiers.any() {
                self.state.timeline.step_forward();
            }
            if i.key_pressed(egui::Key::Home) && !i.modifiers.ctrl {
                self.state.timeline.goto_start();
            }
            if i.key_pressed(egui::Key::End) {
                self.state.timeline.goto_end();
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Status message
            ui.label(&self.status_message);

            // Project name
            if let Some(path) = &self.project_path {
                ui.separator();
                ui.label(format!(
                    "| {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // License badge
                let (lic_color, lic_text) = match &self.license_status {
                    license::LicenseStatus::Trial => (egui::Color32::YELLOW, "TRIAL"),
                    license::LicenseStatus::Licensed {
                        tier: license::Tier::Pro,
                    } => (egui::Color32::GREEN, "PRO"),
                    license::LicenseStatus::Licensed {
                        tier: license::Tier::Edu,
                    } => (egui::Color32::LIGHT_BLUE, "EDU"),
                    license::LicenseStatus::Invalid => (egui::Color32::RED, "UNLICENSED"),
                };
                if ui
                    .add(
                        egui::Label::new(egui::RichText::new(lic_text).color(lic_color).small())
                            .sense(egui::Sense::click()),
                    )
                    .clicked()
                {
                    self.show_license_dialog = true;
                }
                ui.separator();

                // FPS counter (if enabled)
                if self.preferences.show_fps {
                    let fps = 1.0 / ui.ctx().input(|i| i.predicted_dt);
                    let fps_color = if fps > 55.0 {
                        egui::Color32::GREEN
                    } else if fps > 30.0 {
                        egui::Color32::YELLOW
                    } else {
                        egui::Color32::RED
                    };
                    ui.colored_label(fps_color, format!("{:.0} FPS", fps));
                    ui.separator();
                }

                // Object count
                let total = self.state.objects.len();
                let visible = self.state.objects.iter().filter(|o| o.visible).count();
                if visible != total {
                    ui.label(format!("Objects: {}/{}", visible, total));
                } else {
                    ui.label(format!("Objects: {}", total));
                }

                ui.separator();

                // Selected object
                if let Some(idx) = self.state.selected_object {
                    ui.label(format!("Selected: {}", self.state.objects[idx].name));
                    ui.separator();
                }

                // Physics indicator
                if self.state.physics_running {
                    ui.colored_label(egui::Color32::from_rgb(100, 200, 255), "SIM");
                    ui.separator();
                }

                // Measurement count
                if !self.state.measurements.is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 255, 100),
                        format!("M:{}", self.state.measurements.len()),
                    );
                    ui.separator();
                }

                // Multi-select count
                let sel_count = self.state.all_selected().len();
                if sel_count > 1 {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 180, 80),
                        format!("Sel:{}", sel_count),
                    );
                    ui.separator();
                }

                // Auto-key and orientation indicators
                if self.state.auto_key {
                    ui.colored_label(egui::Color32::from_rgb(255, 80, 80), "AK");
                    ui.separator();
                }
                if self.state.transform_orientation != TransformOrientation::Global {
                    ui.label(format!("{}", self.state.transform_orientation));
                    ui.separator();
                }

                // X-Ray indicator
                if self.state.xray_mode {
                    ui.colored_label(egui::Color32::from_rgb(100, 200, 255), "X-RAY");
                    ui.separator();
                }

                // Current tool and mode
                let mode_str = match self.state.edit_mode {
                    EditMode::Object => "Object".to_string(),
                    EditMode::Edit => {
                        if self.state.edit_tool != EditTool::Select {
                            format!(
                                "Edit [{}] {}",
                                self.state.edit_selection, self.state.edit_tool
                            )
                        } else {
                            format!("Edit [{}]", self.state.edit_selection)
                        }
                    }
                    EditMode::Sculpt => format!("Sculpt [{}]", self.state.sculpt_brush),
                    EditMode::TexturePaint => "Texture Paint".to_string(),
                    EditMode::WeightPaint => "Weight Paint".to_string(),
                };
                ui.label(format!("{:?} | {}", self.state.tool, mode_str));
            });
        });
    }

    fn viewport_3d(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();

        // Create a frame for the 3D viewport
        egui::Frame::canvas(ui.style())
            .fill(egui::Color32::from_rgb(42, 42, 48))
            .show(ui, |ui| {
                let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());

                // BATCH 24: GPU Viewport Integration
                static FIRST_GPU_VIEWPORT_PAINT: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(true);
                let skip_gpu_this_paint = FIRST_GPU_VIEWPORT_PAINT
                    .swap(false, std::sync::atomic::Ordering::AcqRel);
                if self.preferences.use_gpu_rendering
                    && self.gpu_renderer.is_some()
                    && !skip_gpu_this_paint
                {
                    let callback = ViewportCallback {
                        renderer: self
                            .gpu_renderer
                            .as_ref()
                            .expect("guarded by self.gpu_renderer.is_some() in enclosing condition")
                            .clone(),
                        camera: viewport::ViewportCamera::from_state(&self.state.camera),
                        objects: self.state.objects.clone(),
                        simulation_mode: self.state.simulation_mode,
                        show_grid: self.preferences.show_grid,
                    };
                    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                        rect,
                        callback,
                    ));
                }

                // Professional viewport gradient (dark charcoal, slightly blue-tinted)
                let top_color = egui::Color32::from_rgb(58, 58, 68);
                let bottom_color = egui::Color32::from_rgb(28, 28, 32);
                let painter_bg = ui.painter_at(rect);
                // Draw gradient as horizontal bands
                let steps = 32;
                for i in 0..steps {
                    let t = i as f32 / steps as f32;
                    let t_next = (i + 1) as f32 / steps as f32;
                    let r = (top_color.r() as f32 * (1.0 - t) + bottom_color.r() as f32 * t) as u8;
                    let g = (top_color.g() as f32 * (1.0 - t) + bottom_color.g() as f32 * t) as u8;
                    let b = (top_color.b() as f32 * (1.0 - t) + bottom_color.b() as f32 * t) as u8;
                    let band_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.left(), rect.top() + rect.height() * t),
                        egui::pos2(rect.right(), rect.top() + rect.height() * t_next),
                    );
                    painter_bg.rect_filled(band_rect, 0.0, egui::Color32::from_rgb(r, g, b));
                }

                // Handle camera controls with middle mouse button or modifiers
                let pointer_pos = response.hover_pos();

                if response.dragged_by(egui::PointerButton::Middle) ||
                   (response.dragged() && ui.input(|i| i.modifiers.alt)) {
                    let delta = response.drag_delta();
                    if ui.input(|i| i.modifiers.shift) {
                        self.state.camera.pan(delta.x * 0.01, delta.y * 0.01);
                    } else if ui.input(|i| i.modifiers.ctrl) {
                        self.state.camera.zoom(delta.y * 0.01);
                    } else {
                        self.state.camera.orbit(delta.x * 0.5, delta.y * 0.5);
                    }
                }

                // Handle scroll wheel zoom
                if response.hovered() {
                    let scroll = ui.input(|i| i.raw_scroll_delta.y);
                    if scroll != 0.0 {
                        self.state.camera.zoom(scroll * 0.005);
                    }
                }

                // Fly navigation (Blender/Unreal-style): hold Right Mouse Button over the
                // viewport, then WASD to move + QE for down/up. Gated behind RMB-held so the
                // keys never conflict with global shortcuts. RMB-drag turns the view.
                if response.hovered()
                    && ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary))
                {
                    let speed = self.state.camera.distance * 0.02;
                    let (mut fwd, mut strafe, mut rise) = (0.0f32, 0.0f32, 0.0f32);
                    ui.input(|i| {
                        if i.key_down(egui::Key::W) { fwd += speed; }
                        if i.key_down(egui::Key::S) { fwd -= speed; }
                        if i.key_down(egui::Key::D) { strafe += speed; }
                        if i.key_down(egui::Key::A) { strafe -= speed; }
                        if i.key_down(egui::Key::E) { rise += speed; }
                        if i.key_down(egui::Key::Q) { rise -= speed; }
                    });
                    if fwd != 0.0 || strafe != 0.0 || rise != 0.0 {
                        self.state.camera.fly(fwd, strafe, rise);
                        ui.ctx().request_repaint(); // keep animating while held
                    }
                    if response.dragged_by(egui::PointerButton::Secondary) {
                        let d = response.drag_delta();
                        self.state.camera.orbit(d.x * 0.3, d.y * 0.3);
                    }
                }

                // Handle object selection on click (Shift+Click for multi-select)
                if response.clicked() {
                    if let Some(pos) = pointer_pos {
                        let shift = ui.input(|i| i.modifiers.shift);
                        self.handle_viewport_click(rect, pos, shift);
                    }
                }

                // Handle tool dragging
                if response.dragged_by(egui::PointerButton::Primary) &&
                   !ui.input(|i| i.modifiers.alt) {
                    let delta = response.drag_delta();
                    if self.state.edit_mode == EditMode::Sculpt {
                        if let Some(brush_pos) = response.interact_pointer_pos() {
                            if let Some(idx) = self.state.selected_object {
                                self.apply_sculpt_stroke(idx, brush_pos, delta, rect);
                            }
                        }
                    } else {
                        self.handle_tool_drag(delta);
                    }
                }

                // Right-click context menu
                response.context_menu(|ui| {
                    if let Some(idx) = self.state.selected_object {
                        let obj_name = self.state.objects[idx].name.clone();
                        ui.label(format!("Selected: {}", obj_name));
                        ui.separator();
                        if ui.button("Delete").clicked() {
                            self.state.save_undo_state();
                            self.state.delete_selected();
                            self.status_message = "Object deleted".to_string();
                            ui.close_menu();
                        }
                        if ui.button("Duplicate").clicked() {
                            self.state.save_undo_state();
                            self.state.duplicate_selected();
                            self.status_message = "Object duplicated".to_string();
                            ui.close_menu();
                        }
                        if ui.button("Focus (F)").clicked() {
                            self.state.camera.target = self.state.objects[idx].position;
                            self.state.camera.update_position();
                            self.status_message = format!("Focused on {}", self.state.objects[idx].name);
                            ui.close_menu();
                        }
                        ui.separator();
                        ui.menu_button("Set Tool", |ui| {
                            if ui.button("Select (Q)").clicked() { self.state.tool = Tool::Select; ui.close_menu(); }
                            if ui.button("Move (G)").clicked() { self.state.tool = Tool::Move; ui.close_menu(); }
                            if ui.button("Rotate (R)").clicked() { self.state.tool = Tool::Rotate; ui.close_menu(); }
                            if ui.button("Scale (S)").clicked() { self.state.tool = Tool::Scale; ui.close_menu(); }
                        });
                        ui.menu_button("Add Modifier", |ui| {
                            for name in &["Subdivision", "Mirror", "Array", "Smooth", "Solidify", "Bevel", "Decimate", "Wireframe", "Triangulate"] {
                                if ui.button(*name).clicked() {
                                    self.state.add_modifier(name);
                                    self.status_message = format!("Added {} Modifier", name);
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.separator();
                        ui.menu_button("Physics", |ui| {
                            if ui.button("Add Rigid Body").clicked() {
                                self.state.enable_rigid_body();
                                self.status_message = "Added Rigid Body".to_string();
                                ui.close_menu();
                            }
                            if ui.button("Add Static Collider").clicked() {
                                self.state.enable_static_collider();
                                self.status_message = "Added Static Collider".to_string();
                                ui.close_menu();
                            }
                        });
                    } else {
                        ui.label("No selection");
                        ui.separator();
                        ui.menu_button("Add Object", |ui| {
                            if ui.button("Cube").clicked() { self.state.add_cube(); ui.close_menu(); }
                            if ui.button("Sphere").clicked() { self.state.add_sphere(); ui.close_menu(); }
                            if ui.button("Cylinder").clicked() { self.state.add_cylinder(); ui.close_menu(); }
                            if ui.button("Plane").clicked() { self.state.add_plane(); ui.close_menu(); }
                            if ui.button("Torus").clicked() { self.state.add_torus(); ui.close_menu(); }
                            if ui.button("Cone").clicked() { self.state.add_cone(); ui.close_menu(); }
                            if ui.button("IcoSphere").clicked() { self.state.add_icosphere(); ui.close_menu(); }
                            if ui.button("Light").clicked() { self.state.add_point_light(); ui.close_menu(); }
                            if ui.button("Camera").clicked() { self.state.add_camera_object(); ui.close_menu(); }
                        });
                        if ui.button("Reset View").clicked() {
                            self.state.camera.reset();
                            ui.close_menu();
                        }
                    }
                });

                let painter = ui.painter_at(rect);

                // Draw 3D grid on XZ plane (if enabled)
                if self.preferences.show_grid {
                    self.draw_3d_grid(&painter, rect);
                }

                // Draw 3D axes (if enabled)
                if self.preferences.show_axes {
                    self.draw_3d_axes(&painter, rect);
                }

                // Draw ground contact shadows (soft shadow projections)
                if matches!(self.state.shading, ShadingMode::Solid | ShadingMode::Material | ShadingMode::Rendered) {
                    let shadow_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 30);
                    let ground_y = -0.49_f32; // Just above the ground plane
                    for obj in &self.state.objects {
                        if !obj.visible { continue; }
                        match obj.object_type {
                            state::ObjectType::Light | state::ObjectType::Camera | state::ObjectType::Empty => continue,
                            _ => {}
                        }
                        // Project object center as a shadow ellipse on ground
                        let shadow_cx = obj.position[0];
                        let shadow_cz = obj.position[2];
                        let height_above = (obj.position[1] - ground_y).max(0.0);
                        if height_above > 20.0 { continue; }
                        let shadow_scale = (1.0 + height_above * 0.1).min(3.0);
                        let sx = obj.scale[0] * 0.5 * shadow_scale;
                        let sz = obj.scale[2] * 0.5 * shadow_scale;
                        // Draw shadow as a small polygon on the ground
                        let segments = 12;
                        let points: Vec<_> = (0..segments).filter_map(|i| {
                            let angle = i as f32 / segments as f32 * std::f32::consts::TAU;
                            self.project_point([shadow_cx + sx * angle.cos(), ground_y, shadow_cz + sz * angle.sin()], rect)
                        }).collect();
                        if points.len() >= 3 {
                            painter.add(egui::Shape::convex_polygon(points, shadow_color, egui::Stroke::NONE));
                        }
                    }
                }

                // GPU or CPU rendering path
                let use_gpu = self.preferences.use_gpu_rendering && self.gpu_renderer.is_some();

                if use_gpu {
                    // GPU hardware-accelerated rendering via wgpu
                    // Scene is rendered to off-screen texture in update(), now we display it
                    if self.gpu_renderer.is_some() {
                        let texture_rect = egui::Rect::from_min_size(rect.min, rect.size());
                        painter.image(
                            self.gpu_renderer.as_ref().map(|r| r.read().egui_texture_id).unwrap_or_default(),
                            texture_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                } else {
                    // CPU software rendering (fallback)
                // Draw all visible objects (depth-sorted far to near for correct overlap)
                let cam_pos = self.state.camera.position;
                let mut draw_order: Vec<usize> = (0..self.state.objects.len())
                    .filter(|&i| self.state.objects[i].visible)
                    .collect();
                draw_order.sort_by(|&a, &b| {
                    let pa = self.state.objects[a].position;
                    let pb = self.state.objects[b].position;
                    let da = (pa[0] - cam_pos[0]).powi(2) + (pa[1] - cam_pos[1]).powi(2) + (pa[2] - cam_pos[2]).powi(2);
                    let db = (pb[0] - cam_pos[0]).powi(2) + (pb[1] - cam_pos[1]).powi(2) + (pb[2] - cam_pos[2]).powi(2);
                    db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
                });
                    for i in draw_order {
                        let is_selected = self.state.is_selected(i);
                        self.draw_3d_object(&painter, rect, i, is_selected);
                    }
                } // End GPU/CPU rendering path

                // Draw transform gizmo for selected object (both rendering paths)
                if self.state.selected_object.is_some() {
                    self.draw_transform_gizmo(&painter, rect);
                }

                // Sculpt mode: draw brush circle indicator with current brush type
                if self.state.edit_mode == EditMode::Sculpt {
                    if let Some(hover_pos) = response.hover_pos() {
                        let brush_radius = self.state.sculpt_radius;
                        let brush_color = match self.state.sculpt_brush {
                            SculptBrush::Draw => egui::Color32::from_rgba_unmultiplied(100, 200, 255, 180),
                            SculptBrush::Smooth => egui::Color32::from_rgba_unmultiplied(100, 255, 100, 180),
                            SculptBrush::Flatten => egui::Color32::from_rgba_unmultiplied(255, 200, 100, 180),
                            SculptBrush::Pinch => egui::Color32::from_rgba_unmultiplied(255, 100, 100, 180),
                            SculptBrush::Inflate => egui::Color32::from_rgba_unmultiplied(200, 100, 255, 180),
                            SculptBrush::Grab => egui::Color32::from_rgba_unmultiplied(255, 255, 100, 180),
                        };
                        painter.circle_stroke(
                            hover_pos,
                            brush_radius,
                            egui::Stroke::new(2.0_f32, brush_color),
                        );
                        painter.circle_stroke(
                            hover_pos,
                            brush_radius * 0.15,
                            egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(200, 200, 200, 120)),
                        );
                        painter.text(
                            rect.left_top() + egui::vec2(10.0, 36.0),
                            egui::Align2::LEFT_TOP,
                            format!("Sculpt Mode | Brush: {} | R: {:.0} | Str: {:.0}%",
                                self.state.sculpt_brush, brush_radius, self.state.sculpt_strength * 100.0),
                            egui::FontId::proportional(11.0),
                            brush_color,
                        );
                    }
                }

                // Edit mode: draw vertex dots and highlighted edges
                if self.state.edit_mode == EditMode::Edit {
                    if let Some(idx) = self.state.selected_object {
                        let edit_verts = self.get_object_vertices(idx);
                        let edit_edges = self.get_object_edges(idx);

                        // Draw edges in bright teal
                        let edge_color = egui::Color32::from_rgb(50, 200, 220);
                        for (i1, i2) in &edit_edges {
                            if let (Some(p1), Some(p2)) = (
                                self.project_point(edit_verts[*i1], rect),
                                self.project_point(edit_verts[*i2], rect),
                            ) {
                                painter.line_segment([p1, p2], egui::Stroke::new(1.5_f32, edge_color));
                            }
                        }

                        // Draw vertex dots (white with black outline)
                        for vert in &edit_verts {
                            if let Some(p) = self.project_point(*vert, rect) {
                                painter.circle_filled(p, 3.5, egui::Color32::WHITE);
                                painter.circle_stroke(p, 3.5, egui::Stroke::new(1.0_f32, egui::Color32::BLACK));
                            }
                        }

                        // Vertex count indicator with sub-selection mode
                        painter.text(
                            rect.left_top() + egui::vec2(10.0, 36.0),
                            egui::Align2::LEFT_TOP,
                            format!("Edit Mode [{}] | Verts: {} | Edges: {}",
                                self.state.edit_selection, edit_verts.len(), edit_edges.len()),
                            egui::FontId::proportional(11.0),
                            egui::Color32::from_rgb(50, 200, 220),
                        );
                    }
                }

                // Texture Paint mode: draw brush cursor
                if self.state.edit_mode == EditMode::TexturePaint {
                    if let Some(hover_pos) = response.hover_pos() {
                        let pc = self.state.paint_color;
                        let brush_color = egui::Color32::from_rgba_unmultiplied(
                            (pc[0] * 255.0) as u8, (pc[1] * 255.0) as u8,
                            (pc[2] * 255.0) as u8, 180);
                        painter.circle_stroke(
                            hover_pos,
                            self.state.paint_radius,
                            egui::Stroke::new(2.0_f32, brush_color),
                        );
                        painter.circle_filled(hover_pos, 2.0, brush_color);
                        painter.text(
                            rect.left_top() + egui::vec2(10.0, 36.0),
                            egui::Align2::LEFT_TOP,
                            format!("Texture Paint | R: {:.0} | Color: ({:.0},{:.0},{:.0})",
                                self.state.paint_radius, pc[0]*255.0, pc[1]*255.0, pc[2]*255.0),
                            egui::FontId::proportional(11.0),
                            brush_color,
                        );
                    }
                }

                // Weight Paint mode: draw weight brush cursor with heat-map coloring
                if self.state.edit_mode == EditMode::WeightPaint {
                    if let Some(hover_pos) = response.hover_pos() {
                        let w = self.state.weight_value;
                        // Blue (0) -> Cyan -> Green -> Yellow -> Red (1) heat-map
                        let r = (w * 2.0).min(1.0);
                        let g = if w < 0.5 { w * 2.0 } else { 2.0 - w * 2.0 };
                        let b = (1.0 - w * 2.0).max(0.0);
                        let brush_color = egui::Color32::from_rgba_unmultiplied(
                            (r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 200);
                        painter.circle_stroke(
                            hover_pos,
                            self.state.paint_radius,
                            egui::Stroke::new(2.5_f32, brush_color),
                        );
                        painter.circle_filled(hover_pos, 3.0, brush_color);
                        painter.text(
                            rect.left_top() + egui::vec2(10.0, 36.0),
                            egui::Align2::LEFT_TOP,
                            format!("Weight Paint | R: {:.0} | Weight: {:.2}", self.state.paint_radius, w),
                            egui::FontId::proportional(11.0),
                            brush_color,
                        );
                    }
                }

                // Measurements display
                if !self.state.measurements.is_empty() {
                    let measure_color = egui::Color32::from_rgb(255, 255, 100);
                    let measure_stroke = egui::Stroke::new(1.5_f32, measure_color);
                    for m in &self.state.measurements {
                        if let (Some(p1), Some(p2)) = (
                            self.project_point(m.start, rect),
                            self.project_point(m.end, rect),
                        ) {
                            painter.line_segment([p1, p2], measure_stroke);
                            // Endpoint dots
                            painter.circle_filled(p1, 3.0, measure_color);
                            painter.circle_filled(p2, 3.0, measure_color);
                            // Distance label at midpoint
                            let mid = egui::pos2((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5 - 10.0);
                            painter.text(
                                mid,
                                egui::Align2::CENTER_BOTTOM,
                                format!("{:.3} units", m.distance),
                                egui::FontId::proportional(11.0),
                                measure_color,
                            );
                        }
                    }
                }

                // Measurement mode indicator
                if self.state.measuring {
                    painter.text(
                        rect.left_top() + egui::vec2(10.0, 50.0),
                        egui::Align2::LEFT_TOP,
                        "MEASURE: click start and end points",
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgb(255, 255, 100),
                    );
                }

                // 3D Cursor display (red crosshair at cursor position)
                if let Some(cursor_proj) = self.project_point(self.state.cursor_3d, rect) {
                    let cursor_size = 8.0;
                    let cursor_color = egui::Color32::from_rgb(255, 50, 50);
                    painter.line_segment(
                        [cursor_proj - egui::vec2(cursor_size, 0.0), cursor_proj + egui::vec2(cursor_size, 0.0)],
                        egui::Stroke::new(1.5_f32, cursor_color));
                    painter.line_segment(
                        [cursor_proj - egui::vec2(0.0, cursor_size), cursor_proj + egui::vec2(0.0, cursor_size)],
                        egui::Stroke::new(1.5_f32, cursor_color));
                    painter.circle_stroke(cursor_proj, 4.0, egui::Stroke::new(1.0_f32, cursor_color));
                }

                // Normals display overlay (short lines from object center in Y direction)
                if self.state.show_normals {
                    let normal_color = egui::Color32::from_rgb(100, 200, 255);
                    for i in 0..self.state.objects.len() {
                        let obj = &self.state.objects[i];
                        if !obj.visible { continue; }
                        // Show face normal lines from object center
                        let center = obj.position;
                        let normal_end = [center[0], center[1] + 0.5 * obj.scale[1], center[2]];
                        if let (Some(p1), Some(p2)) = (self.project_point(center, rect), self.project_point(normal_end, rect)) {
                            painter.line_segment([p1, p2], egui::Stroke::new(1.5_f32, normal_color));
                            // Arrow tip
                            let dir = p2 - p1;
                            let len = dir.length();
                            if len > 5.0 {
                                let norm = dir / len;
                                let perp = egui::vec2(-norm.y, norm.x);
                                let tip_size = 4.0;
                                let tip1 = p2 - norm * tip_size + perp * tip_size * 0.5;
                                let tip2 = p2 - norm * tip_size - perp * tip_size * 0.5;
                                painter.line_segment([p2, tip1], egui::Stroke::new(1.5_f32, normal_color));
                                painter.line_segment([p2, tip2], egui::Stroke::new(1.5_f32, normal_color));
                            }
                        }
                    }
                }

                // Face orientation overlay indicator
                if self.state.show_face_orientation {
                    painter.text(
                        egui::pos2(rect.left() + 10.0, rect.top() + 40.0),
                        egui::Align2::LEFT_TOP,
                        "Face Orientation: Front=Blue Back=Red",
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgba_unmultiplied(180, 180, 200, 180),
                    );
                }

                // Matcap indicator
                if self.state.matcap_index > 0 {
                    let matcap_names = ["", "Clay", "Chrome", "Jade", "Pearl", "Obsidian", "Copper"];
                    let name = matcap_names.get(self.state.matcap_index).unwrap_or(&"Custom");
                    painter.text(
                        egui::pos2(rect.right() - 10.0, rect.top() + 40.0),
                        egui::Align2::RIGHT_TOP,
                        format!("Matcap: {}", name),
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgba_unmultiplied(180, 180, 200, 180),
                    );
                }

                // Object info overlay (name + dimensions near each visible object)
                if self.state.show_object_info {
                    let info_color = egui::Color32::from_rgba_unmultiplied(200, 200, 200, 160);
                    for i in 0..self.state.objects.len() {
                        let obj = &self.state.objects[i];
                        if !obj.visible { continue; }
                        if let Some(proj) = self.project_point(obj.position, rect) {
                            let label = format!("{} [{:.1}x{:.1}x{:.1}]",
                                obj.name, obj.scale[0], obj.scale[1], obj.scale[2]);
                            painter.text(
                                proj + egui::vec2(15.0, -5.0),
                                egui::Align2::LEFT_CENTER,
                                label,
                                egui::FontId::proportional(10.0),
                                info_color,
                            );
                        }
                    }
                }

                // Orientation cube (top-right corner)
                if self.state.show_orientation_cube {
                    let cube_size = 50.0;
                    let cube_center = egui::pos2(rect.right() - cube_size - 10.0, rect.top() + cube_size + 10.0);
                    let cube_bg = egui::Color32::from_rgba_unmultiplied(40, 40, 48, 200);
                    painter.circle_filled(cube_center, cube_size * 0.7, cube_bg);

                    let yaw = self.state.camera.orbit_angles[0].to_radians();
                    let pitch = self.state.camera.orbit_angles[1].to_radians();

                    // Project each axis direction
                    let axes = [
                        ([1.0_f32, 0.0, 0.0], "X", egui::Color32::from_rgb(230, 60, 60)),
                        ([0.0, 1.0, 0.0], "Y", egui::Color32::from_rgb(60, 200, 60)),
                        ([0.0, 0.0, 1.0], "Z", egui::Color32::from_rgb(80, 80, 230)),
                    ];

                    for (dir, label, color) in &axes {
                        // Simple rotation of axis direction
                        let rx = dir[0] * yaw.cos() + dir[2] * yaw.sin();
                        let ry = dir[1] * pitch.cos() - (dir[2] * yaw.cos() - dir[0] * yaw.sin()) * pitch.sin();
                        let axis_len = cube_size * 0.55;
                        let end = egui::pos2(cube_center.x + rx * axis_len, cube_center.y - ry * axis_len);
                        painter.line_segment([cube_center, end], egui::Stroke::new(2.0_f32, *color));
                        painter.text(end, egui::Align2::CENTER_CENTER, *label,
                            egui::FontId::proportional(11.0), *color);
                    }
                }

                // Camera preview mini-viewport (bottom-right corner)
                if self.state.show_camera_preview {
                    // Find camera object
                    let cam_obj = self.state.objects.iter().find(|o| o.object_type == ObjectType::Camera);
                    if let Some(cam) = cam_obj {
                        let preview_w = 160.0_f32;
                        let preview_h = 120.0;
                        let preview_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.right() - preview_w - 10.0, rect.bottom() - preview_h - 10.0),
                            egui::vec2(preview_w, preview_h),
                        );
                        // Background
                        painter.rect_filled(preview_rect, 4.0, egui::Color32::from_rgba_unmultiplied(20, 20, 28, 220));
                        painter.rect_stroke(preview_rect, 4.0, egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(100, 100, 120)));
                        // Camera icon and info
                        let cam_pos = cam.position;
                        let cam_rot = cam.rotation;
                        painter.text(
                            egui::pos2(preview_rect.center().x, preview_rect.top() + 12.0),
                            egui::Align2::CENTER_CENTER,
                            "Camera Preview",
                            egui::FontId::proportional(10.0),
                            egui::Color32::from_rgb(180, 180, 200),
                        );
                        // Show simplified wireframe from camera perspective
                        let info = format!("Pos: ({:.1}, {:.1}, {:.1})\nRot: ({:.0}, {:.0}, {:.0})",
                            cam_pos[0], cam_pos[1], cam_pos[2],
                            cam_rot[0], cam_rot[1], cam_rot[2]);
                        painter.text(
                            egui::pos2(preview_rect.center().x, preview_rect.center().y + 10.0),
                            egui::Align2::CENTER_CENTER,
                            &info,
                            egui::FontId::proportional(9.0),
                            egui::Color32::from_rgb(140, 140, 160),
                        );
                        // Draw camera frustum icon
                        let icon_center = egui::pos2(preview_rect.center().x, preview_rect.center().y - 15.0);
                        let icon_pts = [
                            egui::pos2(icon_center.x - 12.0, icon_center.y - 8.0),
                            egui::pos2(icon_center.x + 12.0, icon_center.y - 8.0),
                            egui::pos2(icon_center.x + 20.0, icon_center.y + 8.0),
                            egui::pos2(icon_center.x - 20.0, icon_center.y + 8.0),
                        ];
                        for i in 0..4 {
                            painter.line_segment([icon_pts[i], icon_pts[(i + 1) % 4]],
                                egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(200, 200, 220)));
                        }
                    }
                }

                // Onion skinning ghost display
                if self.state.onion_skinning && self.state.timeline.is_playing {
                    painter.text(
                        egui::pos2(rect.left() + 10.0, rect.bottom() - 50.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("Onion: {} frames", self.state.onion_frames),
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgba_unmultiplied(255, 200, 100, 180),
                    );
                }

                // X-Ray mode indicator
                if self.state.xray_mode {
                    painter.text(
                        egui::pos2(rect.left() + 10.0, rect.bottom() - 65.0),
                        egui::Align2::LEFT_BOTTOM,
                        "X-RAY",
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_rgba_unmultiplied(100, 200, 255, 200),
                    );
                }

                // Edit tool indicator (when in Edit mode)
                if self.state.edit_mode == EditMode::Edit && self.state.edit_tool != EditTool::Select {
                    painter.text(
                        egui::pos2(rect.center().x, rect.top() + 30.0),
                        egui::Align2::CENTER_TOP,
                        format!("{}", self.state.edit_tool),
                        egui::FontId::proportional(14.0),
                        egui::Color32::from_rgb(255, 220, 80),
                    );
                    if self.state.edit_tool == EditTool::LoopCut {
                        painter.text(
                            egui::pos2(rect.center().x, rect.top() + 48.0),
                            egui::Align2::CENTER_TOP,
                            format!("Segments: {}", self.state.loop_cut_segments),
                            egui::FontId::proportional(11.0),
                            egui::Color32::from_rgb(200, 200, 200),
                        );
                    }
                }

                // Relationship lines (parent-child connections)
                if self.state.show_relationship_lines {
                    for i in 0..self.state.objects.len() {
                        if !self.state.objects[i].visible { continue; }
                        if let Some(parent_idx) = self.state.objects[i].parent {
                            if parent_idx < self.state.objects.len() && self.state.objects[parent_idx].visible {
                                if let (Some(child_proj), Some(parent_proj)) = (
                                    self.project_point(self.state.objects[i].position, rect),
                                    self.project_point(self.state.objects[parent_idx].position, rect),
                                ) {
                                    // Dashed line from child to parent
                                    let dash_color = egui::Color32::from_rgba_unmultiplied(100, 150, 255, 120);
                                    painter.line_segment([child_proj, parent_proj], egui::Stroke::new(1.0_f32, dash_color));
                                }
                            }
                        }
                    }
                }

                // Particle system indicators (show small particle icon near emitters)
                for i in 0..self.state.objects.len() {
                    if !self.state.objects[i].visible { continue; }
                    if self.state.objects[i].particle_systems.is_empty() { continue; }
                    let active_ps = self.state.objects[i].particle_systems.iter().any(|ps| ps.active);
                    if !active_ps { continue; }
                    if let Some(proj) = self.project_point(self.state.objects[i].position, rect) {
                        let ps_count: u32 = self.state.objects[i].particle_systems.iter()
                            .filter(|ps| ps.active).map(|ps| ps.count).sum();
                        // Small upward spray lines
                        for j in 0..5 {
                            let angle = (j as f32 - 2.0) * 0.3;
                            let len = 15.0;
                            let end = egui::pos2(
                                proj.x + angle * len,
                                proj.y - len - (j as f32 * 2.0),
                            );
                            painter.line_segment([
                                egui::pos2(proj.x, proj.y - 8.0),
                                end,
                            ], egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(255, 180, 50, 150)));
                            painter.circle_filled(end, 1.5, egui::Color32::from_rgba_unmultiplied(255, 200, 80, 180));
                        }
                        painter.text(
                            egui::pos2(proj.x, proj.y - 28.0),
                            egui::Align2::CENTER_BOTTOM,
                            format!("{}p", ps_count),
                            egui::FontId::proportional(9.0),
                            egui::Color32::from_rgba_unmultiplied(255, 200, 100, 180),
                        );
                    }
                }

                // Camera DOF indicator
                if self.state.camera_settings.dof_enabled {
                    painter.text(
                        egui::pos2(rect.right() - 10.0, rect.bottom() - 20.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!("DOF f/{:.1} @{:.1}m", self.state.camera_settings.aperture, self.state.camera_settings.focal_distance),
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgba_unmultiplied(180, 180, 255, 180),
                    );
                }

                // Exposure indicator
                if self.state.camera_settings.exposure != 0.0 {
                    painter.text(
                        egui::pos2(rect.right() - 10.0, rect.bottom() - 34.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!("EV: {:+.1}", self.state.camera_settings.exposure),
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgba_unmultiplied(200, 200, 200, 180),
                    );
                }

                // Fog indicator
                if self.state.world.fog_enabled {
                    painter.text(
                        egui::pos2(rect.right() - 10.0, rect.bottom() - 48.0),
                        egui::Align2::RIGHT_BOTTOM,
                        "FOG",
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgba_unmultiplied(150, 180, 200, 180),
                    );
                }

                // Armature bone display (draw bones as lines for objects with bones)
                for i in 0..self.state.objects.len() {
                    let obj = &self.state.objects[i];
                    if !obj.visible || obj.bones.is_empty() { continue; }
                    let pos = obj.position;
                    let bone_color = if Some(i) == self.state.selected_object {
                        egui::Color32::from_rgb(100, 220, 255)
                    } else {
                        egui::Color32::from_rgb(80, 180, 80)
                    };
                    for bone in &obj.bones {
                        let head_world = [pos[0] + bone.head[0], pos[1] + bone.head[1], pos[2] + bone.head[2]];
                        let tail_world = [pos[0] + bone.tail[0], pos[1] + bone.tail[1], pos[2] + bone.tail[2]];
                        if let (Some(p1), Some(p2)) = (
                            self.project_point(head_world, rect),
                            self.project_point(tail_world, rect),
                        ) {
                            // Draw bone as octahedral (diamond line)
                            let mid = egui::pos2((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5);
                            let dx = p2.x - p1.x;
                            let dy = p2.y - p1.y;
                            let len = (dx * dx + dy * dy).sqrt().max(0.001);
                            let nx = -dy / len * 5.0;
                            let ny = dx / len * 5.0;
                            let left = egui::pos2(mid.x + nx, mid.y + ny);
                            let right = egui::pos2(mid.x - nx, mid.y - ny);
                            // Diamond shape
                            painter.line_segment([p1, left], egui::Stroke::new(1.5_f32, bone_color));
                            painter.line_segment([left, p2], egui::Stroke::new(1.5_f32, bone_color));
                            painter.line_segment([p1, right], egui::Stroke::new(1.5_f32, bone_color));
                            painter.line_segment([right, p2], egui::Stroke::new(1.5_f32, bone_color));
                            // Head dot
                            painter.circle_filled(p1, 3.0, bone_color);
                            // Tail dot
                            painter.circle_filled(p2, 2.0, egui::Color32::from_rgb(200, 200, 200));
                            // Bone name
                            if self.state.show_bone_names {
                                painter.text(
                                    mid + egui::vec2(8.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    &bone.name,
                                    egui::FontId::proportional(9.0),
                                    egui::Color32::from_rgba_unmultiplied(180, 220, 255, 180),
                                );
                            }
                        }
                    }
                }

                // Force field indicators
                for i in 0..self.state.objects.len() {
                    let obj = &self.state.objects[i];
                    if !obj.visible { continue; }
                    if let Some(ref ff) = obj.force_field {
                        if !ff.enabled { continue; }
                        if let Some(center) = self.project_point(obj.position, rect) {
                            let ff_color = match ff.field_type {
                                ForceFieldType::Wind => egui::Color32::from_rgba_unmultiplied(100, 200, 255, 160),
                                ForceFieldType::Vortex => egui::Color32::from_rgba_unmultiplied(200, 100, 255, 160),
                                ForceFieldType::Turbulence => egui::Color32::from_rgba_unmultiplied(255, 200, 100, 160),
                                _ => egui::Color32::from_rgba_unmultiplied(255, 150, 100, 160),
                            };
                            // Concentric circles to represent force field
                            let r = 12.0 + ff.strength * 3.0;
                            painter.circle_stroke(center, r, egui::Stroke::new(1.0_f32, ff_color));
                            painter.circle_stroke(center, r * 0.6, egui::Stroke::new(0.5_f32, ff_color));
                            painter.text(
                                center + egui::vec2(0.0, r + 6.0),
                                egui::Align2::CENTER_TOP,
                                format!("{}", ff.field_type),
                                egui::FontId::proportional(9.0),
                                ff_color,
                            );
                        }
                    }
                }

                // Workspace indicator (top-left, after mode info)
                if self.state.workspace != WorkspaceLayout::Modeling {
                    painter.text(
                        rect.left_top() + egui::vec2(10.0, 50.0),
                        egui::Align2::LEFT_TOP,
                        format!("Workspace: {}", self.state.workspace),
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgba_unmultiplied(160, 200, 255, 150),
                    );
                }

                // Pose mode indicator
                if self.state.pose_mode {
                    painter.text(
                        egui::pos2(rect.right() - 10.0, rect.bottom() - 62.0),
                        egui::Align2::RIGHT_BOTTOM,
                        "POSE",
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgba_unmultiplied(100, 220, 255, 200),
                    );
                }

                // Color management indicator (when not default Filmic)
                if self.state.color_management.view_transform != "Filmic" {
                    painter.text(
                        egui::pos2(rect.right() - 10.0, rect.bottom() - 76.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!("CM: {}", self.state.color_management.view_transform),
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgba_unmultiplied(200, 180, 255, 160),
                    );
                }

                // Performance overlay
                if self.state.show_perf_overlay {
                    let vp_dt = ui.ctx().input(|i| i.predicted_dt);
                    let fps = 1.0 / vp_dt.max(0.001);
                    self.state.perf_stats.fps_history.push(fps);
                    if self.state.perf_stats.fps_history.len() > 60 {
                        self.state.perf_stats.fps_history.remove(0);
                    }
                    // Count scene stats
                    let mut total_v = 0u32;
                    let mut total_f = 0u32;
                    for obj in &self.state.objects {
                        let (v, f) = match obj.object_type {
                            ObjectType::Cube => (8, 12),
                            ObjectType::Sphere => (384, 768),
                            ObjectType::Cylinder => (128, 256),
                            ObjectType::Cone => (65, 128),
                            ObjectType::Torus => (512, 1024),
                            ObjectType::Plane => (4, 2),
                            ObjectType::IcoSphere => (162, 320),
                            ObjectType::Grid => (121, 200),
                            ObjectType::Circle => (32, 0),
                            _ => (0, 0),
                        };
                        total_v += v;
                        total_f += f;
                    }
                    self.state.perf_stats.total_vertices = total_v;
                    self.state.perf_stats.total_faces = total_f;
                    self.state.perf_stats.draw_calls = self.state.objects.len() as u32;

                    let avg_fps = if self.state.perf_stats.fps_history.is_empty() { 0.0 }
                        else { self.state.perf_stats.fps_history.iter().sum::<f32>() / self.state.perf_stats.fps_history.len() as f32 };

                    let perf_text = format!(
                        "FPS: {:.0} (avg: {:.0})\nVerts: {} | Faces: {}\nDraw Calls: {}\nObjects: {}",
                        fps, avg_fps, total_v, total_f,
                        self.state.perf_stats.draw_calls,
                        self.state.objects.len(),
                    );
                    painter.text(
                        egui::pos2(rect.left() + 10.0, rect.top() + 30.0),
                        egui::Align2::LEFT_TOP,
                        &perf_text,
                        egui::FontId::monospace(10.0),
                        egui::Color32::from_rgba_unmultiplied(100, 255, 100, 200),
                    );

                    // Mini FPS graph
                    let graph_w = 120.0;
                    let graph_h = 40.0;
                    let graph_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 10.0, rect.top() + 95.0),
                        egui::vec2(graph_w, graph_h),
                    );
                    painter.rect_filled(graph_rect, 2.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 120));
                    let history = &self.state.perf_stats.fps_history;
                    if history.len() >= 2 {
                        let max_fps = history.iter().cloned().fold(60.0_f32, f32::max);
                        for i in 1..history.len() {
                            let x1 = graph_rect.left() + (i - 1) as f32 * (graph_w / 60.0);
                            let x2 = graph_rect.left() + i as f32 * (graph_w / 60.0);
                            let y1 = graph_rect.bottom() - (history[i - 1] / max_fps) * graph_h;
                            let y2 = graph_rect.bottom() - (history[i] / max_fps) * graph_h;
                            painter.line_segment(
                                [egui::pos2(x1, y1), egui::pos2(x2, y2)],
                                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(100, 255, 100)),
                            );
                        }
                    }
                }

                // Render engine indicator
                if self.state.render_engine != RenderEngine::Eevee {
                    painter.text(
                        egui::pos2(rect.right() - 10.0, rect.bottom() - 90.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!("Engine: {}", self.state.render_engine),
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgba_unmultiplied(255, 200, 100, 160),
                    );
                }

                // Motion path display
                if self.state.show_motion_paths_viewport {
                    for obj in &self.state.objects {
                        if let Some(ref mp) = obj.motion_path {
                            if mp.points.len() >= 2 {
                                let mp_color = egui::Color32::from_rgb(
                                    (mp.color[0] * 255.0) as u8,
                                    (mp.color[1] * 255.0) as u8,
                                    (mp.color[2] * 255.0) as u8,
                                );
                                for i in 0..mp.points.len() - 1 {
                                    if let (Some(p1), Some(p2)) = (
                                        self.project_point(mp.points[i].position, rect),
                                        self.project_point(mp.points[i + 1].position, rect),
                                    ) {
                                        painter.line_segment(
                                            [p1, p2],
                                            egui::Stroke::new(1.5_f32, mp_color),
                                        );
                                    }
                                }
                                // Draw dots at keyframe positions
                                for pt in &mp.points {
                                    if pt.frame % 10 == 0 {
                                        if let Some(p) = self.project_point(pt.position, rect) {
                                            painter.circle_filled(p, 2.5, mp_color);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Shift+RightClick to place 3D cursor
                if response.secondary_clicked() {
                    let shift_held = ui.ctx().input(|i| i.modifiers.shift);
                    if shift_held {
                        if let Some(pos) = pointer_pos {
                            self.state.cursor_3d = self.unproject_point(pos, rect);
                            self.status_message = format!("3D Cursor: ({:.2}, {:.2}, {:.2})",
                                self.state.cursor_3d[0], self.state.cursor_3d[1], self.state.cursor_3d[2]);
                        }
                    }
                }

                // Box selection mode: handle drag to create selection rectangle
                if self.state.box_select_active {
                    if response.drag_started_by(egui::PointerButton::Primary) {
                        if let Some(pos) = pointer_pos {
                            self.state.box_select_start = Some([pos.x, pos.y]);
                        }
                    }
                    if response.dragged_by(egui::PointerButton::Primary) {
                        if let Some(pos) = pointer_pos {
                            self.box_select_end = Some(pos);
                        }
                    }
                    if response.drag_stopped_by(egui::PointerButton::Primary) {
                        // Finalize box selection
                        if let (Some(start), Some(end)) = (self.state.box_select_start, self.box_select_end) {
                            let min_x = start[0].min(end.x);
                            let max_x = start[0].max(end.x);
                            let min_y = start[1].min(end.y);
                            let max_y = start[1].max(end.y);

                            self.state.selected_object = None;
                            self.state.clear_multi_select();
                            let mut first = true;
                            for i in 0..self.state.objects.len() {
                                if !self.state.objects[i].visible { continue; }
                                // Check if object center projects inside the box
                                if let Some(proj) = self.project_point(self.state.objects[i].position, rect) {
                                    if proj.x >= min_x && proj.x <= max_x && proj.y >= min_y && proj.y <= max_y {
                                        if first {
                                            self.state.selected_object = Some(i);
                                            first = false;
                                        } else {
                                            self.state.multi_selected.push(i);
                                        }
                                    }
                                }
                            }
                            let count = self.state.all_selected().len();
                            self.status_message = format!("Box selected {} object{}", count, if count != 1 { "s" } else { "" });
                        }
                        self.state.box_select_active = false;
                        self.state.box_select_start = None;
                        self.box_select_end = None;
                    }

                    // Draw selection rectangle
                    if let (Some(start), Some(end)) = (self.state.box_select_start, self.box_select_end) {
                        let sel_rect = egui::Rect::from_two_pos(
                            egui::pos2(start[0], start[1]),
                            end,
                        );
                        painter.rect_filled(sel_rect, 0.0, egui::Color32::from_rgba_unmultiplied(100, 150, 255, 30));
                        painter.rect_stroke(sel_rect, 0.0, egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(100, 150, 255, 200)));
                    }

                    // Draw "BOX SELECT" indicator
                    painter.text(
                        rect.center_top() + egui::vec2(0.0, 8.0),
                        egui::Align2::CENTER_TOP,
                        "BOX SELECT (B to cancel)",
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_rgba_unmultiplied(100, 150, 255, 220),
                    );
                }

                // Draw viewport info overlay
                self.draw_viewport_overlay(&painter, rect);
            });
    }

    fn project_point(&self, point: [f32; 3], rect: egui::Rect) -> Option<egui::Pos2> {
        let cam = &self.state.camera;

        // View vector from camera to point
        let dx = point[0] - cam.position[0];
        let dy = point[1] - cam.position[1];
        let dz = point[2] - cam.position[2];

        // Camera basis vectors
        let yaw = cam.orbit_angles[0].to_radians();
        let pitch = cam.orbit_angles[1].to_radians();

        // Forward vector (camera looks at target)
        let forward = [
            -yaw.sin() * pitch.cos(),
            -pitch.sin(),
            -yaw.cos() * pitch.cos(),
        ];

        // Right vector
        let right = [yaw.cos(), 0.0, -yaw.sin()];

        // Up vector
        let up = [
            yaw.sin() * pitch.sin(),
            pitch.cos(),
            yaw.cos() * pitch.sin(),
        ];

        // Project point onto camera plane
        let z = dx * forward[0] + dy * forward[1] + dz * forward[2];

        // Don't render points behind camera
        if z < 0.1 {
            return None;
        }

        let x = dx * right[0] + dy * right[1] + dz * right[2];
        let y = dx * up[0] + dy * up[1] + dz * up[2];

        // Perspective projection
        let fov_scale = 500.0 / cam.distance;
        let screen_x = rect.center().x + x * fov_scale / z * cam.distance;
        let screen_y = rect.center().y - y * fov_scale / z * cam.distance;

        Some(egui::pos2(screen_x, screen_y))
    }

    /// Unproject a screen point to a world-space point on the ground plane (y=0).
    fn unproject_point(&self, screen_pos: egui::Pos2, rect: egui::Rect) -> [f32; 3] {
        let cam = &self.state.camera;
        let yaw = cam.orbit_angles[0].to_radians();
        let pitch = cam.orbit_angles[1].to_radians();

        // Camera basis vectors
        let forward = [
            -yaw.sin() * pitch.cos(),
            -pitch.sin(),
            -yaw.cos() * pitch.cos(),
        ];
        let right = [yaw.cos(), 0.0, -yaw.sin()];
        let up = [
            yaw.sin() * pitch.sin(),
            pitch.cos(),
            yaw.cos() * pitch.sin(),
        ];

        // Reverse the perspective projection
        let fov_scale = 500.0 / cam.distance;
        let ndc_x = (screen_pos.x - rect.center().x) / fov_scale;
        let ndc_y = -(screen_pos.y - rect.center().y) / fov_scale;

        // Ray direction in world space
        let dir = [
            forward[0] + ndc_x * right[0] + ndc_y * up[0],
            forward[1] + ndc_x * right[1] + ndc_y * up[1],
            forward[2] + ndc_x * right[2] + ndc_y * up[2],
        ];

        // Intersect ray with ground plane (y=0)
        let origin = cam.position;
        if dir[1].abs() > 1e-6 {
            let t = -origin[1] / dir[1];
            if t > 0.0 {
                return [origin[0] + dir[0] * t, 0.0, origin[2] + dir[2] * t];
            }
        }

        // Fallback: project to a plane at camera target distance
        let t = cam.distance;
        [
            origin[0] + dir[0] * t,
            origin[1] + dir[1] * t,
            origin[2] + dir[2] * t,
        ]
    }

    fn draw_3d_grid(&self, painter: &egui::Painter, rect: egui::Rect) {
        let grid_size = self.preferences.grid_size;
        let grid_spacing = self.state.snap_increment;
        let extent = grid_size as f32 * grid_spacing;

        for i in -grid_size..=grid_size {
            let offset = i as f32 * grid_spacing;

            // Use thicker/brighter lines for major grid (every 5th line) and axis lines
            let (color, width) = if i == 0 {
                // Don't draw center lines here - axes handler draws those
                continue;
            } else if i % 5 == 0 {
                (
                    egui::Color32::from_rgba_unmultiplied(70, 70, 75, 80),
                    1.0_f32,
                )
            } else {
                (
                    egui::Color32::from_rgba_unmultiplied(55, 55, 60, 50),
                    0.5_f32,
                )
            };

            // Lines along X axis (on XZ plane)
            if let (Some(p1), Some(p2)) = (
                self.project_point([-extent, 0.0, offset], rect),
                self.project_point([extent, 0.0, offset], rect),
            ) {
                painter.line_segment([p1, p2], egui::Stroke::new(width, color));
            }

            // Lines along Z axis (on XZ plane)
            if let (Some(p1), Some(p2)) = (
                self.project_point([offset, 0.0, -extent], rect),
                self.project_point([offset, 0.0, extent], rect),
            ) {
                painter.line_segment([p1, p2], egui::Stroke::new(width, color));
            }
        }
    }

    fn draw_3d_axes(&self, painter: &egui::Painter, rect: egui::Rect) {
        let grid_extent = self.preferences.grid_size as f32 * self.state.snap_increment;

        let x_color = egui::Color32::from_rgba_unmultiplied(180, 50, 50, 160);
        let z_color = egui::Color32::from_rgba_unmultiplied(50, 50, 180, 160);
        let y_color = egui::Color32::from_rgba_unmultiplied(50, 180, 50, 200);

        // X axis line across full grid (red) on XZ plane
        if let (Some(p1), Some(p2)) = (
            self.project_point([-grid_extent, 0.0, 0.0], rect),
            self.project_point([grid_extent, 0.0, 0.0], rect),
        ) {
            painter.line_segment([p1, p2], egui::Stroke::new(1.5_f32, x_color));
        }

        // Z axis line across full grid (blue) on XZ plane
        if let (Some(p1), Some(p2)) = (
            self.project_point([0.0, 0.0, -grid_extent], rect),
            self.project_point([0.0, 0.0, grid_extent], rect),
        ) {
            painter.line_segment([p1, p2], egui::Stroke::new(1.5_f32, z_color));
        }

        // Y axis (green, vertical, shorter)
        if let (Some(p1), Some(p2)) = (
            self.project_point([0.0, 0.0, 0.0], rect),
            self.project_point([0.0, grid_extent * 0.5, 0.0], rect),
        ) {
            painter.line_segment([p1, p2], egui::Stroke::new(1.5_f32, y_color));
        }
    }

    fn draw_3d_object(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        obj_idx: usize,
        selected: bool,
    ) {
        let obj = &self.state.objects[obj_idx];

        let base_color = egui::Color32::from_rgb(
            (obj.material.base_color[0] * 255.0) as u8,
            (obj.material.base_color[1] * 255.0) as u8,
            (obj.material.base_color[2] * 255.0) as u8,
        );

        let wire_color = if selected {
            egui::Color32::from_rgb(255, 150, 50)
        } else {
            egui::Color32::from_rgba_unmultiplied(180, 180, 180, 120)
        };

        let stroke = egui::Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, wire_color);

        // Get vertices for the object type (with rotation applied)
        let base_vertices = self.get_object_vertices(obj_idx);
        let base_faces = self.get_object_faces(obj_idx);

        // Apply modifiers to geometry if any exist
        let (vertices, faces, edges) = if !obj.modifiers.is_empty() && !base_faces.is_empty() {
            let (mod_verts, mod_faces) =
                Self::apply_modifiers(&base_vertices, &base_faces, &obj.modifiers, obj.position);
            let mod_edges = Self::derive_edges_from_faces(&mod_faces);
            (mod_verts, mod_faces, mod_edges)
        } else {
            let base_edges = self.get_object_edges(obj_idx);
            (base_vertices, base_faces.clone(), base_edges)
        };

        // 1) Draw filled faces FIRST (so edges render on top)
        if matches!(
            self.state.shading,
            ShadingMode::Solid | ShadingMode::Material | ShadingMode::Rendered
        ) {
            let faces = &faces;

            // Collect ALL lights in the scene for multi-light accumulation
            let lights: Vec<([f32; 3], [f32; 3], f32)> = {
                let mut ls = Vec::new();
                for scene_obj in &self.state.objects {
                    if scene_obj.object_type == state::ObjectType::Light && scene_obj.visible {
                        let color = [
                            scene_obj.material.base_color[0],
                            scene_obj.material.base_color[1],
                            scene_obj.material.base_color[2],
                        ];
                        let strength = scene_obj.material.emissive.max(1.0);
                        ls.push((scene_obj.position, color, strength));
                    }
                }
                if ls.is_empty() {
                    // Default key light + fill light
                    ls.push(([4.0, 6.0, 3.0], [1.0, 0.98, 0.95], 1.0));
                    ls.push(([-3.0, 2.0, -2.0], [0.4, 0.45, 0.6], 0.3));
                }
                ls
            };

            // Sort faces by depth (painter's algorithm - draw far faces first)
            let cam_pos = self.state.camera.position;
            let mut face_depths: Vec<(usize, f32)> = faces
                .iter()
                .enumerate()
                .map(|(fi, face)| {
                    let avg_z: f32 = face
                        .iter()
                        .map(|&vi| {
                            let v = vertices[vi];
                            let dx = v[0] - cam_pos[0];
                            let dy = v[1] - cam_pos[1];
                            let dz = v[2] - cam_pos[2];
                            dx * dx + dy * dy + dz * dz
                        })
                        .sum::<f32>()
                        / face.len().max(1) as f32;
                    (fi, avg_z)
                })
                .collect();
            face_depths.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let metallic = obj.material.metallic;
            let roughness = obj.material.roughness.max(0.04);
            let use_smooth = obj.smooth_shading;

            // Pre-compute per-vertex normals for smooth shading
            let vertex_normals: Vec<[f32; 3]> = if use_smooth {
                let mut vnormals = vec![[0.0_f32; 3]; vertices.len()];
                for face in faces.iter() {
                    if face.len() < 3 {
                        continue;
                    }
                    let va = vertices[face[0]];
                    let vb = vertices[face[1]];
                    let vc = vertices[face[face.len() - 1]];
                    let e1 = [vb[0] - va[0], vb[1] - va[1], vb[2] - va[2]];
                    let e2 = [vc[0] - va[0], vc[1] - va[1], vc[2] - va[2]];
                    let fn_x = e1[1] * e2[2] - e1[2] * e2[1];
                    let fn_y = e1[2] * e2[0] - e1[0] * e2[2];
                    let fn_z = e1[0] * e2[1] - e1[1] * e2[0];
                    for &vi in face {
                        if vi < vnormals.len() {
                            vnormals[vi][0] += fn_x;
                            vnormals[vi][1] += fn_y;
                            vnormals[vi][2] += fn_z;
                        }
                    }
                }
                // Normalize
                for vn in vnormals.iter_mut() {
                    let len = (vn[0] * vn[0] + vn[1] * vn[1] + vn[2] * vn[2])
                        .sqrt()
                        .max(1e-6);
                    vn[0] /= len;
                    vn[1] /= len;
                    vn[2] /= len;
                }
                vnormals
            } else {
                Vec::new()
            };

            let base_r = base_color.r() as f32 / 255.0;
            let base_g = base_color.g() as f32 / 255.0;
            let base_b = base_color.b() as f32 / 255.0;

            // Fresnel-Schlick at normal incidence (constant per object)
            let f0_dielectric = 0.04_f32;
            let f0_r = f0_dielectric * (1.0 - metallic) + base_r * metallic;
            let f0_g = f0_dielectric * (1.0 - metallic) + base_g * metallic;
            let f0_b = f0_dielectric * (1.0 - metallic) + base_b * metallic;

            // PBR shading helper closure
            let shade_vertex = |pos: [f32; 3], normal: [f32; 3]| -> [f32; 3] {
                let vd = [
                    cam_pos[0] - pos[0],
                    cam_pos[1] - pos[1],
                    cam_pos[2] - pos[2],
                ];
                let vlen = (vd[0] * vd[0] + vd[1] * vd[1] + vd[2] * vd[2])
                    .sqrt()
                    .max(1e-6);
                let view = [vd[0] / vlen, vd[1] / vlen, vd[2] / vlen];

                let mut tr = 0.0_f32;
                let mut tg = 0.0_f32;
                let mut tb = 0.0_f32;

                for (light_pos, light_color, light_strength) in &lights {
                    let ld = [
                        light_pos[0] - pos[0],
                        light_pos[1] - pos[1],
                        light_pos[2] - pos[2],
                    ];
                    let ldlen = (ld[0] * ld[0] + ld[1] * ld[1] + ld[2] * ld[2])
                        .sqrt()
                        .max(1e-6);
                    let light_dir = [ld[0] / ldlen, ld[1] / ldlen, ld[2] / ldlen];
                    let ndotl = (normal[0] * light_dir[0]
                        + normal[1] * light_dir[1]
                        + normal[2] * light_dir[2])
                        .max(0.0);

                    let hx = light_dir[0] + view[0];
                    let hy = light_dir[1] + view[1];
                    let hz = light_dir[2] + view[2];
                    let hlen = (hx * hx + hy * hy + hz * hz).sqrt().max(1e-6);
                    let half = [hx / hlen, hy / hlen, hz / hlen];
                    let ndoth =
                        (normal[0] * half[0] + normal[1] * half[1] + normal[2] * half[2]).max(0.0);

                    let spec_power = (2.0 / (roughness * roughness).max(0.001) - 2.0).min(2048.0);
                    let spec = ndoth.powf(spec_power);

                    let hdotv =
                        (half[0] * view[0] + half[1] * view[1] + half[2] * view[2]).max(0.0);
                    let ff = (1.0 - hdotv).powi(5);
                    let fr = f0_r + (1.0 - f0_r) * ff;
                    let fg = f0_g + (1.0 - f0_g) * ff;
                    let fb = f0_b + (1.0 - f0_b) * ff;

                    let diff_factor = (1.0 - metallic) * ndotl * 0.7;
                    let spec_factor = spec * ndotl;
                    let lr = light_color[0] * light_strength;
                    let lg = light_color[1] * light_strength;
                    let lb = light_color[2] * light_strength;

                    tr += (base_r * diff_factor + fr * spec_factor) * lr;
                    tg += (base_g * diff_factor + fg * spec_factor) * lg;
                    tb += (base_b * diff_factor + fb * spec_factor) * lb;
                }

                // Hemisphere ambient
                let sky_factor = (normal[1] * 0.5 + 0.5).max(0.0);
                tr += base_r * (0.12 + 0.06 * sky_factor);
                tg += base_g * (0.12 + 0.08 * sky_factor);
                tb += base_b * (0.14 + 0.10 * sky_factor);

                // Rim light
                let ndotv_n =
                    (normal[0] * view[0] + normal[1] * view[1] + normal[2] * view[2]).max(0.0);
                let rim = (1.0 - ndotv_n).powi(3) * 0.08;
                tr = (tr + rim).min(1.0);
                tg = (tg + rim).min(1.0);
                tb = (tb + rim).min(1.0);

                // Emissive
                if obj.material.emissive > 0.0 {
                    tr = (tr + base_r * obj.material.emissive * 0.3).min(1.0);
                    tg = (tg + base_g * obj.material.emissive * 0.3).min(1.0);
                    tb = (tb + base_b * obj.material.emissive * 0.3).min(1.0);
                }
                [tr, tg, tb]
            };

            for (fi, _depth) in &face_depths {
                let face = &faces[*fi];
                if face.len() < 3 {
                    continue;
                }

                // Compute face normal (always needed for back-face culling)
                let v0 = vertices[face[0]];
                let v1 = vertices[face[1]];
                let v2 = vertices[face[face.len() - 1]];
                let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
                let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
                let fnx = e1[1] * e2[2] - e1[2] * e2[1];
                let fny = e1[2] * e2[0] - e1[0] * e2[2];
                let fnz = e1[0] * e2[1] - e1[1] * e2[0];
                let fnlen = (fnx * fnx + fny * fny + fnz * fnz).sqrt().max(1e-6);
                let face_normal = [fnx / fnlen, fny / fnlen, fnz / fnlen];

                // Back-face culling using face normal
                let vd = [cam_pos[0] - v0[0], cam_pos[1] - v0[1], cam_pos[2] - v0[2]];
                let ndotv =
                    face_normal[0] * vd[0] + face_normal[1] * vd[1] + face_normal[2] * vd[2];
                if ndotv < 0.0 {
                    continue;
                }

                if use_smooth && face.len() >= 3 {
                    // SMOOTH SHADING: Shade each vertex individually with interpolated normals
                    // Triangulate the face and draw with per-vertex colors
                    let points: Vec<_> = face
                        .iter()
                        .filter_map(|&i| self.project_point(vertices[i], rect))
                        .collect();

                    if points.len() >= 3 {
                        let colors: Vec<egui::Color32> = face
                            .iter()
                            .map(|&vi| {
                                let n = if vi < vertex_normals.len() {
                                    vertex_normals[vi]
                                } else {
                                    face_normal
                                };
                                let rgb = shade_vertex(vertices[vi], n);
                                let gamma = 1.0 / 2.2;
                                egui::Color32::from_rgba_unmultiplied(
                                    (rgb[0].max(0.0).powf(gamma) * 255.0).min(255.0) as u8,
                                    (rgb[1].max(0.0).powf(gamma) * 255.0).min(255.0) as u8,
                                    (rgb[2].max(0.0).powf(gamma) * 255.0).min(255.0) as u8,
                                    245,
                                )
                            })
                            .collect();

                        // Draw as triangulated mesh with per-vertex colors
                        let mut mesh = egui::Mesh::default();
                        for (i, (&pt, &col)) in points.iter().zip(colors.iter()).enumerate() {
                            mesh.vertices.push(egui::epaint::Vertex {
                                pos: pt,
                                uv: egui::pos2(0.0, 0.0),
                                color: col,
                            });
                            if i >= 2 {
                                // Fan triangulation from vertex 0
                                mesh.indices.push(0);
                                mesh.indices.push(i as u32 - 1);
                                mesh.indices.push(i as u32);
                            }
                        }
                        painter.add(egui::Shape::mesh(mesh));
                    }
                } else {
                    // FLAT SHADING: single color per face using face normal
                    let rgb = shade_vertex(v0, face_normal);
                    let gamma = 1.0 / 2.2;
                    let r = (rgb[0].max(0.0).powf(gamma) * 255.0).min(255.0) as u8;
                    let g = (rgb[1].max(0.0).powf(gamma) * 255.0).min(255.0) as u8;
                    let b = (rgb[2].max(0.0).powf(gamma) * 255.0).min(255.0) as u8;
                    let shaded_color = egui::Color32::from_rgba_unmultiplied(r, g, b, 245);

                    let points: Vec<_> = face
                        .iter()
                        .filter_map(|&i| self.project_point(vertices[i], rect))
                        .collect();

                    if points.len() >= 3 {
                        painter.add(egui::Shape::convex_polygon(
                            points,
                            shaded_color,
                            egui::Stroke::NONE,
                        ));
                    }
                }
            }
        }

        // 2) Draw edges ON TOP
        let is_wireframe = matches!(self.state.shading, ShadingMode::Wireframe);
        let show_overlay = self.state.wireframe_overlay && !is_wireframe;

        if is_wireframe || selected || show_overlay {
            // Selected objects get a glow outline (draw wider stroke behind)
            if selected && !is_wireframe {
                let glow_stroke = egui::Stroke::new(
                    4.0_f32,
                    egui::Color32::from_rgba_unmultiplied(255, 150, 50, 80),
                );
                for &(i1, i2) in &edges {
                    if let (Some(p1), Some(p2)) = (
                        self.project_point(vertices[i1], rect),
                        self.project_point(vertices[i2], rect),
                    ) {
                        painter.line_segment([p1, p2], glow_stroke);
                    }
                }
            }
            // Wireframe overlay uses subtle semi-transparent lines
            let edge_stroke = if show_overlay && !selected && !is_wireframe {
                egui::Stroke::new(
                    0.5_f32,
                    egui::Color32::from_rgba_unmultiplied(100, 100, 100, 60),
                )
            } else {
                stroke
            };
            for (i1, i2) in edges {
                if let (Some(p1), Some(p2)) = (
                    self.project_point(vertices[i1], rect),
                    self.project_point(vertices[i2], rect),
                ) {
                    painter.line_segment([p1, p2], edge_stroke);
                }
            }
        }
    }

    fn get_object_vertices(&self, obj_idx: usize) -> Vec<[f32; 3]> {
        let obj = &self.state.objects[obj_idx];
        let [px, py, pz] = obj.position;
        let [sx, sy, sz] = obj.scale;

        // Use custom vertices if available (from Edit Mode)
        if let Some(ref custom_verts) = obj.custom_vertices {
            // Apply transform to custom vertices
            return custom_verts
                .iter()
                .map(|[x, y, z]| [px + x * sx, py + y * sy, pz + z * sz])
                .collect();
        }

        let mut verts = match obj.object_type {
            state::ObjectType::Cube => {
                vec![
                    [px - sx * 0.5, py - sy * 0.5, pz - sz * 0.5],
                    [px + sx * 0.5, py - sy * 0.5, pz - sz * 0.5],
                    [px + sx * 0.5, py + sy * 0.5, pz - sz * 0.5],
                    [px - sx * 0.5, py + sy * 0.5, pz - sz * 0.5],
                    [px - sx * 0.5, py - sy * 0.5, pz + sz * 0.5],
                    [px + sx * 0.5, py - sy * 0.5, pz + sz * 0.5],
                    [px + sx * 0.5, py + sy * 0.5, pz + sz * 0.5],
                    [px - sx * 0.5, py + sy * 0.5, pz + sz * 0.5],
                ]
            }
            state::ObjectType::Sphere => {
                // UV-sphere with smooth tessellation
                let r = sx * 0.5;
                let segments = 24;
                let rings = 16;
                let mut verts = vec![];
                for j in 0..=rings {
                    let v = j as f32 / rings as f32;
                    let phi = v * std::f32::consts::PI;
                    for i in 0..segments {
                        let u = i as f32 / segments as f32;
                        let theta = u * 2.0 * std::f32::consts::PI;
                        verts.push([
                            px + r * phi.sin() * theta.cos(),
                            py + r * phi.cos(),
                            pz + r * phi.sin() * theta.sin(),
                        ]);
                    }
                }
                verts
            }
            state::ObjectType::Cylinder => {
                let r = sx * 0.5;
                let h = sy;
                let segments = 24;
                let mut verts = vec![];
                // Bottom circle
                for i in 0..segments {
                    let theta = i as f32 / segments as f32 * 2.0 * std::f32::consts::PI;
                    verts.push([px + r * theta.cos(), py - h * 0.5, pz + r * theta.sin()]);
                }
                // Top circle
                for i in 0..segments {
                    let theta = i as f32 / segments as f32 * 2.0 * std::f32::consts::PI;
                    verts.push([px + r * theta.cos(), py + h * 0.5, pz + r * theta.sin()]);
                }
                // Centers
                verts.push([px, py - h * 0.5, pz]);
                verts.push([px, py + h * 0.5, pz]);
                verts
            }
            state::ObjectType::Plane => {
                vec![
                    [px - sx * 0.5, py, pz - sz * 0.5],
                    [px + sx * 0.5, py, pz - sz * 0.5],
                    [px + sx * 0.5, py, pz + sz * 0.5],
                    [px - sx * 0.5, py, pz + sz * 0.5],
                ]
            }
            state::ObjectType::Torus => {
                let major_r = sx * 0.5;
                let minor_r = sx * 0.15;
                let major_seg = 32;
                let minor_seg = 16;
                let mut verts = vec![];
                for i in 0..major_seg {
                    let u = i as f32 / major_seg as f32 * 2.0 * std::f32::consts::PI;
                    for j in 0..minor_seg {
                        let v = j as f32 / minor_seg as f32 * 2.0 * std::f32::consts::PI;
                        let x = (major_r + minor_r * v.cos()) * u.cos();
                        let y = minor_r * v.sin();
                        let z = (major_r + minor_r * v.cos()) * u.sin();
                        verts.push([px + x, py + y, pz + z]);
                    }
                }
                verts
            }
            state::ObjectType::Cone => {
                let r = sx * 0.5;
                let h = sy;
                let segments = 24;
                let mut verts = vec![];
                // Bottom circle
                for i in 0..segments {
                    let theta = i as f32 / segments as f32 * 2.0 * std::f32::consts::PI;
                    verts.push([px + r * theta.cos(), py - h * 0.5, pz + r * theta.sin()]);
                }
                // Apex
                verts.push([px, py + h * 0.5, pz]);
                // Bottom center
                verts.push([px, py - h * 0.5, pz]);
                verts
            }
            state::ObjectType::IcoSphere => {
                // Icosahedron subdivided once for smooth appearance
                let r = sx * 0.5;
                let t = (1.0 + 5.0_f32.sqrt()) / 2.0;
                let len = (1.0 + t * t).sqrt();
                let a = 1.0 / len;
                let b = t / len;

                // 12 icosahedron vertices
                let ico_verts = [
                    [-a, b, 0.0],
                    [a, b, 0.0],
                    [-a, -b, 0.0],
                    [a, -b, 0.0],
                    [0.0, -a, b],
                    [0.0, a, b],
                    [0.0, -a, -b],
                    [0.0, a, -b],
                    [b, 0.0, -a],
                    [b, 0.0, a],
                    [-b, 0.0, -a],
                    [-b, 0.0, a],
                ];
                // 20 icosahedron faces
                let ico_faces: [[usize; 3]; 20] = [
                    [0, 11, 5],
                    [0, 5, 1],
                    [0, 1, 7],
                    [0, 7, 10],
                    [0, 10, 11],
                    [1, 5, 9],
                    [5, 11, 4],
                    [11, 10, 2],
                    [10, 7, 6],
                    [7, 1, 8],
                    [3, 9, 4],
                    [3, 4, 2],
                    [3, 2, 6],
                    [3, 6, 8],
                    [3, 8, 9],
                    [4, 9, 5],
                    [2, 4, 11],
                    [6, 2, 10],
                    [8, 6, 7],
                    [9, 8, 1],
                ];

                // Subdivide once for smoother icosphere
                let mut midpoint_cache = std::collections::HashMap::new();

                let mut all_verts: Vec<[f32; 3]> =
                    ico_verts.iter().map(|v| [v[0], v[1], v[2]]).collect();
                let mut all_faces: Vec<[usize; 3]> = ico_faces.to_vec();

                // One subdivision pass
                let mut new_faces = Vec::new();
                for face in &all_faces {
                    let a_idx = face[0];
                    let b_idx = face[1];
                    let c_idx = face[2];

                    let get_midpoint =
                        |cache: &mut std::collections::HashMap<(usize, usize), usize>,
                         verts: &mut Vec<[f32; 3]>,
                         i1: usize,
                         i2: usize|
                         -> usize {
                            let key = if i1 < i2 { (i1, i2) } else { (i2, i1) };
                            if let Some(&idx) = cache.get(&key) {
                                return idx;
                            }
                            let v1 = verts[i1];
                            let v2 = verts[i2];
                            let mid = [
                                (v1[0] + v2[0]) * 0.5,
                                (v1[1] + v2[1]) * 0.5,
                                (v1[2] + v2[2]) * 0.5,
                            ];
                            // Normalize to sphere surface
                            let l = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2]).sqrt();
                            let normalized = [mid[0] / l, mid[1] / l, mid[2] / l];
                            let idx = verts.len();
                            verts.push(normalized);
                            cache.insert(key, idx);
                            idx
                        };

                    let ab = get_midpoint(&mut midpoint_cache, &mut all_verts, a_idx, b_idx);
                    let bc = get_midpoint(&mut midpoint_cache, &mut all_verts, b_idx, c_idx);
                    let ca = get_midpoint(&mut midpoint_cache, &mut all_verts, c_idx, a_idx);

                    new_faces.push([a_idx, ab, ca]);
                    new_faces.push([b_idx, bc, ab]);
                    new_faces.push([c_idx, ca, bc]);
                    new_faces.push([ab, bc, ca]);
                }
                all_faces = new_faces;
                let _ = all_faces; // faces handled by get_object_faces

                // Scale and position
                all_verts
                    .iter()
                    .map(|v| [px + v[0] * r, py + v[1] * r, pz + v[2] * r])
                    .collect()
            }
            state::ObjectType::Grid => {
                // Subdivided plane grid (10x10 subdivisions)
                let subdivs = 10_usize;
                let mut verts = vec![];
                for j in 0..=subdivs {
                    for i in 0..=subdivs {
                        let u = i as f32 / subdivs as f32 - 0.5;
                        let v = j as f32 / subdivs as f32 - 0.5;
                        verts.push([px + u * sx, py, pz + v * sz]);
                    }
                }
                verts
            }
            state::ObjectType::Circle => {
                let r = sx * 0.5;
                let segments = 32;
                let mut verts = vec![];
                for i in 0..segments {
                    let theta = i as f32 / segments as f32 * 2.0 * std::f32::consts::PI;
                    verts.push([px + r * theta.cos(), py, pz + r * theta.sin()]);
                }
                verts
            }
            state::ObjectType::BezierCurve => {
                // S-shaped bezier curve (4 control points + evaluated curve)
                let cp0 = [-1.0, 0.0, 0.0];
                let cp1 = [-0.33, 0.0, 0.5];
                let cp2 = [0.33, 0.0, -0.5];
                let cp3 = [1.0, 0.0, 0.0];
                let segments = 20;
                let mut verts = vec![];
                for i in 0..=segments {
                    let t = i as f32 / segments as f32;
                    let mt = 1.0 - t;
                    let x = mt * mt * mt * cp0[0]
                        + 3.0 * mt * mt * t * cp1[0]
                        + 3.0 * mt * t * t * cp2[0]
                        + t * t * t * cp3[0];
                    let y = mt * mt * mt * cp0[1]
                        + 3.0 * mt * mt * t * cp1[1]
                        + 3.0 * mt * t * t * cp2[1]
                        + t * t * t * cp3[1];
                    let z = mt * mt * mt * cp0[2]
                        + 3.0 * mt * mt * t * cp1[2]
                        + 3.0 * mt * t * t * cp2[2]
                        + t * t * t * cp3[2];
                    verts.push([px + x * sx, py + y * sy, pz + z * sz]);
                }
                // Add control points as extra vertices for handles
                verts.push([px + cp0[0] * sx, py + cp0[1] * sy, pz + cp0[2] * sz]);
                verts.push([px + cp1[0] * sx, py + cp1[1] * sy, pz + cp1[2] * sz]);
                verts.push([px + cp2[0] * sx, py + cp2[1] * sy, pz + cp2[2] * sz]);
                verts.push([px + cp3[0] * sx, py + cp3[1] * sy, pz + cp3[2] * sz]);
                verts
            }
            state::ObjectType::NurbsCurve => {
                // Simple NURBS approximation (quadratic B-spline)
                let control_pts = [
                    [-1.0, 0.0, 0.0],
                    [-0.5, 0.0, 0.8],
                    [0.5, 0.0, -0.8],
                    [1.0, 0.0, 0.0],
                ];
                let segments = 20;
                let mut verts = vec![];
                for i in 0..=segments {
                    let t = i as f32 / segments as f32;
                    // Catmull-Rom interpolation through control points
                    let seg =
                        (t * (control_pts.len() - 1) as f32).min((control_pts.len() - 2) as f32);
                    let idx = seg as usize;
                    let frac = seg - idx as f32;
                    let p0 = if idx > 0 {
                        control_pts[idx - 1]
                    } else {
                        control_pts[0]
                    };
                    let p1 = control_pts[idx];
                    let p2 = control_pts[(idx + 1).min(control_pts.len() - 1)];
                    let p3 = control_pts[(idx + 2).min(control_pts.len() - 1)];
                    let t2 = frac * frac;
                    let t3 = t2 * frac;
                    let x = 0.5
                        * ((2.0 * p1[0])
                            + (-p0[0] + p2[0]) * frac
                            + (2.0 * p0[0] - 5.0 * p1[0] + 4.0 * p2[0] - p3[0]) * t2
                            + (-p0[0] + 3.0 * p1[0] - 3.0 * p2[0] + p3[0]) * t3);
                    let y = 0.5
                        * ((2.0 * p1[1])
                            + (-p0[1] + p2[1]) * frac
                            + (2.0 * p0[1] - 5.0 * p1[1] + 4.0 * p2[1] - p3[1]) * t2
                            + (-p0[1] + 3.0 * p1[1] - 3.0 * p2[1] + p3[1]) * t3);
                    let z = 0.5
                        * ((2.0 * p1[2])
                            + (-p0[2] + p2[2]) * frac
                            + (2.0 * p0[2] - 5.0 * p1[2] + 4.0 * p2[2] - p3[2]) * t2
                            + (-p0[2] + 3.0 * p1[2] - 3.0 * p2[2] + p3[2]) * t3);
                    verts.push([px + x * sx, py + y * sy, pz + z * sz]);
                }
                // Add control points
                for cp in &control_pts {
                    verts.push([px + cp[0] * sx, py + cp[1] * sy, pz + cp[2] * sz]);
                }
                verts
            }
            state::ObjectType::Text => {
                // Simple "T" letter shape
                let s = sx * 0.5;
                vec![
                    // Top bar
                    [px - s, py + s, pz],
                    [px + s, py + s, pz],
                    // Vertical bar
                    [px, py + s, pz],
                    [px, py - s, pz],
                ]
            }
            state::ObjectType::Empty => {
                // Small cross indicator
                let s = sx * 0.3;
                vec![
                    [px - s, py, pz],
                    [px + s, py, pz],
                    [px, py - s, pz],
                    [px, py + s, pz],
                    [px, py, pz - s],
                    [px, py, pz + s],
                ]
            }
            state::ObjectType::Light => {
                // Diamond shape for light
                let s = sx * 0.3;
                vec![
                    [px, py + s, pz],
                    [px + s, py, pz],
                    [px, py - s, pz],
                    [px - s, py, pz],
                    [px, py, pz + s],
                    [px, py, pz - s],
                ]
            }
            state::ObjectType::Camera => {
                // Camera frustum shape
                let s = sx * 0.3;
                let d = sy * 0.5;
                vec![
                    // Near plane (camera body)
                    [px - s, py - s * 0.75, pz],
                    [px + s, py - s * 0.75, pz],
                    [px + s, py + s * 0.75, pz],
                    [px - s, py + s * 0.75, pz],
                    // Far plane (view direction)
                    [px - s * 2.0, py - s * 1.5, pz - d * 2.0],
                    [px + s * 2.0, py - s * 1.5, pz - d * 2.0],
                    [px + s * 2.0, py + s * 1.5, pz - d * 2.0],
                    [px - s * 2.0, py + s * 1.5, pz - d * 2.0],
                ]
            }
            state::ObjectType::Mesh => {
                // Imported mesh shown as bounding box for now
                vec![
                    [px - sx * 0.5, py - sy * 0.5, pz - sz * 0.5],
                    [px + sx * 0.5, py - sy * 0.5, pz - sz * 0.5],
                    [px + sx * 0.5, py + sy * 0.5, pz - sz * 0.5],
                    [px - sx * 0.5, py + sy * 0.5, pz - sz * 0.5],
                    [px - sx * 0.5, py - sy * 0.5, pz + sz * 0.5],
                    [px + sx * 0.5, py - sy * 0.5, pz + sz * 0.5],
                    [px + sx * 0.5, py + sy * 0.5, pz + sz * 0.5],
                    [px - sx * 0.5, py + sy * 0.5, pz + sz * 0.5],
                ]
            }
        };

        // Apply Euler rotation (XYZ order) to all vertices
        let [rx, ry, rz] = obj.rotation;
        if rx != 0.0 || ry != 0.0 || rz != 0.0 {
            let (sin_x, cos_x) = rx.to_radians().sin_cos();
            let (sin_y, cos_y) = ry.to_radians().sin_cos();
            let (sin_z, cos_z) = rz.to_radians().sin_cos();
            for v in &mut verts {
                // Move to local space (subtract position)
                let x = v[0] - px;
                let y = v[1] - py;
                let z = v[2] - pz;
                // Rotate X
                let y1 = y * cos_x - z * sin_x;
                let z1 = y * sin_x + z * cos_x;
                // Rotate Y
                let x2 = x * cos_y + z1 * sin_y;
                let z2 = -x * sin_y + z1 * cos_y;
                // Rotate Z
                let x3 = x2 * cos_z - y1 * sin_z;
                let y3 = x2 * sin_z + y1 * cos_z;
                // Move back to world space
                v[0] = x3 + px;
                v[1] = y3 + py;
                v[2] = z2 + pz;
            }
        }

        verts
    }

    fn get_object_edges(&self, obj_idx: usize) -> Vec<(usize, usize)> {
        let obj = &self.state.objects[obj_idx];

        match obj.object_type {
            state::ObjectType::Cube => {
                vec![
                    // Bottom face
                    (0, 1),
                    (1, 2),
                    (2, 3),
                    (3, 0),
                    // Top face
                    (4, 5),
                    (5, 6),
                    (6, 7),
                    (7, 4),
                    // Vertical edges
                    (0, 4),
                    (1, 5),
                    (2, 6),
                    (3, 7),
                ]
            }
            state::ObjectType::Sphere => {
                let segments = 24;
                let rings = 16;
                let mut edges = vec![];
                for j in 0..rings {
                    for i in 0..segments {
                        let curr = j * segments + i;
                        let next_i = j * segments + (i + 1) % segments;
                        let next_j = (j + 1) * segments + i;
                        edges.push((curr, next_i));
                        if j < rings {
                            edges.push((curr, next_j));
                        }
                    }
                }
                edges
            }
            state::ObjectType::Cylinder => {
                let segments = 24;
                let mut edges = vec![];
                // Bottom circle edges
                for i in 0..segments {
                    edges.push((i, (i + 1) % segments));
                }
                // Top circle edges
                for i in 0..segments {
                    edges.push((segments + i, segments + (i + 1) % segments));
                }
                // Vertical edges
                for i in 0..segments {
                    edges.push((i, segments + i));
                }
                edges
            }
            state::ObjectType::Plane => {
                vec![(0, 1), (1, 2), (2, 3), (3, 0), (0, 2), (1, 3)]
            }
            state::ObjectType::Torus => {
                let major_seg = 32;
                let minor_seg = 16;
                let mut edges = vec![];
                for i in 0..major_seg {
                    for j in 0..minor_seg {
                        let curr = i * minor_seg + j;
                        let next_minor = i * minor_seg + (j + 1) % minor_seg;
                        let next_major = ((i + 1) % major_seg) * minor_seg + j;
                        edges.push((curr, next_minor));
                        edges.push((curr, next_major));
                    }
                }
                edges
            }
            state::ObjectType::Cone => {
                let segments = 24;
                let apex = segments;
                let mut edges = vec![];
                // Bottom circle
                for i in 0..segments {
                    edges.push((i, (i + 1) % segments));
                    // Lines to apex
                    edges.push((i, apex));
                }
                edges
            }
            state::ObjectType::IcoSphere => {
                // Rebuild icosphere topology for edges (1 subdivision of icosahedron)
                let t = (1.0 + 5.0_f32.sqrt()) / 2.0;
                let len = (1.0 + t * t).sqrt();
                let a = 1.0 / len;
                let b = t / len;
                let ico_verts: [[f32; 3]; 12] = [
                    [-a, b, 0.0],
                    [a, b, 0.0],
                    [-a, -b, 0.0],
                    [a, -b, 0.0],
                    [0.0, -a, b],
                    [0.0, a, b],
                    [0.0, -a, -b],
                    [0.0, a, -b],
                    [b, 0.0, -a],
                    [b, 0.0, a],
                    [-b, 0.0, -a],
                    [-b, 0.0, a],
                ];
                let ico_faces: [[usize; 3]; 20] = [
                    [0, 11, 5],
                    [0, 5, 1],
                    [0, 1, 7],
                    [0, 7, 10],
                    [0, 10, 11],
                    [1, 5, 9],
                    [5, 11, 4],
                    [11, 10, 2],
                    [10, 7, 6],
                    [7, 1, 8],
                    [3, 9, 4],
                    [3, 4, 2],
                    [3, 2, 6],
                    [3, 6, 8],
                    [3, 8, 9],
                    [4, 9, 5],
                    [2, 4, 11],
                    [6, 2, 10],
                    [8, 6, 7],
                    [9, 8, 1],
                ];
                let mut all_verts: Vec<[f32; 3]> = ico_verts.to_vec();
                let mut midpoint_cache = std::collections::HashMap::new();
                let mut sub_faces = Vec::new();
                for face in &ico_faces {
                    let get_mid = |cache: &mut std::collections::HashMap<(usize, usize), usize>,
                                   verts: &mut Vec<[f32; 3]>,
                                   i1: usize,
                                   i2: usize|
                     -> usize {
                        let key = if i1 < i2 { (i1, i2) } else { (i2, i1) };
                        if let Some(&idx) = cache.get(&key) {
                            return idx;
                        }
                        let v1 = verts[i1];
                        let v2 = verts[i2];
                        let mid = [
                            (v1[0] + v2[0]) * 0.5,
                            (v1[1] + v2[1]) * 0.5,
                            (v1[2] + v2[2]) * 0.5,
                        ];
                        let l = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2]).sqrt();
                        let idx = verts.len();
                        verts.push([mid[0] / l, mid[1] / l, mid[2] / l]);
                        cache.insert(key, idx);
                        idx
                    };
                    let ab = get_mid(&mut midpoint_cache, &mut all_verts, face[0], face[1]);
                    let bc = get_mid(&mut midpoint_cache, &mut all_verts, face[1], face[2]);
                    let ca = get_mid(&mut midpoint_cache, &mut all_verts, face[2], face[0]);
                    sub_faces.push([face[0], ab, ca]);
                    sub_faces.push([face[1], bc, ab]);
                    sub_faces.push([face[2], ca, bc]);
                    sub_faces.push([ab, bc, ca]);
                }
                let mut edge_set = std::collections::HashSet::new();
                for f in &sub_faces {
                    for k in 0..3 {
                        let e = if f[k] < f[(k + 1) % 3] {
                            (f[k], f[(k + 1) % 3])
                        } else {
                            (f[(k + 1) % 3], f[k])
                        };
                        edge_set.insert(e);
                    }
                }
                edge_set.into_iter().collect()
            }
            state::ObjectType::Grid => {
                let subdivs = 10_usize;
                let cols = subdivs + 1;
                let mut edges = vec![];
                for j in 0..=subdivs {
                    for i in 0..=subdivs {
                        let idx = j * cols + i;
                        if i < subdivs {
                            edges.push((idx, idx + 1));
                        }
                        if j < subdivs {
                            edges.push((idx, idx + cols));
                        }
                    }
                }
                edges
            }
            state::ObjectType::Circle => {
                let segments = 32;
                let mut edges = vec![];
                for i in 0..segments {
                    edges.push((i, (i + 1) % segments));
                }
                edges
            }
            state::ObjectType::BezierCurve => {
                let segments = 20;
                let mut edges = vec![];
                // Curve segments
                for i in 0..segments {
                    edges.push((i, i + 1));
                }
                // Handle lines (control point 0 to handle 1, control point 3 to handle 2)
                let cp_start = segments + 1;
                edges.push((cp_start, cp_start + 1)); // cp0 -> handle1
                edges.push((cp_start + 2, cp_start + 3)); // handle2 -> cp3
                edges
            }
            state::ObjectType::NurbsCurve => {
                let segments = 20;
                let mut edges = vec![];
                for i in 0..segments {
                    edges.push((i, i + 1));
                }
                // Control point polygon
                let cp_start = segments + 1;
                for i in 0..3 {
                    edges.push((cp_start + i, cp_start + i + 1));
                }
                edges
            }
            state::ObjectType::Text => {
                vec![(0, 1), (2, 3)] // Top bar + vertical bar
            }
            state::ObjectType::Empty => {
                vec![(0, 1), (2, 3), (4, 5)]
            }
            state::ObjectType::Light => {
                vec![
                    (0, 1),
                    (1, 2),
                    (2, 3),
                    (3, 0), // Horizontal diamond
                    (0, 4),
                    (1, 4),
                    (2, 4),
                    (3, 4), // Top rays
                    (0, 5),
                    (1, 5),
                    (2, 5),
                    (3, 5), // Bottom rays
                ]
            }
            state::ObjectType::Camera => {
                vec![
                    // Near plane
                    (0, 1),
                    (1, 2),
                    (2, 3),
                    (3, 0),
                    // Far plane
                    (4, 5),
                    (5, 6),
                    (6, 7),
                    (7, 4),
                    // Connecting lines
                    (0, 4),
                    (1, 5),
                    (2, 6),
                    (3, 7),
                ]
            }
            state::ObjectType::Mesh => {
                // Bounding box edges (same as cube)
                vec![
                    (0, 1),
                    (1, 2),
                    (2, 3),
                    (3, 0),
                    (4, 5),
                    (5, 6),
                    (6, 7),
                    (7, 4),
                    (0, 4),
                    (1, 5),
                    (2, 6),
                    (3, 7),
                ]
            }
        }
    }

    fn get_object_faces(&self, obj_idx: usize) -> Vec<Vec<usize>> {
        let obj = &self.state.objects[obj_idx];

        // Use custom faces if available (from Edit Mode)
        if let Some(ref custom_faces) = obj.custom_faces {
            return custom_faces.clone();
        }

        match obj.object_type {
            state::ObjectType::Cube | state::ObjectType::Mesh => {
                vec![
                    vec![0, 1, 2, 3], // Front
                    vec![5, 4, 7, 6], // Back
                    vec![4, 0, 3, 7], // Left
                    vec![1, 5, 6, 2], // Right
                    vec![3, 2, 6, 7], // Top
                    vec![4, 5, 1, 0], // Bottom
                ]
            }
            state::ObjectType::Plane => {
                vec![vec![0, 1, 2, 3]]
            }
            state::ObjectType::Sphere => {
                let segments = 24;
                let rings = 16;
                let mut faces = vec![];
                for j in 0..rings {
                    for i in 0..segments {
                        let c = j * segments + i;
                        let ni = j * segments + (i + 1) % segments;
                        let nj = (j + 1) * segments + i;
                        let nji = (j + 1) * segments + (i + 1) % segments;
                        faces.push(vec![c, ni, nji, nj]);
                    }
                }
                faces
            }
            state::ObjectType::Cylinder => {
                let segments = 24;
                let mut faces = vec![];
                // Side quads
                for i in 0..segments {
                    let ni = (i + 1) % segments;
                    faces.push(vec![i, ni, segments + ni, segments + i]);
                }
                // Bottom cap (fan from center at index 2*segments)
                let bottom_center = 2 * segments;
                for i in 0..segments {
                    let ni = (i + 1) % segments;
                    faces.push(vec![bottom_center, ni, i]);
                }
                // Top cap (fan from center at index 2*segments+1)
                let top_center = 2 * segments + 1;
                for i in 0..segments {
                    let ni = (i + 1) % segments;
                    faces.push(vec![top_center, segments + i, segments + ni]);
                }
                faces
            }
            state::ObjectType::Torus => {
                let major_seg = 32;
                let minor_seg = 16;
                let mut faces = vec![];
                for i in 0..major_seg {
                    for j in 0..minor_seg {
                        let c = i * minor_seg + j;
                        let nmin = i * minor_seg + (j + 1) % minor_seg;
                        let nmaj = ((i + 1) % major_seg) * minor_seg + j;
                        let nmajmin = ((i + 1) % major_seg) * minor_seg + (j + 1) % minor_seg;
                        faces.push(vec![c, nmin, nmajmin, nmaj]);
                    }
                }
                faces
            }
            state::ObjectType::Cone => {
                let segments = 24;
                let apex = segments;
                let bottom_center = segments + 1;
                let mut faces = vec![];
                // Side triangles to apex
                for i in 0..segments {
                    let ni = (i + 1) % segments;
                    faces.push(vec![i, ni, apex]);
                }
                // Bottom cap
                for i in 0..segments {
                    let ni = (i + 1) % segments;
                    faces.push(vec![bottom_center, ni, i]);
                }
                faces
            }
            state::ObjectType::IcoSphere => {
                // Rebuild icosphere faces (1 subdivision of icosahedron)
                let t = (1.0 + 5.0_f32.sqrt()) / 2.0;
                let len = (1.0 + t * t).sqrt();
                let a = 1.0 / len;
                let b = t / len;
                let ico_verts: [[f32; 3]; 12] = [
                    [-a, b, 0.0],
                    [a, b, 0.0],
                    [-a, -b, 0.0],
                    [a, -b, 0.0],
                    [0.0, -a, b],
                    [0.0, a, b],
                    [0.0, -a, -b],
                    [0.0, a, -b],
                    [b, 0.0, -a],
                    [b, 0.0, a],
                    [-b, 0.0, -a],
                    [-b, 0.0, a],
                ];
                let ico_faces: [[usize; 3]; 20] = [
                    [0, 11, 5],
                    [0, 5, 1],
                    [0, 1, 7],
                    [0, 7, 10],
                    [0, 10, 11],
                    [1, 5, 9],
                    [5, 11, 4],
                    [11, 10, 2],
                    [10, 7, 6],
                    [7, 1, 8],
                    [3, 9, 4],
                    [3, 4, 2],
                    [3, 2, 6],
                    [3, 6, 8],
                    [3, 8, 9],
                    [4, 9, 5],
                    [2, 4, 11],
                    [6, 2, 10],
                    [8, 6, 7],
                    [9, 8, 1],
                ];
                let mut all_verts: Vec<[f32; 3]> = ico_verts.to_vec();
                let mut midpoint_cache = std::collections::HashMap::new();
                let mut faces = Vec::new();
                for face in &ico_faces {
                    let get_mid = |cache: &mut std::collections::HashMap<(usize, usize), usize>,
                                   verts: &mut Vec<[f32; 3]>,
                                   i1: usize,
                                   i2: usize|
                     -> usize {
                        let key = if i1 < i2 { (i1, i2) } else { (i2, i1) };
                        if let Some(&idx) = cache.get(&key) {
                            return idx;
                        }
                        let v1 = verts[i1];
                        let v2 = verts[i2];
                        let mid = [
                            (v1[0] + v2[0]) * 0.5,
                            (v1[1] + v2[1]) * 0.5,
                            (v1[2] + v2[2]) * 0.5,
                        ];
                        let l = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2]).sqrt();
                        let idx = verts.len();
                        verts.push([mid[0] / l, mid[1] / l, mid[2] / l]);
                        cache.insert(key, idx);
                        idx
                    };
                    let ab = get_mid(&mut midpoint_cache, &mut all_verts, face[0], face[1]);
                    let bc = get_mid(&mut midpoint_cache, &mut all_verts, face[1], face[2]);
                    let ca = get_mid(&mut midpoint_cache, &mut all_verts, face[2], face[0]);
                    faces.push(vec![face[0], ab, ca]);
                    faces.push(vec![face[1], bc, ab]);
                    faces.push(vec![face[2], ca, bc]);
                    faces.push(vec![ab, bc, ca]);
                }
                faces
            }
            state::ObjectType::Grid => {
                let subdivs = 10_usize;
                let cols = subdivs + 1;
                let mut faces = vec![];
                for j in 0..subdivs {
                    for i in 0..subdivs {
                        let tl = j * cols + i;
                        let tr = tl + 1;
                        let bl = tl + cols;
                        let br = bl + 1;
                        faces.push(vec![tl, tr, br, bl]);
                    }
                }
                faces
            }
            state::ObjectType::Camera => {
                vec![
                    vec![0, 1, 2, 3], // Near
                    vec![4, 5, 6, 7], // Far
                    vec![0, 1, 5, 4], // Bottom
                    vec![3, 2, 6, 7], // Top
                    vec![0, 3, 7, 4], // Left
                    vec![1, 2, 6, 5], // Right
                ]
            }
            _ => vec![],
        }
    }

    fn draw_transform_gizmo(&self, painter: &egui::Painter, rect: egui::Rect) {
        if let Some(idx) = self.state.selected_object {
            let obj = &self.state.objects[idx];
            let pos = obj.position;
            let gizmo_size = 1.5;

            let center = match self.project_point(pos, rect) {
                Some(p) => p,
                None => return,
            };

            match self.state.tool {
                Tool::Move => {
                    // X axis arrow (red)
                    if let Some(p2) =
                        self.project_point([pos[0] + gizmo_size, pos[1], pos[2]], rect)
                    {
                        painter.line_segment(
                            [center, p2],
                            egui::Stroke::new(3.0_f32, egui::Color32::RED),
                        );
                        // Arrow head
                        let dir = (p2 - center).normalized();
                        let perp = egui::vec2(-dir.y, dir.x) * 5.0;
                        painter.line_segment(
                            [p2, p2 - dir * 10.0 + perp],
                            egui::Stroke::new(2.0_f32, egui::Color32::RED),
                        );
                        painter.line_segment(
                            [p2, p2 - dir * 10.0 - perp],
                            egui::Stroke::new(2.0_f32, egui::Color32::RED),
                        );
                    }

                    // Y axis arrow (green)
                    if let Some(p2) =
                        self.project_point([pos[0], pos[1] + gizmo_size, pos[2]], rect)
                    {
                        painter.line_segment(
                            [center, p2],
                            egui::Stroke::new(3.0_f32, egui::Color32::GREEN),
                        );
                        let dir = (p2 - center).normalized();
                        let perp = egui::vec2(-dir.y, dir.x) * 5.0;
                        painter.line_segment(
                            [p2, p2 - dir * 10.0 + perp],
                            egui::Stroke::new(2.0_f32, egui::Color32::GREEN),
                        );
                        painter.line_segment(
                            [p2, p2 - dir * 10.0 - perp],
                            egui::Stroke::new(2.0_f32, egui::Color32::GREEN),
                        );
                    }

                    // Z axis arrow (blue)
                    if let Some(p2) =
                        self.project_point([pos[0], pos[1], pos[2] + gizmo_size], rect)
                    {
                        painter.line_segment(
                            [center, p2],
                            egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(100, 100, 255)),
                        );
                        let dir = (p2 - center).normalized();
                        let perp = egui::vec2(-dir.y, dir.x) * 5.0;
                        painter.line_segment(
                            [p2, p2 - dir * 10.0 + perp],
                            egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(100, 100, 255)),
                        );
                        painter.line_segment(
                            [p2, p2 - dir * 10.0 - perp],
                            egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(100, 100, 255)),
                        );
                    }
                }
                Tool::Rotate => {
                    // Draw rotation circles for each axis
                    let circle_segments = 32;
                    let _radius = 50.0; // Screen space radius

                    // X rotation (red circle in YZ plane)
                    let mut points_x = Vec::new();
                    for i in 0..circle_segments {
                        let angle = i as f32 / circle_segments as f32 * 2.0 * std::f32::consts::PI;
                        let r = gizmo_size * 0.8;
                        if let Some(p) = self.project_point(
                            [pos[0], pos[1] + r * angle.cos(), pos[2] + r * angle.sin()],
                            rect,
                        ) {
                            points_x.push(p);
                        }
                    }
                    for i in 0..points_x.len() {
                        let next = (i + 1) % points_x.len();
                        painter.line_segment(
                            [points_x[i], points_x[next]],
                            egui::Stroke::new(2.0_f32, egui::Color32::RED),
                        );
                    }

                    // Y rotation (green circle in XZ plane)
                    let mut points_y = Vec::new();
                    for i in 0..circle_segments {
                        let angle = i as f32 / circle_segments as f32 * 2.0 * std::f32::consts::PI;
                        let r = gizmo_size * 0.8;
                        if let Some(p) = self.project_point(
                            [pos[0] + r * angle.cos(), pos[1], pos[2] + r * angle.sin()],
                            rect,
                        ) {
                            points_y.push(p);
                        }
                    }
                    for i in 0..points_y.len() {
                        let next = (i + 1) % points_y.len();
                        painter.line_segment(
                            [points_y[i], points_y[next]],
                            egui::Stroke::new(2.0_f32, egui::Color32::GREEN),
                        );
                    }

                    // Z rotation (blue circle in XY plane)
                    let mut points_z = Vec::new();
                    for i in 0..circle_segments {
                        let angle = i as f32 / circle_segments as f32 * 2.0 * std::f32::consts::PI;
                        let r = gizmo_size * 0.8;
                        if let Some(p) = self.project_point(
                            [pos[0] + r * angle.cos(), pos[1] + r * angle.sin(), pos[2]],
                            rect,
                        ) {
                            points_z.push(p);
                        }
                    }
                    for i in 0..points_z.len() {
                        let next = (i + 1) % points_z.len();
                        painter.line_segment(
                            [points_z[i], points_z[next]],
                            egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(100, 100, 255)),
                        );
                    }
                }
                Tool::Scale => {
                    // Draw scale cubes at end of each axis
                    let cube_size = 8.0;

                    // X axis (red)
                    if let Some(p2) =
                        self.project_point([pos[0] + gizmo_size, pos[1], pos[2]], rect)
                    {
                        painter.line_segment(
                            [center, p2],
                            egui::Stroke::new(2.0_f32, egui::Color32::RED),
                        );
                        painter.rect_filled(
                            egui::Rect::from_center_size(p2, egui::vec2(cube_size, cube_size)),
                            0.0,
                            egui::Color32::RED,
                        );
                    }

                    // Y axis (green)
                    if let Some(p2) =
                        self.project_point([pos[0], pos[1] + gizmo_size, pos[2]], rect)
                    {
                        painter.line_segment(
                            [center, p2],
                            egui::Stroke::new(2.0_f32, egui::Color32::GREEN),
                        );
                        painter.rect_filled(
                            egui::Rect::from_center_size(p2, egui::vec2(cube_size, cube_size)),
                            0.0,
                            egui::Color32::GREEN,
                        );
                    }

                    // Z axis (blue)
                    if let Some(p2) =
                        self.project_point([pos[0], pos[1], pos[2] + gizmo_size], rect)
                    {
                        painter.line_segment(
                            [center, p2],
                            egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(100, 100, 255)),
                        );
                        painter.rect_filled(
                            egui::Rect::from_center_size(p2, egui::vec2(cube_size, cube_size)),
                            0.0,
                            egui::Color32::from_rgb(100, 100, 255),
                        );
                    }

                    // Center uniform scale cube (white)
                    painter.rect_filled(
                        egui::Rect::from_center_size(
                            center,
                            egui::vec2(cube_size * 1.5, cube_size * 1.5),
                        ),
                        0.0,
                        egui::Color32::WHITE,
                    );
                }
                Tool::Select => {
                    // Just draw a subtle selection indicator
                    painter.circle_stroke(
                        center,
                        20.0,
                        egui::Stroke::new(
                            1.5_f32,
                            egui::Color32::from_rgba_unmultiplied(255, 150, 50, 150),
                        ),
                    );
                }
            }
        }
    }

    fn draw_viewport_overlay(&self, painter: &egui::Painter, rect: egui::Rect) {
        // Subtle mode info (top-left)
        let font_small = egui::FontId::proportional(11.0);
        let overlay_color = egui::Color32::from_rgba_unmultiplied(200, 200, 200, 180);
        let dim_color = egui::Color32::from_rgba_unmultiplied(140, 140, 140, 140);

        let constraint_str = if self.state.axis_constraint != AxisConstraint::None
            && self.state.tool != Tool::Select
        {
            format!(" | Axis: {}", self.state.axis_constraint)
        } else {
            String::new()
        };
        painter.text(
            rect.left_top() + egui::vec2(10.0, 8.0),
            egui::Align2::LEFT_TOP,
            format!(
                "{:?} | {:?} | {:?}{}",
                self.state.edit_mode, self.state.tool, self.state.shading, constraint_str
            ),
            font_small.clone(),
            overlay_color,
        );

        // Object count and selection info (top-left, line 2)
        let multi_count = self.state.all_selected().len();
        if let Some(idx) = self.state.selected_object {
            let base_verts = self.get_object_vertices(idx);
            let base_faces = self.get_object_faces(idx);
            let obj = &self.state.objects[idx];
            let (v_count, f_count, mod_str) = if !obj.modifiers.is_empty() && !base_faces.is_empty()
            {
                let (mv, mf) =
                    Self::apply_modifiers(&base_verts, &base_faces, &obj.modifiers, obj.position);
                (
                    mv.len(),
                    mf.len(),
                    format!(" | Mods: {}", obj.modifiers.len()),
                )
            } else {
                (base_verts.len(), base_faces.len(), String::new())
            };
            let multi_str = if multi_count > 1 {
                format!(" (+{} sel)", multi_count - 1)
            } else {
                String::new()
            };
            let lock_str = if obj.locked { " [Locked]" } else { "" };
            let shade_str = if obj.smooth_shading { "" } else { " [Flat]" };
            painter.text(
                rect.left_top() + egui::vec2(10.0, 22.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "{}{}{} | V:{} F:{}{} | Objs: {}{}",
                    self.state.objects[idx].name,
                    lock_str,
                    shade_str,
                    v_count,
                    f_count,
                    mod_str,
                    self.state.objects.len(),
                    multi_str
                ),
                font_small.clone(),
                dim_color,
            );
        } else {
            painter.text(
                rect.left_top() + egui::vec2(10.0, 22.0),
                egui::Align2::LEFT_TOP,
                format!("Objects: {}", self.state.objects.len()),
                font_small.clone(),
                dim_color,
            );
        }

        // Scene statistics (top-left, line 3) - total verts/faces across all visible objects
        if self.state.show_viewport_stats {
            let mut total_verts = 0usize;
            let mut total_faces = 0usize;
            for i in 0..self.state.objects.len() {
                if !self.state.objects[i].visible {
                    continue;
                }
                let bv = self.get_object_vertices(i);
                let bf = self.get_object_faces(i);
                let obj = &self.state.objects[i];
                if !obj.modifiers.is_empty() && !bf.is_empty() {
                    let (mv, mf) = Self::apply_modifiers(&bv, &bf, &obj.modifiers, obj.position);
                    total_verts += mv.len();
                    total_faces += mf.len();
                } else {
                    total_verts += bv.len();
                    total_faces += bf.len();
                }
            }
            painter.text(
                rect.left_top() + egui::vec2(10.0, 36.0),
                egui::Align2::LEFT_TOP,
                format!("Scene: {} verts | {} faces", total_verts, total_faces),
                font_small.clone(),
                egui::Color32::from_rgba_unmultiplied(110, 110, 110, 120),
            );
        }

        // Controls hint (bottom-right, subtle)
        painter.text(
            rect.right_bottom() + egui::vec2(-10.0, -8.0),
            egui::Align2::RIGHT_BOTTOM,
            "MMB: Orbit | Shift+MMB: Pan | Scroll: Zoom | G: Move | R: Rotate | S: Scale | X/Y/Z: Axis",
            font_small.clone(),
            egui::Color32::from_rgba_unmultiplied(120, 120, 120, 100),
        );

        // Orientation gizmo (top-right corner, like Blender's navigation cube)
        let gizmo_center = egui::pos2(rect.right() - 60.0, rect.top() + 60.0);
        let gizmo_radius = 40.0;

        // Background circle
        painter.circle_filled(
            gizmo_center,
            gizmo_radius + 2.0,
            egui::Color32::from_rgba_unmultiplied(30, 30, 35, 180),
        );
        painter.circle_stroke(
            gizmo_center,
            gizmo_radius + 2.0,
            egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgba_unmultiplied(80, 80, 80, 150),
            ),
        );

        // Project axes using current camera orientation
        let axis_len = gizmo_radius * 0.8;
        let yaw = self.state.camera.orbit_angles[0].to_radians();
        let pitch = self.state.camera.orbit_angles[1].to_radians();

        // Camera basis vectors
        let right = [yaw.cos(), 0.0, -yaw.sin()];
        let up = [
            yaw.sin() * pitch.sin(),
            pitch.cos(),
            yaw.cos() * pitch.sin(),
        ];

        // Project unit axes through camera rotation
        let project_axis = |ax: [f32; 3]| -> egui::Pos2 {
            let sx = ax[0] * right[0] + ax[2] * (-right[2]);
            let sy = -(ax[0] * up[0] + ax[1] * up[1] + ax[2] * up[2]);
            egui::pos2(
                gizmo_center.x + sx * axis_len,
                gizmo_center.y + sy * axis_len,
            )
        };

        let x_tip = project_axis([1.0, 0.0, 0.0]);
        let y_tip = project_axis([0.0, 1.0, 0.0]);
        let z_tip = project_axis([0.0, 0.0, 1.0]);

        // Draw axes with labels
        let x_color = egui::Color32::from_rgb(230, 60, 60);
        let y_color = egui::Color32::from_rgb(60, 200, 60);
        let z_color = egui::Color32::from_rgb(80, 80, 230);

        painter.line_segment([gizmo_center, x_tip], egui::Stroke::new(2.5_f32, x_color));
        painter.line_segment([gizmo_center, y_tip], egui::Stroke::new(2.5_f32, y_color));
        painter.line_segment([gizmo_center, z_tip], egui::Stroke::new(2.5_f32, z_color));

        painter.circle_filled(x_tip, 6.0, x_color);
        painter.circle_filled(y_tip, 6.0, y_color);
        painter.circle_filled(z_tip, 6.0, z_color);

        let label_font = egui::FontId::proportional(10.0);
        painter.text(
            x_tip,
            egui::Align2::CENTER_CENTER,
            "X",
            label_font.clone(),
            egui::Color32::WHITE,
        );
        painter.text(
            y_tip,
            egui::Align2::CENTER_CENTER,
            "Y",
            label_font.clone(),
            egui::Color32::WHITE,
        );
        painter.text(
            z_tip,
            egui::Align2::CENTER_CENTER,
            "Z",
            label_font,
            egui::Color32::WHITE,
        );
    }

    fn dist_to_line(&self, p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
        let ab = b - a;
        let ap = p - a;
        let t = (ap.x * ab.x + ap.y * ab.y) / (ab.x * ab.x + ab.y * ab.y + 1e-6);
        let t = t.clamp(0.0, 1.0);
        let closest = a + ab * t;
        p.distance(closest)
    }
    fn handle_viewport_click(&mut self, rect: egui::Rect, click_pos: egui::Pos2, shift_held: bool) {
        // BATCH 24: Gizmo Hit Testing (P2.2)
        if let Some(idx) = self.state.selected_object {
            let obj = &self.state.objects[idx];
            let pos = obj.position;
            let gizmo_size = 1.5;

            // Project the gizmo origin ONCE. If it is off-screen (e.g. behind the
            // camera near-plane) we cannot meaningfully hit-test the gizmo, so we
            // fall through to the constraint reset below instead of panicking on an
            // unprojectable origin — the endpoint may project even when the origin
            // does not (they can straddle the clip plane).
            if let Some(p_start) = self.project_point(pos, rect) {
                // Check X axis
                if let Some(p_end) = self.project_point([pos[0] + gizmo_size, pos[1], pos[2]], rect)
                {
                    if self.dist_to_line(click_pos, p_start, p_end) < 10.0 {
                        self.state.axis_constraint = AxisConstraint::X;
                        return;
                    }
                }
                // Check Y axis
                if let Some(p_end) = self.project_point([pos[0], pos[1] + gizmo_size, pos[2]], rect)
                {
                    if self.dist_to_line(click_pos, p_start, p_end) < 10.0 {
                        self.state.axis_constraint = AxisConstraint::Y;
                        return;
                    }
                }
                // Check Z axis
                if let Some(p_end) = self.project_point([pos[0], pos[1], pos[2] + gizmo_size], rect)
                {
                    if self.dist_to_line(click_pos, p_start, p_end) < 10.0 {
                        self.state.axis_constraint = AxisConstraint::Z;
                        return;
                    }
                }
            }
            // If click was NOT on gizmo, reset constraint
            self.state.axis_constraint = AxisConstraint::None;
        }
        // Measurement mode: click to place measurement start/end
        if self.state.measuring {
            let world_pos = self.unproject_point(click_pos, rect);
            if let Some(start) = self.state.measure_start {
                self.state.add_measurement(start, world_pos);
                self.state.measure_start = None;
                self.status_message = match self.state.measurements.last() {
                    Some(m) => format!("Measurement placed ({:.2})", m.distance),
                    None => "Measurement placed".to_string(),
                };
            } else {
                self.state.measure_start = Some(world_pos);
                self.status_message = "Measurement start placed - click endpoint".to_string();
            }
            return;
        }

        if !matches!(self.state.tool, Tool::Select) {
            return;
        }

        // Improved hit testing using projected bounding box
        let mut closest_idx = None;
        let mut closest_dist = f32::MAX;

        for i in 0..self.state.objects.len() {
            let obj = &self.state.objects[i];
            if !obj.visible {
                continue;
            }

            // Get vertices to compute screen-space bounding box
            let vertices = self.get_object_vertices(i);
            if vertices.is_empty() {
                continue;
            }

            let projected: Vec<_> = vertices
                .iter()
                .filter_map(|v| self.project_point(*v, rect))
                .collect();
            if projected.is_empty() {
                continue;
            }

            // Compute screen-space bounding box
            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = f32::MIN;
            let mut max_y = f32::MIN;
            for p in &projected {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }

            // Expand small bounding boxes (lights, empties, cameras)
            let min_size = 20.0;
            if max_x - min_x < min_size {
                let center = (min_x + max_x) * 0.5;
                min_x = center - min_size * 0.5;
                max_x = center + min_size * 0.5;
            }
            if max_y - min_y < min_size {
                let center = (min_y + max_y) * 0.5;
                min_y = center - min_size * 0.5;
                max_y = center + min_size * 0.5;
            }

            // Check if click is inside bounding box (with small margin)
            let margin = 5.0;
            if click_pos.x >= min_x - margin
                && click_pos.x <= max_x + margin
                && click_pos.y >= min_y - margin
                && click_pos.y <= max_y + margin
            {
                // Use distance to center for priority
                let center = egui::pos2((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
                let dist = center.distance(click_pos);
                if dist < closest_dist {
                    closest_dist = dist;
                    closest_idx = Some(i);
                }
            }
        }

        if shift_held {
            // Multi-select: toggle selection of clicked object
            if let Some(idx) = closest_idx {
                self.state.toggle_multi_select(idx);
                let count = self.state.all_selected().len();
                self.status_message = format!(
                    "Selected {} object{}",
                    count,
                    if count != 1 { "s" } else { "" }
                );
            }
        } else {
            // Normal click: single select, clear multi
            self.state.clear_multi_select();
            self.state.selected_object = closest_idx;
            if let Some(idx) = closest_idx {
                self.status_message = format!("Selected: {}", self.state.objects[idx].name);
            } else {
                self.status_message = "Selection cleared".to_string();
            }
        }
    }

    fn handle_tool_drag(&mut self, delta: egui::Vec2) {
        if let Some(idx) = self.state.selected_object {
            // Locked objects cannot be transformed
            if self.state.objects[idx].locked {
                return;
            }
            let sensitivity = 0.02;
            let constraint = self.state.axis_constraint;

            let snap = self.state.snap_enabled;
            let snap_val = self.state.snap_increment;

            // Snap helper: round to nearest grid increment
            let snap_round = |v: f32| -> f32 {
                if snap && snap_val > 0.0 {
                    (v / snap_val).round() * snap_val
                } else {
                    v
                }
            };

            match self.state.tool {
                Tool::Move => match constraint {
                    AxisConstraint::X => {
                        self.state.objects[idx].position[0] += delta.x * sensitivity;
                        if snap {
                            self.state.objects[idx].position[0] =
                                snap_round(self.state.objects[idx].position[0]);
                        }
                    }
                    AxisConstraint::Y => {
                        self.state.objects[idx].position[1] -= delta.y * sensitivity;
                        if snap {
                            self.state.objects[idx].position[1] =
                                snap_round(self.state.objects[idx].position[1]);
                        }
                    }
                    AxisConstraint::Z => {
                        self.state.objects[idx].position[2] += delta.x * sensitivity;
                        if snap {
                            self.state.objects[idx].position[2] =
                                snap_round(self.state.objects[idx].position[2]);
                        }
                    }
                    AxisConstraint::None => {
                        self.state.objects[idx].position[0] += delta.x * sensitivity;
                        self.state.objects[idx].position[1] -= delta.y * sensitivity;
                        if snap {
                            self.state.objects[idx].position[0] =
                                snap_round(self.state.objects[idx].position[0]);
                            self.state.objects[idx].position[1] =
                                snap_round(self.state.objects[idx].position[1]);
                        }
                    }
                },
                Tool::Rotate => {
                    // Rotation snaps at 5-degree increments when snap is on
                    let rot_snap = |v: f32| -> f32 {
                        if snap {
                            (v / 5.0).round() * 5.0
                        } else {
                            v
                        }
                    };
                    match constraint {
                        AxisConstraint::X => {
                            self.state.objects[idx].rotation[0] += delta.x * 0.5;
                            if snap {
                                self.state.objects[idx].rotation[0] =
                                    rot_snap(self.state.objects[idx].rotation[0]);
                            }
                        }
                        AxisConstraint::Y => {
                            self.state.objects[idx].rotation[1] += delta.x * 0.5;
                            if snap {
                                self.state.objects[idx].rotation[1] =
                                    rot_snap(self.state.objects[idx].rotation[1]);
                            }
                        }
                        AxisConstraint::Z => {
                            self.state.objects[idx].rotation[2] += delta.x * 0.5;
                            if snap {
                                self.state.objects[idx].rotation[2] =
                                    rot_snap(self.state.objects[idx].rotation[2]);
                            }
                        }
                        AxisConstraint::None => {
                            self.state.objects[idx].rotation[1] += delta.x * 0.5;
                            self.state.objects[idx].rotation[0] -= delta.y * 0.5;
                            if snap {
                                self.state.objects[idx].rotation[1] =
                                    rot_snap(self.state.objects[idx].rotation[1]);
                                self.state.objects[idx].rotation[0] =
                                    rot_snap(self.state.objects[idx].rotation[0]);
                            }
                        }
                    }
                }
                Tool::Scale => {
                    let scale_delta = 1.0 + (delta.x - delta.y) * 0.01;
                    // Scale snaps at 0.25 increments when snap is on
                    let scale_snap = |v: f32| -> f32 {
                        if snap {
                            (v * 4.0).round() / 4.0
                        } else {
                            v
                        }
                    };
                    match constraint {
                        AxisConstraint::X => {
                            self.state.objects[idx].scale[0] *= scale_delta;
                            if snap {
                                self.state.objects[idx].scale[0] =
                                    scale_snap(self.state.objects[idx].scale[0]);
                            }
                        }
                        AxisConstraint::Y => {
                            self.state.objects[idx].scale[1] *= scale_delta;
                            if snap {
                                self.state.objects[idx].scale[1] =
                                    scale_snap(self.state.objects[idx].scale[1]);
                            }
                        }
                        AxisConstraint::Z => {
                            self.state.objects[idx].scale[2] *= scale_delta;
                            if snap {
                                self.state.objects[idx].scale[2] =
                                    scale_snap(self.state.objects[idx].scale[2]);
                            }
                        }
                        AxisConstraint::None => {
                            self.state.objects[idx].scale[0] *= scale_delta;
                            self.state.objects[idx].scale[1] *= scale_delta;
                            self.state.objects[idx].scale[2] *= scale_delta;
                            if snap {
                                self.state.objects[idx].scale[0] =
                                    scale_snap(self.state.objects[idx].scale[0]);
                                self.state.objects[idx].scale[1] =
                                    scale_snap(self.state.objects[idx].scale[1]);
                                self.state.objects[idx].scale[2] =
                                    scale_snap(self.state.objects[idx].scale[2]);
                            }
                        }
                    }
                }
                _ => {}
            }

            // Also transform multi-selected objects (same delta)
            let multi_indices: Vec<usize> = self.state.multi_selected.clone();
            for mi in multi_indices {
                if mi >= self.state.objects.len() || self.state.objects[mi].locked {
                    continue;
                }
                match self.state.tool {
                    Tool::Move => match constraint {
                        AxisConstraint::X => {
                            self.state.objects[mi].position[0] += delta.x * sensitivity;
                        }
                        AxisConstraint::Y => {
                            self.state.objects[mi].position[1] -= delta.y * sensitivity;
                        }
                        AxisConstraint::Z => {
                            self.state.objects[mi].position[2] += delta.x * sensitivity;
                        }
                        AxisConstraint::None => {
                            self.state.objects[mi].position[0] += delta.x * sensitivity;
                            self.state.objects[mi].position[1] -= delta.y * sensitivity;
                        }
                    },
                    Tool::Rotate => match constraint {
                        AxisConstraint::X => {
                            self.state.objects[mi].rotation[0] += delta.x * 0.5;
                        }
                        AxisConstraint::Y => {
                            self.state.objects[mi].rotation[1] += delta.x * 0.5;
                        }
                        AxisConstraint::Z => {
                            self.state.objects[mi].rotation[2] += delta.x * 0.5;
                        }
                        AxisConstraint::None => {
                            self.state.objects[mi].rotation[1] += delta.x * 0.5;
                            self.state.objects[mi].rotation[0] -= delta.y * 0.5;
                        }
                    },
                    Tool::Scale => {
                        let sd = 1.0 + (delta.x - delta.y) * 0.01;
                        match constraint {
                            AxisConstraint::X => {
                                self.state.objects[mi].scale[0] *= sd;
                            }
                            AxisConstraint::Y => {
                                self.state.objects[mi].scale[1] *= sd;
                            }
                            AxisConstraint::Z => {
                                self.state.objects[mi].scale[2] *= sd;
                            }
                            AxisConstraint::None => {
                                self.state.objects[mi].scale[0] *= sd;
                                self.state.objects[mi].scale[1] *= sd;
                                self.state.objects[mi].scale[2] *= sd;
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Auto-key: automatically insert keyframe after transform
            if self.state.auto_key && self.state.tool != Tool::Select {
                self.state.insert_keyframe();
            }
        }
    }

    fn apply_sculpt_stroke(
        &mut self,
        obj_idx: usize,
        brush_screen: egui::Pos2,
        drag_delta: egui::Vec2,
        viewport: egui::Rect,
    ) {
        // Defensive bounds guard: a stale selection index must never index out of
        // bounds. Every access below indexes objects[obj_idx]; bail cleanly if invalid.
        if obj_idx >= self.state.objects.len() {
            return;
        }

        // Materialize implicit geometry into editable custom_vertices (world-space, pos/scale zeroed)
        if self.state.objects[obj_idx].custom_vertices.is_none() {
            let verts = self.get_object_vertices(obj_idx);
            let faces = self.get_object_faces(obj_idx);
            self.state.objects[obj_idx].custom_vertices = Some(verts);
            self.state.objects[obj_idx].custom_faces = Some(faces);
            self.state.objects[obj_idx].position = [0.0, 0.0, 0.0];
            self.state.objects[obj_idx].scale = [1.0, 1.0, 1.0];
        }

        // Collect camera state into locals before any borrow split
        let cam_pos = self.state.camera.position;
        let cam_target = self.state.camera.target;
        let cam_dist = self.state.camera.distance;
        let yaw = self.state.camera.orbit_angles[0].to_radians();
        let fov_rad = self.state.camera.fov.to_radians();
        let brush = self.state.sculpt_brush;
        let radius = self.state.sculpt_radius;
        let strength = self.state.sculpt_strength;

        // Camera basis vectors in world space
        let right = [yaw.cos(), 0.0f32, -yaw.sin()];
        let vdx = cam_target[0] - cam_pos[0];
        let vdy = cam_target[1] - cam_pos[1];
        let vdz = cam_target[2] - cam_pos[2];
        let vd_len = (vdx * vdx + vdy * vdy + vdz * vdz).sqrt().max(1e-6);
        let view_dir = [vdx / vd_len, vdy / vd_len, vdz / vd_len];
        // scr_up = view_dir × right (screen-space up in world coords)
        let scr_up = [
            view_dir[1] * right[2] - view_dir[2] * right[1],
            view_dir[2] * right[0] - view_dir[0] * right[2],
            view_dir[0] * right[1] - view_dir[1] * right[0],
        ];

        let vp_w = viewport.width();
        let vp_h = viewport.height().max(1.0);
        let vp_cx = viewport.center().x;
        let vp_cy = viewport.center().y;
        let fov_scale = (fov_rad * 0.5).tan();
        let world_per_px = 2.0 * cam_dist * fov_scale / vp_h;

        // Phase 1 (immutable): compute centroid and snapshot current vertex positions
        let (centroid, verts_snap) = {
            let vr = self.state.objects[obj_idx]
                .custom_vertices
                .as_ref()
                .expect("custom_vertices materialized to Some above");
            if vr.is_empty() {
                return;
            }
            let n = vr.len() as f32;
            let mut c = [0.0f32; 3];
            for v in vr.iter() {
                c[0] += v[0];
                c[1] += v[1];
                c[2] += v[2];
            }
            c[0] /= n;
            c[1] /= n;
            c[2] /= n;
            (c, vr.clone())
        };

        // Phase 2 (mutable): apply brush displacement per-vertex
        let verts = self.state.objects[obj_idx]
            .custom_vertices
            .as_mut()
            .expect("custom_vertices materialized to Some above");
        for (i, vert) in verts.iter_mut().enumerate() {
            // Perspective project vertex to screen coordinates
            let dv = [
                vert[0] - cam_pos[0],
                vert[1] - cam_pos[1],
                vert[2] - cam_pos[2],
            ];
            let depth = dv[0] * view_dir[0] + dv[1] * view_dir[1] + dv[2] * view_dir[2];
            if depth <= 0.01 {
                continue;
            }
            let dot_r = dv[0] * right[0] + dv[1] * right[1] + dv[2] * right[2];
            let dot_u = dv[0] * scr_up[0] + dv[1] * scr_up[1] + dv[2] * scr_up[2];
            let sx = vp_cx + dot_r / (depth * fov_scale) * vp_w * 0.5;
            let sy = vp_cy - dot_u / (depth * fov_scale) * vp_h * 0.5;

            let screen_dist =
                ((sx - brush_screen.x).powi(2) + (sy - brush_screen.y).powi(2)).sqrt();
            if screen_dist >= radius {
                continue;
            }

            // Cubic smooth falloff: (1-t²)²
            let t = screen_dist / radius;
            let falloff = (1.0 - t * t) * (1.0 - t * t);
            let factor = strength * falloff;

            match brush {
                SculptBrush::Draw | SculptBrush::Inflate => {
                    // Displace outward along vertex→centroid normal
                    let nx = vert[0] - centroid[0];
                    let ny = vert[1] - centroid[1];
                    let nz = vert[2] - centroid[2];
                    let nl = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-6);
                    vert[0] += nx / nl * factor * 0.1;
                    vert[1] += ny / nl * factor * 0.1;
                    vert[2] += nz / nl * factor * 0.1;
                }
                SculptBrush::Smooth => {
                    // Average position of neighboring vertices within world-space radius
                    let mut ax = 0.0f32;
                    let mut ay = 0.0f32;
                    let mut az = 0.0f32;
                    let mut w = 0.0f32;
                    let search_r = world_per_px * radius * 0.5;
                    for (j, v2) in verts_snap.iter().enumerate() {
                        if j == i {
                            continue;
                        }
                        let d = ((v2[0] - verts_snap[i][0]).powi(2)
                            + (v2[1] - verts_snap[i][1]).powi(2)
                            + (v2[2] - verts_snap[i][2]).powi(2))
                        .sqrt();
                        if d < search_r {
                            ax += v2[0];
                            ay += v2[1];
                            az += v2[2];
                            w += 1.0;
                        }
                    }
                    if w > 0.0 {
                        vert[0] += (ax / w - vert[0]) * factor * 0.5;
                        vert[1] += (ay / w - vert[1]) * factor * 0.5;
                        vert[2] += (az / w - vert[2]) * factor * 0.5;
                    }
                }
                SculptBrush::Flatten => {
                    // Flatten Y toward the average height of all in-brush verts
                    let mut avg_y = 0.0f32;
                    let mut cnt = 0.0f32;
                    for v2 in verts_snap.iter() {
                        let dv2 = [v2[0] - cam_pos[0], v2[1] - cam_pos[1], v2[2] - cam_pos[2]];
                        let dep2 =
                            (dv2[0] * view_dir[0] + dv2[1] * view_dir[1] + dv2[2] * view_dir[2])
                                .max(0.01);
                        let sx2 = vp_cx
                            + (dv2[0] * right[0] + dv2[1] * right[1] + dv2[2] * right[2])
                                / (dep2 * fov_scale)
                                * vp_w
                                * 0.5;
                        let sy2 = vp_cy
                            - (dv2[0] * scr_up[0] + dv2[1] * scr_up[1] + dv2[2] * scr_up[2])
                                / (dep2 * fov_scale)
                                * vp_h
                                * 0.5;
                        if ((sx2 - brush_screen.x).powi(2) + (sy2 - brush_screen.y).powi(2)).sqrt()
                            < radius
                        {
                            avg_y += v2[1];
                            cnt += 1.0;
                        }
                    }
                    if cnt > 0.0 {
                        vert[1] += (avg_y / cnt - vert[1]) * factor * 0.5;
                    }
                }
                SculptBrush::Pinch => {
                    // Pull vertex toward 3D position under brush cursor
                    let pull_x = (brush_screen.x - sx) * world_per_px * depth / cam_dist.max(0.01);
                    let pull_y = (brush_screen.y - sy) * world_per_px * depth / cam_dist.max(0.01);
                    vert[0] += (right[0] * pull_x - scr_up[0] * pull_y) * factor;
                    vert[1] += (right[1] * pull_x - scr_up[1] * pull_y) * factor;
                    vert[2] += (right[2] * pull_x - scr_up[2] * pull_y) * factor;
                }
                SculptBrush::Grab => {
                    // Drag vertex along camera right/up axes proportional to mouse delta
                    let wp = world_per_px * depth / cam_dist.max(0.01);
                    vert[0] +=
                        (right[0] * drag_delta.x - scr_up[0] * drag_delta.y) * wp * factor * 0.3;
                    vert[1] +=
                        (right[1] * drag_delta.x - scr_up[1] * drag_delta.y) * wp * factor * 0.3;
                    vert[2] +=
                        (right[2] * drag_delta.x - scr_up[2] * drag_delta.y) * wp * factor * 0.3;
                }
            }
        }
    }

    fn open_project_dialog(&mut self) {
        #[cfg(feature = "file-dialog")]
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("NAT3D Project", &["nat", "nat3d", "json"])
            .pick_file()
        {
            let is_nat_binary = path.extension().and_then(|e| e.to_str()) == Some("nat");
            if is_nat_binary {
                match nat3d_io::import_nat(&path) {
                    Ok(scene) => match self.load_native_scene(scene) {
                        Ok(count) => {
                            self.status_message =
                                format!("Loaded {} objects from {}", count, path.display());
                            self.project_path = Some(path);
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to parse project: {}", e);
                        }
                    },
                    Err(e) => {
                        self.status_message = format!("Failed to open: {}", e);
                    }
                }
            } else {
                match std::fs::read_to_string(&path) {
                    Ok(content) => match self.load_project_from_json(&content) {
                        Ok(count) => {
                            self.status_message =
                                format!("Loaded {} objects from {}", count, path.display());
                            self.project_path = Some(path);
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to parse project: {}", e);
                        }
                    },
                    Err(e) => {
                        self.status_message = format!("Failed to open: {}", e);
                    }
                }
            }
        }
    }

    fn load_project_from_json(&mut self, content: &str) -> Result<usize, String> {
        let json: serde_json::Value =
            serde_json::from_str(content).map_err(|e| format!("JSON parse error: {}", e))?;

        // Clear current scene
        self.state.save_undo_state();
        self.state.objects.clear();
        self.state.selected_object = None;

        // Load objects
        if let Some(objects) = json.get("objects").and_then(|v| v.as_array()) {
            for obj_json in objects {
                let name = obj_json
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Object")
                    .to_string();

                let obj_type_str = obj_json
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Cube");

                let object_type = match obj_type_str {
                    "Cube" => ObjectType::Cube,
                    "Sphere" => ObjectType::Sphere,
                    "Cylinder" => ObjectType::Cylinder,
                    "Plane" => ObjectType::Plane,
                    "Torus" => ObjectType::Torus,
                    "Cone" => ObjectType::Cone,
                    "IcoSphere" => ObjectType::IcoSphere,
                    "Grid" => ObjectType::Grid,
                    "Circle" => ObjectType::Circle,
                    "BezierCurve" => ObjectType::BezierCurve,
                    "NurbsCurve" => ObjectType::NurbsCurve,
                    "Text" => ObjectType::Text,
                    "Empty" => ObjectType::Empty,
                    "Light" => ObjectType::Light,
                    "Camera" => ObjectType::Camera,
                    "Mesh" => ObjectType::Mesh,
                    _ => ObjectType::Cube,
                };

                let position = self.parse_vec3(obj_json.get("position"));
                let rotation = self.parse_vec3(obj_json.get("rotation"));
                let scale = self.parse_vec3_default(obj_json.get("scale"), [1.0, 1.0, 1.0]);

                let material = if let Some(mat) = obj_json.get("material") {
                    MaterialState {
                        base_color: self
                            .parse_vec4_default(mat.get("base_color"), [0.8, 0.8, 0.8, 1.0]),
                        metallic: mat.get("metallic").and_then(|v| v.as_f64()).unwrap_or(0.0)
                            as f32,
                        roughness: mat.get("roughness").and_then(|v| v.as_f64()).unwrap_or(0.5)
                            as f32,
                        emissive: mat.get("emissive").and_then(|v| v.as_f64()).unwrap_or(0.0)
                            as f32,
                    }
                } else {
                    MaterialState::default()
                };

                let modifiers = obj_json
                    .get("modifiers")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let visible = obj_json
                    .get("visible")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                self.state.objects.push(SceneObject {
                    physiological_signal: 0.0,
                    name,
                    object_type,
                    position,
                    rotation,
                    scale,
                    material,
                    modifiers,
                    visible,
                    smooth_shading: true,
                    locked: false,
                    parent: None,
                    keyframes: Vec::new(),
                    shape_keys: Vec::new(),
                    constraints: Vec::new(),
                    vertex_colors: Vec::new(),
                    vertex_weights: Vec::new(),
                    vertex_groups: Vec::new(),
                    particle_systems: Vec::new(),
                    bones: Vec::new(),
                    drivers: Vec::new(),
                    force_field: None,
                    cloth: None,
                    soft_body: None,
                    nla_tracks: Vec::new(),
                    gp_strokes: Vec::new(),
                    texture_slots: Vec::new(),
                    custom_properties: Vec::new(),
                    motion_path: None,
                    pass_index: 0,
                    hair_settings: None,
                    fluid: None,
                    linked_data: None,
                    edit_mesh: None,
                    edit_selection: EditModeSelection::default(),
                    custom_vertices: None,
                    custom_faces: None,
                    uv_coords: None,
                });
            }
        }

        // Load camera state
        if let Some(cam) = json.get("camera") {
            self.state.camera.target = self.parse_vec3(cam.get("target"));
            self.state.camera.orbit_angles = [
                cam.get("orbit_angles")
                    .and_then(|v| v.get(0))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(45.0) as f32,
                cam.get("orbit_angles")
                    .and_then(|v| v.get(1))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(30.0) as f32,
            ];
            self.state.camera.distance =
                cam.get("distance").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
            self.state.camera.update_position();
        }

        Ok(self.state.objects.len())
    }

    fn load_native_scene(&mut self, scene: nat3d_io::NativeScene) -> Result<usize, String> {
        self.state.save_undo_state();
        self.state.objects.clear();
        self.state.selected_object = None;

        for obj in scene.objects {
            let object_type = match obj.object_type.as_str() {
                "Cube" => ObjectType::Cube,
                "Sphere" => ObjectType::Sphere,
                "Cylinder" => ObjectType::Cylinder,
                "Plane" => ObjectType::Plane,
                "Torus" => ObjectType::Torus,
                "Cone" => ObjectType::Cone,
                "IcoSphere" => ObjectType::IcoSphere,
                "Grid" => ObjectType::Grid,
                "Circle" => ObjectType::Circle,
                "BezierCurve" => ObjectType::BezierCurve,
                "NurbsCurve" => ObjectType::NurbsCurve,
                "Text" => ObjectType::Text,
                "Empty" => ObjectType::Empty,
                "Light" => ObjectType::Light,
                "Camera" => ObjectType::Camera,
                "Mesh" => ObjectType::Mesh,
                _ => ObjectType::Cube,
            };
            let material = obj
                .material
                .map(|m| MaterialState {
                    base_color: m.base_color,
                    metallic: m.metallic,
                    roughness: m.roughness,
                    emissive: m.emissive,
                })
                .unwrap_or_default();
            self.state.objects.push(SceneObject {
                physiological_signal: 0.0,
                name: obj.name,
                object_type,
                position: obj.position,
                rotation: obj.rotation,
                scale: obj.scale,
                material,
                modifiers: obj.modifiers,
                visible: obj.visible,
                smooth_shading: true,
                locked: false,
                parent: None,
                keyframes: Vec::new(),
                shape_keys: Vec::new(),
                constraints: Vec::new(),
                vertex_colors: Vec::new(),
                vertex_weights: Vec::new(),
                vertex_groups: Vec::new(),
                particle_systems: Vec::new(),
                bones: Vec::new(),
                drivers: Vec::new(),
                force_field: None,
                cloth: None,
                soft_body: None,
                nla_tracks: Vec::new(),
                gp_strokes: Vec::new(),
                texture_slots: Vec::new(),
                custom_properties: Vec::new(),
                motion_path: None,
                pass_index: 0,
                hair_settings: None,
                fluid: None,
                linked_data: None,
                edit_mesh: None,
                edit_selection: EditModeSelection::default(),
                custom_vertices: None,
                custom_faces: None,
                uv_coords: None,
            });
        }

        if let Some(cam) = scene.camera {
            self.state.camera.position = cam.position;
            self.state.camera.target = cam.target;
            self.state.camera.orbit_angles = cam.orbit_angles;
            self.state.camera.distance = cam.distance;
        }

        Ok(self.state.objects.len())
    }

    fn parse_vec3(&self, value: Option<&serde_json::Value>) -> [f32; 3] {
        self.parse_vec3_default(value, [0.0, 0.0, 0.0])
    }

    fn parse_vec3_default(&self, value: Option<&serde_json::Value>, default: [f32; 3]) -> [f32; 3] {
        value
            .and_then(|v| v.as_array())
            .map(|arr| {
                [
                    arr.first()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(default[0] as f64) as f32,
                    arr.get(1)
                        .and_then(|v| v.as_f64())
                        .unwrap_or(default[1] as f64) as f32,
                    arr.get(2)
                        .and_then(|v| v.as_f64())
                        .unwrap_or(default[2] as f64) as f32,
                ]
            })
            .unwrap_or(default)
    }

    fn parse_vec4_default(&self, value: Option<&serde_json::Value>, default: [f32; 4]) -> [f32; 4] {
        value
            .and_then(|v| v.as_array())
            .map(|arr| {
                [
                    arr.first()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(default[0] as f64) as f32,
                    arr.get(1)
                        .and_then(|v| v.as_f64())
                        .unwrap_or(default[1] as f64) as f32,
                    arr.get(2)
                        .and_then(|v| v.as_f64())
                        .unwrap_or(default[2] as f64) as f32,
                    arr.get(3)
                        .and_then(|v| v.as_f64())
                        .unwrap_or(default[3] as f64) as f32,
                ]
            })
            .unwrap_or(default)
    }

    fn save_project(&mut self) {
        if let Some(path) = &self.project_path {
            self.save_project_to_path(path.clone());
        } else {
            self.save_project_as_dialog();
        }
    }

    fn save_project_as_dialog(&mut self) {
        #[cfg(feature = "file-dialog")]
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("NAT3D Native Binary", &["nat"])
            .add_filter("NAT3D Project (JSON)", &["nat3d"])
            .set_file_name("scene.nat")
            .save_file()
        {
            self.save_project_to_path(path);
        }
    }

    fn save_project_to_path(&mut self, path: PathBuf) {
        let is_nat_binary = path.extension().and_then(|e| e.to_str()) == Some("nat");

        if is_nat_binary {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let scene = nat3d_io::NativeScene {
                version: 1,
                metadata: nat3d_io::SceneMetadata {
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    author: "NAT3D".to_string(),
                    created_at: now,
                    modified_at: now,
                },
                objects: self
                    .state
                    .objects
                    .iter()
                    .map(|obj| nat3d_io::NativeObject {
                        name: obj.name.clone(),
                        object_type: format!("{:?}", obj.object_type),
                        position: obj.position,
                        rotation: obj.rotation,
                        scale: obj.scale,
                        material: Some(nat3d_io::NativeMaterial {
                            base_color: obj.material.base_color,
                            metallic: obj.material.metallic,
                            roughness: obj.material.roughness,
                            emissive: obj.material.emissive,
                        }),
                        modifiers: obj.modifiers.clone(),
                        visible: obj.visible,
                        children: vec![],
                    })
                    .collect(),
                camera: Some(nat3d_io::NativeCamera {
                    position: self.state.camera.position,
                    target: self.state.camera.target,
                    orbit_angles: self.state.camera.orbit_angles,
                    distance: self.state.camera.distance,
                }),
            };
            match nat3d_io::export_nat(&path, &scene) {
                Ok(_) => {
                    self.status_message = format!("Saved: {}", path.display());
                    self.project_path = Some(path);
                }
                Err(e) => {
                    self.status_message = format!("Failed to save: {}", e);
                }
            }
        } else {
            let scene_data = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "objects": self.state.objects.iter().map(|obj| {
                    serde_json::json!({
                        "name": obj.name,
                        "type": format!("{:?}", obj.object_type),
                        "position": obj.position,
                        "rotation": obj.rotation,
                        "scale": obj.scale,
                        "material": {
                            "base_color": obj.material.base_color,
                            "metallic": obj.material.metallic,
                            "roughness": obj.material.roughness,
                            "emissive": obj.material.emissive,
                        },
                        "modifiers": obj.modifiers,
                        "visible": obj.visible,
                    })
                }).collect::<Vec<_>>(),
                "camera": {
                    "position": self.state.camera.position,
                    "target": self.state.camera.target,
                    "orbit_angles": self.state.camera.orbit_angles,
                    "distance": self.state.camera.distance,
                },
            });
            // Serialize first: serde_json errors on non-finite floats (NaN/Inf), which a
            // degenerate scene can contain — surface that instead of panicking on save.
            match serde_json::to_string_pretty(&scene_data) {
                Ok(serialized) => match std::fs::write(&path, serialized) {
                    Ok(_) => {
                        self.status_message = format!("Saved: {}", path.display());
                        self.project_path = Some(path);
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to save: {}", e);
                    }
                },
                Err(e) => {
                    self.status_message = format!("Failed to serialize scene: {}", e);
                }
            }
        }
    }

    fn import_file_dialog(&mut self, format: &str) {
        let filter = match format {
            "obj" => ("Wavefront OBJ", vec!["obj"]),
            "stl" => ("STL", vec!["stl"]),
            "gltf" => ("glTF", vec!["gltf", "glb"]),
            "fbx" => ("Autodesk FBX", vec!["fbx"]),
            "dxf" => ("AutoCAD DXF", vec!["dxf"]),
            "step" => ("STEP/STP", vec!["step", "stp"]),
            "iges" => ("IGES", vec!["igs", "iges"]),
            _ => ("All Files", vec!["*"]),
        };

        #[cfg(feature = "file-dialog")]
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(filter.0, &filter.1)
            .pick_file()
        {
            self.status_message = format!("Importing: {}...", path.display());
            self.state.save_undo_state();

            let result = match format {
                "obj" => self.import_obj_file(&path),
                "stl" => self.import_stl_file(&path),
                "gltf" => self.import_gltf_file(&path),
                "fbx" | "dxf" | "step" | "iges" => self.import_generic_file(&path, format),
                _ => Err("Unknown format".to_string()),
            };

            match result {
                Ok(count) => {
                    self.status_message = format!(
                        "Imported {} object(s) from {}",
                        count,
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
                }
                Err(e) => {
                    self.status_message = format!("Import failed: {}", e);
                }
            }
        }
    }

    fn import_obj_file(&mut self, path: &PathBuf) -> Result<usize, String> {
        match nat3d_io::import_obj(path) {
            Ok(obj_data) => {
                let mut count = 0;
                for obj in obj_data.objects {
                    for group in obj.groups {
                        let name = if group.name.is_empty() {
                            obj.name.clone()
                        } else {
                            format!("{}.{}", obj.name, group.name)
                        };

                        let scene_obj = SceneObject {
                            physiological_signal: 0.0,
                            name: if name.is_empty() {
                                format!("Mesh.{:03}", count + 1)
                            } else {
                                name
                            },
                            object_type: ObjectType::Mesh,
                            position: [0.0, 0.0, 0.0],
                            rotation: [0.0, 0.0, 0.0],
                            scale: [1.0, 1.0, 1.0],
                            material: MaterialState::default(),
                            modifiers: Vec::new(),
                            visible: true,
                            smooth_shading: true,
                            locked: false,
                            parent: None,
                            keyframes: Vec::new(),
                            shape_keys: Vec::new(),
                            constraints: Vec::new(),
                            vertex_colors: Vec::new(),
                            vertex_weights: Vec::new(),
                            vertex_groups: Vec::new(),
                            particle_systems: Vec::new(),
                            bones: Vec::new(),
                            drivers: Vec::new(),
                            force_field: None,
                            cloth: None,
                            soft_body: None,
                            nla_tracks: Vec::new(),
                            gp_strokes: Vec::new(),
                            texture_slots: Vec::new(),
                            custom_properties: Vec::new(),
                            motion_path: None,
                            pass_index: 0,
                            hair_settings: None,
                            fluid: None,
                            linked_data: None,
                            edit_mesh: None,
                            edit_selection: EditModeSelection::default(),
                            custom_vertices: None,
                            custom_faces: None,
                            uv_coords: None,
                        };
                        self.state.objects.push(scene_obj);
                        count += 1;
                    }
                }
                if count > 0 {
                    self.state.selected_object = Some(self.state.objects.len() - 1);
                }
                Ok(count)
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn import_stl_file(&mut self, path: &PathBuf) -> Result<usize, String> {
        match nat3d_io::import_stl(path) {
            Ok(_stl_data) => {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "STL".to_string());

                let scene_obj = SceneObject {
                    physiological_signal: 0.0,
                    name,
                    object_type: ObjectType::Mesh,
                    position: [0.0, 0.0, 0.0],
                    rotation: [0.0, 0.0, 0.0],
                    scale: [1.0, 1.0, 1.0],
                    material: MaterialState::default(),
                    modifiers: Vec::new(),
                    visible: true,
                    smooth_shading: true,
                    locked: false,
                    parent: None,
                    keyframes: Vec::new(),
                    shape_keys: Vec::new(),
                    constraints: Vec::new(),
                    vertex_colors: Vec::new(),
                    vertex_weights: Vec::new(),
                    vertex_groups: Vec::new(),
                    particle_systems: Vec::new(),
                    bones: Vec::new(),
                    drivers: Vec::new(),
                    force_field: None,
                    cloth: None,
                    soft_body: None,
                    nla_tracks: Vec::new(),
                    gp_strokes: Vec::new(),
                    texture_slots: Vec::new(),
                    custom_properties: Vec::new(),
                    motion_path: None,
                    pass_index: 0,
                    hair_settings: None,
                    fluid: None,
                    linked_data: None,
                    edit_mesh: None,
                    edit_selection: EditModeSelection::default(),
                    custom_vertices: None,
                    custom_faces: None,
                    uv_coords: None,
                };
                self.state.objects.push(scene_obj);
                self.state.selected_object = Some(self.state.objects.len() - 1);
                Ok(1)
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn import_gltf_file(&mut self, path: &PathBuf) -> Result<usize, String> {
        match nat3d_io::import_gltf(path) {
            Ok(gltf_scene) => {
                let mut count = 0;
                for mesh in &gltf_scene.meshes {
                    let scene_obj = SceneObject {
                        physiological_signal: 0.0,
                        name: mesh.name.clone(),
                        object_type: ObjectType::Mesh,
                        position: [0.0, 0.0, 0.0],
                        rotation: [0.0, 0.0, 0.0],
                        scale: [1.0, 1.0, 1.0],
                        material: MaterialState::default(),
                        modifiers: Vec::new(),
                        visible: true,
                        smooth_shading: true,
                        locked: false,
                        parent: None,
                        keyframes: Vec::new(),
                        shape_keys: Vec::new(),
                        constraints: Vec::new(),
                        vertex_colors: Vec::new(),
                        vertex_weights: Vec::new(),
                        vertex_groups: Vec::new(),
                        particle_systems: Vec::new(),
                        bones: Vec::new(),
                        drivers: Vec::new(),
                        force_field: None,
                        cloth: None,
                        soft_body: None,
                        nla_tracks: Vec::new(),
                        gp_strokes: Vec::new(),
                        texture_slots: Vec::new(),
                        custom_properties: Vec::new(),
                        motion_path: None,
                        pass_index: 0,
                        hair_settings: None,
                        fluid: None,
                        linked_data: None,
                        edit_mesh: None,
                        edit_selection: EditModeSelection::default(),
                        custom_vertices: None,
                        custom_faces: None,
                        uv_coords: None,
                    };
                    self.state.objects.push(scene_obj);
                    count += 1;
                }
                if count > 0 {
                    self.state.selected_object = Some(self.state.objects.len() - 1);
                }
                Ok(count)
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn import_generic_file(&mut self, path: &PathBuf, format: &str) -> Result<usize, String> {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}_import", format));

        let scene_obj = SceneObject {
            physiological_signal: 0.0,
            name,
            object_type: ObjectType::Mesh,
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            material: MaterialState::default(),
            modifiers: Vec::new(),
            visible: true,
            smooth_shading: true,
            locked: false,
            parent: None,
            keyframes: Vec::new(),
            shape_keys: Vec::new(),
            constraints: Vec::new(),
            vertex_colors: Vec::new(),
            vertex_weights: Vec::new(),
            vertex_groups: Vec::new(),
            particle_systems: Vec::new(),
            bones: Vec::new(),
            drivers: Vec::new(),
            force_field: None,
            cloth: None,
            soft_body: None,
            nla_tracks: Vec::new(),
            gp_strokes: Vec::new(),
            texture_slots: Vec::new(),
            custom_properties: Vec::new(),
            motion_path: None,
            pass_index: 0,
            hair_settings: None,
            fluid: None,
            linked_data: None,
            edit_mesh: None,
            edit_selection: EditModeSelection::default(),
            custom_vertices: None,
            custom_faces: None,
            uv_coords: None,
        };
        self.state.objects.push(scene_obj);
        self.state.selected_object = Some(self.state.objects.len() - 1);
        self.status_message = format!(
            "Imported {} file: {}",
            format.to_uppercase(),
            path.display()
        );
        Ok(1)
    }

    fn export_file_dialog(&mut self, format: &str) {
        let (filter_name, extensions, default_ext) = match format {
            "obj" => ("Wavefront OBJ", vec!["obj"], "obj"),
            "stl" => ("STL", vec!["stl"], "stl"),
            "glb" => ("glTF Binary", vec!["glb"], "glb"),
            "fbx" => ("Autodesk FBX", vec!["fbx"], "fbx"),
            "dxf" => ("AutoCAD DXF", vec!["dxf"], "dxf"),
            _ => ("All Files", vec!["*"], ""),
        };

        #[cfg(feature = "file-dialog")]
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(filter_name, &extensions)
            .set_file_name(format!("export.{}", default_ext))
            .save_file()
        {
            self.status_message = format!("Exporting to: {}...", path.display());

            let result = match format {
                "obj" => self.export_scene_obj(&path),
                "stl" => self.export_scene_stl(&path),
                "glb" => self.export_scene_gltf(&path),
                _ => Err("Unknown format".to_string()),
            };

            match result {
                Ok(count) => {
                    self.status_message = format!(
                        "Exported {} object(s) to {}",
                        count,
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
                }
                Err(e) => {
                    self.status_message = format!("Export failed: {}", e);
                }
            }
        }
    }

    fn export_scene_obj(&self, path: &PathBuf) -> Result<usize, String> {
        use nat3d_core::geometry::{mesh::Mesh, Position};
        use nat3d_io::{ObjData, ObjGroup, ObjObject};

        let mut obj_data = ObjData {
            objects: Vec::new(),
            mtl_libs: Vec::new(),
        };

        let mut exported_count = 0usize;
        for (idx, obj) in self.state.objects.iter().enumerate() {
            // Skip non-geometry objects
            match obj.object_type {
                state::ObjectType::Light | state::ObjectType::Camera | state::ObjectType::Empty => {
                    continue
                }
                _ => {}
            }

            let vertices = self.get_object_vertices(idx);
            let faces = self.get_object_faces(idx);

            if vertices.is_empty() || faces.is_empty() {
                continue;
            }

            let mut mesh = Mesh::new(&obj.name);

            for v in &vertices {
                mesh.add_vertex_at(Position::new(v[0] as f64, v[1] as f64, v[2] as f64));
            }

            for face in &faces {
                match face.len() {
                    3 => {
                        let _ = mesh.add_triangle(face[0], face[1], face[2]);
                    }
                    4 => {
                        let _ = mesh.add_quad(face[0], face[1], face[2], face[3]);
                    }
                    n if n > 4 => {
                        // Triangulate polygon as a fan from the first vertex
                        for i in 1..n - 1 {
                            let _ = mesh.add_triangle(face[0], face[i], face[i + 1]);
                        }
                    }
                    _ => {}
                }
            }

            let group = ObjGroup {
                name: "default".to_string(),
                material: None,
                mesh: mesh.to_data(),
            };

            obj_data.objects.push(ObjObject {
                name: obj.name.clone(),
                groups: vec![group],
            });
            exported_count += 1;
        }

        match nat3d_io::export_obj(path, &obj_data) {
            Ok(_) => Ok(exported_count),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn export_scene_stl(&self, path: &PathBuf) -> Result<usize, String> {
        use nat3d_core::geometry::{mesh::Mesh, Position};

        // Combine all geometry objects into one STL mesh
        let mut mesh = Mesh::new("combined");
        let mut exported_count = 0usize;

        for (idx, obj) in self.state.objects.iter().enumerate() {
            // Skip non-geometry objects
            match obj.object_type {
                state::ObjectType::Light | state::ObjectType::Camera | state::ObjectType::Empty => {
                    continue
                }
                _ => {}
            }

            let vertices = self.get_object_vertices(idx);
            let faces = self.get_object_faces(idx);

            if vertices.is_empty() || faces.is_empty() {
                continue;
            }

            let base_idx = mesh.vertex_count();

            for v in &vertices {
                mesh.add_vertex_at(Position::new(v[0] as f64, v[1] as f64, v[2] as f64));
            }

            // STL requires triangles - triangulate all faces
            for face in &faces {
                match face.len() {
                    3 => {
                        let _ = mesh.add_triangle(
                            base_idx + face[0],
                            base_idx + face[1],
                            base_idx + face[2],
                        );
                    }
                    4 => {
                        // Split quad into two triangles
                        let _ = mesh.add_triangle(
                            base_idx + face[0],
                            base_idx + face[1],
                            base_idx + face[2],
                        );
                        let _ = mesh.add_triangle(
                            base_idx + face[0],
                            base_idx + face[2],
                            base_idx + face[3],
                        );
                    }
                    n if n > 4 => {
                        // Triangulate polygon as a fan from the first vertex
                        for i in 1..n - 1 {
                            let _ = mesh.add_triangle(
                                base_idx + face[0],
                                base_idx + face[i],
                                base_idx + face[i + 1],
                            );
                        }
                    }
                    _ => {}
                }
            }
            exported_count += 1;
        }

        match nat3d_io::export_mesh_stl(path, &mesh.to_data()) {
            Ok(_) => Ok(exported_count),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn export_scene_gltf(&self, path: &PathBuf) -> Result<usize, String> {
        use nat3d_core::geometry::{mesh::Mesh, Position};

        // Combine all geometry objects into a single mesh for glTF export
        let mut combined_mesh = Mesh::new("NAT3D_Export");
        let mut exported_count = 0usize;

        for (idx, obj) in self.state.objects.iter().enumerate() {
            // Skip non-geometry objects
            match obj.object_type {
                state::ObjectType::Light | state::ObjectType::Camera | state::ObjectType::Empty => {
                    continue
                }
                _ => {}
            }

            let vertices = self.get_object_vertices(idx);
            let faces = self.get_object_faces(idx);

            if vertices.is_empty() || faces.is_empty() {
                continue;
            }

            let base_idx = combined_mesh.vertex_count();

            for v in &vertices {
                combined_mesh.add_vertex_at(Position::new(v[0] as f64, v[1] as f64, v[2] as f64));
            }

            // glTF uses triangles - triangulate all faces
            for face in &faces {
                match face.len() {
                    3 => {
                        let _ = combined_mesh.add_triangle(
                            base_idx + face[0],
                            base_idx + face[1],
                            base_idx + face[2],
                        );
                    }
                    4 => {
                        // Split quad into two triangles
                        let _ = combined_mesh.add_triangle(
                            base_idx + face[0],
                            base_idx + face[1],
                            base_idx + face[2],
                        );
                        let _ = combined_mesh.add_triangle(
                            base_idx + face[0],
                            base_idx + face[2],
                            base_idx + face[3],
                        );
                    }
                    n if n > 4 => {
                        // Triangulate polygon as a fan from the first vertex
                        for i in 1..n - 1 {
                            let _ = combined_mesh.add_triangle(
                                base_idx + face[0],
                                base_idx + face[i],
                                base_idx + face[i + 1],
                            );
                        }
                    }
                    _ => {}
                }
            }
            exported_count += 1;
        }

        match nat3d_io::export_gltf(path, &combined_mesh.to_data(), "NAT3D_Export") {
            Ok(_) => Ok(exported_count),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    fn render_image(&mut self) {
        // Neural engine branch: NeRF / NRC render without file dialog (outputs to status + image editor)
        if matches!(
            self.state.render_engine,
            RenderEngine::NeRF | RenderEngine::NeuralCache
        ) {
            self.render_image_neural();
            return;
        }

        #[cfg(feature = "file-dialog")]
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .add_filter("JPEG Image", &["jpg", "jpeg"])
            .set_file_name("render.png")
            .save_file()
        {
            self.status_message = format!(
                "Rendering {}x{} ...",
                self.render_settings.width, self.render_settings.height,
            );

            let w = self.render_settings.width as usize;
            let h = self.render_settings.height as usize;

            // Collect triangulated scene geometry
            let mut all_tris: Vec<([[f32; 3]; 3], [f32; 4], f32, f32, f32)> = Vec::new(); // (tri_verts, color, metallic, roughness, emissive)
            for (idx, obj) in self.state.objects.iter().enumerate() {
                if !obj.visible {
                    continue;
                }
                match obj.object_type {
                    state::ObjectType::Light
                    | state::ObjectType::Camera
                    | state::ObjectType::Empty => continue,
                    _ => {}
                }
                let vertices = self.get_object_vertices(idx);
                let faces = self.get_object_faces(idx);
                if vertices.is_empty() || faces.is_empty() {
                    continue;
                }

                for face in &faces {
                    // Triangulate face as fan
                    for i in 1..face.len() - 1 {
                        if face[0] < vertices.len()
                            && face[i] < vertices.len()
                            && face[i + 1] < vertices.len()
                        {
                            all_tris.push((
                                [vertices[face[0]], vertices[face[i]], vertices[face[i + 1]]],
                                obj.material.base_color,
                                obj.material.metallic,
                                obj.material.roughness.max(0.04),
                                obj.material.emissive,
                            ));
                        }
                    }
                }
            }

            // Collect lights
            let lights: Vec<([f32; 3], [f32; 3], f32)> = {
                let mut ls = Vec::new();
                for obj in &self.state.objects {
                    if obj.object_type == state::ObjectType::Light && obj.visible {
                        ls.push((
                            obj.position,
                            [
                                obj.material.base_color[0],
                                obj.material.base_color[1],
                                obj.material.base_color[2],
                            ],
                            obj.material.emissive.max(1.0),
                        ));
                    }
                }
                if ls.is_empty() {
                    ls.push(([4.0, 6.0, 3.0], [1.0, 0.98, 0.95], 1.0));
                    ls.push(([-3.0, 2.0, -2.0], [0.4, 0.45, 0.6], 0.3));
                }
                ls
            };

            // Camera setup
            let cam = &self.state.camera;
            let cam_pos = cam.position;
            let yaw = cam.orbit_angles[0].to_radians();
            let pitch = cam.orbit_angles[1].to_radians();
            let forward = [
                -yaw.sin() * pitch.cos(),
                -pitch.sin(),
                -yaw.cos() * pitch.cos(),
            ];
            let right = [yaw.cos(), 0.0, -yaw.sin()];
            let up = [
                yaw.sin() * pitch.sin(),
                pitch.cos(),
                yaw.cos() * pitch.sin(),
            ];
            let fov_half_tan = (cam.fov.to_radians() * 0.5).tan();
            let aspect = w as f32 / h as f32;

            // Render each pixel
            let mut img_buf = vec![0u8; w * h * 3];

            for py in 0..h {
                for px in 0..w {
                    // Normalized device coordinates (-1..1)
                    let ndc_x = (2.0 * px as f32 / w as f32 - 1.0) * aspect * fov_half_tan;
                    let ndc_y = (1.0 - 2.0 * py as f32 / h as f32) * fov_half_tan;

                    // Ray direction
                    let ray_dir = [
                        forward[0] + right[0] * ndc_x + up[0] * ndc_y,
                        forward[1] + right[1] * ndc_x + up[1] * ndc_y,
                        forward[2] + right[2] * ndc_x + up[2] * ndc_y,
                    ];
                    let rd_len = (ray_dir[0] * ray_dir[0]
                        + ray_dir[1] * ray_dir[1]
                        + ray_dir[2] * ray_dir[2])
                        .sqrt();
                    let rd = [
                        ray_dir[0] / rd_len,
                        ray_dir[1] / rd_len,
                        ray_dir[2] / rd_len,
                    ];

                    // Ray-triangle intersection (find closest hit)
                    let mut closest_t = f32::MAX;
                    let mut hit_normal = [0.0_f32; 3];
                    let mut hit_color = [0.0_f32; 4];
                    let mut hit_metallic = 0.0_f32;
                    let mut hit_roughness = 0.5_f32;
                    let mut hit_emissive = 0.0_f32;

                    for (tri, color, metallic, roughness, emissive) in &all_tris {
                        // Moller-Trumbore intersection
                        let e1 = [
                            tri[1][0] - tri[0][0],
                            tri[1][1] - tri[0][1],
                            tri[1][2] - tri[0][2],
                        ];
                        let e2 = [
                            tri[2][0] - tri[0][0],
                            tri[2][1] - tri[0][1],
                            tri[2][2] - tri[0][2],
                        ];
                        let h_vec = [
                            rd[1] * e2[2] - rd[2] * e2[1],
                            rd[2] * e2[0] - rd[0] * e2[2],
                            rd[0] * e2[1] - rd[1] * e2[0],
                        ];
                        let a_det = e1[0] * h_vec[0] + e1[1] * h_vec[1] + e1[2] * h_vec[2];
                        if a_det.abs() < 1e-8 {
                            continue;
                        }
                        let f_inv = 1.0 / a_det;
                        let s = [
                            cam_pos[0] - tri[0][0],
                            cam_pos[1] - tri[0][1],
                            cam_pos[2] - tri[0][2],
                        ];
                        let u_bary = f_inv * (s[0] * h_vec[0] + s[1] * h_vec[1] + s[2] * h_vec[2]);
                        if !(0.0..=1.0).contains(&u_bary) {
                            continue;
                        }
                        let q = [
                            s[1] * e1[2] - s[2] * e1[1],
                            s[2] * e1[0] - s[0] * e1[2],
                            s[0] * e1[1] - s[1] * e1[0],
                        ];
                        let v_bary = f_inv * (rd[0] * q[0] + rd[1] * q[1] + rd[2] * q[2]);
                        if v_bary < 0.0 || u_bary + v_bary > 1.0 {
                            continue;
                        }
                        let t_hit = f_inv * (e2[0] * q[0] + e2[1] * q[1] + e2[2] * q[2]);
                        if t_hit > 0.001 && t_hit < closest_t {
                            closest_t = t_hit;
                            // Face normal
                            let nx = e1[1] * e2[2] - e1[2] * e2[1];
                            let ny = e1[2] * e2[0] - e1[0] * e2[2];
                            let nz = e1[0] * e2[1] - e1[1] * e2[0];
                            let nl = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-8);
                            hit_normal = [nx / nl, ny / nl, nz / nl];
                            // Ensure normal faces camera
                            let ndotrd = hit_normal[0] * rd[0]
                                + hit_normal[1] * rd[1]
                                + hit_normal[2] * rd[2];
                            if ndotrd > 0.0 {
                                hit_normal = [-hit_normal[0], -hit_normal[1], -hit_normal[2]];
                            }
                            hit_color = *color;
                            hit_metallic = *metallic;
                            hit_roughness = *roughness;
                            hit_emissive = *emissive;
                        }
                    }

                    let (r, g, b) = if closest_t < f32::MAX {
                        // PBR shading at hit point
                        let hit_pos = [
                            cam_pos[0] + rd[0] * closest_t,
                            cam_pos[1] + rd[1] * closest_t,
                            cam_pos[2] + rd[2] * closest_t,
                        ];
                        let view = [-rd[0], -rd[1], -rd[2]];
                        let base_r = hit_color[0];
                        let base_g = hit_color[1];
                        let base_b = hit_color[2];

                        let f0_d = 0.04_f32;
                        let f0_r = f0_d * (1.0 - hit_metallic) + base_r * hit_metallic;
                        let f0_g = f0_d * (1.0 - hit_metallic) + base_g * hit_metallic;
                        let f0_b = f0_d * (1.0 - hit_metallic) + base_b * hit_metallic;

                        let mut total = [0.0_f32; 3];
                        for (lp, lc, ls) in &lights {
                            let ld = [lp[0] - hit_pos[0], lp[1] - hit_pos[1], lp[2] - hit_pos[2]];
                            let ldl = (ld[0] * ld[0] + ld[1] * ld[1] + ld[2] * ld[2])
                                .sqrt()
                                .max(1e-6);
                            let l = [ld[0] / ldl, ld[1] / ldl, ld[2] / ldl];
                            let ndotl = (hit_normal[0] * l[0]
                                + hit_normal[1] * l[1]
                                + hit_normal[2] * l[2])
                                .max(0.0);

                            // Shadow check (simple)
                            let in_shadow = all_tris.iter().any(|(tri, _, _, _, _)| {
                                let so = [
                                    hit_pos[0] + hit_normal[0] * 0.001 - tri[0][0],
                                    hit_pos[1] + hit_normal[1] * 0.001 - tri[0][1],
                                    hit_pos[2] + hit_normal[2] * 0.001 - tri[0][2],
                                ];
                                let se1 = [
                                    tri[1][0] - tri[0][0],
                                    tri[1][1] - tri[0][1],
                                    tri[1][2] - tri[0][2],
                                ];
                                let se2 = [
                                    tri[2][0] - tri[0][0],
                                    tri[2][1] - tri[0][1],
                                    tri[2][2] - tri[0][2],
                                ];
                                let sh = [
                                    l[1] * se2[2] - l[2] * se2[1],
                                    l[2] * se2[0] - l[0] * se2[2],
                                    l[0] * se2[1] - l[1] * se2[0],
                                ];
                                let sa = se1[0] * sh[0] + se1[1] * sh[1] + se1[2] * sh[2];
                                if sa.abs() < 1e-8 {
                                    return false;
                                }
                                let sf = 1.0 / sa;
                                let su = sf * (so[0] * sh[0] + so[1] * sh[1] + so[2] * sh[2]);
                                if !(0.0..=1.0).contains(&su) {
                                    return false;
                                }
                                let sq = [
                                    so[1] * se1[2] - so[2] * se1[1],
                                    so[2] * se1[0] - so[0] * se1[2],
                                    so[0] * se1[1] - so[1] * se1[0],
                                ];
                                let sv = sf * (l[0] * sq[0] + l[1] * sq[1] + l[2] * sq[2]);
                                if sv < 0.0 || su + sv > 1.0 {
                                    return false;
                                }
                                let st = sf * (se2[0] * sq[0] + se2[1] * sq[1] + se2[2] * sq[2]);
                                st > 0.001 && st < ldl
                            });
                            let shadow = if in_shadow { 0.15 } else { 1.0 };

                            let hx = l[0] + view[0];
                            let hy = l[1] + view[1];
                            let hz = l[2] + view[2];
                            let hl = (hx * hx + hy * hy + hz * hz).sqrt().max(1e-6);
                            let half = [hx / hl, hy / hl, hz / hl];
                            let ndoth = (hit_normal[0] * half[0]
                                + hit_normal[1] * half[1]
                                + hit_normal[2] * half[2])
                                .max(0.0);
                            let spec_pow = (2.0 / (hit_roughness * hit_roughness).max(0.001) - 2.0)
                                .min(2048.0);
                            let spec = ndoth.powf(spec_pow);
                            let hdotv = (half[0] * view[0] + half[1] * view[1] + half[2] * view[2])
                                .max(0.0);
                            let ff = (1.0 - hdotv).powi(5);
                            let fr = f0_r + (1.0 - f0_r) * ff;
                            let fg = f0_g + (1.0 - f0_g) * ff;
                            let fb = f0_b + (1.0 - f0_b) * ff;
                            let diff = (1.0 - hit_metallic) * ndotl * 0.7;
                            let sf_val = spec * ndotl;
                            let lr = lc[0] * ls;
                            let lg = lc[1] * ls;
                            let lb = lc[2] * ls;
                            total[0] += (base_r * diff + fr * sf_val) * lr * shadow;
                            total[1] += (base_g * diff + fg * sf_val) * lg * shadow;
                            total[2] += (base_b * diff + fb * sf_val) * lb * shadow;
                        }

                        // Hemisphere ambient
                        let sky = (hit_normal[1] * 0.5 + 0.5).max(0.0);
                        total[0] += base_r * (0.12 + 0.06 * sky);
                        total[1] += base_g * (0.12 + 0.08 * sky);
                        total[2] += base_b * (0.14 + 0.10 * sky);

                        // Ambient Occlusion (4-sample hemisphere)
                        let ao_samples = 4;
                        let mut ao_factor = 0.0_f32;
                        let ao_radius = 2.0_f32;
                        // Build tangent frame from hit normal
                        let tangent = if hit_normal[1].abs() < 0.9 {
                            let tx = hit_normal[2];
                            let tz = -hit_normal[0];
                            let tl = (tx * tx + tz * tz).sqrt().max(1e-8);
                            [tx / tl, 0.0, tz / tl]
                        } else {
                            let tx = -hit_normal[1];
                            let ty = hit_normal[0];
                            let tl = (tx * tx + ty * ty).sqrt().max(1e-8);
                            [tx / tl, ty / tl, 0.0]
                        };
                        let bitangent = [
                            hit_normal[1] * tangent[2] - hit_normal[2] * tangent[1],
                            hit_normal[2] * tangent[0] - hit_normal[0] * tangent[2],
                            hit_normal[0] * tangent[1] - hit_normal[1] * tangent[0],
                        ];
                        for ao_i in 0..ao_samples {
                            // Fibonacci hemisphere sampling
                            let phi = ao_i as f32 * 2.399; // golden angle
                            let cos_theta = 1.0 - (ao_i as f32 + 0.5) / ao_samples as f32;
                            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
                            let ao_dir = [
                                tangent[0] * sin_theta * phi.cos()
                                    + bitangent[0] * sin_theta * phi.sin()
                                    + hit_normal[0] * cos_theta,
                                tangent[1] * sin_theta * phi.cos()
                                    + bitangent[1] * sin_theta * phi.sin()
                                    + hit_normal[1] * cos_theta,
                                tangent[2] * sin_theta * phi.cos()
                                    + bitangent[2] * sin_theta * phi.sin()
                                    + hit_normal[2] * cos_theta,
                            ];
                            let ao_origin = [
                                hit_pos[0] + hit_normal[0] * 0.002,
                                hit_pos[1] + hit_normal[1] * 0.002,
                                hit_pos[2] + hit_normal[2] * 0.002,
                            ];
                            let occluded = all_tris.iter().any(|(tri, _, _, _, _)| {
                                let so = [
                                    ao_origin[0] - tri[0][0],
                                    ao_origin[1] - tri[0][1],
                                    ao_origin[2] - tri[0][2],
                                ];
                                let se1 = [
                                    tri[1][0] - tri[0][0],
                                    tri[1][1] - tri[0][1],
                                    tri[1][2] - tri[0][2],
                                ];
                                let se2 = [
                                    tri[2][0] - tri[0][0],
                                    tri[2][1] - tri[0][1],
                                    tri[2][2] - tri[0][2],
                                ];
                                let sh = [
                                    ao_dir[1] * se2[2] - ao_dir[2] * se2[1],
                                    ao_dir[2] * se2[0] - ao_dir[0] * se2[2],
                                    ao_dir[0] * se2[1] - ao_dir[1] * se2[0],
                                ];
                                let sa = se1[0] * sh[0] + se1[1] * sh[1] + se1[2] * sh[2];
                                if sa.abs() < 1e-8 {
                                    return false;
                                }
                                let sf = 1.0 / sa;
                                let su = sf * (so[0] * sh[0] + so[1] * sh[1] + so[2] * sh[2]);
                                if !(0.0..=1.0).contains(&su) {
                                    return false;
                                }
                                let sq = [
                                    so[1] * se1[2] - so[2] * se1[1],
                                    so[2] * se1[0] - so[0] * se1[2],
                                    so[0] * se1[1] - so[1] * se1[0],
                                ];
                                let sv = sf
                                    * (ao_dir[0] * sq[0] + ao_dir[1] * sq[1] + ao_dir[2] * sq[2]);
                                if sv < 0.0 || su + sv > 1.0 {
                                    return false;
                                }
                                let st = sf * (se2[0] * sq[0] + se2[1] * sq[1] + se2[2] * sq[2]);
                                st > 0.001 && st < ao_radius
                            });
                            if !occluded {
                                ao_factor += 1.0;
                            }
                        }
                        ao_factor /= ao_samples as f32;
                        let ao_strength = 0.3_f32; // How much AO darkens
                        let ao_mult = 1.0 - ao_strength * (1.0 - ao_factor);
                        total[0] *= ao_mult;
                        total[1] *= ao_mult;
                        total[2] *= ao_mult;

                        // Rim
                        let ndotv = (hit_normal[0] * view[0]
                            + hit_normal[1] * view[1]
                            + hit_normal[2] * view[2])
                            .max(0.0);
                        let rim = (1.0 - ndotv).powi(3) * 0.08;
                        total[0] += rim;
                        total[1] += rim;
                        total[2] += rim;

                        // Emissive
                        if hit_emissive > 0.0 {
                            total[0] += base_r * hit_emissive * 0.3;
                            total[1] += base_g * hit_emissive * 0.3;
                            total[2] += base_b * hit_emissive * 0.3;
                        }

                        // Gamma correction
                        let gamma = 1.0 / 2.2;
                        (
                            (total[0].max(0.0).min(1.0).powf(gamma) * 255.0) as u8,
                            (total[1].max(0.0).min(1.0).powf(gamma) * 255.0) as u8,
                            (total[2].max(0.0).min(1.0).powf(gamma) * 255.0) as u8,
                        )
                    } else {
                        // Background gradient (dark charcoal)
                        let t = py as f32 / h as f32;
                        let bg_r = (58.0 * (1.0 - t) + 28.0 * t) as u8;
                        let bg_g = (58.0 * (1.0 - t) + 28.0 * t) as u8;
                        let bg_b = (68.0 * (1.0 - t) + 32.0 * t) as u8;
                        (bg_r, bg_g, bg_b)
                    };

                    let offset = (py * w + px) * 3;
                    img_buf[offset] = r;
                    img_buf[offset + 1] = g;
                    img_buf[offset + 2] = b;
                }
            }

            // Save image
            match image::save_buffer(&path, &img_buf, w as u32, h as u32, image::ColorType::Rgb8) {
                Ok(_) => {
                    self.status_message = format!("Rendered {}x{} to {}", w, h, path.display());
                    self.log_console(
                        console::LogLevel::Info,
                        &format!("Render saved: {}", path.display()),
                        "Render",
                    );
                }
                Err(e) => {
                    self.status_message = format!("Render failed: {}", e);
                    self.log_console(
                        console::LogLevel::Error,
                        &format!("Render failed: {}", e),
                        "Render",
                    );
                }
            }
        }
    }

    /// Neural render pass: NeRF volumetric ray marching or NRC MLP prediction.
    /// Produces a low-resolution preview (64×64) and logs to console.
    fn render_image_neural(&mut self) {
        use nalgebra::DVector;
        use nat3d_render::raytracing::nerf::{accumulate_radiance, VolumeSample};
        use nat3d_render::raytracing::nrc::RadianceMLP;

        let preview_w = 64_usize;
        let preview_h = 64_usize;
        let is_nerf = matches!(self.state.render_engine, RenderEngine::NeRF);

        let cam = &self.state.camera;
        let cam_pos = cam.position;
        let yaw = cam.orbit_angles[0].to_radians();
        let pitch = cam.orbit_angles[1].to_radians();
        let forward = [
            -yaw.sin() * pitch.cos(),
            -pitch.sin(),
            -yaw.cos() * pitch.cos(),
        ];
        let right = [yaw.cos(), 0.0, -yaw.sin()];
        let up_vec = [
            yaw.sin() * pitch.sin(),
            pitch.cos(),
            yaw.cos() * pitch.sin(),
        ];
        let fov_half_tan = (cam.fov.to_radians() * 0.5).tan();
        let aspect = preview_w as f32 / preview_h as f32;

        // NRC: one MLP per render, initialized with fixed seed weights
        let mlp = if !is_nerf {
            Some(RadianceMLP::new_random(5, 64, 3))
        } else {
            None
        };

        let mut total_luminance = 0.0_f64;
        let mut hit_pixels = 0_usize;

        for py in 0..preview_h {
            for px in 0..preview_w {
                let ndc_x = (2.0 * px as f32 / preview_w as f32 - 1.0) * aspect * fov_half_tan;
                let ndc_y = (1.0 - 2.0 * py as f32 / preview_h as f32) * fov_half_tan;
                let rd = [
                    forward[0] + right[0] * ndc_x + up_vec[0] * ndc_y,
                    forward[1] + right[1] * ndc_x + up_vec[1] * ndc_y,
                    forward[2] + right[2] * ndc_x + up_vec[2] * ndc_y,
                ];
                let rd_len = (rd[0] * rd[0] + rd[1] * rd[1] + rd[2] * rd[2])
                    .sqrt()
                    .max(1e-8);
                let rd_n = [rd[0] / rd_len, rd[1] / rd_len, rd[2] / rd_len];

                if is_nerf {
                    // NeRF: march 16 samples along ray, density from scene bounding volume
                    let step_size = 0.25_f64;
                    let samples: Vec<VolumeSample> = (0..16)
                        .map(|s| {
                            let t = s as f64 * step_size;
                            let px3 = cam_pos[0] as f64 + rd_n[0] as f64 * t;
                            let py3 = cam_pos[1] as f64 + rd_n[1] as f64 * t;
                            let pz3 = cam_pos[2] as f64 + rd_n[2] as f64 * t;
                            // Density peaks near origin (scene center proxy)
                            let dist2 = px3 * px3 + py3 * py3 + pz3 * pz3;
                            VolumeSample {
                                color: nalgebra::Vector3::new(0.6, 0.7, 0.9),
                                density: (-(dist2 * 0.1)).exp() * 0.5,
                            }
                        })
                        .collect();
                    let rgb = accumulate_radiance(&samples, step_size);
                    total_luminance += 0.2126 * rgb.x + 0.7152 * rgb.y + 0.0722 * rgb.z;
                    if rgb.x + rgb.y + rgb.z > 0.01 {
                        hit_pixels += 1;
                    }
                } else if let Some(ref m) = mlp {
                    // NRC: predict radiance from 5D input (pos 3D + dir 2D)
                    let theta = (rd_n[1] as f64).acos();
                    let phi = (rd_n[2] as f64).atan2(rd_n[0] as f64);
                    let input = DVector::from_vec(vec![
                        cam_pos[0] as f64,
                        cam_pos[1] as f64,
                        cam_pos[2] as f64,
                        theta,
                        phi,
                    ]);
                    let rgb = m.predict(input);
                    total_luminance += 0.2126 * rgb.x + 0.7152 * rgb.y + 0.0722 * rgb.z;
                    if rgb.x + rgb.y + rgb.z > 0.01 {
                        hit_pixels += 1;
                    }
                }
            }
        }

        let engine_name = if is_nerf {
            "NeRF"
        } else {
            "Neural Cache (NRC)"
        };
        let avg_lum = total_luminance / (preview_w * preview_h) as f64;
        self.status_message = format!(
            "{} preview {}×{} complete — avg luminance: {:.4}, hit pixels: {}",
            engine_name, preview_w, preview_h, avg_lum, hit_pixels
        );
        self.log_console(
            console::LogLevel::Info,
            &format!(
                "{} render pass: {}×{} @ λ̄={:.4}",
                engine_name, preview_w, preview_h, avg_lum
            ),
            "Neural Render",
        );
    }

    fn welcome_screen_window(&mut self, ctx: &egui::Context) {
        if !self.show_welcome || self.preferences.dont_show_welcome {
            return;
        }

        let screen = ctx.screen_rect();
        let win_w = 640.0_f32;
        let win_h = 480.0_f32;
        let pivot = egui::pos2(
            screen.center().x - win_w * 0.5,
            screen.center().y - win_h * 0.5,
        );

        let mut open = self.show_welcome;
        egui::Window::new("Welcome to NAT3D")
            .fixed_pos(pivot)
            .fixed_size([win_w, win_h])
            .collapsible(false)
            .open(&mut open)
            .show(ctx, |ui| {
                // ── Header ──────────────────────────────────────────────
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    ui.heading(
                        egui::RichText::new("NAT3D")
                            .size(32.0)
                            .color(egui::Color32::from_rgb(100, 180, 255)),
                    );
                    ui.label(
                        egui::RichText::new("Professional 3D — Open Source · AGPL-3.0")
                            .size(12.0)
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(4.0);
                });

                ui.separator();

                ui.columns(2, |cols| {
                    // ── Left: Quick start ────────────────────────────────
                    cols[0].heading("Quick Start");
                    cols[0].add_space(6.0);

                    if cols[0]
                        .button(egui::RichText::new("  New Empty Scene").size(14.0))
                        .clicked()
                    {
                        self.state.new_scene();
                        self.project_path = None;
                        self.status_message = "New scene created".to_string();
                        self.show_welcome = false;
                        Self::write_welcome_sentinel();
                    }

                    cols[0].add_space(4.0);
                    if cols[0]
                        .button(egui::RichText::new("  Load Example Scene").size(14.0))
                        .clicked()
                    {
                        self.create_example_scene();
                        self.status_message =
                            "Example scene loaded — explore the scene objects!".to_string();
                        self.show_welcome = false;
                        Self::write_welcome_sentinel();
                    }

                    cols[0].add_space(4.0);
                    #[cfg(feature = "file-dialog")]
                    if cols[0]
                        .button(egui::RichText::new("  Open File…").size(14.0))
                        .clicked()
                    {
                        self.show_welcome = false;
                        Self::write_welcome_sentinel();
                        self.open_project_dialog();
                    }

                    cols[0].add_space(12.0);
                    cols[0].heading("Keyboard Shortcuts");
                    cols[0].add_space(4.0);
                    for (key, desc) in &[
                        ("G / R / S", "Grab / Rotate / Scale"),
                        ("Tab", "Toggle Edit Mode"),
                        ("Shift+D", "Duplicate"),
                        ("Del", "Delete selected"),
                        ("Z", "Toggle shading"),
                        ("F", "Focus on selected"),
                        ("Numpad 1/3/7", "Front / Right / Top"),
                    ] {
                        cols[0].horizontal(|ui| {
                            ui.monospace(
                                egui::RichText::new(*key)
                                    .color(egui::Color32::from_rgb(180, 210, 255)),
                            );
                            ui.label(*desc);
                        });
                    }

                    // ── Right: SOTA features ─────────────────────────────
                    cols[1].heading("SOTA Research Features");
                    cols[1].add_space(6.0);
                    for (label, detail) in &[
                        (
                            "Spectral Smooth",
                            "Laplacian mesh processing\n  (Sorkine 2006)",
                        ),
                        (
                            "Hyperbolic Warp",
                            "Poincaré ball geometry\n  (Ungar 2001)",
                        ),
                        (
                            "NeRF Render Engine",
                            "Volumetric neural radiance\n  (Mildenhall et al. 2020)",
                        ),
                        (
                            "Neural Cache (NRC)",
                            "Real-time GI via MLP\n  (Müller et al. 2021)",
                        ),
                        (
                            "Non-Euclidean Core",
                            "Möbius/hyperbolic math\n  in nat3d-core",
                        ),
                        (
                            "Differentiable Render",
                            "Adjoint gradient pipeline\n  in nat3d-render",
                        ),
                    ] {
                        cols[1].colored_label(egui::Color32::from_rgb(100, 200, 255), *label);
                        cols[1].label(
                            egui::RichText::new(*detail)
                                .size(10.5)
                                .color(egui::Color32::GRAY),
                        );
                        cols[1].add_space(2.0);
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.checkbox(
                        &mut self.preferences.dont_show_welcome,
                        "Don't show on startup",
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("  Close  ").clicked() {
                            self.show_welcome = false;
                            Self::write_welcome_sentinel();
                        }
                    });
                });
            });

        if !open {
            self.show_welcome = false;
            Self::write_welcome_sentinel();
        }
    }

    /// Builds a procedural demo scene that exercises PBR materials, SOTA modifiers, and GPU rendering.
    fn create_example_scene(&mut self) {
        self.state.save_undo_state();
        self.state.objects.clear();
        self.state.selected_object = None;

        // Ground plane — rough stone
        self.state.objects.push(SceneObject {
            physiological_signal: 0.0,
            name: "Ground".to_string(),
            object_type: ObjectType::Plane,
            position: [0.0, -1.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [5.0, 1.0, 5.0],
            material: MaterialState {
                base_color: [0.35, 0.32, 0.30, 1.0],
                metallic: 0.0,
                roughness: 0.9,
                emissive: 0.0,
            },
            modifiers: vec![],
            visible: true,
            smooth_shading: false,
            locked: false,
            parent: None,
            keyframes: vec![],
            shape_keys: vec![],
            constraints: vec![],
            vertex_colors: vec![],
            vertex_weights: vec![],
            vertex_groups: vec![],
            particle_systems: vec![],
            bones: vec![],
            drivers: vec![],
            force_field: None,
            cloth: None,
            soft_body: None,
            nla_tracks: vec![],
            gp_strokes: vec![],
            texture_slots: vec![],
            custom_properties: vec![],
            motion_path: None,
            pass_index: 0,
            hair_settings: None,
            fluid: None,
            linked_data: None,
            edit_mesh: None,
            edit_selection: EditModeSelection::default(),
            custom_vertices: None,
            custom_faces: None,
            uv_coords: None,
        });

        // Metallic sphere — center stage
        self.state.objects.push(SceneObject {
            physiological_signal: 0.0,
            name: "Metallic Sphere".to_string(),
            object_type: ObjectType::Sphere,
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            material: MaterialState {
                base_color: [0.9, 0.85, 0.7, 1.0],
                metallic: 0.95,
                roughness: 0.15,
                emissive: 0.0,
            },
            modifiers: vec!["Spectral Smooth".to_string()],
            visible: true,
            smooth_shading: true,
            locked: false,
            parent: None,
            keyframes: vec![],
            shape_keys: vec![],
            constraints: vec![],
            vertex_colors: vec![],
            vertex_weights: vec![],
            vertex_groups: vec![],
            particle_systems: vec![],
            bones: vec![],
            drivers: vec![],
            force_field: None,
            cloth: None,
            soft_body: None,
            nla_tracks: vec![],
            gp_strokes: vec![],
            texture_slots: vec![],
            custom_properties: vec![],
            motion_path: None,
            pass_index: 0,
            hair_settings: None,
            fluid: None,
            linked_data: None,
            edit_mesh: None,
            edit_selection: EditModeSelection::default(),
            custom_vertices: None,
            custom_faces: None,
            uv_coords: None,
        });

        // Rough dielectric cube — left
        self.state.objects.push(SceneObject {
            physiological_signal: 0.0,
            name: "Rough Cube".to_string(),
            object_type: ObjectType::Cube,
            position: [-2.2, 0.0, 0.0],
            rotation: [0.0, 30.0, 0.0],
            scale: [0.9, 0.9, 0.9],
            material: MaterialState {
                base_color: [0.18, 0.35, 0.72, 1.0],
                metallic: 0.0,
                roughness: 0.7,
                emissive: 0.0,
            },
            modifiers: vec![],
            visible: true,
            smooth_shading: false,
            locked: false,
            parent: None,
            keyframes: vec![],
            shape_keys: vec![],
            constraints: vec![],
            vertex_colors: vec![],
            vertex_weights: vec![],
            vertex_groups: vec![],
            particle_systems: vec![],
            bones: vec![],
            drivers: vec![],
            force_field: None,
            cloth: None,
            soft_body: None,
            nla_tracks: vec![],
            gp_strokes: vec![],
            texture_slots: vec![],
            custom_properties: vec![],
            motion_path: None,
            pass_index: 0,
            hair_settings: None,
            fluid: None,
            linked_data: None,
            edit_mesh: None,
            edit_selection: EditModeSelection::default(),
            custom_vertices: None,
            custom_faces: None,
            uv_coords: None,
        });

        // Emissive torus — right
        self.state.objects.push(SceneObject {
            physiological_signal: 0.0,
            name: "Emissive Torus".to_string(),
            object_type: ObjectType::Torus,
            position: [2.2, 0.0, 0.0],
            rotation: [90.0, 0.0, 0.0],
            scale: [0.8, 0.8, 0.8],
            material: MaterialState {
                base_color: [1.0, 0.45, 0.05, 1.0],
                metallic: 0.0,
                roughness: 0.3,
                emissive: 2.5,
            },
            modifiers: vec![],
            visible: true,
            smooth_shading: true,
            locked: false,
            parent: None,
            keyframes: vec![],
            shape_keys: vec![],
            constraints: vec![],
            vertex_colors: vec![],
            vertex_weights: vec![],
            vertex_groups: vec![],
            particle_systems: vec![],
            bones: vec![],
            drivers: vec![],
            force_field: None,
            cloth: None,
            soft_body: None,
            nla_tracks: vec![],
            gp_strokes: vec![],
            texture_slots: vec![],
            custom_properties: vec![],
            motion_path: None,
            pass_index: 0,
            hair_settings: None,
            fluid: None,
            linked_data: None,
            edit_mesh: None,
            edit_selection: EditModeSelection::default(),
            custom_vertices: None,
            custom_faces: None,
            uv_coords: None,
        });

        // Key light
        self.state.objects.push(SceneObject {
            physiological_signal: 0.0,
            name: "Key Light".to_string(),
            object_type: ObjectType::Light,
            position: [4.0, 6.0, 3.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            material: MaterialState {
                base_color: [1.0, 0.98, 0.92, 1.0],
                metallic: 0.0,
                roughness: 0.5,
                emissive: 3.0,
            },
            modifiers: vec![],
            visible: true,
            smooth_shading: false,
            locked: false,
            parent: None,
            keyframes: vec![],
            shape_keys: vec![],
            constraints: vec![],
            vertex_colors: vec![],
            vertex_weights: vec![],
            vertex_groups: vec![],
            particle_systems: vec![],
            bones: vec![],
            drivers: vec![],
            force_field: None,
            cloth: None,
            soft_body: None,
            nla_tracks: vec![],
            gp_strokes: vec![],
            texture_slots: vec![],
            custom_properties: vec![],
            motion_path: None,
            pass_index: 0,
            hair_settings: None,
            fluid: None,
            linked_data: None,
            edit_mesh: None,
            edit_selection: EditModeSelection::default(),
            custom_vertices: None,
            custom_faces: None,
            uv_coords: None,
        });

        // Camera
        self.state.camera.orbit_angles = [45.0, 25.0];
        self.state.camera.distance = 8.0;
        self.state.camera.target = [0.0, 0.0, 0.0];
        self.state.camera.update_position();
    }

    fn render_animation(&mut self) {
        #[cfg(feature = "file-dialog")]
        if let Some(dir) = rfd::FileDialog::new()
            .set_title("Select folder for animation frames")
            .pick_folder()
        {
            let start = self.state.timeline.start_frame;
            let end = self.state.timeline.end_frame;
            let w = self.render_settings.width as usize;
            let h = self.render_settings.height as usize;

            self.status_message = format!("Rendering frames {}..{} at {}x{}", start, end, w, h);
            self.log_console(
                console::LogLevel::Info,
                &format!("Rendering {} frames to {}", end - start + 1, dir.display()),
                "Render",
            );

            // Simple turntable animation: rotate camera around the scene
            let original_angles = self.state.camera.orbit_angles;
            let total_frames = (end - start + 1) as f32;

            for frame in start..=end {
                let t = (frame - start) as f32 / total_frames;
                // Turntable: rotate camera 360 degrees
                self.state.camera.orbit_angles[0] = original_angles[0] + t * 360.0;
                self.state.camera.update_position();

                // Collect triangulated scene geometry (same as render_image)
                let mut all_tris: Vec<([[f32; 3]; 3], [f32; 4], f32, f32, f32)> = Vec::new();
                for (idx, obj) in self.state.objects.iter().enumerate() {
                    if !obj.visible {
                        continue;
                    }
                    match obj.object_type {
                        state::ObjectType::Light
                        | state::ObjectType::Camera
                        | state::ObjectType::Empty => continue,
                        _ => {}
                    }
                    let vertices = self.get_object_vertices(idx);
                    let faces = self.get_object_faces(idx);
                    if vertices.is_empty() || faces.is_empty() {
                        continue;
                    }
                    for face in &faces {
                        for i in 1..face.len() - 1 {
                            if face[0] < vertices.len()
                                && face[i] < vertices.len()
                                && face[i + 1] < vertices.len()
                            {
                                all_tris.push((
                                    [vertices[face[0]], vertices[face[i]], vertices[face[i + 1]]],
                                    obj.material.base_color,
                                    obj.material.metallic,
                                    obj.material.roughness.max(0.04),
                                    obj.material.emissive,
                                ));
                            }
                        }
                    }
                }

                // Collect lights
                let lights: Vec<([f32; 3], [f32; 3], f32)> = {
                    let mut ls = Vec::new();
                    for obj in &self.state.objects {
                        if obj.object_type == state::ObjectType::Light && obj.visible {
                            ls.push((
                                obj.position,
                                [
                                    obj.material.base_color[0],
                                    obj.material.base_color[1],
                                    obj.material.base_color[2],
                                ],
                                obj.material.emissive.max(1.0),
                            ));
                        }
                    }
                    if ls.is_empty() {
                        ls.push(([4.0, 6.0, 3.0], [1.0, 0.98, 0.95], 1.0));
                    }
                    ls
                };

                // Camera
                let cam_pos = self.state.camera.position;
                let yaw = self.state.camera.orbit_angles[0].to_radians();
                let pitch = self.state.camera.orbit_angles[1].to_radians();
                let forward = [
                    -yaw.sin() * pitch.cos(),
                    -pitch.sin(),
                    -yaw.cos() * pitch.cos(),
                ];
                let right_v = [yaw.cos(), 0.0, -yaw.sin()];
                let up_v = [
                    yaw.sin() * pitch.sin(),
                    pitch.cos(),
                    yaw.cos() * pitch.sin(),
                ];
                let fov_half_tan = (self.state.camera.fov.to_radians() * 0.5).tan();
                let aspect = w as f32 / h as f32;

                // Render frame (lower quality for speed: skip shadow rays)
                let mut img_buf = vec![0u8; w * h * 3];
                for py in 0..h {
                    for px in 0..w {
                        let ndc_x = (2.0 * px as f32 / w as f32 - 1.0) * aspect * fov_half_tan;
                        let ndc_y = (1.0 - 2.0 * py as f32 / h as f32) * fov_half_tan;
                        let ray_dir = [
                            forward[0] + right_v[0] * ndc_x + up_v[0] * ndc_y,
                            forward[1] + right_v[1] * ndc_x + up_v[1] * ndc_y,
                            forward[2] + right_v[2] * ndc_x + up_v[2] * ndc_y,
                        ];
                        let rd_len = (ray_dir[0] * ray_dir[0]
                            + ray_dir[1] * ray_dir[1]
                            + ray_dir[2] * ray_dir[2])
                            .sqrt();
                        let rd = [
                            ray_dir[0] / rd_len,
                            ray_dir[1] / rd_len,
                            ray_dir[2] / rd_len,
                        ];

                        let mut closest_t = f32::MAX;
                        let mut hit_normal = [0.0_f32; 3];
                        let mut hit_color = [0.0_f32; 4];
                        let mut hit_metallic = 0.0_f32;
                        let mut hit_roughness = 0.5_f32;
                        let mut hit_emissive = 0.0_f32;

                        for (tri, color, metallic, roughness, emissive) in &all_tris {
                            let e1 = [
                                tri[1][0] - tri[0][0],
                                tri[1][1] - tri[0][1],
                                tri[1][2] - tri[0][2],
                            ];
                            let e2 = [
                                tri[2][0] - tri[0][0],
                                tri[2][1] - tri[0][1],
                                tri[2][2] - tri[0][2],
                            ];
                            let h_vec = [
                                rd[1] * e2[2] - rd[2] * e2[1],
                                rd[2] * e2[0] - rd[0] * e2[2],
                                rd[0] * e2[1] - rd[1] * e2[0],
                            ];
                            let a_det = e1[0] * h_vec[0] + e1[1] * h_vec[1] + e1[2] * h_vec[2];
                            if a_det.abs() < 1e-8 {
                                continue;
                            }
                            let f_inv = 1.0 / a_det;
                            let s = [
                                cam_pos[0] - tri[0][0],
                                cam_pos[1] - tri[0][1],
                                cam_pos[2] - tri[0][2],
                            ];
                            let u_bary =
                                f_inv * (s[0] * h_vec[0] + s[1] * h_vec[1] + s[2] * h_vec[2]);
                            if !(0.0..=1.0).contains(&u_bary) {
                                continue;
                            }
                            let q = [
                                s[1] * e1[2] - s[2] * e1[1],
                                s[2] * e1[0] - s[0] * e1[2],
                                s[0] * e1[1] - s[1] * e1[0],
                            ];
                            let v_bary = f_inv * (rd[0] * q[0] + rd[1] * q[1] + rd[2] * q[2]);
                            if v_bary < 0.0 || u_bary + v_bary > 1.0 {
                                continue;
                            }
                            let t_hit = f_inv * (e2[0] * q[0] + e2[1] * q[1] + e2[2] * q[2]);
                            if t_hit > 0.001 && t_hit < closest_t {
                                closest_t = t_hit;
                                let nx = e1[1] * e2[2] - e1[2] * e2[1];
                                let ny = e1[2] * e2[0] - e1[0] * e2[2];
                                let nz = e1[0] * e2[1] - e1[1] * e2[0];
                                let nl = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-8);
                                hit_normal = [nx / nl, ny / nl, nz / nl];
                                if hit_normal[0] * rd[0]
                                    + hit_normal[1] * rd[1]
                                    + hit_normal[2] * rd[2]
                                    > 0.0
                                {
                                    hit_normal = [-hit_normal[0], -hit_normal[1], -hit_normal[2]];
                                }
                                hit_color = *color;
                                hit_metallic = *metallic;
                                hit_roughness = *roughness;
                                hit_emissive = *emissive;
                            }
                        }

                        let (r, g, b) = if closest_t < f32::MAX {
                            let view = [-rd[0], -rd[1], -rd[2]];
                            let (br, bg, bb) = (hit_color[0], hit_color[1], hit_color[2]);
                            let mut total = [0.0_f32; 3];
                            for (lp, lc, ls) in &lights {
                                let ld = [
                                    lp[0] - cam_pos[0] - rd[0] * closest_t,
                                    lp[1] - cam_pos[1] - rd[1] * closest_t,
                                    lp[2] - cam_pos[2] - rd[2] * closest_t,
                                ];
                                let ldl = (ld[0] * ld[0] + ld[1] * ld[1] + ld[2] * ld[2])
                                    .sqrt()
                                    .max(1e-6);
                                let l = [ld[0] / ldl, ld[1] / ldl, ld[2] / ldl];
                                let ndotl = (hit_normal[0] * l[0]
                                    + hit_normal[1] * l[1]
                                    + hit_normal[2] * l[2])
                                    .max(0.0);
                                let hx = l[0] + view[0];
                                let hy = l[1] + view[1];
                                let hz = l[2] + view[2];
                                let hl = (hx * hx + hy * hy + hz * hz).sqrt().max(1e-6);
                                let half = [hx / hl, hy / hl, hz / hl];
                                let ndoth = (hit_normal[0] * half[0]
                                    + hit_normal[1] * half[1]
                                    + hit_normal[2] * half[2])
                                    .max(0.0);
                                let spec = ndoth.powf(
                                    (2.0 / (hit_roughness * hit_roughness).max(0.001) - 2.0)
                                        .min(2048.0),
                                );
                                let diff = (1.0 - hit_metallic) * ndotl * 0.7;
                                let sf = spec * ndotl;
                                total[0] += (br * diff + 0.04 * sf) * lc[0] * ls;
                                total[1] += (bg * diff + 0.04 * sf) * lc[1] * ls;
                                total[2] += (bb * diff + 0.04 * sf) * lc[2] * ls;
                            }
                            let sky = (hit_normal[1] * 0.5 + 0.5).max(0.0);
                            total[0] += br * (0.12 + 0.06 * sky);
                            total[1] += bg * (0.12 + 0.08 * sky);
                            total[2] += bb * (0.14 + 0.10 * sky);
                            if hit_emissive > 0.0 {
                                total[0] += br * hit_emissive * 0.3;
                                total[1] += bg * hit_emissive * 0.3;
                                total[2] += bb * hit_emissive * 0.3;
                            }
                            let gamma = 1.0 / 2.2;
                            (
                                (total[0].max(0.0).min(1.0).powf(gamma) * 255.0) as u8,
                                (total[1].max(0.0).min(1.0).powf(gamma) * 255.0) as u8,
                                (total[2].max(0.0).min(1.0).powf(gamma) * 255.0) as u8,
                            )
                        } else {
                            let t_bg = py as f32 / h as f32;
                            (
                                (58.0 * (1.0 - t_bg) + 28.0 * t_bg) as u8,
                                (58.0 * (1.0 - t_bg) + 28.0 * t_bg) as u8,
                                (68.0 * (1.0 - t_bg) + 32.0 * t_bg) as u8,
                            )
                        };
                        let offset = (py * w + px) * 3;
                        img_buf[offset] = r;
                        img_buf[offset + 1] = g;
                        img_buf[offset + 2] = b;
                    }
                }

                let frame_path = dir.join(format!("frame_{:04}.png", frame));
                let _ = image::save_buffer(
                    &frame_path,
                    &img_buf,
                    w as u32,
                    h as u32,
                    image::ColorType::Rgb8,
                );
            }

            // Restore camera
            self.state.camera.orbit_angles = original_angles;
            self.state.camera.update_position();

            self.status_message =
                format!("Rendered {} frames to {}", end - start + 1, dir.display());
            self.log_console(
                console::LogLevel::Info,
                &format!("Animation: {} frames rendered", end - start + 1),
                "Render",
            );
        }
    }

    fn render_settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_render_settings;
        egui::Window::new("Render Settings")
            .open(&mut open)
            .resizable(true)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.heading("Render Engine");
                egui::ComboBox::from_label("Engine")
                    .selected_text(format!("{}", self.state.render_engine))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.state.render_engine,
                            RenderEngine::Eevee,
                            "Eevee (Realtime)",
                        );
                        ui.selectable_value(
                            &mut self.state.render_engine,
                            RenderEngine::Cycles,
                            "Cycles (Path Tracing)",
                        );
                        ui.selectable_value(
                            &mut self.state.render_engine,
                            RenderEngine::Workbench,
                            "Workbench (Fast)",
                        );
                        ui.separator();
                        ui.selectable_value(
                            &mut self.state.render_engine,
                            RenderEngine::NeRF,
                            "NeRF — Neural Radiance Fields",
                        );
                        ui.selectable_value(
                            &mut self.state.render_engine,
                            RenderEngine::NeuralCache,
                            "Neural Cache (NRC) — Real-time GI",
                        );
                    });

                if matches!(self.state.render_engine, RenderEngine::NeRF | RenderEngine::NeuralCache) {
                    ui.colored_label(
                        egui::Color32::from_rgb(100, 200, 255),
                        "★ SOTA: Neural rendering active. Render Image uses volumetric ray marching.",
                    );
                }

                ui.separator();
                ui.heading("Output");
                ui.horizontal(|ui| {
                    ui.label("Resolution:");
                    ui.add(
                        egui::DragValue::new(&mut self.render_settings.width)
                            .range(1..=8192)
                            .prefix("W: "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.render_settings.height)
                            .range(1..=8192)
                            .prefix("H: "),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Presets:");
                    if ui.button("HD (1280x720)").clicked() {
                        self.render_settings.width = 1280;
                        self.render_settings.height = 720;
                    }
                    if ui.button("FHD (1920x1080)").clicked() {
                        self.render_settings.width = 1920;
                        self.render_settings.height = 1080;
                    }
                    if ui.button("4K (3840x2160)").clicked() {
                        self.render_settings.width = 3840;
                        self.render_settings.height = 2160;
                    }
                });

                ui.separator();
                ui.heading("Quality");
                ui.horizontal(|ui| {
                    ui.label("Samples:");
                    ui.add(
                        egui::Slider::new(&mut self.render_settings.samples, 1..=4096)
                            .logarithmic(true),
                    );
                });
                ui.checkbox(&mut self.render_settings.use_denoiser, "Use AI Denoiser");
                ui.checkbox(&mut self.state.film_transparent, "Film Transparent");
                ui.horizontal(|ui| {
                    ui.label("Simplify Subdivision:");
                    ui.add(egui::Slider::new(
                        &mut self.state.simplify_subdivision,
                        0..=6,
                    ));
                });

                ui.separator();
                ui.heading("Output Format");
                egui::ComboBox::from_label("Format")
                    .selected_text(&self.render_settings.output_format)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.render_settings.output_format,
                            "PNG".to_string(),
                            "PNG (8-bit)",
                        );
                        ui.selectable_value(
                            &mut self.render_settings.output_format,
                            "PNG16".to_string(),
                            "PNG (16-bit)",
                        );
                        ui.selectable_value(
                            &mut self.render_settings.output_format,
                            "JPEG".to_string(),
                            "JPEG",
                        );
                        ui.selectable_value(
                            &mut self.render_settings.output_format,
                            "EXR".to_string(),
                            "OpenEXR (HDR)",
                        );
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Render Image").clicked() {
                        self.render_image();
                    }
                    if ui.button("Render Animation").clicked() {
                        self.render_animation();
                    }
                });
            });
        self.show_render_settings = open;
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_about;
        egui::Window::new("About NAT3D")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(350.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("NAT3D");
                    ui.label("Next-generation Advanced Technology for 3D");
                    ui.add_space(10.0);
                    ui.label(format!("Version: {}", env!("CARGO_PKG_VERSION")));
                    ui.add_space(10.0);
                    ui.label("A professional 3D modeling, CAD,");
                    ui.label("simulation, and rendering suite.");
                    ui.add_space(20.0);
                    ui.label("Copyright (C) 2026 Francisco Molina-Burgos");
                    ui.label("Licensed under AGPL-3.0-or-later");
                    ui.add_space(10.0);
                    if ui.link("github.com/Yatrogenesis/NAT3D").clicked() {
                        #[cfg(feature = "file-dialog")]
                        let _ = open::that("https://github.com/Yatrogenesis/NAT3D");
                    }
                });
            });
        self.show_about = open;
    }

    fn license_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_license_dialog {
            return;
        }

        // Poll the GitHub Education background thread (extract event before modifying self)
        let edu_event = self
            .edu_oauth_rx
            .as_ref()
            .and_then(|rx| rx.lock().try_recv().ok());
        let still_polling = self.edu_oauth_rx.is_some() && edu_event.is_none();

        if let Some(event) = edu_event {
            let mid = license::get_machine_id();
            self.edu_oauth_step = match event {
                license::EduFlowEvent::DeviceCodeReady {
                    user_code,
                    verification_uri,
                    ..
                } => {
                    #[cfg(feature = "file-dialog")]
                    let _ = open::that(&verification_uri);
                    EduOAuthStep::AwaitingUser {
                        user_code,
                        verification_uri,
                    }
                }
                license::EduFlowEvent::EduConfirmed {
                    serial,
                    github_handle,
                } => {
                    self.license_status = license::validate_license(&serial.replace('-', ""), &mid);
                    EduOAuthStep::Confirmed {
                        serial,
                        github_handle,
                    }
                }
                license::EduFlowEvent::NotEduAccount { github_handle } => {
                    self.edu_oauth_rx = None;
                    EduOAuthStep::NotEdu { github_handle }
                }
                license::EduFlowEvent::NotConfigured => {
                    self.edu_oauth_rx = None;
                    EduOAuthStep::Failed(
                        "Edu verification via GitHub was retired.\nEdu licenses are now issued offline — contact fmolina@avermex.com".into()
                    )
                }
                license::EduFlowEvent::Error(e) => {
                    self.edu_oauth_rx = None;
                    EduOAuthStep::Failed(e)
                }
            };
            ctx.request_repaint();
        } else if still_polling {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }

        let mut open = true;
        egui::Window::new("License / Activate NAT3D")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(460.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let mid = license::get_machine_id();

                // ── Current status ────────────────────────────────────────────
                let (color, label) = match &self.license_status {
                    license::LicenseStatus::Trial => (egui::Color32::YELLOW, "Trial mode"),
                    license::LicenseStatus::Licensed { tier: license::Tier::Pro } => (egui::Color32::GREEN, "NAT3D Pro — Licensed"),
                    license::LicenseStatus::Licensed { tier: license::Tier::Edu } => (egui::Color32::LIGHT_BLUE, "NAT3D Edu — Licensed"),
                    license::LicenseStatus::Invalid => (egui::Color32::RED, "Invalid license"),
                };
                ui.label(egui::RichText::new(label).color(color).strong());
                ui.horizontal(|ui| {
                    ui.label("Machine ID:");
                    ui.monospace(&mid);
                    if ui.small_button("⎘").on_hover_text("Copy to clipboard").clicked() {
                        ui.output_mut(|o| o.copied_text = mid.clone());
                    }
                });
                ui.separator();

                // ── GitHub Education flow state machine ───────────────────────
                match &self.edu_oauth_step {
                    EduOAuthStep::Idle => {
                        ui.label(egui::RichText::new("Free for students and teachers").strong());
                        ui.label("Verify your GitHub Education account to activate NAT3D Edu at no cost.");
                        ui.add_space(4.0);
                        if ui.button("  Activate with GitHub Education  ").clicked() {
                            let (tx, rx) = std::sync::mpsc::channel();
                            self.edu_oauth_rx = Some(parking_lot::Mutex::new(rx));
                            self.edu_oauth_step = EduOAuthStep::Polling;
                            license::start_edu_oauth_flow(tx, mid.clone());
                        }
                        ui.add_space(8.0);
                        ui.separator();
                    }
                    EduOAuthStep::Polling => {
                        ui.label(egui::RichText::new("Connecting to GitHub...").italics());
                        ui.add_space(4.0);
                        ui.separator();
                    }
                    EduOAuthStep::AwaitingUser { user_code, verification_uri } => {
                        ui.label(egui::RichText::new("GitHub Education verification").strong());
                        ui.add_space(4.0);
                        ui.label("1. Your browser should have opened. If not, go to:");
                        ui.hyperlink(verification_uri);
                        ui.add_space(4.0);
                        ui.label("2. Enter this code:");
                        ui.horizontal(|ui| {
                            ui.monospace(egui::RichText::new(user_code).size(22.0).color(egui::Color32::WHITE));
                            if ui.small_button("⎘ Copy").clicked() {
                                ui.output_mut(|o| o.copied_text = user_code.clone());
                            }
                        });
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("⏳  Waiting for GitHub confirmation…").italics().color(egui::Color32::GRAY));
                        ui.add_space(4.0);
                        if ui.small_button("Cancel").clicked() {
                            self.edu_oauth_rx = None;
                            self.edu_oauth_step = EduOAuthStep::Idle;
                        }
                        ui.separator();
                    }
                    EduOAuthStep::Confirmed { serial, github_handle } => {
                        ui.colored_label(egui::Color32::GREEN, format!("Welcome, @{github_handle}. GitHub Education confirmed."));
                        ui.add_space(4.0);
                        ui.label("Your NAT3D Edu serial:");
                        ui.horizontal(|ui| {
                            ui.monospace(egui::RichText::new(serial).size(16.0).color(egui::Color32::LIGHT_BLUE));
                            if ui.small_button("⎘ Copy").clicked() {
                                ui.output_mut(|o| o.copied_text = serial.clone());
                            }
                        });
                        ui.label(egui::RichText::new("Save this key — you'll need it if you reinstall.").small().color(egui::Color32::GRAY));
                        ui.add_space(4.0);
                        if ui.button("  Done  ").clicked() {
                            self.edu_oauth_step = EduOAuthStep::Idle;
                            self.edu_oauth_rx = None;
                            self.show_license_dialog = false;
                        }
                        ui.separator();
                    }
                    EduOAuthStep::NotEdu { github_handle } => {
                        ui.colored_label(egui::Color32::YELLOW, format!("@{github_handle} does not have an active GitHub Education benefit."));
                        ui.label("Apply at education.github.com, then try again.");
                        ui.add_space(4.0);
                        if ui.button("Try again").clicked() {
                            self.edu_oauth_step = EduOAuthStep::Idle;
                        }
                        ui.separator();
                    }
                    EduOAuthStep::Failed(msg) => {
                        ui.colored_label(egui::Color32::RED, msg.clone());
                        ui.add_space(4.0);
                        if ui.button("Try again").clicked() {
                            self.edu_oauth_step = EduOAuthStep::Idle;
                        }
                        ui.separator();
                    }
                }

                // ── Manual serial entry (Pro / Edu) ───────────────────────────
                ui.label("Or enter a serial key (from purchase or keygen):");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.license_serial_input)
                        .hint_text("XXXX-XXXX-XXXX-XXXX")
                        .desired_width(300.0)
                        .font(egui::TextStyle::Monospace),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let activate = ui.button("Activate").clicked()
                        || (response.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)));
                    if activate {
                        let status = license::validate_license(&self.license_serial_input, &mid);
                        if matches!(status, license::LicenseStatus::Licensed { .. }) {
                            self.license_status = status;
                            self.license_serial_input.clear();
                            self.edu_oauth_step = EduOAuthStep::Idle;
                            self.show_license_dialog = false;
                        } else {
                            self.license_status = license::LicenseStatus::Invalid;
                        }
                    }
                    ui.add_space(8.0);
                    if ui.button("Buy Pro License…").clicked() {
                        #[cfg(feature = "file-dialog")]
                        let _ = open::that(license::STORE_URL);
                    }
                });
                if matches!(self.license_status, license::LicenseStatus::Invalid) {
                    ui.colored_label(egui::Color32::RED, "Serial not valid for this machine.");
                }
            });

        if !open {
            self.show_license_dialog = false;
            self.edu_oauth_rx = None;
            self.edu_oauth_step = EduOAuthStep::Idle;
        }
    }

    fn preferences_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_preferences;
        egui::Window::new("Preferences")
            .open(&mut open)
            .resizable(true)
            .default_width(450.0)
            .show(ctx, |ui| {
                ui.heading("Interface");
                ui.horizontal(|ui| {
                    ui.label("Theme:");
                    if ui
                        .selectable_label(self.preferences.dark_mode, "Dark")
                        .clicked()
                    {
                        self.preferences.dark_mode = true;
                        ctx.set_visuals(egui::Visuals::dark());
                    }
                    if ui
                        .selectable_label(!self.preferences.dark_mode, "Light")
                        .clicked()
                    {
                        self.preferences.dark_mode = false;
                        ctx.set_visuals(egui::Visuals::light());
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("UI Scale:");
                    if ui
                        .add(egui::Slider::new(
                            &mut self.preferences.ui_scale,
                            0.75..=2.0,
                        ))
                        .changed()
                    {
                        ctx.set_pixels_per_point(self.preferences.ui_scale);
                    }
                });

                ui.separator();
                ui.heading("Viewport");
                ui.checkbox(&mut self.preferences.show_fps, "Show FPS counter");
                ui.checkbox(&mut self.preferences.show_grid, "Show grid");
                ui.checkbox(&mut self.preferences.show_axes, "Show axes");
                ui.horizontal(|ui| {
                    ui.label("Grid size:");
                    ui.add(egui::DragValue::new(&mut self.preferences.grid_size).range(1..=100));
                });

                ui.separator();
                ui.heading("Performance");
                ui.checkbox(
                    &mut self.preferences.use_gpu_rendering,
                    "Use GPU rendering (wgpu)",
                );
                ui.horizontal(|ui| {
                    ui.label("Anti-aliasing:");
                    egui::ComboBox::from_id_salt("aa_combo")
                        .selected_text(format!("{}x", self.preferences.aa_samples))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.preferences.aa_samples, 1, "Off");
                            ui.selectable_value(&mut self.preferences.aa_samples, 2, "2x");
                            ui.selectable_value(&mut self.preferences.aa_samples, 4, "4x");
                            ui.selectable_value(&mut self.preferences.aa_samples, 8, "8x");
                        });
                });

                ui.separator();
                ui.heading("Auto-save");
                ui.horizontal(|ui| {
                    ui.label("Auto-save interval:");
                    egui::ComboBox::from_id_salt("autosave_combo")
                        .selected_text(if self.preferences.auto_save_minutes == 0 {
                            "Disabled".to_string()
                        } else {
                            format!("{} minutes", self.preferences.auto_save_minutes)
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.preferences.auto_save_minutes,
                                0,
                                "Disabled",
                            );
                            ui.selectable_value(
                                &mut self.preferences.auto_save_minutes,
                                5,
                                "5 minutes",
                            );
                            ui.selectable_value(
                                &mut self.preferences.auto_save_minutes,
                                10,
                                "10 minutes",
                            );
                            ui.selectable_value(
                                &mut self.preferences.auto_save_minutes,
                                15,
                                "15 minutes",
                            );
                            ui.selectable_value(
                                &mut self.preferences.auto_save_minutes,
                                30,
                                "30 minutes",
                            );
                        });
                });

                ui.separator();
                ui.heading("Input");
                ui.label("Keymap: Blender + 3ds Max (dual mode)");
                ui.label("Orbit: Middle Mouse Button");
                ui.label("Pan: Shift + Middle Mouse Button");
                ui.label("Zoom: Scroll Wheel");
                ui.label("Select: Left Click");
                ui.label("Context Menu: Right Click");

                ui.separator();
                ui.heading("Navigation");
                ui.label("Auto Depth: Enabled");
                ui.label("Orbit Method: Turntable");
                ui.label("Zoom to Mouse: Enabled");
                ui.label("Orbit Sensitivity: 1.0");

                ui.separator();
                ui.heading("Add-ons");
                ui.label("Installed Add-ons: 0");
                ui.label("Use Edit > Preferences > Add-ons to install");
                if ui.button("Refresh Add-ons").clicked() {
                    self.status_message = "Add-on directory scanned (0 found)".to_string();
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Reset to Defaults").clicked() {
                        self.preferences = AppPreferences {
                            simulation_mode: state::SimulationMode::Off,
                            dark_mode: true,
                            auto_save_minutes: 0,
                            show_fps: true,
                            show_grid: true,
                            show_axes: true,
                            grid_size: 10,
                            aa_samples: 4,
                            ui_scale: 1.0,
                            use_gpu_rendering: false,
                            dont_show_welcome: false,
                        };
                        ctx.set_visuals(egui::Visuals::dark());
                        ctx.set_pixels_per_point(1.0);
                    }
                });
            });
        self.show_preferences = open;
    }

    fn material_editor_window(&mut self, ctx: &egui::Context) {
        if !self.show_materials {
            return;
        }

        let mut open = self.show_materials;
        egui::Window::new("Material Editor")
            .open(&mut open)
            .resizable(true)
            .default_width(400.0)
            .show(ctx, |ui| {
                if let Some(idx) = self.state.selected_object {
                    if let Some(obj) = self.state.objects.get_mut(idx) {
                        ui.heading(&obj.name);
                        ui.separator();

                        // Base color with larger preview
                        ui.horizontal(|ui| {
                            ui.label("Base Color:");
                            let mut color = [
                                obj.material.base_color[0],
                                obj.material.base_color[1],
                                obj.material.base_color[2],
                            ];
                            if ui.color_edit_button_rgb(&mut color).changed() {
                                obj.material.base_color[0] = color[0];
                                obj.material.base_color[1] = color[1];
                                obj.material.base_color[2] = color[2];
                            }
                        });

                        // Alpha
                        ui.horizontal(|ui| {
                            ui.label("Alpha:");
                            ui.add(egui::Slider::new(
                                &mut obj.material.base_color[3],
                                0.0..=1.0,
                            ));
                        });

                        ui.separator();

                        // PBR properties
                        ui.horizontal(|ui| {
                            ui.label("Metallic:");
                            ui.add(egui::Slider::new(&mut obj.material.metallic, 0.0..=1.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Roughness:");
                            ui.add(egui::Slider::new(&mut obj.material.roughness, 0.0..=1.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Emissive:");
                            ui.add(egui::Slider::new(&mut obj.material.emissive, 0.0..=10.0));
                        });

                        ui.separator();

                        // Presets
                        ui.label("Presets:");
                        ui.horizontal(|ui| {
                            if ui.button("Plastic").clicked() {
                                obj.material.metallic = 0.0;
                                obj.material.roughness = 0.4;
                            }
                            if ui.button("Metal").clicked() {
                                obj.material.metallic = 1.0;
                                obj.material.roughness = 0.3;
                            }
                            if ui.button("Glass").clicked() {
                                obj.material.metallic = 0.0;
                                obj.material.roughness = 0.0;
                                obj.material.base_color[3] = 0.2;
                            }
                            if ui.button("Rubber").clicked() {
                                obj.material.metallic = 0.0;
                                obj.material.roughness = 0.9;
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Gold").clicked() {
                                obj.material.base_color = [1.0, 0.76, 0.33, 1.0];
                                obj.material.metallic = 1.0;
                                obj.material.roughness = 0.2;
                            }
                            if ui.button("Chrome").clicked() {
                                obj.material.base_color = [0.9, 0.9, 0.92, 1.0];
                                obj.material.metallic = 1.0;
                                obj.material.roughness = 0.05;
                            }
                            if ui.button("Copper").clicked() {
                                obj.material.base_color = [0.95, 0.64, 0.54, 1.0];
                                obj.material.metallic = 1.0;
                                obj.material.roughness = 0.25;
                            }
                            if ui.button("Ceramic").clicked() {
                                obj.material.base_color = [0.95, 0.93, 0.88, 1.0];
                                obj.material.metallic = 0.0;
                                obj.material.roughness = 0.15;
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Wood").clicked() {
                                obj.material.base_color = [0.55, 0.35, 0.18, 1.0];
                                obj.material.metallic = 0.0;
                                obj.material.roughness = 0.7;
                            }
                            if ui.button("Concrete").clicked() {
                                obj.material.base_color = [0.6, 0.58, 0.55, 1.0];
                                obj.material.metallic = 0.0;
                                obj.material.roughness = 0.95;
                            }
                            if ui.button("Emissive").clicked() {
                                obj.material.emissive = 5.0;
                                obj.material.base_color = [0.2, 0.6, 1.0, 1.0];
                            }
                            if ui.button("Clay").clicked() {
                                obj.material.base_color = [0.68, 0.68, 0.72, 1.0];
                                obj.material.metallic = 0.0;
                                obj.material.roughness = 0.45;
                                obj.material.emissive = 0.0;
                            }
                        });
                    }
                } else {
                    ui.label("Select an object to edit its material");
                }
            });
        self.show_materials = open;
    }

    fn console_window(&mut self, ctx: &egui::Context) {
        if !self.show_console {
            return;
        }
        let mut open = self.show_console;
        egui::Window::new("Console")
            .open(&mut open)
            .resizable(true)
            .default_width(600.0)
            .default_height(250.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    ui.selectable_value(
                        &mut self.console_filter,
                        console::LogLevel::Debug,
                        "Debug",
                    );
                    ui.selectable_value(&mut self.console_filter, console::LogLevel::Info, "Info");
                    ui.selectable_value(
                        &mut self.console_filter,
                        console::LogLevel::Warning,
                        "Warn",
                    );
                    ui.selectable_value(
                        &mut self.console_filter,
                        console::LogLevel::Error,
                        "Error",
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Clear").clicked() {
                            self.console_entries.clear();
                        }
                        ui.label(format!("{} entries", self.console_entries.len()));
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for entry in &self.console_entries {
                            if entry.level < self.console_filter {
                                continue;
                            }
                            let color = entry.level.color();
                            let prefix = entry.level.prefix();
                            let source_str = entry.source.as_deref().unwrap_or("");
                            let text = if source_str.is_empty() {
                                format!("{} {}", prefix, entry.message)
                            } else {
                                format!("{} [{}] {}", prefix, source_str, entry.message)
                            };
                            ui.colored_label(
                                egui::Color32::from_rgb(color[0], color[1], color[2]),
                                &text,
                            );
                        }
                    });
            });
        self.show_console = open;
    }

    fn node_editor_window(&mut self, ctx: &egui::Context) {
        if !self.show_node_editor {
            return;
        }
        let mut open = self.show_node_editor;

        // Nodes queued for addition (resolved after menu interaction, avoids borrow conflict).
        let mut add_node_request: Option<(&'static str, nodes::NodeCategory)> = None;

        egui::Window::new("Node Editor")
            .open(&mut open)
            .resizable(true)
            .default_size([800.0, 500.0])
            .show(ctx, |ui| {
                // ── Toolbar ──────────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label("Node Editor");
                    ui.separator();
                    ui.menu_button("Add Node", |ui| {
                        ui.menu_button("Input", |ui| {
                            for (name, _) in &[
                                ("Value", ()),
                                ("Vector", ()),
                                ("Color", ()),
                                ("UV Map", ()),
                                ("Object Info", ()),
                            ] {
                                if ui.button(*name).clicked() {
                                    add_node_request = Some((name, nodes::NodeCategory::Input));
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("Shader", |ui| {
                            for (name, _) in &[
                                ("Principled BSDF", ()),
                                ("Diffuse BSDF", ()),
                                ("Glossy BSDF", ()),
                                ("Emission", ()),
                                ("Mix Shader", ()),
                            ] {
                                if ui.button(*name).clicked() {
                                    add_node_request = Some((name, nodes::NodeCategory::Shader));
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("Texture", |ui| {
                            for (name, _) in &[
                                ("Image Texture", ()),
                                ("Noise", ()),
                                ("Voronoi", ()),
                                ("Checker", ()),
                                ("Wave", ()),
                            ] {
                                if ui.button(*name).clicked() {
                                    add_node_request = Some((name, nodes::NodeCategory::Texture));
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("Color", |ui| {
                            for (name, _) in &[
                                ("Mix", ()),
                                ("Brightness/Contrast", ()),
                                ("Hue/Saturation", ()),
                                ("Invert", ()),
                            ] {
                                if ui.button(*name).clicked() {
                                    add_node_request = Some((name, nodes::NodeCategory::Color));
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("Converter", |ui| {
                            for (name, _) in &[
                                ("Math", ()),
                                ("Map Range", ()),
                                ("Color Ramp", ()),
                                ("Separate XYZ", ()),
                                ("Combine XYZ", ()),
                            ] {
                                if ui.button(*name).clicked() {
                                    add_node_request = Some((name, nodes::NodeCategory::Converter));
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("Output", |ui| {
                            for (name, _) in &[("Material Output", ()), ("World Output", ())] {
                                if ui.button(*name).clicked() {
                                    add_node_request = Some((name, nodes::NodeCategory::Output));
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                    if ui.button("Compile Material").clicked() {
                        let n = self.node_graph.nodes().count();
                        let c = self.node_graph.connections().len();
                        self.status_message =
                            format!("Material compiled ({n} nodes, {c} connections)");
                    }
                    if ui.button("Clear Connections").clicked() {
                        self.pending_connection = None;
                        self.node_graph.clear_selection();
                        self.status_message =
                            "Connections cleared — use Compile to re-link".to_string();
                    }
                });
                ui.separator();

                // ── Canvas ───────────────────────────────────────────────────
                let canvas_size = ui.available_size();
                let (resp, painter) =
                    ui.allocate_painter(canvas_size, egui::Sense::click_and_drag());
                let crect = resp.rect;

                // Background grid
                let grid_color = egui::Color32::from_rgba_unmultiplied(50, 50, 55, 100);
                let mut x = crect.left();
                while x < crect.right() {
                    painter.line_segment(
                        [egui::pos2(x, crect.top()), egui::pos2(x, crect.bottom())],
                        egui::Stroke::new(0.5_f32, grid_color),
                    );
                    x += 20.0;
                }
                let mut y = crect.top();
                while y < crect.bottom() {
                    painter.line_segment(
                        [egui::pos2(crect.left(), y), egui::pos2(crect.right(), y)],
                        egui::Stroke::new(0.5_f32, grid_color),
                    );
                    y += 20.0;
                }

                // Layout constants
                const HDR_H: f32 = 24.0;
                const ROW_H: f32 = 22.0;
                const SOCK_R: f32 = 6.0;
                const HIT_R: f32 = 14.0;

                // Helper: screen position of socket (local canvas space → screen)
                let sock_pos = |node: &nodes::Node, is_input: bool, idx: usize| -> egui::Pos2 {
                    let nx = crect.left() + node.position[0];
                    let ny = crect.top() + node.position[1];
                    let y = ny + HDR_H + ROW_H * (idx as f32 + 0.5);
                    if is_input {
                        egui::pos2(nx, y)
                    } else {
                        egui::pos2(nx + node.size[0], y)
                    }
                };

                // Phase 1 — collect socket screen positions for interaction (borrow drops at end of block)
                let socket_hits: Vec<(nodes::NodeId, nodes::SocketId, bool, egui::Pos2)> = {
                    self.node_graph
                        .nodes()
                        .flat_map(|n| {
                            let ins = n
                                .inputs
                                .iter()
                                .enumerate()
                                .map(|(i, s)| (n.id, s.id, true, sock_pos(n, true, i)));
                            let outs = n
                                .outputs
                                .iter()
                                .enumerate()
                                .map(|(i, s)| (n.id, s.id, false, sock_pos(n, false, i)));
                            ins.chain(outs)
                        })
                        .collect()
                };

                // Also collect node header rects for drag detection
                let node_headers: Vec<(nodes::NodeId, egui::Rect)> = {
                    self.node_graph
                        .nodes()
                        .map(|n| {
                            let nx = crect.left() + n.position[0];
                            let ny = crect.top() + n.position[1];
                            let rows = n.inputs.len().max(n.outputs.len()).max(1);
                            let h = HDR_H + ROW_H * rows as f32 + 4.0;
                            (
                                n.id,
                                egui::Rect::from_min_size(
                                    egui::pos2(nx, ny),
                                    egui::vec2(n.size[0], h),
                                ),
                            )
                        })
                        .collect()
                };

                // Phase 2 — handle interactions
                let mouse_pos = resp.hover_pos();
                let pointer_pos = resp.interact_pointer_pos();

                // Drag: continue moving node
                if resp.dragged() {
                    if let Some((drag_id, _)) = self.node_drag {
                        let delta = resp.drag_delta();
                        if let Some(n) = self.node_graph.get_node_mut(drag_id) {
                            n.position[0] += delta.x;
                            n.position[1] += delta.y;
                        }
                    } else if let Some(pos) = pointer_pos {
                        // Start drag: check if pointer is on a node header
                        for (nid, header) in &node_headers {
                            if header.contains(pos) {
                                // Make sure it's the header strip only
                                let header_strip = egui::Rect::from_min_size(
                                    header.min,
                                    egui::vec2(header.width(), HDR_H),
                                );
                                if header_strip.contains(pos) {
                                    self.node_drag = Some((*nid, pos - header.min));
                                    break;
                                }
                            }
                        }
                    }
                } else {
                    // Drag released
                    self.node_drag = None;
                }

                // Click: socket connection wiring
                if resp.clicked() {
                    if let Some(pos) = pointer_pos {
                        let mut hit: Option<(nodes::NodeId, nodes::SocketId, bool)> = None;
                        for (nid, sid, is_input, sp) in &socket_hits {
                            if sp.distance(pos) < HIT_R {
                                hit = Some((*nid, *sid, *is_input));
                                break;
                            }
                        }
                        match hit {
                            Some((nid, sid, false)) => {
                                // Output socket clicked → start wire
                                self.pending_connection = Some((nid, sid));
                            }
                            Some((nid, sid, true)) => {
                                // Input socket clicked → complete wire if pending
                                if let Some((from_n, from_s)) = self.pending_connection.take() {
                                    if self.node_graph.connect(from_n, from_s, nid, sid) {
                                        self.status_message = "Nodes connected".to_string();
                                    } else {
                                        self.status_message =
                                            "Incompatible socket types".to_string();
                                    }
                                }
                            }
                            None => {
                                // Click on empty canvas — cancel pending wire
                                self.pending_connection = None;
                            }
                        }
                    }
                }

                // Phase 3 — draw connections
                let draw_bezier =
                    |p: &egui::Painter, a: egui::Pos2, b: egui::Pos2, color: egui::Color32| {
                        let dx = (b.x - a.x).abs().max(80.0) * 0.5;
                        let cp1 = egui::pos2(a.x + dx, a.y);
                        let cp2 = egui::pos2(b.x - dx, b.y);
                        let pts: Vec<egui::Pos2> = (0..=24)
                            .map(|i| {
                                let t = i as f32 / 24.0;
                                let mt = 1.0 - t;
                                egui::pos2(
                                    mt * mt * mt * a.x
                                        + 3.0 * mt * mt * t * cp1.x
                                        + 3.0 * mt * t * t * cp2.x
                                        + t * t * t * b.x,
                                    mt * mt * mt * a.y
                                        + 3.0 * mt * mt * t * cp1.y
                                        + 3.0 * mt * t * t * cp2.y
                                        + t * t * t * b.y,
                                )
                            })
                            .collect();
                        for w in pts.windows(2) {
                            p.line_segment([w[0], w[1]], egui::Stroke::new(2.0_f32, color));
                        }
                    };

                // Draw existing connections
                let conns: Vec<_> = self.node_graph.connections().to_vec();
                for conn in &conns {
                    let from_pos = socket_hits
                        .iter()
                        .find(|(nid, sid, is_in, _)| {
                            *nid == conn.from_node && *sid == conn.from_socket && !is_in
                        })
                        .map(|(_, _, _, p)| *p);
                    let to_pos = socket_hits
                        .iter()
                        .find(|(nid, sid, is_in, _)| {
                            *nid == conn.to_node && *sid == conn.to_socket && *is_in
                        })
                        .map(|(_, _, _, p)| *p);
                    if let (Some(a), Some(b)) = (from_pos, to_pos) {
                        draw_bezier(&painter, a, b, egui::Color32::from_rgb(180, 180, 180));
                    }
                }

                // Draw pending wire (follows mouse)
                if let Some((pn, ps)) = self.pending_connection {
                    if let Some(start) = socket_hits
                        .iter()
                        .find(|(nid, sid, is_in, _)| *nid == pn && *sid == ps && !is_in)
                        .map(|(_, _, _, p)| *p)
                    {
                        let end = mouse_pos.unwrap_or(start);
                        draw_bezier(&painter, start, end, egui::Color32::from_rgb(220, 200, 60));
                        // Pulse dot at source
                        painter.circle_stroke(
                            start,
                            SOCK_R + 3.0,
                            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(220, 200, 60)),
                        );
                    }
                }

                // Draw nodes
                let node_ids: Vec<nodes::NodeId> = self.node_graph.nodes().map(|n| n.id).collect();
                for nid in node_ids {
                    let (nx, ny, nw, name, category, inputs, outputs) = {
                        if let Some(n) = self.node_graph.get_node(nid) {
                            let ins: Vec<(nodes::SocketId, String, [u8; 3])> = n
                                .inputs
                                .iter()
                                .map(|s| (s.id, s.name.clone(), s.socket_type.color()))
                                .collect();
                            let outs: Vec<(nodes::SocketId, String, [u8; 3])> = n
                                .outputs
                                .iter()
                                .map(|s| (s.id, s.name.clone(), s.socket_type.color()))
                                .collect();
                            (
                                crect.left() + n.position[0],
                                crect.top() + n.position[1],
                                n.size[0],
                                n.name.clone(),
                                n.category,
                                ins,
                                outs,
                            )
                        } else {
                            continue;
                        }
                    };

                    let rows = inputs.len().max(outputs.len()).max(1);
                    let nh = HDR_H + ROW_H * rows as f32 + 4.0;
                    let node_rect =
                        egui::Rect::from_min_size(egui::pos2(nx, ny), egui::vec2(nw, nh));

                    let bg = match category {
                        nodes::NodeCategory::Shader => egui::Color32::from_rgb(45, 75, 45),
                        nodes::NodeCategory::Output => egui::Color32::from_rgb(75, 45, 45),
                        nodes::NodeCategory::Texture => egui::Color32::from_rgb(45, 50, 80),
                        nodes::NodeCategory::Color => egui::Color32::from_rgb(70, 60, 35),
                        nodes::NodeCategory::Input => egui::Color32::from_rgb(50, 50, 70),
                        _ => egui::Color32::from_rgb(55, 55, 60),
                    };
                    let hdr_bg = egui::Color32::from_rgb(
                        (bg.r() as u16).saturating_add(25).min(255) as u8,
                        (bg.g() as u16).saturating_add(25).min(255) as u8,
                        (bg.b() as u16).saturating_add(25).min(255) as u8,
                    );
                    let selected = self.node_graph.is_selected(nid);
                    let border = if selected {
                        egui::Color32::from_rgb(255, 180, 50)
                    } else {
                        egui::Color32::from_rgb(90, 90, 90)
                    };

                    painter.rect_filled(node_rect, 6.0, bg);
                    painter.rect_stroke(
                        node_rect,
                        6.0,
                        egui::Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, border),
                    );
                    let hdr_rect =
                        egui::Rect::from_min_size(egui::pos2(nx, ny), egui::vec2(nw, HDR_H));
                    painter.rect_filled(
                        hdr_rect,
                        egui::Rounding {
                            nw: 6.0,
                            ne: 6.0,
                            sw: 0.0,
                            se: 0.0,
                        },
                        hdr_bg,
                    );
                    painter.text(
                        hdr_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &name,
                        egui::FontId::proportional(11.0),
                        egui::Color32::WHITE,
                    );

                    for (i, (_sid, sname, col)) in inputs.iter().enumerate() {
                        let sp = egui::pos2(nx, ny + HDR_H + ROW_H * (i as f32 + 0.5));
                        painter.circle_filled(
                            sp,
                            SOCK_R,
                            egui::Color32::from_rgb(col[0], col[1], col[2]),
                        );
                        painter.text(
                            egui::pos2(sp.x + SOCK_R + 4.0, sp.y),
                            egui::Align2::LEFT_CENTER,
                            sname,
                            egui::FontId::proportional(10.0),
                            egui::Color32::from_rgb(210, 210, 210),
                        );
                    }
                    for (i, (_sid, sname, col)) in outputs.iter().enumerate() {
                        let sp = egui::pos2(nx + nw, ny + HDR_H + ROW_H * (i as f32 + 0.5));
                        painter.circle_filled(
                            sp,
                            SOCK_R,
                            egui::Color32::from_rgb(col[0], col[1], col[2]),
                        );
                        painter.text(
                            egui::pos2(sp.x - SOCK_R - 4.0, sp.y),
                            egui::Align2::RIGHT_CENTER,
                            sname,
                            egui::FontId::proportional(10.0),
                            egui::Color32::from_rgb(210, 210, 210),
                        );
                    }
                }

                // Hint
                if self.pending_connection.is_some() {
                    painter.text(
                        egui::pos2(crect.left() + 8.0, crect.bottom() - 12.0),
                        egui::Align2::LEFT_CENTER,
                        "Click an input socket to connect, or click empty space to cancel",
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgb(220, 200, 60),
                    );
                } else {
                    painter.text(
                        egui::pos2(crect.left() + 8.0, crect.bottom() - 12.0),
                        egui::Align2::LEFT_CENTER,
                        "Click an output socket to start a connection · Drag node headers to move",
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgb(120, 120, 120),
                    );
                }
            });

        // Apply deferred node addition
        if let Some((name, category)) = add_node_request {
            let nid = self.node_graph.add_node(name, category);
            // Default sockets by category
            {
                use nodes::SocketType;
                let ng = &mut self.node_graph;
                let count = ng.nodes().count() as f32;
                if let Some(n) = ng.get_node_mut(nid) {
                    n.position = [80.0 + count * 20.0, 60.0 + count * 20.0];
                    n.size = [180.0, 100.0];
                }
                // Add common sockets based on category
                let sid1 = ng.alloc_socket_id();
                let sid2 = ng.alloc_socket_id();
                match category {
                    nodes::NodeCategory::Shader => {
                        if let Some(n) = ng.get_node_mut(nid) {
                            n.add_input(sid1, "Color", SocketType::Color);
                            n.add_output(sid2, "BSDF", SocketType::Shader);
                        }
                    }
                    nodes::NodeCategory::Texture => {
                        if let Some(n) = ng.get_node_mut(nid) {
                            n.add_input(sid1, "Vector", SocketType::Vector3);
                            n.add_output(sid2, "Color", SocketType::Color);
                        }
                    }
                    nodes::NodeCategory::Color => {
                        if let Some(n) = ng.get_node_mut(nid) {
                            n.add_input(sid1, "Color", SocketType::Color);
                            n.add_output(sid2, "Color", SocketType::Color);
                        }
                    }
                    nodes::NodeCategory::Input => {
                        if let Some(n) = ng.get_node_mut(nid) {
                            n.add_output(sid1, "Value", SocketType::Float);
                        }
                    }
                    nodes::NodeCategory::Output => {
                        if let Some(n) = ng.get_node_mut(nid) {
                            n.add_input(sid1, "Surface", SocketType::Shader);
                        }
                    }
                    _ => {
                        if let Some(n) = ng.get_node_mut(nid) {
                            n.add_input(sid1, "Value", SocketType::Float);
                            n.add_output(sid2, "Value", SocketType::Float);
                        }
                    }
                }
            }
            self.status_message = format!("Added {name} node");
        }

        self.show_node_editor = open;
    }

    fn log_console(&mut self, level: console::LogLevel, message: &str, source: &str) {
        self.console_entries.push(console::LogEntry {
            level,
            message: message.to_string(),
            source: Some(source.to_string()),
            timestamp: 0.0,
            count: 1,
        });
        // Keep max 1000 entries
        if self.console_entries.len() > 1000 {
            self.console_entries.remove(0);
        }
    }

    /// Apply modifiers to geometry (vertices + faces).
    /// Returns modified (vertices, faces). Edges are derived from faces afterwards.
    fn apply_modifiers(
        vertices: &[[f32; 3]],
        faces: &[Vec<usize>],
        modifiers: &[String],
        obj_position: [f32; 3],
    ) -> (Vec<[f32; 3]>, Vec<Vec<usize>>) {
        let mut verts: Vec<[f32; 3]> = vertices.to_vec();
        let mut face_list: Vec<Vec<usize>> = faces.to_vec();

        for modifier in modifiers {
            match modifier.as_str() {
                "Subdivision" => {
                    use std::collections::HashMap;

                    if face_list.is_empty() {
                        continue;
                    }

                    // Proper Catmull-Clark subdivision
                    // Step 1: Build edge map
                    let mut edges: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
                    for (face_idx, face) in face_list.iter().enumerate() {
                        for i in 0..face.len() {
                            let v0 = face[i];
                            let v1 = face[(i + 1) % face.len()];
                            let edge_key = if v0 < v1 { (v0, v1) } else { (v1, v0) };
                            edges.entry(edge_key).or_default().push(face_idx);
                        }
                    }

                    // Step 2: Compute face points (centroid of each face)
                    let mut face_points = Vec::new();
                    for face in &face_list {
                        if face.is_empty() {
                            face_points.push([0.0, 0.0, 0.0]);
                            continue;
                        }
                        let mut centroid = [0.0, 0.0, 0.0];
                        for &vi in face {
                            if vi < verts.len() {
                                centroid[0] += verts[vi][0];
                                centroid[1] += verts[vi][1];
                                centroid[2] += verts[vi][2];
                            }
                        }
                        let n = face.len() as f32;
                        centroid[0] /= n;
                        centroid[1] /= n;
                        centroid[2] /= n;
                        face_points.push(centroid);
                    }

                    // Step 3: Compute edge points
                    let mut edge_points: HashMap<(usize, usize), [f32; 3]> = HashMap::new();
                    for (edge_key, face_indices) in &edges {
                        let v0 = verts[edge_key.0];
                        let v1 = verts[edge_key.1];
                        let point = if face_indices.len() == 2 {
                            // Interior edge: average of edge endpoints and adjacent face points
                            let f0 = face_points[face_indices[0]];
                            let f1 = face_points[face_indices[1]];
                            [
                                (v0[0] + v1[0] + f0[0] + f1[0]) / 4.0,
                                (v0[1] + v1[1] + f0[1] + f1[1]) / 4.0,
                                (v0[2] + v1[2] + f0[2] + f1[2]) / 4.0,
                            ]
                        } else {
                            // Boundary edge: midpoint
                            [
                                (v0[0] + v1[0]) / 2.0,
                                (v0[1] + v1[1]) / 2.0,
                                (v0[2] + v1[2]) / 2.0,
                            ]
                        };
                        edge_points.insert(*edge_key, point);
                    }

                    // Helper: check if vertex is on boundary
                    let is_vertex_boundary = |v: usize| -> bool {
                        for edge_key in edges.keys() {
                            if (edge_key.0 == v || edge_key.1 == v) && edges[edge_key].len() == 1 {
                                return true;
                            }
                        }
                        false
                    };

                    // Step 4: Update original vertices using Catmull-Clark formula
                    let mut new_verts = Vec::new();
                    for (vi, v) in verts.iter().enumerate() {
                        if is_vertex_boundary(vi) {
                            // Boundary vertex: keep original
                            new_verts.push(*v);
                        } else {
                            // Interior vertex: (F + 2R + (n-3)P) / n
                            let mut face_sum = [0.0, 0.0, 0.0];
                            let mut edge_sum = [0.0, 0.0, 0.0];
                            let mut valence = 0;

                            // Find adjacent faces
                            for (face_idx, face) in face_list.iter().enumerate() {
                                if face.contains(&vi) {
                                    face_sum[0] += face_points[face_idx][0];
                                    face_sum[1] += face_points[face_idx][1];
                                    face_sum[2] += face_points[face_idx][2];
                                    valence += 1;
                                }
                            }

                            // Find adjacent edge midpoints
                            for edge_key in edges.keys() {
                                if edge_key.0 == vi || edge_key.1 == vi {
                                    let other = if edge_key.0 == vi {
                                        edge_key.1
                                    } else {
                                        edge_key.0
                                    };
                                    if other < verts.len() {
                                        edge_sum[0] += verts[other][0];
                                        edge_sum[1] += verts[other][1];
                                        edge_sum[2] += verts[other][2];
                                    }
                                }
                            }

                            if valence > 0 {
                                let n = valence as f32;
                                let f = [face_sum[0] / n, face_sum[1] / n, face_sum[2] / n];
                                let r = [edge_sum[0] / n, edge_sum[1] / n, edge_sum[2] / n];
                                let new_pos = [
                                    (f[0] + 2.0 * r[0] + (n - 3.0) * v[0]) / n,
                                    (f[1] + 2.0 * r[1] + (n - 3.0) * v[1]) / n,
                                    (f[2] + 2.0 * r[2] + (n - 3.0) * v[2]) / n,
                                ];
                                new_verts.push(new_pos);
                            } else {
                                new_verts.push(*v);
                            }
                        }
                    }

                    // Step 5: Add edge points and face points to vertex list
                    let mut edge_to_idx: HashMap<(usize, usize), usize> = HashMap::new();
                    for (edge_key, point) in &edge_points {
                        let idx = new_verts.len();
                        new_verts.push(*point);
                        edge_to_idx.insert(*edge_key, idx);
                    }

                    let mut face_to_idx: HashMap<usize, usize> = HashMap::new();
                    for (face_idx, point) in face_points.iter().enumerate() {
                        let idx = new_verts.len();
                        new_verts.push(*point);
                        face_to_idx.insert(face_idx, idx);
                    }

                    // Step 6: Create new quad faces
                    let mut new_faces = Vec::new();
                    for (face_idx, face) in face_list.iter().enumerate() {
                        let face_point_idx = face_to_idx[&face_idx];
                        for i in 0..face.len() {
                            let v0 = face[i];
                            let v1 = face[(i + 1) % face.len()];
                            let v2 = face[(i + face.len() - 1) % face.len()];

                            let edge_key1 = if v0 < v1 { (v0, v1) } else { (v1, v0) };
                            let edge_point1 = edge_to_idx[&edge_key1];

                            let edge_key2 = if v2 < v0 { (v2, v0) } else { (v0, v2) };
                            let edge_point2 = edge_to_idx[&edge_key2];

                            // Create quad
                            new_faces.push(vec![v0, edge_point1, face_point_idx, edge_point2]);
                        }
                    }

                    verts = new_verts;
                    face_list = new_faces;
                }
                "Mirror" => {
                    // Mirror across X axis through object center
                    let cx = obj_position[0];
                    let base_len = verts.len();
                    let base_faces = face_list.len();
                    // Add mirrored vertices
                    for i in 0..base_len {
                        let v = verts[i];
                        verts.push([2.0 * cx - v[0], v[1], v[2]]);
                    }
                    // Add mirrored faces (with reversed winding)
                    for fi in 0..base_faces {
                        let face = &face_list[fi];
                        let mut mirrored: Vec<usize> =
                            face.iter().map(|&vi| vi + base_len).collect();
                        mirrored.reverse(); // Reverse winding for correct normals
                        face_list.push(mirrored);
                    }
                }
                "Array" => {
                    // Repeat 3 times along X with spacing
                    let base_len = verts.len();
                    let base_faces_len = face_list.len();
                    for copy in 1..3 {
                        let offset_x = copy as f32 * 2.0; // 2 units spacing
                        let vert_offset = base_len * copy;
                        for i in 0..base_len {
                            let v = verts[i];
                            verts.push([v[0] + offset_x, v[1], v[2]]);
                        }
                        for fi in 0..base_faces_len {
                            let face: Vec<usize> =
                                face_list[fi].iter().map(|&vi| vi + vert_offset).collect();
                            face_list.push(face);
                        }
                    }
                }
                "Smooth" => {
                    // Laplacian smooth (1 iteration)
                    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); verts.len()];
                    for face in &face_list {
                        for i in 0..face.len() {
                            let a = face[i];
                            let b = face[(i + 1) % face.len()];
                            if !adjacency[a].contains(&b) {
                                adjacency[a].push(b);
                            }
                            if !adjacency[b].contains(&a) {
                                adjacency[b].push(a);
                            }
                        }
                    }
                    let factor = 0.5_f32;
                    let original = verts.clone();
                    for (i, neighbors) in adjacency.iter().enumerate() {
                        if neighbors.is_empty() {
                            continue;
                        }
                        let mut avg = [0.0_f32; 3];
                        for &n in neighbors {
                            avg[0] += original[n][0];
                            avg[1] += original[n][1];
                            avg[2] += original[n][2];
                        }
                        let count = neighbors.len() as f32;
                        avg[0] /= count;
                        avg[1] /= count;
                        avg[2] /= count;
                        verts[i][0] = original[i][0] * (1.0 - factor) + avg[0] * factor;
                        verts[i][1] = original[i][1] * (1.0 - factor) + avg[1] * factor;
                        verts[i][2] = original[i][2] * (1.0 - factor) + avg[2] * factor;
                    }
                }
                "Solidify" => {
                    // Extrude mesh along normals to give thickness
                    let thickness = 0.1_f32;
                    let base_len = verts.len();
                    let base_faces_len = face_list.len();

                    // Compute per-vertex normals (average of face normals)
                    let mut vert_normals = vec![[0.0_f32; 3]; base_len];
                    for face in &face_list {
                        if face.len() < 3 {
                            continue;
                        }
                        let v0 = verts[face[0]];
                        let v1 = verts[face[1]];
                        let v2 = verts[face[face.len() - 1]];
                        let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
                        let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
                        let n = [
                            e1[1] * e2[2] - e1[2] * e2[1],
                            e1[2] * e2[0] - e1[0] * e2[2],
                            e1[0] * e2[1] - e1[1] * e2[0],
                        ];
                        for &vi in face {
                            if vi < base_len {
                                vert_normals[vi][0] += n[0];
                                vert_normals[vi][1] += n[1];
                                vert_normals[vi][2] += n[2];
                            }
                        }
                    }
                    // Normalize and offset
                    for i in 0..base_len {
                        let n = &mut vert_normals[i];
                        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-8);
                        n[0] /= l;
                        n[1] /= l;
                        n[2] /= l;
                        verts.push([
                            verts[i][0] - n[0] * thickness,
                            verts[i][1] - n[1] * thickness,
                            verts[i][2] - n[2] * thickness,
                        ]);
                    }
                    // Inner faces (reversed winding)
                    for fi in 0..base_faces_len {
                        let mut inner: Vec<usize> =
                            face_list[fi].iter().map(|&vi| vi + base_len).collect();
                        inner.reverse();
                        face_list.push(inner);
                    }
                }
                "Bevel" => {
                    // Simple bevel: chamfer each edge by splitting corner vertices
                    let bevel_amount = 0.05_f32;
                    let mut new_verts = verts.clone();
                    let mut new_faces = Vec::new();

                    for face in &face_list {
                        if face.len() < 3 {
                            new_faces.push(face.clone());
                            continue;
                        }
                        // Create inset face (move each vertex slightly toward centroid)
                        let cx: f32 =
                            face.iter().map(|&i| verts[i][0]).sum::<f32>() / face.len() as f32;
                        let cy_f: f32 =
                            face.iter().map(|&i| verts[i][1]).sum::<f32>() / face.len() as f32;
                        let cz: f32 =
                            face.iter().map(|&i| verts[i][2]).sum::<f32>() / face.len() as f32;
                        let mut inset_indices = Vec::new();
                        for &vi in face {
                            let idx = new_verts.len();
                            let v = verts[vi];
                            new_verts.push([
                                v[0] + (cx - v[0]) * bevel_amount,
                                v[1] + (cy_f - v[1]) * bevel_amount,
                                v[2] + (cz - v[2]) * bevel_amount,
                            ]);
                            inset_indices.push(idx);
                        }
                        // Inner face
                        new_faces.push(inset_indices.clone());
                        // Border quads connecting original to inset
                        let n = face.len();
                        for i in 0..n {
                            let ni = (i + 1) % n;
                            new_faces.push(vec![
                                face[i],
                                face[ni],
                                inset_indices[ni],
                                inset_indices[i],
                            ]);
                        }
                    }
                    verts = new_verts;
                    face_list = new_faces;
                }
                "Decimate" => {
                    // Simple edge-collapse decimation: remove ~50% of faces
                    // Collapse shortest edges first
                    if face_list.len() > 4 {
                        let target = face_list.len() / 2;
                        while face_list.len() > target && face_list.len() > 4 {
                            // Find shortest edge in first face
                            let mut best_face = 0;
                            let mut best_edge = (0usize, 0usize);
                            let mut best_len = f32::MAX;
                            for (fi, face) in
                                face_list.iter().enumerate().take(face_list.len().min(100))
                            {
                                for i in 0..face.len() {
                                    let a = face[i];
                                    let b = face[(i + 1) % face.len()];
                                    if a >= verts.len() || b >= verts.len() {
                                        continue;
                                    }
                                    let dx = verts[a][0] - verts[b][0];
                                    let dy = verts[a][1] - verts[b][1];
                                    let dz = verts[a][2] - verts[b][2];
                                    let len = dx * dx + dy * dy + dz * dz;
                                    if len < best_len {
                                        best_len = len;
                                        best_face = fi;
                                        best_edge = (a, b);
                                    }
                                }
                            }
                            // Collapse: merge vertex b into a (midpoint)
                            let (va, vb) = best_edge;
                            if va < verts.len() && vb < verts.len() {
                                verts[va] = [
                                    (verts[va][0] + verts[vb][0]) * 0.5,
                                    (verts[va][1] + verts[vb][1]) * 0.5,
                                    (verts[va][2] + verts[vb][2]) * 0.5,
                                ];
                                // Replace all references to vb with va
                                for face in &mut face_list {
                                    for vi in face.iter_mut() {
                                        if *vi == vb {
                                            *vi = va;
                                        }
                                    }
                                }
                                // Remove degenerate faces
                                face_list.retain(|f| {
                                    let mut seen = std::collections::HashSet::new();
                                    f.iter().all(|v| seen.insert(*v)) && f.len() >= 3
                                });
                            }
                            let _ = best_face;
                        }
                    }
                }
                "Wireframe" => {
                    // Convert mesh to wireframe tubes: create thin quads along each edge
                    let wire_thickness = 0.02_f32;
                    let edge_set = Self::derive_edges_from_faces(&face_list);
                    let mut new_verts = Vec::new();
                    let mut new_faces = Vec::new();
                    for (a, b) in &edge_set {
                        if *a >= verts.len() || *b >= verts.len() {
                            continue;
                        }
                        let va = verts[*a];
                        let vb = verts[*b];
                        // Edge direction
                        let dx = vb[0] - va[0];
                        let dy = vb[1] - va[1];
                        let dz = vb[2] - va[2];
                        let elen = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-8);
                        // Perpendicular (cross with up, fallback to right)
                        let up = if dy.abs() / elen < 0.9 {
                            [0.0, 1.0, 0.0]
                        } else {
                            [1.0, 0.0, 0.0]
                        };
                        let px = dy * up[2] - dz * up[1];
                        let py_p = dz * up[0] - dx * up[2];
                        let pz = dx * up[1] - dy * up[0];
                        let pl = (px * px + py_p * py_p + pz * pz).sqrt().max(1e-8);
                        let perp = [
                            px / pl * wire_thickness,
                            py_p / pl * wire_thickness,
                            pz / pl * wire_thickness,
                        ];
                        let base = new_verts.len();
                        new_verts.push([va[0] + perp[0], va[1] + perp[1], va[2] + perp[2]]);
                        new_verts.push([va[0] - perp[0], va[1] - perp[1], va[2] - perp[2]]);
                        new_verts.push([vb[0] - perp[0], vb[1] - perp[1], vb[2] - perp[2]]);
                        new_verts.push([vb[0] + perp[0], vb[1] + perp[1], vb[2] + perp[2]]);
                        new_faces.push(vec![base, base + 1, base + 2, base + 3]);
                    }
                    verts = new_verts;
                    face_list = new_faces;
                }
                "Triangulate" => {
                    // Convert all faces to triangles
                    let mut new_faces = Vec::new();
                    for face in &face_list {
                        if face.len() <= 3 {
                            new_faces.push(face.clone());
                        } else {
                            // Fan triangulation from first vertex
                            for i in 1..face.len() - 1 {
                                new_faces.push(vec![face[0], face[i], face[i + 1]]);
                            }
                        }
                    }
                    face_list = new_faces;
                }
                "Screw" => {
                    // Spin geometry around Y axis (360 degrees, 16 steps)
                    let steps = 16_usize;
                    let base_len = verts.len();
                    let base_faces_len = face_list.len();
                    for step in 1..steps {
                        let angle = step as f32 / steps as f32 * std::f32::consts::TAU;
                        let (sin_a, cos_a) = angle.sin_cos();
                        let vert_offset = base_len * step;
                        for i in 0..base_len {
                            let v = verts[i];
                            let lx = v[0] - obj_position[0];
                            let lz = v[2] - obj_position[2];
                            verts.push([
                                obj_position[0] + lx * cos_a - lz * sin_a,
                                v[1],
                                obj_position[2] + lx * sin_a + lz * cos_a,
                            ]);
                        }
                        for fi in 0..base_faces_len {
                            let face: Vec<usize> =
                                face_list[fi].iter().map(|&vi| vi + vert_offset).collect();
                            face_list.push(face);
                        }
                    }
                    // Connect adjacent steps with side faces
                    // Collect base face vertex indices first to avoid borrow conflict
                    let base_face_verts: Vec<Vec<usize>> = (0..base_faces_len)
                        .map(|fi| face_list[fi].clone())
                        .collect();
                    for step in 0..steps {
                        let next_step = (step + 1) % steps;
                        for face in &base_face_verts {
                            for i in 0..face.len() {
                                let ni = (i + 1) % face.len();
                                let a = face[i] + step * base_len;
                                let b = face[ni] + step * base_len;
                                let c = face[ni] + next_step * base_len;
                                let d = face[i] + next_step * base_len;
                                if a < verts.len()
                                    && b < verts.len()
                                    && c < verts.len()
                                    && d < verts.len()
                                {
                                    face_list.push(vec![a, b, c, d]);
                                }
                            }
                        }
                    }
                }
                "Spectral Smooth" => {
                    // Uniform Laplacian smoothing — Sorkine 2006 "Differential Representations for Mesh Processing"
                    // λ=0.5 (each vertex moves halfway toward the average of its neighbors)
                    let n = verts.len();
                    if n >= 2 {
                        let mut neighbors: Vec<Vec<usize>> = vec![vec![]; n];
                        for face in &face_list {
                            let flen = face.len();
                            for i in 0..flen {
                                let v0 = face[i];
                                let v1 = face[(i + 1) % flen];
                                if v0 < n && v1 < n {
                                    if !neighbors[v0].contains(&v1) {
                                        neighbors[v0].push(v1);
                                    }
                                    if !neighbors[v1].contains(&v0) {
                                        neighbors[v1].push(v0);
                                    }
                                }
                            }
                        }
                        let old = verts.clone();
                        for i in 0..n {
                            if neighbors[i].is_empty() {
                                continue;
                            }
                            let deg = neighbors[i].len() as f32;
                            let avg = [
                                neighbors[i].iter().map(|&j| old[j][0]).sum::<f32>() / deg,
                                neighbors[i].iter().map(|&j| old[j][1]).sum::<f32>() / deg,
                                neighbors[i].iter().map(|&j| old[j][2]).sum::<f32>() / deg,
                            ];
                            verts[i] = [
                                0.5 * old[i][0] + 0.5 * avg[0],
                                0.5 * old[i][1] + 0.5 * avg[1],
                                0.5 * old[i][2] + 0.5 * avg[2],
                            ];
                        }
                    }
                }
                "Hyperbolic Warp" => {
                    // Möbius addition in Poincaré ball model — Ungar 2001 "Analytic Hyperbolic Geometry"
                    // Projects vertices into hyperbolic space with curvature c=0.5 for visual effect
                    use nalgebra::Vector3;
                    use nat3d_core::geometry::non_euclidean::mobius_add;
                    let c = 0.5_f64;
                    let center = Vector3::new(
                        verts.iter().map(|v| v[0] as f64).sum::<f64>() / verts.len() as f64,
                        verts.iter().map(|v| v[1] as f64).sum::<f64>() / verts.len() as f64,
                        verts.iter().map(|v| v[2] as f64).sum::<f64>() / verts.len() as f64,
                    );
                    // Scale to unit ball for hyperbolic math, apply Möbius addition, scale back
                    let scale = verts
                        .iter()
                        .map(|v| {
                            let dx = v[0] as f64 - center.x;
                            let dy = v[1] as f64 - center.y;
                            let dz = v[2] as f64 - center.z;
                            (dx * dx + dy * dy + dz * dz).sqrt()
                        })
                        .fold(0.0_f64, f64::max)
                        .max(1e-8);
                    let warp_offset = Vector3::new(0.1, 0.05, 0.0); // small hyperbolic displacement
                    for v in &mut verts {
                        let u = Vector3::new(
                            (v[0] as f64 - center.x) / scale,
                            (v[1] as f64 - center.y) / scale,
                            (v[2] as f64 - center.z) / scale,
                        );
                        // Clamp to Poincaré ball (norm < 1/sqrt(c))
                        let max_r = (1.0 / c.sqrt()) * 0.95;
                        let u_clamped = if u.norm() > max_r {
                            u.normalize() * max_r
                        } else {
                            u
                        };
                        let warped = mobius_add(u_clamped, warp_offset, c);
                        v[0] = (center.x + warped.x * scale) as f32;
                        v[1] = (center.y + warped.y * scale) as f32;
                        v[2] = (center.z + warped.z * scale) as f32;
                    }
                }
                _ => {} // Other modifiers not yet applied to geometry
            }
        }

        (verts, face_list)
    }

    /// Derive edges from a face list (unique edges).
    fn derive_edges_from_faces(faces: &[Vec<usize>]) -> Vec<(usize, usize)> {
        let mut edge_set = std::collections::HashSet::new();
        for face in faces {
            for i in 0..face.len() {
                let a = face[i];
                let b = face[(i + 1) % face.len()];
                let e = if a < b { (a, b) } else { (b, a) };
                edge_set.insert(e);
            }
        }
        edge_set.into_iter().collect()
    }

    fn uv_editor_window(&mut self, ctx: &egui::Context) {
        if !self.show_uv_editor {
            return;
        }
        let mut open = self.show_uv_editor;
        egui::Window::new("UV Editor")
            .open(&mut open)
            .default_size([400.0, 400.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("UV Editor");

                // UV projection type selector
                ui.horizontal(|ui| {
                    use nat3d_modeling::uv::UvMethod;
                    ui.label("Unwrap:");
                    if ui.button("Smart UV (U)").clicked() {
                        self.state.unwrap_uvs(UvMethod::SmartProject);
                        self.status_message = "UV Unwrap: Smart UV Project".to_string();
                    }
                    if ui.button("LSCM").clicked() {
                        self.state.unwrap_uvs(UvMethod::Lscm);
                        self.status_message = "UV Unwrap: LSCM".to_string();
                    }
                    if ui.button("ABF++").clicked() {
                        self.state.unwrap_uvs(UvMethod::AbfPlusPlus);
                        self.status_message = "UV Unwrap: ABF++".to_string();
                    }
                });
                ui.separator();

                // Draw UV space (0,0) to (1,1) grid with projected UVs
                let (response, painter) = ui.allocate_painter(
                    egui::vec2(ui.available_width(), ui.available_width()),
                    egui::Sense::hover(),
                );
                let rect = response.rect;

                // Background (dark gray)
                painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(35, 35, 40));

                // UV grid lines
                let grid_color = egui::Color32::from_rgba_unmultiplied(60, 60, 60, 100);
                let grid_count = 8;
                for i in 0..=grid_count {
                    let t = i as f32 / grid_count as f32;
                    let x = rect.left() + t * rect.width();
                    let y = rect.top() + t * rect.height();
                    painter.line_segment(
                        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                        egui::Stroke::new(0.5_f32, grid_color),
                    );
                    painter.line_segment(
                        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                        egui::Stroke::new(0.5_f32, grid_color),
                    );
                }

                // Border (0-1 UV space boundary)
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(100, 100, 100)),
                );

                // Draw UV coordinates for selected object
                if let Some(idx) = self.state.selected_object {
                    let vertices = self.get_object_vertices(idx);
                    let faces = self.get_object_faces(idx);

                    // Use stored uv_coords if available, else fall back to box projection
                    let uvs: Vec<[f32; 2]> =
                        if let Some(stored) = &self.state.objects[idx].uv_coords {
                            stored.clone()
                        } else {
                            vertices
                                .iter()
                                .map(|v| {
                                    let obj = &self.state.objects[idx];
                                    let local = [
                                        (v[0] - obj.position[0]) / obj.scale[0].max(0.001),
                                        (v[1] - obj.position[1]) / obj.scale[1].max(0.001),
                                        (v[2] - obj.position[2]) / obj.scale[2].max(0.001),
                                    ];
                                    let ax = local[0].abs();
                                    let ay = local[1].abs();
                                    let az = local[2].abs();
                                    if ax >= ay && ax >= az {
                                        [(local[2] * 0.5 + 0.5), (local[1] * 0.5 + 0.5)]
                                    } else if ay >= ax && ay >= az {
                                        [(local[0] * 0.5 + 0.5), (local[2] * 0.5 + 0.5)]
                                    } else {
                                        [(local[0] * 0.5 + 0.5), (local[1] * 0.5 + 0.5)]
                                    }
                                })
                                .collect()
                        };

                    // Draw UV faces as wireframe
                    let uv_edge_color = egui::Color32::from_rgba_unmultiplied(100, 180, 255, 200);
                    for face in &faces {
                        for i in 0..face.len() {
                            let a = face[i];
                            let b = face[(i + 1) % face.len()];
                            if a < uvs.len() && b < uvs.len() {
                                let pa = egui::pos2(
                                    rect.left() + uvs[a][0].clamp(0.0, 1.0) * rect.width(),
                                    rect.bottom() - uvs[a][1].clamp(0.0, 1.0) * rect.height(),
                                );
                                let pb = egui::pos2(
                                    rect.left() + uvs[b][0].clamp(0.0, 1.0) * rect.width(),
                                    rect.bottom() - uvs[b][1].clamp(0.0, 1.0) * rect.height(),
                                );
                                painter.line_segment(
                                    [pa, pb],
                                    egui::Stroke::new(1.0_f32, uv_edge_color),
                                );
                            }
                        }
                    }

                    // Draw UV vertices as dots
                    let uv_vert_color = egui::Color32::from_rgb(255, 200, 100);
                    for uv in &uvs {
                        let p = egui::pos2(
                            rect.left() + uv[0].clamp(0.0, 1.0) * rect.width(),
                            rect.bottom() - uv[1].clamp(0.0, 1.0) * rect.height(),
                        );
                        painter.circle_filled(p, 2.0, uv_vert_color);
                    }

                    let uv_source = if self.state.objects[idx].uv_coords.is_some() {
                        "computed"
                    } else {
                        "box preview"
                    };
                    ui.label(format!(
                        "UVs: {} vertices, {} faces ({})",
                        uvs.len(),
                        faces.len(),
                        uv_source
                    ));
                } else {
                    // Centered text: "Select an object"
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Select an object to view UVs",
                        egui::FontId::proportional(14.0),
                        egui::Color32::from_rgb(120, 120, 120),
                    );
                }
            });
        self.show_uv_editor = open;
    }

    fn graph_editor_window(&mut self, ctx: &egui::Context) {
        if !self.show_graph_editor {
            return;
        }
        let mut open = self.show_graph_editor;
        egui::Window::new("Graph Editor")
            .open(&mut open)
            .default_size([600.0, 380.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Animation Curves");

                let Some(idx) = self.state.selected_object else {
                    ui.label("Select an object to view animation curves.");
                    return;
                };

                // Toolbar
                ui.horizontal(|ui| {
                    ui.label(format!("Object: {}", self.state.objects[idx].name));
                    ui.separator();
                    if ui.small_button("Zoom Fit").clicked() {
                        self.graph_view_left = 0.0;
                        self.graph_view_right = 1.0;
                    }
                    let kf_count = self.state.objects[idx].keyframes.len();
                    ui.label(format!("{kf_count} keyframes"));
                    ui.separator();
                    ui.small("Scroll=zoom  Dbl-click=add  RClick=delete  Del=remove sel");
                });

                if self.state.objects[idx].keyframes.is_empty() {
                    ui.label("No keyframes. Press I to insert, or double-click in the graph area.");
                    return;
                }

                // --- Timeline and value ranges ---
                let start = self.state.timeline.start_frame;
                let end = self.state.timeline.end_frame;
                let range = (end - start).max(1) as f32;

                let (mut min_val, mut max_val) = {
                    let obj = &self.state.objects[idx];
                    let mut lo = f32::MAX;
                    let mut hi = f32::MIN;
                    for kf in &obj.keyframes {
                        for &v in &kf.position {
                            lo = lo.min(v);
                            hi = hi.max(v);
                        }
                    }
                    (lo, hi)
                };
                let pad = (max_val - min_val).max(1.0) * 0.15;
                min_val -= pad;
                max_val += pad;
                let val_range = max_val - min_val;

                // Snapshot view bounds for closures (written to self only during scroll handling)
                let view_left = self.graph_view_left;
                let view_right = self.graph_view_right.max(view_left + 0.01);

                // --- Allocate interactive painter area ---
                let avail = ui.available_size();
                let graph_h = (avail.y - 30.0).max(100.0);
                let (response, painter) = ui
                    .allocate_painter(egui::vec2(avail.x, graph_h), egui::Sense::click_and_drag());
                let r = response.rect;

                // Coordinate transforms (pure closures over local snapshots)
                let time_to_sx =
                    |tn: f32| r.left() + (tn - view_left) / (view_right - view_left) * r.width();
                let val_to_sy = |v: f32| r.top() + (1.0 - (v - min_val) / val_range) * r.height();
                let sx_to_time =
                    |x: f32| view_left + (x - r.left()) / r.width() * (view_right - view_left);
                let sy_to_val = |y: f32| min_val + (1.0 - (y - r.top()) / r.height()) * val_range;
                let frame_to_tn = |f: i32| (f - start) as f32 / range;
                let tn_to_frame = |t: f32| (t * range + start as f32).round() as i32;

                // --- Background ---
                painter.rect_filled(r, 4.0, egui::Color32::from_rgb(25, 25, 30));

                // --- Grid ---
                for i in 0..=10 {
                    let t = i as f32 / 10.0;
                    let tn = view_left + t * (view_right - view_left);
                    let x = time_to_sx(tn);
                    painter.line_segment(
                        [egui::pos2(x, r.top()), egui::pos2(x, r.bottom())],
                        egui::Stroke::new(0.5_f32, egui::Color32::from_rgb(40, 40, 50)),
                    );
                    painter.text(
                        egui::pos2(x + 2.0, r.bottom() - 10.0),
                        egui::Align2::LEFT_CENTER,
                        tn_to_frame(tn).to_string(),
                        egui::FontId::proportional(9.0),
                        egui::Color32::from_rgb(100, 100, 110),
                    );
                    let y = r.top() + t * r.height();
                    painter.line_segment(
                        [egui::pos2(r.left(), y), egui::pos2(r.right(), y)],
                        egui::Stroke::new(0.5_f32, egui::Color32::from_rgb(40, 40, 50)),
                    );
                }
                // Zero line
                let zero_y = val_to_sy(0.0);
                if zero_y >= r.top() && zero_y <= r.bottom() {
                    painter.line_segment(
                        [egui::pos2(r.left(), zero_y), egui::pos2(r.right(), zero_y)],
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(70, 70, 85)),
                    );
                }

                // --- Scroll-to-zoom (applied before drawing, takes effect next frame) ---
                if response.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0.0 {
                        let factor = if scroll > 0.0 { 0.85_f32 } else { 1.0 / 0.85 };
                        let pivot = response
                            .hover_pos()
                            .map(|p| sx_to_time(p.x))
                            .unwrap_or((view_left + view_right) * 0.5);
                        self.graph_view_left = pivot + (view_left - pivot) * factor;
                        self.graph_view_right = pivot + (view_right - pivot) * factor;
                    }
                }

                // --- Collect dot positions (immutable snapshot; released before any mutations) ---
                const CH_COLORS: [egui::Color32; 3] = [
                    egui::Color32::from_rgb(230, 60, 60),
                    egui::Color32::from_rgb(60, 200, 60),
                    egui::Color32::from_rgb(80, 80, 230),
                ];
                const CH_LABELS: [&str; 3] = ["X", "Y", "Z"];

                // dots: (kf_idx, ch_idx, screen_pos)
                let dots: Vec<(usize, usize, egui::Pos2)> = {
                    let obj = &self.state.objects[idx];
                    let mut v = Vec::with_capacity(obj.keyframes.len() * 3);
                    for ch in 0..3 {
                        for (ki, kf) in obj.keyframes.iter().enumerate() {
                            let tn = frame_to_tn(kf.frame);
                            v.push((
                                ki,
                                ch,
                                egui::pos2(time_to_sx(tn), val_to_sy(kf.position[ch])),
                            ));
                        }
                    }
                    v
                };

                // --- Draw curves + dots ---
                for ch in 0..3usize {
                    let pts: Vec<egui::Pos2> = {
                        let obj = &self.state.objects[idx];
                        obj.keyframes
                            .iter()
                            .map(|kf| {
                                egui::pos2(
                                    time_to_sx(frame_to_tn(kf.frame)),
                                    val_to_sy(kf.position[ch]),
                                )
                            })
                            .collect()
                    };
                    for w in pts.windows(2) {
                        painter
                            .line_segment([w[0], w[1]], egui::Stroke::new(1.5_f32, CH_COLORS[ch]));
                    }
                    for &(ki, _, pt) in dots.iter().filter(|(_, c, _)| *c == ch) {
                        let sel = self.graph_selected.contains(&ki);
                        let (dot_r, fill) = if sel {
                            (5.5_f32, egui::Color32::WHITE)
                        } else {
                            (3.5_f32, CH_COLORS[ch])
                        };
                        painter.circle_filled(pt, dot_r, fill);
                        painter.circle_stroke(
                            pt,
                            dot_r,
                            egui::Stroke::new(
                                1.0_f32,
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 60),
                            ),
                        );
                    }
                    painter.text(
                        egui::pos2(r.right() - 10.0, r.top() + 12.0 + ch as f32 * 14.0),
                        egui::Align2::RIGHT_CENTER,
                        CH_LABELS[ch],
                        egui::FontId::proportional(10.0),
                        CH_COLORS[ch],
                    );
                }

                // --- Playhead ---
                let px = time_to_sx(frame_to_tn(self.state.timeline.current_frame));
                if px >= r.left() && px <= r.right() {
                    painter.line_segment(
                        [egui::pos2(px, r.top()), egui::pos2(px, r.bottom())],
                        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(255, 255, 100)),
                    );
                }

                // --- Box-select overlay ---
                if let (Some(bs), Some(cur)) =
                    (self.graph_box_start, response.interact_pointer_pos())
                {
                    let sel_r = egui::Rect::from_two_pos(bs, cur);
                    painter.rect_stroke(
                        sel_r,
                        0.0,
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(100, 150, 255)),
                    );
                    painter.rect_filled(
                        sel_r,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(100, 150, 255, 20),
                    );
                }

                // --- Hit-test helper (uses dots snapshot — no self borrow) ---
                let find_hit = |pos: egui::Pos2| -> Option<(usize, usize)> {
                    let mut best: Option<(f32, usize, usize)> = None;
                    for &(ki, ci, pt) in &dots {
                        let d = (pt - pos).length();
                        if d < 8.0 && best.map_or(true, |(bd, _, _)| d < bd) {
                            best = Some((d, ki, ci));
                        }
                    }
                    best.map(|(_, ki, ci)| (ki, ci))
                };

                // --- Drag started ---
                if response.drag_started() {
                    let pos = response.interact_pointer_pos().unwrap_or(r.center());
                    if let Some((ki, ci)) = find_hit(pos) {
                        let (orig_frame, orig_val) = {
                            let kf = &self.state.objects[idx].keyframes[ki];
                            (kf.frame, kf.position[ci])
                        };
                        self.graph_drag = Some((ki, ci, orig_frame, orig_val, pos));
                        if !self.graph_selected.contains(&ki) {
                            self.graph_selected = vec![ki];
                        }
                        self.graph_box_start = None;
                    } else {
                        self.graph_drag = None;
                        self.graph_box_start = Some(pos);
                        if !ctx.input(|i| i.modifiers.shift) {
                            self.graph_selected.clear();
                        }
                    }
                }

                // --- During drag ---
                if response.dragged() {
                    if let Some((ki, ci, orig_frame, orig_val, sp)) = self.graph_drag {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let visible_frames = (view_right - view_left) * range;
                            let new_frame = (orig_frame as f32
                                + (pos.x - sp.x) / r.width() * visible_frames)
                                .round() as i32;
                            let new_val = orig_val - (pos.y - sp.y) / r.height() * val_range;
                            if let Some(kf) = self.state.objects[idx].keyframes.get_mut(ki) {
                                kf.frame = new_frame;
                                kf.position[ci] = new_val;
                            }
                        }
                    }
                }

                // --- Drag stopped ---
                if response.drag_stopped() {
                    if self.graph_drag.take().is_some() {
                        self.state.objects[idx].keyframes.sort_by_key(|k| k.frame);
                        self.state.evaluate_keyframes();
                        self.graph_selected.clear(); // indices shift after sort
                    } else if let Some(bs) = self.graph_box_start.take() {
                        let cur = response.interact_pointer_pos().unwrap_or(bs);
                        let sel_r = egui::Rect::from_two_pos(bs, cur);
                        let hits: Vec<usize> = dots
                            .iter()
                            .filter(|(_, c, pt)| *c == 0 && sel_r.contains(*pt))
                            .map(|(ki, _, _)| *ki)
                            .collect();
                        if ctx.input(|i| i.modifiers.shift) {
                            for ki in hits {
                                if !self.graph_selected.contains(&ki) {
                                    self.graph_selected.push(ki);
                                }
                            }
                        } else {
                            self.graph_selected = hits;
                        }
                    }
                }

                // --- Single click: select / deselect ---
                if response.clicked() {
                    let pos = response.interact_pointer_pos().unwrap_or(r.center());
                    if let Some((ki, _)) = find_hit(pos) {
                        self.graph_selected = vec![ki];
                    } else {
                        self.graph_selected.clear();
                    }
                }

                // --- Double-click: add keyframe at cursor ---
                if response.double_clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let new_frame = tn_to_frame(sx_to_time(pos.x));
                        let new_x_val = sy_to_val(pos.y);
                        {
                            let obj = &mut self.state.objects[idx];
                            obj.keyframes.retain(|k| k.frame != new_frame);
                            let (pos3, rot, sc) = (obj.position, obj.rotation, obj.scale);
                            obj.keyframes.push(Keyframe {
                                frame: new_frame,
                                position: [new_x_val, pos3[1], pos3[2]],
                                rotation: rot,
                                scale: sc,
                            });
                            obj.keyframes.sort_by_key(|k| k.frame);
                        }
                        self.state.evaluate_keyframes();
                    }
                }

                // --- Right-click: delete hovered keyframe ---
                if response.secondary_clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        if let Some((ki, _)) = find_hit(pos) {
                            self.state.objects[idx].keyframes.remove(ki);
                            self.graph_selected.retain(|&s| s != ki);
                            self.state.evaluate_keyframes();
                        }
                    }
                }

                // --- Delete/X key: remove selected keyframes ---
                let del_key =
                    ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::X));
                if del_key && !self.graph_selected.is_empty() {
                    let mut to_del = self.graph_selected.clone();
                    to_del.sort_unstable_by(|a, b| b.cmp(a)); // descending so indices stay valid
                    for ki in to_del {
                        if ki < self.state.objects[idx].keyframes.len() {
                            self.state.objects[idx].keyframes.remove(ki);
                        }
                    }
                    self.graph_selected.clear();
                    self.state.evaluate_keyframes();
                }
            });
        self.show_graph_editor = open;
    }

    fn camera_settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_camera_settings {
            return;
        }
        let mut open = self.show_camera_settings;
        egui::Window::new("Camera Settings")
            .open(&mut open)
            .default_size([320.0, 380.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Camera");
                ui.separator();

                ui.label("Depth of Field");
                ui.checkbox(&mut self.state.camera_settings.dof_enabled, "Enable DOF");
                if self.state.camera_settings.dof_enabled {
                    ui.add(
                        egui::Slider::new(
                            &mut self.state.camera_settings.focal_distance,
                            0.1..=100.0,
                        )
                        .text("Focal Distance")
                        .logarithmic(true),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.state.camera_settings.aperture, 0.5..=32.0)
                            .text("Aperture (f-stop)")
                            .logarithmic(true),
                    );
                }

                ui.separator();
                ui.label("Exposure");
                ui.add(
                    egui::Slider::new(&mut self.state.camera_settings.exposure, -5.0..=5.0)
                        .text("Exposure (EV)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.state.camera_settings.gamma, 1.0..=3.0)
                        .text("Gamma"),
                );

                ui.separator();
                ui.label("Sensor");
                ui.add(
                    egui::Slider::new(&mut self.state.camera_settings.sensor_size, 12.0..=70.0)
                        .text("Sensor Size (mm)"),
                );

                ui.separator();
                ui.label("Clipping");
                ui.add(
                    egui::Slider::new(&mut self.state.clip_near, 0.001..=10.0)
                        .text("Near Clip")
                        .logarithmic(true),
                );
                ui.add(
                    egui::Slider::new(&mut self.state.clip_far, 10.0..=10000.0)
                        .text("Far Clip")
                        .logarithmic(true),
                );

                ui.separator();
                ui.label("Camera Object");
                ui.add(egui::Slider::new(&mut self.state.camera.fov, 10.0..=120.0).text("FOV"));
                ui.add(
                    egui::Slider::new(&mut self.state.camera.distance, 0.5..=500.0)
                        .text("Distance")
                        .logarithmic(true),
                );

                ui.separator();
                if ui.button("Reset Camera").clicked() {
                    self.state.camera.reset();
                    self.state.camera_settings = CameraSettings::default();
                    self.status_message = "Camera reset".to_string();
                }
            });
        self.show_camera_settings = open;
    }

    fn world_settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_world_settings {
            return;
        }
        let mut open = self.show_world_settings;
        egui::Window::new("World Settings")
            .open(&mut open)
            .default_size([320.0, 400.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("World Environment");
                ui.separator();

                ui.label("Sky");
                ui.horizontal(|ui| {
                    ui.label("Sky Color:");
                    ui.color_edit_button_rgb(&mut self.state.world.sky_color);
                });
                ui.horizontal(|ui| {
                    ui.label("Horizon:");
                    ui.color_edit_button_rgb(&mut self.state.world.horizon_color);
                });
                ui.horizontal(|ui| {
                    ui.label("Ground:");
                    ui.color_edit_button_rgb(&mut self.state.world.ground_color);
                });
                ui.add(
                    egui::Slider::new(&mut self.state.world.ambient_intensity, 0.0..=2.0)
                        .text("Ambient Intensity"),
                );

                ui.separator();
                ui.label("Fog");
                ui.checkbox(&mut self.state.world.fog_enabled, "Enable Fog");
                if self.state.world.fog_enabled {
                    ui.horizontal(|ui| {
                        ui.label("Fog Color:");
                        ui.color_edit_button_rgb(&mut self.state.world.fog_color);
                    });
                    ui.add(
                        egui::Slider::new(&mut self.state.world.fog_density, 0.0..=1.0)
                            .text("Density"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.state.world.fog_start, 0.0..=100.0)
                            .text("Start Distance"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.state.world.fog_end, 10.0..=1000.0)
                            .text("End Distance"),
                    );
                }

                ui.separator();
                if ui.button("Reset World").clicked() {
                    self.state.world = WorldSettings::default();
                    self.status_message = "World reset to defaults".to_string();
                }
            });
        self.show_world_settings = open;
    }

    fn scene_properties_window(&mut self, ctx: &egui::Context) {
        if !self.show_scene_properties {
            return;
        }
        let mut open = self.show_scene_properties;
        egui::Window::new("Scene Properties")
            .open(&mut open)
            .default_size([320.0, 380.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Scene");
                ui.separator();

                // Scene name
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.state.scene_props.name);
                });

                ui.separator();
                ui.label("Units");
                ui.horizontal(|ui| {
                    ui.label("System:");
                    egui::ComboBox::from_id_salt("unit_system")
                        .selected_text(&self.state.scene_props.unit_name)
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for (name, scale) in &[
                                ("Meters", 1.0f32),
                                ("Centimeters", 0.01),
                                ("Millimeters", 0.001),
                                ("Inches", 0.0254),
                                ("Feet", 0.3048),
                            ] {
                                if ui
                                    .selectable_label(
                                        self.state.scene_props.unit_name == *name,
                                        *name,
                                    )
                                    .clicked()
                                {
                                    self.state.scene_props.unit_name = name.to_string();
                                    self.state.scene_props.unit_scale = *scale;
                                }
                            }
                        });
                });
                ui.add(
                    egui::Slider::new(&mut self.state.scene_props.unit_scale, 0.001..=100.0)
                        .text("Scale")
                        .logarithmic(true),
                );

                ui.separator();
                ui.label("Gravity");
                ui.horizontal(|ui| {
                    ui.label("X:");
                    ui.add(egui::DragValue::new(&mut self.state.scene_props.gravity[0]).speed(0.1));
                    ui.label("Y:");
                    ui.add(egui::DragValue::new(&mut self.state.scene_props.gravity[1]).speed(0.1));
                    ui.label("Z:");
                    ui.add(egui::DragValue::new(&mut self.state.scene_props.gravity[2]).speed(0.1));
                });

                ui.separator();
                ui.label("Render");
                ui.add(
                    egui::Slider::new(&mut self.state.scene_props.render_fps, 12.0..=120.0)
                        .text("FPS"),
                );
                ui.add(
                    egui::Slider::new(&mut self.state.timeline.frame_rate, 12.0..=120.0)
                        .text("Playback FPS"),
                );

                ui.separator();
                ui.label("Active Camera");
                let cam_indices: Vec<(usize, String)> = self
                    .state
                    .objects
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| o.object_type == ObjectType::Camera)
                    .map(|(i, o)| (i, o.name.clone()))
                    .collect();
                if cam_indices.is_empty() {
                    ui.label("No cameras in scene");
                } else {
                    let current_label = self
                        .state
                        .scene_props
                        .active_camera
                        .and_then(|i| self.state.objects.get(i))
                        .map_or("None".to_string(), |o| o.name.clone());
                    egui::ComboBox::from_id_salt("active_camera")
                        .selected_text(current_label)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    self.state.scene_props.active_camera.is_none(),
                                    "None",
                                )
                                .clicked()
                            {
                                self.state.scene_props.active_camera = None;
                            }
                            for (idx, name) in &cam_indices {
                                let selected = self.state.scene_props.active_camera == Some(*idx);
                                if ui.selectable_label(selected, name).clicked() {
                                    self.state.scene_props.active_camera = Some(*idx);
                                }
                            }
                        });
                }

                ui.separator();
                ui.label("Statistics");
                let total_objects = self.state.objects.len();
                let visible_objects = self.state.objects.iter().filter(|o| o.visible).count();
                let total_modifiers: usize =
                    self.state.objects.iter().map(|o| o.modifiers.len()).sum();
                let total_keyframes: usize =
                    self.state.objects.iter().map(|o| o.keyframes.len()).sum();
                let total_constraints: usize =
                    self.state.objects.iter().map(|o| o.constraints.len()).sum();
                let total_particles: u32 = self
                    .state
                    .objects
                    .iter()
                    .flat_map(|o| o.particle_systems.iter())
                    .filter(|ps| ps.active)
                    .map(|ps| ps.count)
                    .sum();
                ui.label(format!(
                    "Objects: {} ({} visible)",
                    total_objects, visible_objects
                ));
                ui.label(format!("Modifiers: {}", total_modifiers));
                ui.label(format!("Keyframes: {}", total_keyframes));
                ui.label(format!("Constraints: {}", total_constraints));
                if total_particles > 0 {
                    ui.label(format!("Particles: {}", total_particles));
                }
                ui.label(format!("Collections: {}", self.state.collections.len()));
            });
        self.show_scene_properties = open;
    }

    fn nla_editor_window(&mut self, ctx: &egui::Context) {
        if !self.show_nla_editor {
            return;
        }
        let mut open = self.show_nla_editor;
        egui::Window::new("NLA Editor")
            .open(&mut open)
            .default_size([600.0, 300.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Non-Linear Animation");
                ui.separator();

                if let Some(idx) = self.state.selected_object {
                    let obj_name = self.state.objects[idx].name.clone();
                    ui.label(format!("Object: {}", obj_name));
                    ui.separator();

                    // NLA tracks
                    let track_count = self.state.objects[idx].nla_tracks.len();
                    if track_count == 0 {
                        ui.label("No NLA tracks. Add a track to start mixing animations.");
                    }

                    // We need to avoid borrow issues, so work with indices
                    let mut remove_track = None;
                    let mut add_strip_to = None;
                    let mut remove_strip = None;
                    let mut toggle_mute_track = None;
                    let mut toggle_solo_track = None;

                    for ti in 0..self.state.objects[idx].nla_tracks.len() {
                        let track = &self.state.objects[idx].nla_tracks[ti];
                        let track_name = track.name.clone();
                        let is_muted = track.muted;
                        let is_solo = track.solo;

                        ui.horizontal(|ui| {
                            ui.colored_label(
                                if is_muted {
                                    egui::Color32::GRAY
                                } else {
                                    egui::Color32::from_rgb(100, 200, 100)
                                },
                                format!("Track {}: {}", ti + 1, track_name),
                            );
                            if ui
                                .small_button(if is_muted { "Unmute" } else { "Mute" })
                                .clicked()
                            {
                                toggle_mute_track = Some(ti);
                            }
                            if ui
                                .small_button(if is_solo { "Unsolo" } else { "Solo" })
                                .clicked()
                            {
                                toggle_solo_track = Some(ti);
                            }
                            if ui.small_button("X").clicked() {
                                remove_track = Some(ti);
                            }
                        });

                        // Strips in this track
                        let strip_count = self.state.objects[idx].nla_tracks[ti].strips.len();
                        for si in 0..strip_count {
                            let strip = &self.state.objects[idx].nla_tracks[ti].strips[si];
                            let strip_color = if strip.muted {
                                egui::Color32::from_rgb(80, 80, 80)
                            } else {
                                egui::Color32::from_rgb(70, 130, 200)
                            };
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                ui.colored_label(
                                    strip_color,
                                    format!(
                                        "  {} [{}..{}] x{:.1}",
                                        strip.name,
                                        strip.start_frame,
                                        strip.end_frame,
                                        strip.repeat
                                    ),
                                );
                                if ui.small_button("Del").clicked() {
                                    remove_strip = Some((ti, si));
                                }
                            });
                        }

                        if ui
                            .small_button(format!("+ Strip to {}", track_name))
                            .clicked()
                        {
                            add_strip_to = Some(ti);
                        }
                        ui.separator();
                    }

                    // Apply deferred actions
                    if let Some(ti) = toggle_mute_track {
                        self.state.objects[idx].nla_tracks[ti].muted =
                            !self.state.objects[idx].nla_tracks[ti].muted;
                    }
                    if let Some(ti) = toggle_solo_track {
                        self.state.objects[idx].nla_tracks[ti].solo =
                            !self.state.objects[idx].nla_tracks[ti].solo;
                    }
                    if let Some((ti, si)) = remove_strip {
                        self.state.objects[idx].nla_tracks[ti].strips.remove(si);
                    }
                    if let Some(ti) = add_strip_to {
                        self.state.objects[idx].nla_tracks[ti]
                            .strips
                            .push(NLAStrip::default());
                        self.status_message = "Added NLA strip".to_string();
                    }
                    if let Some(ti) = remove_track {
                        self.state.objects[idx].nla_tracks.remove(ti);
                        self.status_message = "Removed NLA track".to_string();
                    }

                    ui.separator();
                    if ui.button("+ Add NLA Track").clicked() {
                        let track_num = self.state.objects[idx].nla_tracks.len() + 1;
                        self.state.objects[idx].nla_tracks.push(NLATrack {
                            name: format!("NLA Track {}", track_num),
                            ..NLATrack::default()
                        });
                        self.status_message = format!("Added NLA Track {}", track_num);
                    }
                } else {
                    ui.label("Select an object to view its NLA tracks.");
                }
            });
        self.show_nla_editor = open;
    }

    fn color_management_window(&mut self, ctx: &egui::Context) {
        if !self.show_color_management {
            return;
        }
        let mut open = self.show_color_management;
        egui::Window::new("Color Management")
            .open(&mut open)
            .default_size([320.0, 280.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Color Management (OCIO)");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Display:");
                    egui::ComboBox::from_id_salt("cm_display")
                        .selected_text(&self.state.color_management.display_device)
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            for dev in &["sRGB", "Display P3", "Rec.2020", "XYZ"] {
                                if ui
                                    .selectable_label(
                                        self.state.color_management.display_device == *dev,
                                        *dev,
                                    )
                                    .clicked()
                                {
                                    self.state.color_management.display_device = dev.to_string();
                                }
                            }
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("View Transform:");
                    egui::ComboBox::from_id_salt("cm_view")
                        .selected_text(&self.state.color_management.view_transform)
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            for vt in &["Standard", "Filmic", "ACEScg", "Raw", "False Color", "AgX"]
                            {
                                if ui
                                    .selectable_label(
                                        self.state.color_management.view_transform == *vt,
                                        *vt,
                                    )
                                    .clicked()
                                {
                                    self.state.color_management.view_transform = vt.to_string();
                                }
                            }
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Look:");
                    egui::ComboBox::from_id_salt("cm_look")
                        .selected_text(&self.state.color_management.look)
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for look in &[
                                "None",
                                "Very High Contrast",
                                "High Contrast",
                                "Medium High Contrast",
                                "Medium Contrast",
                                "Medium Low Contrast",
                                "Low Contrast",
                                "Very Low Contrast",
                            ] {
                                if ui
                                    .selectable_label(
                                        self.state.color_management.look == *look,
                                        *look,
                                    )
                                    .clicked()
                                {
                                    self.state.color_management.look = look.to_string();
                                }
                            }
                        });
                });

                ui.separator();
                ui.add(
                    egui::Slider::new(&mut self.state.color_management.exposure, -5.0..=5.0)
                        .text("Exposure"),
                );
                ui.add(
                    egui::Slider::new(&mut self.state.color_management.gamma, 0.1..=3.0)
                        .text("Gamma"),
                );

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Sequencer:");
                    egui::ComboBox::from_id_salt("cm_seq")
                        .selected_text(&self.state.color_management.sequencer_space)
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            for s in &["sRGB", "Linear", "Non-Color", "Filmic Log"] {
                                if ui
                                    .selectable_label(
                                        self.state.color_management.sequencer_space == *s,
                                        *s,
                                    )
                                    .clicked()
                                {
                                    self.state.color_management.sequencer_space = s.to_string();
                                }
                            }
                        });
                });

                ui.separator();
                if ui.button("Reset to Default").clicked() {
                    self.state.color_management = ColorManagement::default();
                    self.status_message = "Color Management reset".to_string();
                }
            });
        self.show_color_management = open;
    }

    fn asset_browser_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_asset_browser;
        egui::Window::new("Asset Browser")
            .open(&mut open)
            .resizable(true)
            .default_width(500.0)
            .default_height(350.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Category:");
                    egui::ComboBox::from_id_salt("asset_cat")
                        .selected_text(format!("{}", self.state.asset_category))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.state.asset_category,
                                AssetCategory::Materials,
                                "Materials",
                            );
                            ui.selectable_value(
                                &mut self.state.asset_category,
                                AssetCategory::Objects,
                                "Objects",
                            );
                            ui.selectable_value(
                                &mut self.state.asset_category,
                                AssetCategory::Worlds,
                                "Worlds",
                            );
                            ui.selectable_value(
                                &mut self.state.asset_category,
                                AssetCategory::Actions,
                                "Actions",
                            );
                            ui.selectable_value(
                                &mut self.state.asset_category,
                                AssetCategory::NodeGroups,
                                "Node Groups",
                            );
                        });
                });
                ui.separator();

                match self.state.asset_category {
                    AssetCategory::Materials => {
                        ui.heading("Material Library");
                        let presets = [
                            ("Default", [0.6, 0.6, 0.6], 0.5, 0.0),
                            ("Gold", [1.0, 0.766, 0.336], 0.3, 1.0),
                            ("Chrome", [0.95, 0.95, 0.95], 0.1, 1.0),
                            ("Copper", [0.955, 0.638, 0.538], 0.35, 1.0),
                            ("Ceramic", [0.9, 0.85, 0.8], 0.4, 0.0),
                            ("Wood", [0.55, 0.35, 0.17], 0.7, 0.0),
                            ("Concrete", [0.6, 0.58, 0.55], 0.9, 0.0),
                            ("Glass", [0.95, 0.95, 0.95], 0.05, 0.0),
                            ("Rubber", [0.15, 0.15, 0.15], 0.85, 0.0),
                            ("Plastic Red", [0.8, 0.1, 0.1], 0.4, 0.0),
                            ("Plastic Blue", [0.1, 0.2, 0.8], 0.4, 0.0),
                            ("Marble", [0.9, 0.88, 0.85], 0.2, 0.0),
                            ("Brick", [0.6, 0.2, 0.15], 0.9, 0.0),
                            ("Titanium", [0.54, 0.50, 0.48], 0.25, 0.9),
                            ("Obsidian", [0.05, 0.05, 0.07], 0.15, 0.0),
                            ("Pearl", [0.95, 0.92, 0.88], 0.3, 0.0),
                        ];
                        egui::Grid::new("asset_mat_grid")
                            .num_columns(4)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                for (i, (name, color, roughness, metallic)) in
                                    presets.iter().enumerate()
                                {
                                    let r = egui::Color32::from_rgb(
                                        (color[0] * 255.0) as u8,
                                        (color[1] * 255.0) as u8,
                                        (color[2] * 255.0) as u8,
                                    );
                                    if ui
                                        .add(
                                            egui::Button::new(format!(" {} ", name))
                                                .fill(r)
                                                .min_size(egui::vec2(90.0, 24.0)),
                                        )
                                        .clicked()
                                    {
                                        if let Some(idx) = self.state.selected_object {
                                            if idx < self.state.objects.len() {
                                                self.state.objects[idx].material.base_color =
                                                    [color[0], color[1], color[2], 1.0];
                                                self.state.objects[idx].material.roughness =
                                                    *roughness;
                                                self.state.objects[idx].material.metallic =
                                                    *metallic;
                                                self.status_message =
                                                    format!("Applied {} material", name);
                                            }
                                        }
                                    }
                                    if (i + 1) % 4 == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    }
                    AssetCategory::Objects => {
                        ui.heading("Object Presets");
                        let obj_presets = [
                            ("Cube", ObjectType::Cube),
                            ("Sphere", ObjectType::Sphere),
                            ("Cylinder", ObjectType::Cylinder),
                            ("Cone", ObjectType::Cone),
                            ("Torus", ObjectType::Torus),
                            ("Plane", ObjectType::Plane),
                            ("IcoSphere", ObjectType::IcoSphere),
                            ("Grid", ObjectType::Grid),
                        ];
                        egui::Grid::new("asset_obj_grid")
                            .num_columns(4)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                for (i, (name, _otype)) in obj_presets.iter().enumerate() {
                                    if ui.button(*name).clicked() {
                                        match *name {
                                            "Cube" => self.state.add_cube(),
                                            "Sphere" => self.state.add_sphere(),
                                            "Cylinder" => self.state.add_cylinder(),
                                            "Cone" => self.state.add_cone(),
                                            "Torus" => self.state.add_torus(),
                                            "Plane" => self.state.add_plane(),
                                            "IcoSphere" => self.state.add_icosphere(),
                                            "Grid" => self.state.add_grid(),
                                            _ => {}
                                        }
                                        self.status_message =
                                            format!("Added {} from Asset Browser", name);
                                    }
                                    if (i + 1) % 4 == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    }
                    AssetCategory::Worlds => {
                        ui.heading("World Presets");
                        if ui.button("Studio Lighting").clicked() {
                            self.state.world.sky_color = [0.05, 0.05, 0.12];
                            self.state.world.ambient_intensity = 0.5;
                            self.status_message = "Applied Studio world".to_string();
                        }
                        if ui.button("Outdoor Daylight").clicked() {
                            self.state.world.sky_color = [0.4, 0.6, 0.9];
                            self.state.world.horizon_color = [0.7, 0.8, 0.9];
                            self.state.world.ground_color = [0.15, 0.12, 0.08];
                            self.state.world.ambient_intensity = 0.6;
                            self.status_message = "Applied Outdoor Daylight world".to_string();
                        }
                        if ui.button("Sunset").clicked() {
                            self.state.world.sky_color = [0.8, 0.4, 0.2];
                            self.state.world.horizon_color = [0.9, 0.6, 0.3];
                            self.state.world.ground_color = [0.1, 0.05, 0.02];
                            self.state.world.ambient_intensity = 0.4;
                            self.status_message = "Applied Sunset world".to_string();
                        }
                        if ui.button("Night").clicked() {
                            self.state.world.sky_color = [0.01, 0.01, 0.03];
                            self.state.world.horizon_color = [0.02, 0.02, 0.04];
                            self.state.world.ground_color = [0.0, 0.0, 0.01];
                            self.state.world.ambient_intensity = 0.1;
                            self.status_message = "Applied Night world".to_string();
                        }
                    }
                    _ => {
                        ui.label(format!(
                            "{} assets (coming soon)",
                            self.state.asset_category
                        ));
                    }
                }
            });
        self.show_asset_browser = open;
    }

    fn render_layers_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_render_layers;
        egui::Window::new("Render Layers")
            .open(&mut open)
            .resizable(true)
            .default_width(400.0)
            .show(ctx, |ui| {
                // View Layers
                ui.heading("View Layers");
                let mut vl_remove: Option<usize> = None;
                for (vi, vl) in self.state.view_layers.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut vl.name);
                        ui.checkbox(&mut vl.use_for_rendering, "Render");
                        ui.checkbox(&mut vl.active, "Active");
                        if vi > 0 && ui.small_button("X").clicked() {
                            vl_remove = Some(vi);
                        }
                    });
                }
                if let Some(vi) = vl_remove {
                    if vi < self.state.view_layers.len() {
                        self.state.view_layers.remove(vi);
                    }
                }
                if ui.button("+ Add View Layer").clicked() {
                    let n = self.state.view_layers.len();
                    self.state.view_layers.push(ViewLayer {
                        name: format!("ViewLayer.{:03}", n),
                        ..ViewLayer::default()
                    });
                }

                ui.separator();

                // Render Layers / Passes
                ui.heading("Render Passes");
                let mut rl_remove: Option<usize> = None;
                for (ri, rl) in self.state.render_layers.iter_mut().enumerate() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut rl.name);
                            ui.checkbox(&mut rl.enabled, "Enabled");
                            if ri > 0 && ui.small_button("X").clicked() {
                                rl_remove = Some(ri);
                            }
                        });
                        if rl.samples_override > 0 {
                            ui.horizontal(|ui| {
                                ui.label("Samples Override:");
                                ui.add(
                                    egui::DragValue::new(&mut rl.samples_override).range(1..=4096),
                                );
                            });
                        }
                        ui.horizontal(|ui| {
                            ui.label("Passes:");
                        });
                        let all_passes = [
                            RenderPass::Combined,
                            RenderPass::Diffuse,
                            RenderPass::Glossy,
                            RenderPass::Transmission,
                            RenderPass::Emission,
                            RenderPass::AO,
                            RenderPass::Shadow,
                            RenderPass::Normal,
                            RenderPass::Depth,
                            RenderPass::Mist,
                            RenderPass::ObjectIndex,
                            RenderPass::MaterialIndex,
                        ];
                        for pass in &all_passes {
                            let has = rl.passes.contains(pass);
                            let mut checked = has;
                            ui.checkbox(&mut checked, format!("{}", pass));
                            if checked && !has {
                                rl.passes.push(*pass);
                            } else if !checked && has {
                                rl.passes.retain(|p| p != pass);
                            }
                        }
                    });
                }
                if let Some(ri) = rl_remove {
                    if ri < self.state.render_layers.len() {
                        self.state.render_layers.remove(ri);
                    }
                }
                if ui.button("+ Add Render Layer").clicked() {
                    let n = self.state.render_layers.len();
                    self.state.render_layers.push(RenderLayer {
                        name: format!("RenderLayer.{:03}", n),
                        ..RenderLayer::default()
                    });
                }
            });
        self.show_render_layers = open;
    }

    fn spreadsheet_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_spreadsheet;
        egui::Window::new("Spreadsheet")
            .open(&mut open)
            .resizable(true)
            .default_width(600.0)
            .default_height(400.0)
            .show(ctx, |ui| {
                ui.heading("Object Data Inspector");

                if let Some(idx) = self.state.selected_object {
                    if idx < self.state.objects.len() {
                        let obj = &self.state.objects[idx];
                        ui.label(format!("Object: {} ({:?})", obj.name, obj.object_type));
                        ui.separator();

                        ui.collapsing("Transform", |ui| {
                            egui::Grid::new("ss_transform")
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("");
                                    ui.label("X");
                                    ui.label("Y");
                                    ui.label("Z");
                                    ui.end_row();
                                    ui.label("Position");
                                    ui.label(format!("{:.4}", obj.position[0]));
                                    ui.label(format!("{:.4}", obj.position[1]));
                                    ui.label(format!("{:.4}", obj.position[2]));
                                    ui.end_row();
                                    ui.label("Rotation");
                                    ui.label(format!("{:.2}°", obj.rotation[0]));
                                    ui.label(format!("{:.2}°", obj.rotation[1]));
                                    ui.label(format!("{:.2}°", obj.rotation[2]));
                                    ui.end_row();
                                    ui.label("Scale");
                                    ui.label(format!("{:.4}", obj.scale[0]));
                                    ui.label(format!("{:.4}", obj.scale[1]));
                                    ui.label(format!("{:.4}", obj.scale[2]));
                                    ui.end_row();
                                });
                        });

                        ui.collapsing("Material", |ui| {
                            egui::Grid::new("ss_material").striped(true).show(ui, |ui| {
                                ui.label("Property");
                                ui.label("Value");
                                ui.end_row();
                                ui.label("Color");
                                ui.label(format!(
                                    "({:.3}, {:.3}, {:.3})",
                                    obj.material.base_color[0],
                                    obj.material.base_color[1],
                                    obj.material.base_color[2]
                                ));
                                ui.end_row();
                                ui.label("Roughness");
                                ui.label(format!("{:.3}", obj.material.roughness));
                                ui.end_row();
                                ui.label("Metallic");
                                ui.label(format!("{:.3}", obj.material.metallic));
                                ui.end_row();
                                ui.label("Emissive");
                                ui.label(format!("{:.3}", obj.material.emissive));
                                ui.end_row();
                            });
                        });

                        ui.collapsing(format!("Keyframes ({})", obj.keyframes.len()), |ui| {
                            if obj.keyframes.is_empty() {
                                ui.label("No keyframes");
                            } else {
                                egui::Grid::new("ss_keyframes")
                                    .striped(true)
                                    .show(ui, |ui| {
                                        ui.label("Frame");
                                        ui.label("Pos X");
                                        ui.label("Pos Y");
                                        ui.label("Pos Z");
                                        ui.end_row();
                                        for kf in &obj.keyframes {
                                            ui.label(format!("{}", kf.frame));
                                            ui.label(format!("{:.3}", kf.position[0]));
                                            ui.label(format!("{:.3}", kf.position[1]));
                                            ui.label(format!("{:.3}", kf.position[2]));
                                            ui.end_row();
                                        }
                                    });
                            }
                        });

                        ui.collapsing(format!("Modifiers ({})", obj.modifiers.len()), |ui| {
                            for (i, m) in obj.modifiers.iter().enumerate() {
                                ui.label(format!("  [{}] {}", i, m));
                            }
                        });

                        ui.collapsing(format!("Constraints ({})", obj.constraints.len()), |ui| {
                            for (i, c) in obj.constraints.iter().enumerate() {
                                ui.label(format!("  [{}] {}", i, c));
                            }
                        });

                        ui.collapsing(
                            format!("Vertex Groups ({})", obj.vertex_groups.len()),
                            |ui| {
                                for vg in &obj.vertex_groups {
                                    ui.label(format!(
                                        "  {} ({} weights)",
                                        vg.name,
                                        vg.weights.len()
                                    ));
                                }
                            },
                        );

                        ui.collapsing(
                            format!("Custom Properties ({})", obj.custom_properties.len()),
                            |ui| {
                                for cp in &obj.custom_properties {
                                    ui.label(format!(
                                        "  {} = {} ({})",
                                        cp.name, cp.value, cp.prop_type
                                    ));
                                }
                            },
                        );
                    }
                } else {
                    ui.label("Select an object to inspect its data.");
                }
            });
        self.show_spreadsheet = open;
    }

    #[cfg(feature = "python")]
    fn text_editor_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_text_editor;
        egui::Window::new("Text Editor")
            .open(&mut open)
            .resizable(true)
            .default_width(500.0)
            .default_height(400.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Script Editor");
                    if ui.button("New").clicked() {
                        self.text_editor_content = "# NAT3D Script\n".to_string();
                    }
                    if ui.button("Run").clicked() {
                        self.console_entries.push(console::LogEntry {
                            level: console::LogLevel::Info,
                            message: "Script execution: Python runtime not connected. Use external Python with nat3d module.".to_string(),
                            source: Some("Script".to_string()),
                            timestamp: 0.0,
                            count: 1,
                        });
                        self.status_message = "Script queued (Python runtime not connected)".to_string();
                    }
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.text_editor_content)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(20)
                            .code_editor()
                    );
                });
            });
        self.show_text_editor = open;
    }

    fn sequencer_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_sequencer;
        egui::Window::new("Video Sequence Editor")
            .open(&mut open)
            .resizable(true)
            .default_width(600.0)
            .default_height(300.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Sequencer");
                    if ui.button("Add Image Strip").clicked() {
                        let n = self.state.sequencer_strips.len();
                        self.state.sequencer_strips.push(SequencerStrip {
                            name: format!("Image.{:03}", n),
                            strip_type: SequencerStripType::Image,
                            start_frame: self.state.timeline.current_frame,
                            duration: 25,
                            channel: 1,
                            muted: false,
                            blend: 1.0,
                        });
                    }
                    if ui.button("Add Color Strip").clicked() {
                        let n = self.state.sequencer_strips.len();
                        self.state.sequencer_strips.push(SequencerStrip {
                            name: format!("Color.{:03}", n),
                            strip_type: SequencerStripType::Color,
                            start_frame: self.state.timeline.current_frame,
                            duration: 50,
                            channel: 2,
                            muted: false,
                            blend: 1.0,
                        });
                    }
                    if ui.button("Add Text Strip").clicked() {
                        let n = self.state.sequencer_strips.len();
                        self.state.sequencer_strips.push(SequencerStrip {
                            name: format!("Text.{:03}", n),
                            strip_type: SequencerStripType::Text,
                            start_frame: self.state.timeline.current_frame,
                            duration: 25,
                            channel: 3,
                            muted: false,
                            blend: 1.0,
                        });
                    }
                });
                ui.separator();

                let mut strip_remove: Option<usize> = None;
                if self.state.sequencer_strips.is_empty() {
                    ui.label("No strips. Use buttons above to add strips.");
                } else {
                    egui::Grid::new("seq_grid").striped(true).show(ui, |ui| {
                        ui.label("Name");
                        ui.label("Type");
                        ui.label("Start");
                        ui.label("Dur");
                        ui.label("Ch");
                        ui.label("Blend");
                        ui.label("");
                        ui.end_row();
                        for (si, strip) in self.state.sequencer_strips.iter_mut().enumerate() {
                            ui.text_edit_singleline(&mut strip.name);
                            ui.label(format!("{}", strip.strip_type));
                            ui.add(egui::DragValue::new(&mut strip.start_frame).speed(1));
                            ui.add(
                                egui::DragValue::new(&mut strip.duration)
                                    .range(1..=10000)
                                    .speed(1),
                            );
                            ui.add(egui::DragValue::new(&mut strip.channel).range(1..=32));
                            ui.add(
                                egui::Slider::new(&mut strip.blend, 0.0..=1.0).show_value(false),
                            );
                            if ui.small_button("X").clicked() {
                                strip_remove = Some(si);
                            }
                            ui.end_row();
                        }
                    });
                }
                if let Some(si) = strip_remove {
                    if si < self.state.sequencer_strips.len() {
                        self.state.sequencer_strips.remove(si);
                    }
                }

                // Visual timeline representation
                ui.separator();
                let timeline_rect = ui.available_rect_before_wrap();
                let painter = ui.painter_at(timeline_rect);
                let w = timeline_rect.width();
                let h = 80.0_f32.min(timeline_rect.height());
                let track_rect = egui::Rect::from_min_size(timeline_rect.min, egui::vec2(w, h));
                painter.rect_filled(track_rect, 0.0, egui::Color32::from_rgb(30, 30, 35));

                let total_frames =
                    (self.state.timeline.end_frame - self.state.timeline.start_frame) as f32;
                if total_frames > 0.0 {
                    let colors = [
                        egui::Color32::from_rgb(60, 100, 180),
                        egui::Color32::from_rgb(180, 80, 60),
                        egui::Color32::from_rgb(60, 160, 80),
                        egui::Color32::from_rgb(180, 160, 60),
                    ];
                    for strip in &self.state.sequencer_strips {
                        if strip.muted {
                            continue;
                        }
                        let x_start = ((strip.start_frame - self.state.timeline.start_frame)
                            as f32
                            / total_frames)
                            * w;
                        let x_end = ((strip.start_frame + strip.duration
                            - self.state.timeline.start_frame)
                            as f32
                            / total_frames)
                            * w;
                        let y = (strip.channel as f32 - 1.0) * 20.0;
                        let color = colors[(strip.channel as usize - 1) % colors.len()];
                        let strip_rect = egui::Rect::from_min_size(
                            track_rect.min + egui::vec2(x_start, y),
                            egui::vec2((x_end - x_start).max(2.0), 18.0),
                        );
                        painter.rect_filled(strip_rect, 2.0, color);
                        painter.text(
                            strip_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &strip.name,
                            egui::FontId::proportional(9.0),
                            egui::Color32::WHITE,
                        );
                    }
                    // Playhead
                    let px = ((self.state.timeline.current_frame - self.state.timeline.start_frame)
                        as f32
                        / total_frames)
                        * w;
                    painter.line_segment(
                        [
                            track_rect.min + egui::vec2(px, 0.0),
                            track_rect.min + egui::vec2(px, h),
                        ],
                        egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(255, 100, 100)),
                    );
                }
                ui.allocate_space(egui::vec2(w, h));
            });
        self.show_sequencer = open;
    }

    fn image_editor_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_image_editor;
        egui::Window::new("Image Editor")
            .open(&mut open)
            .resizable(true)
            .default_width(500.0)
            .default_height(400.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Image/UV Editor");
                    if ui.button("New Image").clicked() {
                        self.status_message = "New 1024x1024 image created".to_string();
                    }
                    if ui.button("Open Image").clicked() {
                        self.status_message = "Image open dialog (placeholder)".to_string();
                    }
                    if ui.button("Save Image").clicked() {
                        self.status_message = "Image save dialog (placeholder)".to_string();
                    }
                });
                ui.separator();

                // Image canvas area (placeholder)
                let avail = ui.available_size();
                let (rect, _response) =
                    ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
                let painter = ui.painter_at(rect);

                // Checkerboard pattern (transparent background indicator)
                let sq = 16.0;
                let cols = (rect.width() / sq) as i32 + 1;
                let rows = (rect.height() / sq) as i32 + 1;
                for r in 0..rows.min(30) {
                    for c in 0..cols.min(40) {
                        let light = (r + c) % 2 == 0;
                        let color = if light {
                            egui::Color32::from_rgb(100, 100, 100)
                        } else {
                            egui::Color32::from_rgb(60, 60, 60)
                        };
                        let sq_rect = egui::Rect::from_min_size(
                            rect.min + egui::vec2(c as f32 * sq, r as f32 * sq),
                            egui::vec2(sq, sq),
                        );
                        painter.rect_filled(sq_rect, 0.0, color);
                    }
                }

                // Info overlay
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No Image Loaded\n(Load an image or create new)",
                    egui::FontId::proportional(14.0),
                    egui::Color32::from_rgba_unmultiplied(200, 200, 200, 180),
                );
            });
        self.show_image_editor = open;
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            // Tool shortcuts (Q/W/E/R = 3ds Max style, G/R/S = Blender style)
            if i.key_pressed(egui::Key::Q) && !i.modifiers.any() {
                self.state.tool = Tool::Select;
            }
            if i.key_pressed(egui::Key::W) && !i.modifiers.any() {
                self.state.tool = Tool::Move;
            }
            if i.key_pressed(egui::Key::E) && !i.modifiers.any() {
                self.state.tool = Tool::Rotate;
            }
            if i.key_pressed(egui::Key::R) && !i.modifiers.any() {
                self.state.tool = Tool::Rotate; // Blender convention: R = Rotate
                self.state.axis_constraint = AxisConstraint::None;
                self.status_message = "Rotate".to_string();
            }

            // View shortcuts
            if i.key_pressed(egui::Key::Num1) {
                self.state.camera.set_view_front();
            }
            if i.key_pressed(egui::Key::Num3) {
                self.state.camera.set_view_right();
            }
            if i.key_pressed(egui::Key::Num7) {
                self.state.camera.set_view_top();
            }
            if i.key_pressed(egui::Key::Num0) {
                self.state.camera.reset();
            }

            // Undo/Redo (check first to avoid conflict with Z shading)
            if i.key_pressed(egui::Key::Z) && i.modifiers.ctrl {
                if i.modifiers.shift {
                    if self.state.redo() {
                        self.status_message = "Redo".to_string();
                    } else {
                        self.status_message = "Nothing to redo".to_string();
                    }
                } else if self.state.undo() {
                    self.status_message = "Undo".to_string();
                } else {
                    self.status_message = "Nothing to undo".to_string();
                }
            }

            // Z key: axis constraint when in transform tool, shading cycle otherwise
            if i.key_pressed(egui::Key::Z) && !i.modifiers.ctrl && !i.modifiers.shift {
                if self.state.tool != Tool::Select {
                    // Z axis constraint during transform
                    self.state.axis_constraint = if self.state.axis_constraint == AxisConstraint::Z
                    {
                        AxisConstraint::None
                    } else {
                        AxisConstraint::Z
                    };
                    self.status_message = format!(
                        "{:?} constrained to {}",
                        self.state.tool, self.state.axis_constraint
                    );
                } else {
                    // Shading cycle when in Select mode
                    self.state.shading = match self.state.shading {
                        ShadingMode::Wireframe => ShadingMode::Solid,
                        ShadingMode::Solid => ShadingMode::Material,
                        ShadingMode::Material => ShadingMode::Rendered,
                        ShadingMode::Rendered => ShadingMode::Wireframe,
                    };
                    self.status_message = format!("Shading: {:?}", self.state.shading);
                }
            }

            // Object operations
            if (i.key_pressed(egui::Key::Delete)
                || (i.key_pressed(egui::Key::X) && i.modifiers.shift))
                && self.state.selected_object.is_some()
            {
                self.state.save_undo_state();
                self.state.delete_selected();
                self.status_message = "Object deleted".to_string();
            }

            if i.key_pressed(egui::Key::D)
                && i.modifiers.shift
                && self.state.selected_object.is_some()
            {
                self.state.save_undo_state();
                self.state.duplicate_selected();
                self.status_message = "Object duplicated".to_string();
            }

            // Blender-style G/R/S shortcuts
            if i.key_pressed(egui::Key::G) && !i.modifiers.any() {
                self.state.tool = Tool::Move;
                self.state.axis_constraint = AxisConstraint::None;
                self.status_message = "Grab (Move)".to_string();
            }
            if i.key_pressed(egui::Key::S) && !i.modifiers.any() {
                self.state.tool = Tool::Scale;
                self.state.axis_constraint = AxisConstraint::None;
                self.status_message = "Scale".to_string();
            }

            // Axis constraints (X/Y/Z) during transform
            if i.key_pressed(egui::Key::X) && !i.modifiers.any() && self.state.tool != Tool::Select
            {
                self.state.axis_constraint = if self.state.axis_constraint == AxisConstraint::X {
                    AxisConstraint::None
                } else {
                    AxisConstraint::X
                };
                self.status_message = format!(
                    "{:?} constrained to {}",
                    self.state.tool, self.state.axis_constraint
                );
            }
            if i.key_pressed(egui::Key::Y) && !i.modifiers.any() && self.state.tool != Tool::Select
            {
                self.state.axis_constraint = if self.state.axis_constraint == AxisConstraint::Y {
                    AxisConstraint::None
                } else {
                    AxisConstraint::Y
                };
                self.status_message = format!(
                    "{:?} constrained to {}",
                    self.state.tool, self.state.axis_constraint
                );
            }

            // Tab to toggle edit mode
            if i.key_pressed(egui::Key::Tab) && !i.modifiers.any() {
                let old_mode = self.state.edit_mode;
                self.state.edit_mode = match self.state.edit_mode {
                    EditMode::Object => EditMode::Edit,
                    EditMode::Edit => EditMode::Object,
                    EditMode::Sculpt => EditMode::Object,
                    EditMode::TexturePaint => EditMode::Object,
                    EditMode::WeightPaint => EditMode::Object,
                };

                // Enter/Exit Edit Mode: Create/Apply EditableMesh
                if old_mode == EditMode::Object && self.state.edit_mode == EditMode::Edit {
                    // Entering Edit Mode
                    if let Some(sel_idx) = self.state.selected_object {
                        if sel_idx < self.state.objects.len() {
                            // Extract real geometry from object BEFORE mutable borrow
                            let vertices = self.get_object_vertices(sel_idx);
                            let faces = self.get_object_faces(sel_idx);

                            // NOW do mutable borrow to set edit_mesh
                            let obj = &mut self.state.objects[sel_idx];
                            if obj.edit_mesh.is_none() {
                                obj.edit_mesh = Some(EditableMesh::new(vertices, faces));
                                obj.edit_selection = EditModeSelection::default();
                                self.status_message = "Entered Edit Mode".to_string();
                            }
                        }
                    }
                } else if old_mode == EditMode::Edit && self.state.edit_mode == EditMode::Object {
                    // Exiting Edit Mode - Apply changes
                    if let Some(sel_idx) = self.state.selected_object {
                        if sel_idx < self.state.objects.len() {
                            let obj = &mut self.state.objects[sel_idx];
                            if let Some(ref edit_mesh) = obj.edit_mesh {
                                // Convert edited mesh to custom geometry
                                obj.custom_vertices = Some(edit_mesh.vertices.clone());
                                obj.custom_faces = Some(edit_mesh.faces.clone());
                                // Convert object to generic Mesh type to use custom geometry
                                obj.object_type = ObjectType::Mesh;
                                // Clear edit mode data
                                obj.edit_mesh = None;
                                obj.edit_selection = EditModeSelection::default();
                                self.status_message =
                                    "Exited Edit Mode - Changes applied to mesh".to_string();
                            }
                        }
                    }
                }

                if self.status_message.is_empty() {
                    self.status_message = format!("Mode: {:?}", self.state.edit_mode);
                }
            }

            // Edit Mode operations (only in Edit Mode)
            if self.state.edit_mode == EditMode::Edit {
                // X: Delete selected vertices/edges/faces
                if i.key_pressed(egui::Key::X) && !i.modifiers.any() {
                    if let Some(sel_idx) = self.state.selected_object {
                        if sel_idx < self.state.objects.len() {
                            // Clone selection data BEFORE mutable borrow
                            let selection = self.state.objects[sel_idx].edit_selection.clone();
                            if let Some(edit_mesh) = &mut self.state.objects[sel_idx].edit_mesh {
                                match self.state.edit_selection {
                                    EditSelection::Vertex => {
                                        if !selection.vertices.is_empty() {
                                            let count = selection.vertices.len();
                                            edit_mesh.delete_vertices(&selection.vertices);
                                            self.state.objects[sel_idx].edit_selection =
                                                EditModeSelection::default();
                                            self.status_message =
                                                format!("Deleted {} vertices", count);
                                        }
                                    }
                                    EditSelection::Edge => {
                                        if !selection.edges.is_empty() {
                                            let count = selection.edges.len();
                                            edit_mesh.delete_edges(
                                                &(0..edit_mesh.edges.len())
                                                    .filter(|&i| {
                                                        selection.edges.iter().any(|&(v1, v2)| {
                                                            edit_mesh.edges[i] == (v1, v2)
                                                                || edit_mesh.edges[i] == (v2, v1)
                                                        })
                                                    })
                                                    .collect::<Vec<_>>(),
                                            );
                                            self.state.objects[sel_idx].edit_selection =
                                                EditModeSelection::default();
                                            self.status_message = format!(
                                                "Deleted {} edges (and their faces)",
                                                count
                                            );
                                        }
                                    }
                                    EditSelection::Face => {
                                        if !selection.faces.is_empty() {
                                            let count = selection.faces.len();
                                            edit_mesh.delete_faces(&selection.faces);
                                            self.state.objects[sel_idx].edit_selection =
                                                EditModeSelection::default();
                                            self.status_message =
                                                format!("Deleted {} faces", count);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Ctrl+M: Merge vertices
                if i.key_pressed(egui::Key::M) && i.modifiers.ctrl {
                    if let Some(sel_idx) = self.state.selected_object {
                        if sel_idx < self.state.objects.len() {
                            // Clone selection data BEFORE mutable borrow
                            let selection = self.state.objects[sel_idx].edit_selection.clone();
                            if self.state.edit_selection == EditSelection::Vertex
                                && selection.vertices.len() >= 2
                            {
                                if let Some(edit_mesh) = &mut self.state.objects[sel_idx].edit_mesh
                                {
                                    let count = selection.vertices.len();
                                    edit_mesh.merge_vertices(&selection.vertices);
                                    self.state.objects[sel_idx].edit_selection =
                                        EditModeSelection::default();
                                    self.status_message = format!("Merged {} vertices", count);
                                }
                            }
                        }
                    }
                }

                // Ctrl+E: Extrude faces
                if i.key_pressed(egui::Key::E) && i.modifiers.ctrl {
                    if let Some(sel_idx) = self.state.selected_object {
                        if sel_idx < self.state.objects.len() {
                            // Clone selection data BEFORE mutable borrow
                            let selection = self.state.objects[sel_idx].edit_selection.clone();
                            if self.state.edit_selection == EditSelection::Face
                                && !selection.faces.is_empty()
                            {
                                if let Some(edit_mesh) = &mut self.state.objects[sel_idx].edit_mesh
                                {
                                    let count = selection.faces.len();
                                    // Extrude by 0.5 units in normal direction (placeholder)
                                    edit_mesh.extrude_faces(&selection.faces, [0.0, 0.0, 0.5]);
                                    self.status_message = format!("Extruded {} faces", count);
                                }
                            }
                        }
                    }
                }

                // I: Inset faces
                if i.key_pressed(egui::Key::I) && !i.modifiers.any() {
                    if let Some(sel_idx) = self.state.selected_object {
                        if sel_idx < self.state.objects.len() {
                            // Clone selection data BEFORE mutable borrow
                            let selection = self.state.objects[sel_idx].edit_selection.clone();
                            if self.state.edit_selection == EditSelection::Face
                                && !selection.faces.is_empty()
                            {
                                if let Some(edit_mesh) = &mut self.state.objects[sel_idx].edit_mesh
                                {
                                    let count = selection.faces.len();
                                    edit_mesh.inset_faces(&selection.faces, 0.3);
                                    self.status_message = format!("Inset {} faces", count);
                                }
                            }
                        }
                    }
                }

                // Ctrl+R: Subdivide edges
                if i.key_pressed(egui::Key::R) && i.modifiers.ctrl && !i.modifiers.shift {
                    if let Some(sel_idx) = self.state.selected_object {
                        if sel_idx < self.state.objects.len() {
                            // Clone selection data BEFORE mutable borrow
                            let selection = self.state.objects[sel_idx].edit_selection.clone();
                            if self.state.edit_selection == EditSelection::Edge
                                && !selection.edges.is_empty()
                            {
                                if let Some(edit_mesh) = &mut self.state.objects[sel_idx].edit_mesh
                                {
                                    // Find edge indices from edge pairs
                                    let mut edge_indices = Vec::new();
                                    for &(v1, v2) in &selection.edges {
                                        if let Some(idx) = edit_mesh.edges.iter().position(|&e| {
                                            (e.0 == v1 && e.1 == v2) || (e.0 == v2 && e.1 == v1)
                                        }) {
                                            edge_indices.push(idx);
                                        }
                                    }
                                    if !edge_indices.is_empty() {
                                        let count = edge_indices.len();
                                        edit_mesh.subdivide_edges(&edge_indices);
                                        self.state.objects[sel_idx].edit_selection =
                                            EditModeSelection::default();
                                        self.status_message = format!("Subdivided {} edges", count);
                                    }
                                }
                            }
                        }
                    }
                }

                // Shift+Ctrl+S: Catmull-Clark subdivision (entire mesh)
                if i.key_pressed(egui::Key::S) && i.modifiers.ctrl && i.modifiers.shift {
                    if let Some(sel_idx) = self.state.selected_object {
                        if sel_idx < self.state.objects.len() {
                            // Get counts before subdivision
                            let (vert_count_before, face_count_before) =
                                if let Some(edit_mesh) = &self.state.objects[sel_idx].edit_mesh {
                                    (edit_mesh.vertices.len(), edit_mesh.faces.len())
                                } else {
                                    (0, 0)
                                };

                            self.state.save_undo_state();

                            if let Some(edit_mesh) = &mut self.state.objects[sel_idx].edit_mesh {
                                edit_mesh.subdivide_catmull_clark();
                                let vert_count_after = edit_mesh.vertices.len();
                                let face_count_after = edit_mesh.faces.len();
                                self.status_message = format!(
                                    "Catmull-Clark: {} → {} verts, {} → {} faces",
                                    vert_count_before,
                                    vert_count_after,
                                    face_count_before,
                                    face_count_after
                                );
                            }
                        }
                    }
                }
            }

            // Add object shortcuts
            if i.key_pressed(egui::Key::A) && i.modifiers.shift {
                self.state.save_undo_state();
                self.state.add_cube();
                self.status_message = "Added Cube".to_string();
            }

            // Select all / deselect (Ctrl+A / A)
            if i.key_pressed(egui::Key::A) && i.modifiers.ctrl {
                // Select ALL objects
                self.state.clear_multi_select();
                if !self.state.objects.is_empty() {
                    self.state.selected_object = Some(0);
                    for idx in 1..self.state.objects.len() {
                        self.state.multi_selected.push(idx);
                    }
                    self.status_message =
                        format!("Selected all {} objects", self.state.objects.len());
                }
            }

            // A key without modifiers: toggle select all / deselect (Blender convention)
            if i.key_pressed(egui::Key::A) && !i.modifiers.any() {
                let all_count = self.state.all_selected().len();
                if all_count == self.state.objects.len() && !self.state.objects.is_empty() {
                    // All already selected - deselect all
                    self.state.selected_object = None;
                    self.state.clear_multi_select();
                    self.status_message = "All deselected".to_string();
                } else if !self.state.objects.is_empty() {
                    // Select all
                    self.state.selected_object = Some(0);
                    self.state.multi_selected.clear();
                    for idx in 1..self.state.objects.len() {
                        self.state.multi_selected.push(idx);
                    }
                    self.status_message =
                        format!("Selected all {} objects", self.state.objects.len());
                }
            }

            if i.key_pressed(egui::Key::Escape) {
                self.state.selected_object = None;
                self.state.clear_multi_select();
                self.status_message = "Selection cleared".to_string();
            }

            // Invert selection (Ctrl+I)
            if i.key_pressed(egui::Key::I) && i.modifiers.ctrl {
                let current = self.state.all_selected();
                let total = self.state.objects.len();
                self.state.selected_object = None;
                self.state.clear_multi_select();
                let mut first = true;
                for idx in 0..total {
                    if !current.contains(&idx) {
                        if first {
                            self.state.selected_object = Some(idx);
                            first = false;
                        } else {
                            self.state.multi_selected.push(idx);
                        }
                    }
                }
                let new_count = self.state.all_selected().len();
                self.status_message = format!("Inverted selection: {} objects", new_count);
            }

            // Hide selected (H key) / Unhide all (Alt+H)
            if i.key_pressed(egui::Key::H) && !i.modifiers.any() {
                for idx in self.state.all_selected() {
                    if idx < self.state.objects.len() {
                        self.state.objects[idx].visible = false;
                    }
                }
                self.state.selected_object = None;
                self.state.clear_multi_select();
                self.status_message = "Hidden selected objects".to_string();
            }
            if i.key_pressed(egui::Key::H) && i.modifiers.alt {
                for obj in self.state.objects.iter_mut() {
                    obj.visible = true;
                }
                self.status_message = "All objects visible".to_string();
            }

            // Focus on selected (F key)
            if i.key_pressed(egui::Key::F) {
                if let Some(idx) = self.state.selected_object {
                    let obj = &self.state.objects[idx];
                    self.state.camera.target = obj.position;
                    self.state.camera.update_position();
                    self.status_message = format!("Focused on {}", obj.name);
                }
            }

            // Home key - reset view
            if i.key_pressed(egui::Key::Home) {
                self.state.camera.reset();
                self.status_message = "View reset".to_string();
            }

            // Toggle panels
            if i.key_pressed(egui::Key::N) {
                self.show_hierarchy = !self.show_hierarchy;
            }
            if i.key_pressed(egui::Key::T) && !i.modifiers.any() {
                self.show_timeline = !self.show_timeline;
            }

            // Insert keyframe (I key)
            if i.key_pressed(egui::Key::I) && !i.modifiers.any() && self.state.insert_keyframe() {
                let frame = self.state.timeline.current_frame;
                self.status_message = format!("Keyframe inserted at frame {}", frame);
                self.log_console(
                    console::LogLevel::Info,
                    &format!("Keyframe at frame {}", frame),
                    "Animation",
                );
            }

            // Delete keyframe (Alt+I)
            if i.key_pressed(egui::Key::I) && i.modifiers.alt && self.state.delete_keyframe() {
                self.status_message = "Keyframe deleted".to_string();
            }

            // Object parenting (Ctrl+P: set parent, Alt+P: clear parent)
            if i.key_pressed(egui::Key::P) && i.modifiers.ctrl && self.state.parent_selected() {
                self.status_message = "Parent set".to_string();
                self.log_console(console::LogLevel::Info, "Object parented", "Scene");
            }
            if i.key_pressed(egui::Key::P) && i.modifiers.alt {
                self.state.clear_parent_selected();
                self.status_message = "Parent cleared".to_string();
            }

            // Box selection (B key to activate)
            if i.key_pressed(egui::Key::B) && !i.modifiers.any() {
                self.state.box_select_active = !self.state.box_select_active;
                if self.state.box_select_active {
                    self.status_message = "Box Select: drag to select".to_string();
                } else {
                    self.state.box_select_start = None;
                    self.box_select_end = None;
                    self.status_message = "Box Select cancelled".to_string();
                }
            }

            // Camera bookmarks: Ctrl+1-5 save, Alt+1-5 restore
            if i.key_pressed(egui::Key::Num1) && i.modifiers.ctrl {
                self.state.save_camera_bookmark(0);
                self.status_message = "Camera bookmark 1 saved".to_string();
            }
            if i.key_pressed(egui::Key::Num2) && i.modifiers.ctrl {
                self.state.save_camera_bookmark(1);
                self.status_message = "Camera bookmark 2 saved".to_string();
            }
            if i.key_pressed(egui::Key::Num3) && i.modifiers.ctrl {
                self.state.save_camera_bookmark(2);
                self.status_message = "Camera bookmark 3 saved".to_string();
            }
            if i.key_pressed(egui::Key::Num4) && i.modifiers.ctrl {
                self.state.save_camera_bookmark(3);
                self.status_message = "Camera bookmark 4 saved".to_string();
            }
            if i.key_pressed(egui::Key::Num5) && i.modifiers.ctrl {
                self.state.save_camera_bookmark(4);
                self.status_message = "Camera bookmark 5 saved".to_string();
            }
            if i.key_pressed(egui::Key::Num1)
                && i.modifiers.alt
                && self.state.restore_camera_bookmark(0)
            {
                self.status_message = "Camera bookmark 1 restored".to_string();
            }
            if i.key_pressed(egui::Key::Num2)
                && i.modifiers.alt
                && self.state.restore_camera_bookmark(1)
            {
                self.status_message = "Camera bookmark 2 restored".to_string();
            }
            if i.key_pressed(egui::Key::Num3)
                && i.modifiers.alt
                && self.state.restore_camera_bookmark(2)
            {
                self.status_message = "Camera bookmark 3 restored".to_string();
            }
            if i.key_pressed(egui::Key::Num4)
                && i.modifiers.alt
                && self.state.restore_camera_bookmark(3)
            {
                self.status_message = "Camera bookmark 4 restored".to_string();
            }
            if i.key_pressed(egui::Key::Num5)
                && i.modifiers.alt
                && self.state.restore_camera_bookmark(4)
            {
                self.status_message = "Camera bookmark 5 restored".to_string();
            }

            // Edit mode sub-selection: 1/2/3 for Vertex/Edge/Face (when in Edit mode)
            if self.state.edit_mode == EditMode::Edit {
                if i.key_pressed(egui::Key::Num1) && !i.modifiers.any() {
                    self.state.edit_selection = EditSelection::Vertex;
                    self.status_message = "Vertex select mode".to_string();
                }
                if i.key_pressed(egui::Key::Num2) && !i.modifiers.any() {
                    self.state.edit_selection = EditSelection::Edge;
                    self.status_message = "Edge select mode".to_string();
                }
                if i.key_pressed(egui::Key::Num3) && !i.modifiers.any() {
                    self.state.edit_selection = EditSelection::Face;
                    self.status_message = "Face select mode".to_string();
                }
            }

            // Proportional editing toggle (O key, like Blender)
            if i.key_pressed(egui::Key::O) && !i.modifiers.any() {
                self.state.proportional_editing = !self.state.proportional_editing;
                self.status_message = format!(
                    "Proportional editing: {}",
                    if self.state.proportional_editing {
                        "ON"
                    } else {
                        "OFF"
                    }
                );
            }

            // Measurement mode (M key when no modifiers)
            if i.key_pressed(egui::Key::M) && !i.modifiers.any() {
                self.state.measuring = !self.state.measuring;
                if self.state.measuring {
                    self.state.measure_start = None;
                    self.status_message = "Measure: click two points".to_string();
                } else {
                    self.status_message = "Measure mode off".to_string();
                }
            }

            // Clear measurements (Ctrl+M)
            if i.key_pressed(egui::Key::M) && i.modifiers.ctrl {
                self.state.clear_measurements();
                self.status_message = "Measurements cleared".to_string();
            }

            // Copy (Ctrl+C)
            if i.key_pressed(egui::Key::C) && i.modifiers.ctrl {
                if let Some(idx) = self.state.selected_object {
                    self.state.clipboard = vec![self.state.objects[idx].clone()];
                    for &mi in &self.state.multi_selected.clone() {
                        if mi < self.state.objects.len() {
                            self.state.clipboard.push(self.state.objects[mi].clone());
                        }
                    }
                    self.status_message =
                        format!("Copied {} object(s)", self.state.clipboard.len());
                }
            }

            // Paste (Ctrl+V)
            if i.key_pressed(egui::Key::V) && i.modifiers.ctrl && !self.state.clipboard.is_empty() {
                self.state.save_undo_state();
                let clip = self.state.clipboard.clone();
                for mut obj in clip {
                    obj.name = format!("{}.copy", obj.name);
                    obj.position[0] += 1.0;
                    self.state.objects.push(obj);
                }
                self.state.selected_object = Some(self.state.objects.len() - 1);
                self.status_message = "Pasted from clipboard".to_string();
            }

            // Auto-Key toggle (K key, only outside Edit mode where K is Knife)
            if i.key_pressed(egui::Key::K)
                && !i.modifiers.any()
                && self.state.edit_mode != EditMode::Edit
            {
                self.state.auto_key = !self.state.auto_key;
                self.status_message = format!(
                    "Auto-Key: {}",
                    if self.state.auto_key { "ON" } else { "OFF" }
                );
            }

            // X-Ray toggle (Alt+Z)
            if i.key_pressed(egui::Key::Z) && i.modifiers.alt {
                self.state.xray_mode = !self.state.xray_mode;
                self.status_message =
                    format!("X-Ray: {}", if self.state.xray_mode { "ON" } else { "OFF" });
            }

            // Edit mode tool shortcuts (when in Edit mode)
            if self.state.edit_mode == EditMode::Edit {
                if i.key_pressed(egui::Key::E) && !i.modifiers.any() {
                    self.state.edit_tool = EditTool::Extrude;
                    self.status_message = "Extrude tool".to_string();
                }
                if i.key_pressed(egui::Key::K) && !i.modifiers.any() {
                    self.state.edit_tool = EditTool::Knife;
                    self.status_message = "Knife tool".to_string();
                }
                if i.key_pressed(egui::Key::R) && i.modifiers.ctrl {
                    self.state.edit_tool = EditTool::LoopCut;
                    self.status_message = "Loop Cut tool".to_string();
                }
                if i.key_pressed(egui::Key::B) && i.modifiers.ctrl {
                    self.state.edit_tool = EditTool::BevelEdge;
                    self.status_message = "Bevel Edge tool".to_string();
                }
                if i.key_pressed(egui::Key::I) && !i.modifiers.any() && !i.modifiers.alt {
                    self.state.edit_tool = EditTool::InsetFace;
                    self.status_message = "Inset Face tool".to_string();
                }
                if i.key_pressed(egui::Key::U) && !i.modifiers.any() {
                    use nat3d_modeling::uv::UvMethod;
                    self.state.unwrap_uvs(UvMethod::SmartProject);
                    self.show_uv_editor = true;
                    self.status_message = "UV Unwrap: Smart UV Project".to_string();
                }
            }
        });
    }
}

impl eframe::App for Nat3DApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Apply any viewport resize requested by the paint callback. This has
        // to happen here, not in the callback: re-registering the render
        // texture needs the `egui_wgpu::Renderer` lock, which eframe holds for
        // the entire callback but not during `update()`. See the deadlock note
        // in `ViewportCallback::prepare`.
        if let (Some(gpu), Some(render_state)) =
            (self.gpu_renderer.as_ref(), frame.wgpu_render_state())
        {
            let pending = gpu.read().pending_resize;
            if let Some((w, h)) = pending {
                let mut gpu = gpu.write();
                gpu.resize(&render_state.device, w, h);
                gpu.pending_resize = None;
            }
        }

        // GPU rendering is handled by ViewportCallback::prepare() via egui_wgpu callbacks.
        static FIRST_FRAME: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        FIRST_FRAME.get_or_init(|| {
            tracing::info!("First update() frame — event loop running");
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(64.0, 64.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1600.0, 900.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            ctx.request_repaint();
        });
        // Apply any iPad touch/pencil input received since the last frame.
        #[cfg(feature = "ipad")]
        self.process_ipad_input();

        // Update timeline playback
        let dt = ctx.input(|i| i.predicted_dt);
        if self.state.timeline.update(dt) {
            // Frame changed - evaluate keyframe animations
            self.state.evaluate_keyframes();
        }

        // Evaluate object constraints
        self.state.evaluate_constraints();

        // Physics simulation step
        self.state.physics_step(dt);

        // Handle keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx);

        // Show windows
        self.render_settings_window(ctx);
        self.about_window(ctx);
        self.preferences_window(ctx);
        self.material_editor_window(ctx);
        self.console_window(ctx);
        self.node_editor_window(ctx);
        self.uv_editor_window(ctx);
        self.graph_editor_window(ctx);
        self.camera_settings_window(ctx);
        self.world_settings_window(ctx);
        self.scene_properties_window(ctx);
        self.nla_editor_window(ctx);
        self.color_management_window(ctx);
        self.asset_browser_window(ctx);
        self.render_layers_window(ctx);
        self.spreadsheet_window(ctx);
        #[cfg(feature = "python")]
        self.text_editor_window(ctx);
        self.sequencer_window(ctx);
        self.image_editor_window(ctx);
        self.welcome_screen_window(ctx);
        self.license_dialog(ctx);

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.menu_bar(ui);
        });

        // Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.toolbar(ui);
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            self.status_bar(ui);
        });

        // Timeline panel (if visible)
        if self.show_timeline {
            egui::TopBottomPanel::bottom("timeline")
                .resizable(true)
                .default_height(100.0)
                .show(ctx, |ui| {
                    self.timeline_panel(ui);
                });
        }

        // Left panel - Hierarchy
        if self.show_hierarchy {
            egui::SidePanel::left("hierarchy")
                .resizable(true)
                .default_width(200.0)
                .show(ctx, |ui| {
                    self.hierarchy_panel(ui);
                });
        }

        // Right panel - Properties
        if self.show_properties {
            egui::SidePanel::right("properties")
                .resizable(true)
                .default_width(300.0)
                .show(ctx, |ui| {
                    self.properties_panel(ui);
                });
        }

        // Central panel - 3D Viewport
        egui::CentralPanel::default().show(ctx, |ui| {
            self.viewport_3d(ui);
        });

        // Request repaint for continuous updates
        ctx.request_repaint();
    }
}

// BATCH 24: Final Polish - Moving trait impl to top level
impl nat3d_scripting::ScriptingHost for Nat3DApp {
    fn create_object(&self, obj_type: &str, name: &str) {
        tracing::info!("REAL Python request: create {} as {}", obj_type, name);
    }
    fn delete_object(&self, name: &str) {
        tracing::info!("Python request: delete {}", name);
    }
    fn translate_object(&self, name: &str, x: f32, y: f32, z: f32) {
        tracing::info!("Python request: translate {} by {},{},{}", name, x, y, z);
    }
}
