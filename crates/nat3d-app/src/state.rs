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

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! Application state management.

/// Axis constraint for transform operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AxisConstraint {
    /// No constraint (free movement).
    #[default]
    None,
    /// Constrained to X axis.
    X,
    /// Constrained to Y axis.
    Y,
    /// Constrained to Z axis.
    Z,
}

impl std::fmt::Display for AxisConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "Free"),
            Self::X => write!(f, "X"),
            Self::Y => write!(f, "Y"),
            Self::Z => write!(f, "Z"),
        }
    }
}

// Tool selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Move,
    Rotate,
    Scale,
}

// Edit mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditMode {
    #[default]
    Object,
    Edit,
    Sculpt,
    TexturePaint,
    WeightPaint,
}

// Shading mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadingMode {
    Wireframe,
    #[default]
    Solid,
    Material,
    Rendered,
}

/// Application state containing all scene data.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SimulationMode {
    #[default]
    Off,
    NeuralVoltage,
    O2Saturation,
    Temperature,
    Metabolic,
    Pressure,
}

pub struct AppState {
    /// Scene objects.
    pub objects: Vec<SceneObject>,
    /// Currently selected object index.
    pub selected_object: Option<usize>,
    /// Camera state.
    pub camera: CameraState,
    /// Timeline state.
    pub timeline: TimelineState,
    /// Current tool.
    pub tool: Tool,
    /// Current edit mode.
    pub edit_mode: EditMode,
    /// Current shading mode.
    pub shading: ShadingMode,
    pub simulation_mode: SimulationMode,
    /// Snap enabled.
    pub snap_enabled: bool,
    /// Snap increment.
    pub snap_increment: f32,
    /// Axis constraint for transform tools.
    pub axis_constraint: AxisConstraint,
    /// Undo stack.
    undo_stack: Vec<UndoState>,
    /// Redo stack.
    redo_stack: Vec<UndoState>,
    /// Next object ID.
    next_object_id: usize,
    /// Physics simulation state per object (velocity, is_rigid_body).
    pub physics: Vec<PhysicsBody>,
    /// Whether physics simulation is running.
    pub physics_running: bool,
    /// Multi-select: additional selected objects (in addition to selected_object).
    pub multi_selected: Vec<usize>,
    /// Whether wireframe overlay is enabled on solid shading.
    pub wireframe_overlay: bool,
    /// Whether to show viewport statistics (verts, faces, etc.).
    pub show_viewport_stats: bool,
    /// Box selection mode active (B key).
    pub box_select_active: bool,
    /// Box selection start position (screen space).
    pub box_select_start: Option<[f32; 2]>,
    /// Camera bookmarks (up to 5 saved views).
    pub camera_bookmarks: [Option<CameraBookmark>; 5],
    /// Current sculpt brush type.
    pub sculpt_brush: SculptBrush,
    /// Sculpt brush radius.
    pub sculpt_radius: f32,
    /// Sculpt brush strength.
    pub sculpt_strength: f32,
    /// Edit mode sub-selection (vertex/edge/face).
    pub edit_selection: EditSelection,
    /// Proportional editing enabled.
    pub proportional_editing: bool,
    /// Proportional editing radius.
    pub proportional_radius: f32,
    /// Active measurements in the scene.
    pub measurements: Vec<Measurement>,
    /// Whether measurement mode is active.
    pub measuring: bool,
    /// Measurement start point (during active measurement).
    pub measure_start: Option<[f32; 3]>,
    /// Texture paint brush color.
    pub paint_color: [f32; 4],
    /// Texture paint brush radius.
    pub paint_radius: f32,
    /// Weight paint value (0.0-1.0).
    pub weight_value: f32,
    /// Whether lasso select mode is active (L key).
    #[allow(dead_code)]
    pub lasso_active: bool,
    /// Lasso selection points (screen space).
    #[allow(dead_code)]
    pub lasso_points: Vec<[f32; 2]>,
    /// 3D cursor position (Shift+RightClick to place).
    pub cursor_3d: [f32; 3],
    /// Transform pivot point mode.
    pub pivot_point: PivotPoint,
    /// Show normals overlay in viewport.
    pub show_normals: bool,
    /// Show object info overlay (name/dimensions near objects).
    pub show_object_info: bool,
    /// Show orientation cube in corner.
    pub show_orientation_cube: bool,
    /// Transform orientation mode.
    pub transform_orientation: TransformOrientation,
    /// Snap target mode.
    pub snap_target: SnapTarget,
    /// Show camera preview mini-viewport.
    pub show_camera_preview: bool,
    /// Selection history for cycling (last 10 selections).
    #[allow(dead_code)]
    pub selection_history: Vec<usize>,
    /// Clipboard for copy/paste objects.
    pub clipboard: Vec<SceneObject>,
    /// Auto-key mode (automatically insert keyframes on transform).
    pub auto_key: bool,
    /// Whether onion skinning is active in animation.
    pub onion_skinning: bool,
    /// Number of ghost frames for onion skinning.
    pub onion_frames: i32,
    /// Object collections (named groups of object indices).
    pub collections: Vec<ObjectCollection>,
    /// Render region (sub-region of viewport for partial renders).
    #[allow(dead_code)]
    pub render_region: Option<[f32; 4]>,
    /// Show face orientation overlay (front=blue, back=red).
    pub show_face_orientation: bool,
    /// Show edge lengths in Edit mode.
    #[allow(dead_code)]
    pub show_edge_lengths: bool,
    /// Matcap material override for viewport (0=none, 1-6=presets).
    pub matcap_index: usize,
    /// Timeline markers (named frame positions).
    pub timeline_markers: Vec<TimelineMarker>,
    /// Outliner filter (show all, mesh only, lights only, cameras only).
    pub outliner_filter: OutlinerFilter,
    /// Background reference image path (if set).
    #[allow(dead_code)]
    pub background_image: Option<String>,
    /// Environment map / HDRI name (for rendering context).
    #[allow(dead_code)]
    pub environment_hdri: String,
    /// Selection outline thickness.
    #[allow(dead_code)]
    pub selection_outline_width: f32,
    /// Camera render settings (DOF, exposure, etc.).
    pub camera_settings: CameraSettings,
    /// World/environment settings.
    pub world: WorldSettings,
    /// X-ray mode (see through objects).
    pub xray_mode: bool,
    /// Backface culling toggle.
    pub backface_culling: bool,
    /// Viewport near clip distance.
    pub clip_near: f32,
    /// Viewport far clip distance.
    pub clip_far: f32,
    /// Current edit mode tool.
    pub edit_tool: EditTool,
    /// Loop cut segments count.
    pub loop_cut_segments: u32,
    /// Quick favorites.
    #[allow(dead_code)]
    pub quick_favorites: Vec<QuickFavorite>,
    /// Show only render objects (hide empties, lights, cameras in viewport).
    pub show_only_render: bool,
    /// Show relationship lines (parent-child connections).
    pub show_relationship_lines: bool,
    /// Cavity display (ambient occlusion-like edge emphasis).
    pub show_cavity: bool,
    /// Shadow display in viewport.
    pub show_shadows: bool,
    /// Specular lighting in viewport.
    pub show_specular: bool,
    /// Scene properties.
    pub scene_props: SceneProperties,
    /// Viewport gizmo display mode.
    #[allow(dead_code)]
    pub gizmo_mode: GizmoMode,
    /// Viewport overlays configuration.
    pub overlays: ViewportOverlays,
    /// Random selection seed (for Select Random).
    #[allow(dead_code)]
    pub select_random_seed: u32,
    /// Select linked (contiguous mesh elements).
    #[allow(dead_code)]
    pub select_linked: bool,
    /// Active workspace layout.
    pub workspace: WorkspaceLayout,
    /// Color management settings.
    pub color_management: ColorManagement,
    /// Pose mode active (for armature).
    pub pose_mode: bool,
    /// Whether to show bone names in viewport.
    pub show_bone_names: bool,
    /// Whether to show bone axes in viewport.
    pub show_bone_axes: bool,
    /// Grease pencil active layer name.
    #[allow(dead_code)]
    pub gp_active_layer: String,
    /// Grease pencil brush color.
    #[allow(dead_code)]
    pub gp_color: [f32; 4],
    /// Grease pencil brush size.
    #[allow(dead_code)]
    pub gp_size: f32,
    /// Render layers for compositing.
    pub render_layers: Vec<RenderLayer>,
    /// View layers for scene management.
    pub view_layers: Vec<ViewLayer>,
    /// Proportional editing falloff type.
    pub proportional_falloff: ProportionalFalloff,
    /// Asset browser current category.
    pub asset_category: AssetCategory,
    /// Asset browser search string.
    #[allow(dead_code)]
    pub asset_search: String,
    /// Snap element type.
    pub snap_element: SnapElement,
    /// Render engine selection.
    pub render_engine: RenderEngine,
    /// Film transparent (transparent background for rendering).
    pub film_transparent: bool,
    /// Simplify subdivision level for viewport performance.
    pub simplify_subdivision: u32,
    /// Show motion paths in viewport.
    pub show_motion_paths_viewport: bool,
    /// Sequencer strips.
    pub sequencer_strips: Vec<SequencerStrip>,
    /// Performance statistics.
    pub perf_stats: PerformanceStats,
    /// Show performance overlay.
    pub show_perf_overlay: bool,
}

/// Transform pivot point mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PivotPoint {
    #[default]
    MedianPoint,
    IndividualOrigins,
    Cursor3D,
    ActiveElement,
    BoundingBoxCenter,
}

impl std::fmt::Display for PivotPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MedianPoint => write!(f, "Median"),
            Self::IndividualOrigins => write!(f, "Individual"),
            Self::Cursor3D => write!(f, "3D Cursor"),
            Self::ActiveElement => write!(f, "Active"),
            Self::BoundingBoxCenter => write!(f, "BBox"),
        }
    }
}

/// Transform orientation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransformOrientation {
    #[default]
    Global,
    Local,
    Normal,
    Gimbal,
    View,
}

impl std::fmt::Display for TransformOrientation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Global => write!(f, "Global"),
            Self::Local => write!(f, "Local"),
            Self::Normal => write!(f, "Normal"),
            Self::Gimbal => write!(f, "Gimbal"),
            Self::View => write!(f, "View"),
        }
    }
}

/// Snap target mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapTarget {
    #[default]
    Closest,
    Center,
    Median,
    Active,
}

impl std::fmt::Display for SnapTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closest => write!(f, "Closest"),
            Self::Center => write!(f, "Center"),
            Self::Median => write!(f, "Median"),
            Self::Active => write!(f, "Active"),
        }
    }
}

/// Timeline marker (named frame position).
#[derive(Debug, Clone)]
pub struct TimelineMarker {
    pub frame: i32,
    pub name: String,
    #[allow(dead_code)]
    pub color: [f32; 3],
}

/// Outliner filter mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutlinerFilter {
    #[default]
    All,
    MeshOnly,
    LightsOnly,
    CamerasOnly,
    CurvesOnly,
}

impl std::fmt::Display for OutlinerFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All"),
            Self::MeshOnly => write!(f, "Mesh"),
            Self::LightsOnly => write!(f, "Lights"),
            Self::CamerasOnly => write!(f, "Cameras"),
            Self::CurvesOnly => write!(f, "Curves"),
        }
    }
}

/// Camera render settings (DOF, exposure, etc.).
#[derive(Debug, Clone)]
pub struct CameraSettings {
    /// Depth of field enabled.
    pub dof_enabled: bool,
    /// Focal distance (world units).
    pub focal_distance: f32,
    /// Aperture (f-stop).
    pub aperture: f32,
    /// Exposure value (EV).
    pub exposure: f32,
    /// Gamma correction.
    pub gamma: f32,
    /// Sensor size (mm).
    pub sensor_size: f32,
    /// Film gate aspect ratio.
    #[allow(dead_code)]
    pub film_aspect: f32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            dof_enabled: false,
            focal_distance: 5.0,
            aperture: 2.8,
            exposure: 0.0,
            gamma: 2.2,
            sensor_size: 36.0,
            film_aspect: 1.778,
        }
    }
}

/// Particle system attached to an object.
#[derive(Debug, Clone)]
pub struct ParticleSystem {
    pub name: String,
    /// Number of particles.
    pub count: u32,
    /// Particle lifetime (frames).
    pub lifetime: f32,
    /// Emission velocity.
    #[allow(dead_code)]
    pub velocity: [f32; 3],
    /// Gravity influence (0-1).
    pub gravity: f32,
    /// Size of individual particles.
    pub size: f32,
    /// Whether the system is active.
    pub active: bool,
    /// Particle type (Emitter or Hair).
    pub particle_type: ParticleType,
}

impl Default for ParticleSystem {
    fn default() -> Self {
        Self {
            name: "ParticleSystem".to_string(),
            count: 1000,
            lifetime: 50.0,
            velocity: [0.0, 2.0, 0.0],
            gravity: 1.0,
            size: 0.05,
            active: true,
            particle_type: ParticleType::Emitter,
        }
    }
}

/// Particle type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleType {
    Emitter,
    Hair,
}

impl std::fmt::Display for ParticleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Emitter => write!(f, "Emitter"),
            Self::Hair => write!(f, "Hair"),
        }
    }
}

/// World/environment settings.
#[derive(Debug, Clone)]
pub struct WorldSettings {
    /// Sky color (top).
    pub sky_color: [f32; 3],
    /// Horizon color.
    pub horizon_color: [f32; 3],
    /// Ground color (bottom hemisphere).
    pub ground_color: [f32; 3],
    /// Ambient light intensity.
    pub ambient_intensity: f32,
    /// Fog enabled.
    pub fog_enabled: bool,
    /// Fog density.
    pub fog_density: f32,
    /// Fog color.
    pub fog_color: [f32; 3],
    /// Fog start distance.
    pub fog_start: f32,
    /// Fog end distance.
    pub fog_end: f32,
}

impl Default for WorldSettings {
    fn default() -> Self {
        Self {
            sky_color: [0.05, 0.05, 0.12],
            horizon_color: [0.15, 0.15, 0.20],
            ground_color: [0.03, 0.03, 0.05],
            ambient_intensity: 0.3,
            fog_enabled: false,
            fog_density: 0.01,
            fog_color: [0.5, 0.5, 0.6],
            fog_start: 10.0,
            fog_end: 100.0,
        }
    }
}

/// Edit mode tool type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditTool {
    #[default]
    Select,
    Extrude,
    LoopCut,
    Knife,
    BevelEdge,
    InsetFace,
    PolyBuild,
    SpinTool,
}

impl std::fmt::Display for EditTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Select => write!(f, "Select"),
            Self::Extrude => write!(f, "Extrude"),
            Self::LoopCut => write!(f, "Loop Cut"),
            Self::Knife => write!(f, "Knife"),
            Self::BevelEdge => write!(f, "Bevel"),
            Self::InsetFace => write!(f, "Inset"),
            Self::PolyBuild => write!(f, "PolyBuild"),
            Self::SpinTool => write!(f, "Spin"),
        }
    }
}

/// Sub-element selection in edit mode.
#[derive(Debug, Clone, Default)]
pub struct EditModeSelection {
    /// Selected vertex indices.
    pub vertices: Vec<usize>,
    /// Selected edge indices (pairs of vertex indices).
    pub edges: Vec<(usize, usize)>,
    /// Selected face indices (indices into the mesh's face list).
    pub faces: Vec<usize>,
}

/// Mesh data for editing (simplified for now, real impl would use half-edge).
#[derive(Debug, Clone)]
pub struct EditableMesh {
    /// Vertex positions.
    pub vertices: Vec<[f32; 3]>,
    /// Face vertex indices (triangles and quads).
    pub faces: Vec<Vec<usize>>,
    /// Computed edges (cached for edit operations).
    pub edges: Vec<(usize, usize)>,
}

impl EditableMesh {
    /// Create from vertex positions and faces.
    pub fn new(vertices: Vec<[f32; 3]>, faces: Vec<Vec<usize>>) -> Self {
        let edges = Self::compute_edges(&faces);
        Self {
            vertices,
            faces,
            edges,
        }
    }

    /// Compute unique edges from faces.
    fn compute_edges(faces: &[Vec<usize>]) -> Vec<(usize, usize)> {
        let mut edge_set = std::collections::HashSet::new();
        for face in faces {
            for i in 0..face.len() {
                let v1 = face[i];
                let v2 = face[(i + 1) % face.len()];
                let edge = if v1 < v2 { (v1, v2) } else { (v2, v1) };
                edge_set.insert(edge);
            }
        }
        edge_set.into_iter().collect()
    }

    /// Update edge cache after topology change.
    pub fn update_edges(&mut self) {
        self.edges = Self::compute_edges(&self.faces);
    }

    /// Delete selected vertices and update topology.
    pub fn delete_vertices(&mut self, vertex_indices: &[usize]) {
        if vertex_indices.is_empty() {
            return;
        }
        let mut keep_verts = vec![true; self.vertices.len()];
        for &v in vertex_indices {
            if v < keep_verts.len() {
                keep_verts[v] = false;
            }
        }

        // Build vertex remapping (old index -> new index).
        let mut remap = vec![0; self.vertices.len()];
        let mut new_idx = 0;
        for (old_idx, &keep) in keep_verts.iter().enumerate() {
            if keep {
                remap[old_idx] = new_idx;
                new_idx += 1;
            }
        }

        // Filter vertices.
        let mut new_vertices = Vec::new();
        for (v, &keep) in self.vertices.iter().zip(&keep_verts) {
            if keep {
                new_vertices.push(*v);
            }
        }
        self.vertices = new_vertices;

        // Update faces and remove degenerate ones.
        let mut new_faces = Vec::new();
        for face in &self.faces {
            let mut new_face = Vec::new();
            for &v in face {
                if v < keep_verts.len() && keep_verts[v] {
                    new_face.push(remap[v]);
                }
            }
            // Keep face if it has at least 3 vertices.
            if new_face.len() >= 3 {
                new_faces.push(new_face);
            }
        }
        self.faces = new_faces;
        self.update_edges();
    }

    /// Merge vertices at first selected vertex's position.
    pub fn merge_vertices(&mut self, vertex_indices: &[usize]) {
        if vertex_indices.len() < 2 {
            return;
        }
        let target = vertex_indices[0];
        if target >= self.vertices.len() {
            return;
        }

        // Remap all selected vertices to the target.
        let mut remap = (0..self.vertices.len()).collect::<Vec<_>>();
        for &v in &vertex_indices[1..] {
            if v < remap.len() {
                remap[v] = target;
            }
        }

        // Update face indices.
        for face in &mut self.faces {
            for v in face {
                *v = remap[*v];
            }
        }

        // Remove duplicate vertices and remap again.
        let mut seen = std::collections::HashSet::new();
        let mut final_remap = vec![0; self.vertices.len()];
        let mut new_vertices = Vec::new();
        let mut new_idx = 0;
        for (old_idx, &v_pos) in self.vertices.iter().enumerate() {
            if !seen.contains(&old_idx) || old_idx == target {
                seen.insert(old_idx);
                final_remap[old_idx] = new_idx;
                new_vertices.push(v_pos);
                new_idx += 1;
            } else {
                final_remap[old_idx] = final_remap[target];
            }
        }
        self.vertices = new_vertices;

        // Apply final remap to faces.
        for face in &mut self.faces {
            for v in face {
                *v = final_remap[*v];
            }
        }

        // Remove degenerate faces.
        self.faces.retain(|face| {
            let unique: std::collections::HashSet<_> = face.iter().collect();
            unique.len() >= 3
        });
        self.update_edges();
    }

    /// Subdivide selected edges (split each edge in half).
    pub fn subdivide_edges(&mut self, edge_indices: &[usize]) {
        if edge_indices.is_empty() {
            return;
        }

        // Create midpoint vertices for each selected edge.
        let mut new_verts = Vec::new();
        let mut edge_to_midpoint = std::collections::HashMap::new();
        for &edge_idx in edge_indices {
            if edge_idx >= self.edges.len() {
                continue;
            }
            let (v1, v2) = self.edges[edge_idx];
            if v1 >= self.vertices.len() || v2 >= self.vertices.len() {
                continue;
            }

            let p1 = self.vertices[v1];
            let p2 = self.vertices[v2];
            let mid = [
                (p1[0] + p2[0]) * 0.5,
                (p1[1] + p2[1]) * 0.5,
                (p1[2] + p2[2]) * 0.5,
            ];
            let mid_idx = self.vertices.len() + new_verts.len();
            new_verts.push(mid);
            edge_to_midpoint.insert(self.edges[edge_idx], mid_idx);
        }
        self.vertices.extend(new_verts);

        // Split faces that contain subdivided edges.
        let mut new_faces = Vec::new();
        for face in &self.faces {
            let mut has_subdivided = false;
            let mut split_points = Vec::new();
            for i in 0..face.len() {
                let v1 = face[i];
                let v2 = face[(i + 1) % face.len()];
                let edge = if v1 < v2 { (v1, v2) } else { (v2, v1) };
                if let Some(&mid) = edge_to_midpoint.get(&edge) {
                    has_subdivided = true;
                    split_points.push((i, mid));
                }
            }

            if !has_subdivided {
                new_faces.push(face.clone());
            } else {
                // Simple quad subdivision: if face is quad and has 2 opposite edges subdivided.
                // For now, just triangulate the face with midpoints.
                // NOTE: For proper Catmull-Clark subdivision, use subdivide_catmull_clark() instead.
                let mut new_face = Vec::new();
                for i in 0..face.len() {
                    new_face.push(face[i]);
                    let v1 = face[i];
                    let v2 = face[(i + 1) % face.len()];
                    let edge = if v1 < v2 { (v1, v2) } else { (v2, v1) };
                    if let Some(&mid) = edge_to_midpoint.get(&edge) {
                        new_face.push(mid);
                    }
                }
                // Split into triangles from first vertex.
                for i in 2..new_face.len() {
                    new_faces.push(vec![new_face[0], new_face[i - 1], new_face[i]]);
                }
            }
        }
        self.faces = new_faces;
        self.update_edges();
    }

    /// Apply proper Catmull-Clark subdivision to the entire mesh.
    /// This replaces the TODO in subdivide_edges() with a complete implementation.
    pub fn subdivide_catmull_clark(&mut self) {
        use std::collections::HashMap;

        if self.faces.is_empty() {
            return;
        }

        // Build edge map for fast lookup
        let mut edges: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        for (face_idx, face) in self.faces.iter().enumerate() {
            for i in 0..face.len() {
                let v0 = face[i];
                let v1 = face[(i + 1) % face.len()];
                let edge_key = if v0 < v1 { (v0, v1) } else { (v1, v0) };
                edges.entry(edge_key).or_default().push(face_idx);
            }
        }

        // Step 1: Compute face points (centroid of each face)
        let mut face_points = Vec::new();
        for face in &self.faces {
            if face.is_empty() {
                face_points.push([0.0, 0.0, 0.0]);
                continue;
            }

            let mut centroid = [0.0, 0.0, 0.0];
            for &vi in face {
                if vi < self.vertices.len() {
                    centroid[0] += self.vertices[vi][0];
                    centroid[1] += self.vertices[vi][1];
                    centroid[2] += self.vertices[vi][2];
                }
            }
            let n = face.len() as f32;
            centroid[0] /= n;
            centroid[1] /= n;
            centroid[2] /= n;
            face_points.push(centroid);
        }

        // Step 2: Compute edge points
        let mut edge_points: HashMap<(usize, usize), [f32; 3]> = HashMap::new();
        for (edge_key, face_indices) in &edges {
            let v0 = self.vertices[edge_key.0];
            let v1 = self.vertices[edge_key.1];

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

        // Step 3: Update original vertices using Catmull-Clark formula
        let mut new_vertices = Vec::new();
        for (vi, v) in self.vertices.iter().enumerate() {
            if is_vertex_boundary(vi) {
                // Boundary vertex: keep original position
                new_vertices.push(*v);
            } else {
                // Interior vertex: (F + 2R + (n-3)P) / n
                // F = average of adjacent face points
                // R = average of adjacent edge midpoints
                // P = original vertex
                // n = valence

                let mut face_sum = [0.0, 0.0, 0.0];
                let mut edge_sum = [0.0, 0.0, 0.0];
                let mut valence = 0;

                // Find adjacent faces
                for (face_idx, face) in self.faces.iter().enumerate() {
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
                        if other < self.vertices.len() {
                            edge_sum[0] += self.vertices[other][0];
                            edge_sum[1] += self.vertices[other][1];
                            edge_sum[2] += self.vertices[other][2];
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
                    new_vertices.push(new_pos);
                } else {
                    new_vertices.push(*v);
                }
            }
        }

        // Step 4: Build new mesh
        self.vertices = new_vertices;

        // Add edge points to vertex list and map them
        let mut edge_to_idx: HashMap<(usize, usize), usize> = HashMap::new();
        for (edge_key, point) in &edge_points {
            let idx = self.vertices.len();
            self.vertices.push(*point);
            edge_to_idx.insert(*edge_key, idx);
        }

        // Add face points to vertex list and map them
        let mut face_to_idx: HashMap<usize, usize> = HashMap::new();
        for (face_idx, point) in face_points.iter().enumerate() {
            let idx = self.vertices.len();
            self.vertices.push(*point);
            face_to_idx.insert(face_idx, idx);
        }

        // Step 5: Create new faces (quads from each face corner)
        let old_faces = std::mem::take(&mut self.faces);
        for (face_idx, face) in old_faces.iter().enumerate() {
            let face_point_idx = face_to_idx[&face_idx];

            for i in 0..face.len() {
                let v0 = face[i];
                let v1 = face[(i + 1) % face.len()];
                let v2 = face[(i + face.len() - 1) % face.len()];

                let edge_key1 = if v0 < v1 { (v0, v1) } else { (v1, v0) };
                let edge_point1 = edge_to_idx[&edge_key1];

                let edge_key2 = if v2 < v0 { (v2, v0) } else { (v0, v2) };
                let edge_point2 = edge_to_idx[&edge_key2];

                // Create quad: original vertex, edge point, face point, previous edge point
                self.faces
                    .push(vec![v0, edge_point1, face_point_idx, edge_point2]);
            }
        }

        self.update_edges();
    }

    /// Extrude selected faces (duplicate and offset).
    pub fn extrude_faces(&mut self, face_indices: &[usize], offset: [f32; 3]) {
        if face_indices.is_empty() {
            return;
        }

        // Collect unique vertices from selected faces.
        let mut vert_set = std::collections::HashSet::new();
        for &face_idx in face_indices {
            if face_idx >= self.faces.len() {
                continue;
            }
            for &v in &self.faces[face_idx] {
                vert_set.insert(v);
            }
        }
        let orig_verts: Vec<_> = vert_set.into_iter().collect();

        // Duplicate vertices and offset them.
        let mut vert_remap = std::collections::HashMap::new();
        for &v in &orig_verts {
            if v >= self.vertices.len() {
                continue;
            }
            let mut new_pos = self.vertices[v];
            new_pos[0] += offset[0];
            new_pos[1] += offset[1];
            new_pos[2] += offset[2];
            let new_idx = self.vertices.len();
            self.vertices.push(new_pos);
            vert_remap.insert(v, new_idx);
        }

        // Create side faces (connect original and extruded vertices).
        let mut side_faces = Vec::new();
        for &face_idx in face_indices {
            if face_idx >= self.faces.len() {
                continue;
            }
            let face = &self.faces[face_idx];
            for i in 0..face.len() {
                let v1 = face[i];
                let v2 = face[(i + 1) % face.len()];
                if let (Some(&new_v1), Some(&new_v2)) = (vert_remap.get(&v1), vert_remap.get(&v2)) {
                    // Create quad connecting original edge to extruded edge.
                    side_faces.push(vec![v1, v2, new_v2, new_v1]);
                }
            }
        }

        // Update selected faces to use new vertices.
        for &face_idx in face_indices {
            if face_idx >= self.faces.len() {
                continue;
            }
            let face = &mut self.faces[face_idx];
            for v in face {
                if let Some(&new_v) = vert_remap.get(v) {
                    *v = new_v;
                }
            }
        }

        self.faces.extend(side_faces);
        self.update_edges();
    }

    /// Inset selected faces (shrink toward center).
    pub fn inset_faces(&mut self, face_indices: &[usize], inset_amount: f32) {
        if face_indices.is_empty() || inset_amount <= 0.0 {
            return;
        }

        for &face_idx in face_indices {
            if face_idx >= self.faces.len() {
                continue;
            }
            let face = &self.faces[face_idx];
            if face.len() < 3 {
                continue;
            }

            // Compute face center.
            let mut center = [0.0, 0.0, 0.0];
            for &v in face {
                if v >= self.vertices.len() {
                    continue;
                }
                center[0] += self.vertices[v][0];
                center[1] += self.vertices[v][1];
                center[2] += self.vertices[v][2];
            }
            let count = face.len() as f32;
            center[0] /= count;
            center[1] /= count;
            center[2] /= count;

            // Create inset vertices.
            let mut inset_verts = Vec::new();
            for &v in face {
                if v >= self.vertices.len() {
                    continue;
                }
                let pos = self.vertices[v];
                let dir = [center[0] - pos[0], center[1] - pos[1], center[2] - pos[2]];
                let new_pos = [
                    pos[0] + dir[0] * inset_amount,
                    pos[1] + dir[1] * inset_amount,
                    pos[2] + dir[2] * inset_amount,
                ];
                let new_idx = self.vertices.len() + inset_verts.len();
                self.vertices.push(new_pos);
                inset_verts.push(new_idx);
            }

            // Create bridge quads between original and inset.
            let mut bridge_faces = Vec::new();
            for i in 0..face.len() {
                let v1 = face[i];
                let v2 = face[(i + 1) % face.len()];
                let new_v1 = inset_verts[i];
                let new_v2 = inset_verts[(i + 1) % inset_verts.len()];
                bridge_faces.push(vec![v1, v2, new_v2, new_v1]);
            }

            // Replace original face with inset face.
            self.faces[face_idx] = inset_verts;
            self.faces.extend(bridge_faces);
        }
        self.update_edges();
    }

    /// Delete selected faces.
    pub fn delete_faces(&mut self, face_indices: &[usize]) {
        if face_indices.is_empty() {
            return;
        }
        let mut keep = vec![true; self.faces.len()];
        for &f in face_indices {
            if f < keep.len() {
                keep[f] = false;
            }
        }
        let mut new_faces = Vec::new();
        for (face, &k) in self.faces.iter().zip(&keep) {
            if k {
                new_faces.push(face.clone());
            }
        }
        self.faces = new_faces;
        self.update_edges();
    }

    /// Delete edges and the faces that contain them.
    pub fn delete_edges(&mut self, edge_indices: &[usize]) {
        if edge_indices.is_empty() {
            return;
        }

        // Get the actual edge pairs from indices
        let mut edges_to_delete = Vec::new();
        for &edge_idx in edge_indices {
            if edge_idx < self.edges.len() {
                edges_to_delete.push(self.edges[edge_idx]);
            }
        }

        // Find faces that contain any of these edges and mark for deletion
        let mut keep = vec![true; self.faces.len()];
        for (face_idx, face) in self.faces.iter().enumerate() {
            // Check if this face contains any edge to delete
            for &(v1, v2) in &edges_to_delete {
                // Check if this edge exists in the face
                let mut found_v1 = false;
                let mut found_v2 = false;
                for &v in face {
                    if v == v1 {
                        found_v1 = true;
                    }
                    if v == v2 {
                        found_v2 = true;
                    }
                }
                if found_v1 && found_v2 {
                    keep[face_idx] = false;
                    break;
                }
            }
        }

        // Remove marked faces
        let mut new_faces = Vec::new();
        for (face, &k) in self.faces.iter().zip(&keep) {
            if k {
                new_faces.push(face.clone());
            }
        }
        self.faces = new_faces;
        self.update_edges();
    }
}

/// Quick favorites (saved frequently used operations).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct QuickFavorite {
    pub name: String,
    pub action: String,
}

/// Scene properties (global scene metadata).
#[derive(Debug, Clone)]
pub struct SceneProperties {
    /// Scene name.
    pub name: String,
    /// Scene unit scale (1.0 = meters).
    pub unit_scale: f32,
    /// Scene unit name.
    pub unit_name: String,
    /// Gravity vector.
    pub gravity: [f32; 3],
    /// Scene frame rate (for rendering).
    pub render_fps: f32,
    /// Audio sync enabled.
    #[allow(dead_code)]
    pub audio_sync: bool,
    /// Active camera index.
    pub active_camera: Option<usize>,
}

impl Default for SceneProperties {
    fn default() -> Self {
        Self {
            name: "Scene".to_string(),
            unit_scale: 1.0,
            unit_name: "Meters".to_string(),
            gravity: [0.0, -9.81, 0.0],
            render_fps: 24.0,
            audio_sync: false,
            active_camera: None,
        }
    }
}

/// Align operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignAxis {
    AlignX,
    AlignY,
    AlignZ,
    DistributeX,
    DistributeY,
    DistributeZ,
    CenterToWorld,
    CenterToActive,
    SnapToGrid,
    SnapToGround,
}

impl std::fmt::Display for AlignAxis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlignX => write!(f, "Align X"),
            Self::AlignY => write!(f, "Align Y"),
            Self::AlignZ => write!(f, "Align Z"),
            Self::DistributeX => write!(f, "Distribute X"),
            Self::DistributeY => write!(f, "Distribute Y"),
            Self::DistributeZ => write!(f, "Distribute Z"),
            Self::CenterToWorld => write!(f, "Center to World"),
            Self::CenterToActive => write!(f, "Center to Active"),
            Self::SnapToGrid => write!(f, "Snap to Grid"),
            Self::SnapToGround => write!(f, "Snap to Ground"),
        }
    }
}

/// Viewport gizmo display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
#[derive(Default)]
pub enum GizmoMode {
    None,
    Translate,
    Rotate,
    Scale,
    #[default]
    Combined,
}

impl std::fmt::Display for GizmoMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Translate => write!(f, "Move"),
            Self::Rotate => write!(f, "Rotate"),
            Self::Scale => write!(f, "Scale"),
            Self::Combined => write!(f, "Combined"),
        }
    }
}

/// Armature bone for skeletal animation.
#[derive(Debug, Clone)]
pub struct ArmatureBone {
    /// Bone name (unique within armature).
    pub name: String,
    /// Head position (local space).
    pub head: [f32; 3],
    /// Tail position (local space).
    pub tail: [f32; 3],
    /// Parent bone name (None for root).
    pub parent: Option<String>,
    /// Whether bone is connected to parent tail.
    #[allow(dead_code)]
    pub connected: bool,
    /// Bone roll angle (twist around bone axis).
    #[allow(dead_code)]
    pub roll: f32,
    /// Bone display size.
    #[allow(dead_code)]
    pub display_size: f32,
    /// Inverse kinematics enabled for this bone.
    pub ik_enabled: bool,
    /// IK chain length (0 = auto).
    #[allow(dead_code)]
    pub ik_chain_length: u32,
}

impl Default for ArmatureBone {
    fn default() -> Self {
        Self {
            name: "Bone".to_string(),
            head: [0.0, 0.0, 0.0],
            tail: [0.0, 1.0, 0.0],
            parent: None,
            connected: false,
            roll: 0.0,
            display_size: 0.1,
            ik_enabled: false,
            ik_chain_length: 0,
        }
    }
}

/// NLA (Non-Linear Animation) strip.
#[derive(Debug, Clone)]
pub struct NLAStrip {
    /// Strip name.
    pub name: String,
    /// Action name this strip references.
    #[allow(dead_code)]
    pub action_name: String,
    /// Start frame (on NLA timeline).
    pub start_frame: i32,
    /// End frame (on NLA timeline).
    pub end_frame: i32,
    /// Blend in/out frames.
    #[allow(dead_code)]
    pub blend_in: f32,
    #[allow(dead_code)]
    pub blend_out: f32,
    /// Repeat count (1.0 = once).
    pub repeat: f32,
    /// Scale factor for strip speed.
    #[allow(dead_code)]
    pub scale: f32,
    /// Whether strip is muted.
    pub muted: bool,
}

impl Default for NLAStrip {
    fn default() -> Self {
        Self {
            name: "Strip".to_string(),
            action_name: "Action".to_string(),
            start_frame: 1,
            end_frame: 250,
            blend_in: 0.0,
            blend_out: 0.0,
            repeat: 1.0,
            scale: 1.0,
            muted: false,
        }
    }
}

/// NLA track (holds strips).
#[derive(Debug, Clone)]
pub struct NLATrack {
    pub name: String,
    pub strips: Vec<NLAStrip>,
    pub muted: bool,
    pub solo: bool,
}

impl Default for NLATrack {
    fn default() -> Self {
        Self {
            name: "NLA Track".to_string(),
            strips: Vec::new(),
            muted: false,
            solo: false,
        }
    }
}

/// Animation driver (property linking).
#[derive(Debug, Clone)]
pub struct AnimationDriver {
    /// Human-readable name.
    pub name: String,
    /// Source object index.
    pub source_object: usize,
    /// Source property name (e.g., "position.x", "rotation.z").
    pub source_property: String,
    /// Target property name on the driven object.
    pub target_property: String,
    /// Multiplier applied to the source value.
    pub influence: f32,
    /// Expression type (Direct, Scripted).
    pub driver_type: DriverType,
    /// Whether the driver is enabled.
    pub enabled: bool,
}

/// Driver expression type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverType {
    /// Direct value mapping (source -> target * influence).
    Direct,
    /// Sum of values.
    Sum,
    /// Average of values.
    Average,
    /// Minimum of values.
    Min,
    /// Maximum of values.
    Max,
}

impl std::fmt::Display for DriverType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => write!(f, "Direct"),
            Self::Sum => write!(f, "Sum"),
            Self::Average => write!(f, "Average"),
            Self::Min => write!(f, "Min"),
            Self::Max => write!(f, "Max"),
        }
    }
}

/// Force field type for physics simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForceFieldType {
    Wind,
    Vortex,
    Turbulence,
    Drag,
    Magnetic,
    Harmonic,
    Charge,
    Lennard,
}

impl std::fmt::Display for ForceFieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wind => write!(f, "Wind"),
            Self::Vortex => write!(f, "Vortex"),
            Self::Turbulence => write!(f, "Turbulence"),
            Self::Drag => write!(f, "Drag"),
            Self::Magnetic => write!(f, "Magnetic"),
            Self::Harmonic => write!(f, "Harmonic"),
            Self::Charge => write!(f, "Charge"),
            Self::Lennard => write!(f, "Lennard-Jones"),
        }
    }
}

/// Force field settings for an object.
#[derive(Debug, Clone)]
pub struct ForceFieldSettings {
    pub field_type: ForceFieldType,
    pub strength: f32,
    pub falloff: f32,
    pub noise: f32,
    pub flow: f32,
    pub enabled: bool,
}

impl Default for ForceFieldSettings {
    fn default() -> Self {
        Self {
            field_type: ForceFieldType::Wind,
            strength: 1.0,
            falloff: 2.0,
            noise: 0.0,
            flow: 0.0,
            enabled: true,
        }
    }
}

/// Cloth simulation settings.
#[derive(Debug, Clone)]
pub struct ClothSettings {
    pub quality: u32,
    pub mass: f32,
    pub stiffness: f32,
    pub damping: f32,
    pub air_resistance: f32,
    pub self_collision: bool,
    pub pressure: f32,
    #[allow(dead_code)]
    pub pinned_group: String,
    pub enabled: bool,
}

impl Default for ClothSettings {
    fn default() -> Self {
        Self {
            quality: 5,
            mass: 0.3,
            stiffness: 15.0,
            damping: 5.0,
            air_resistance: 1.0,
            self_collision: false,
            pressure: 0.0,
            pinned_group: String::new(),
            enabled: true,
        }
    }
}

/// Soft body simulation settings.
#[derive(Debug, Clone)]
pub struct SoftBodySettings {
    pub mass: f32,
    pub friction: f32,
    pub speed: f32,
    pub goal_strength: f32,
    pub edge_stiffness: f32,
    pub push: f32,
    pub pull: f32,
    pub damping: f32,
    pub self_collision: bool,
    pub enabled: bool,
}

impl Default for SoftBodySettings {
    fn default() -> Self {
        Self {
            mass: 1.0,
            friction: 0.5,
            speed: 1.0,
            goal_strength: 0.7,
            edge_stiffness: 0.8,
            push: 0.5,
            pull: 0.5,
            damping: 0.5,
            self_collision: false,
            enabled: true,
        }
    }
}

/// Workspace layout presets (like Blender).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceLayout {
    Modeling,
    Sculpting,
    UVEditing,
    TexturePaint,
    Animation,
    Compositing,
    Rendering,
    Scripting,
}

impl std::fmt::Display for WorkspaceLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Modeling => write!(f, "Modeling"),
            Self::Sculpting => write!(f, "Sculpting"),
            Self::UVEditing => write!(f, "UV Editing"),
            Self::TexturePaint => write!(f, "Texture Paint"),
            Self::Animation => write!(f, "Animation"),
            Self::Compositing => write!(f, "Compositing"),
            Self::Rendering => write!(f, "Rendering"),
            Self::Scripting => write!(f, "Scripting"),
        }
    }
}

/// Color management settings (OCIO-style).
#[derive(Debug, Clone)]
pub struct ColorManagement {
    /// Display device (sRGB, P3, Rec.2020).
    pub display_device: String,
    /// View transform (Standard, Filmic, ACEScg, Raw, False Color).
    pub view_transform: String,
    /// Look modifier (None, High Contrast, Medium Contrast, etc.).
    pub look: String,
    /// Exposure compensation (stops).
    pub exposure: f32,
    /// Gamma correction.
    pub gamma: f32,
    /// Sequencer color space.
    pub sequencer_space: String,
}

impl Default for ColorManagement {
    fn default() -> Self {
        Self {
            display_device: "sRGB".to_string(),
            view_transform: "Filmic".to_string(),
            look: "None".to_string(),
            exposure: 0.0,
            gamma: 1.0,
            sequencer_space: "sRGB".to_string(),
        }
    }
}

/// Grease pencil stroke (2D annotation).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GreasePencilStroke {
    /// Points (x, y screen-space).
    pub points: Vec<[f32; 2]>,
    /// Stroke color.
    pub color: [f32; 4],
    /// Line width.
    pub width: f32,
    /// Layer name.
    pub layer: String,
    /// Frame this stroke belongs to.
    pub frame: i32,
}

/// Texture type for material slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureType {
    Diffuse,
    Normal,
    Roughness,
    Metallic,
    AmbientOcclusion,
    Emissive,
    Height,
    Opacity,
}

impl std::fmt::Display for TextureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diffuse => write!(f, "Diffuse"),
            Self::Normal => write!(f, "Normal"),
            Self::Roughness => write!(f, "Roughness"),
            Self::Metallic => write!(f, "Metallic"),
            Self::AmbientOcclusion => write!(f, "AO"),
            Self::Emissive => write!(f, "Emissive"),
            Self::Height => write!(f, "Height"),
            Self::Opacity => write!(f, "Opacity"),
        }
    }
}

/// Texture slot on a material.
#[derive(Debug, Clone)]
pub struct TextureSlot {
    /// Texture type (diffuse, normal, etc.).
    pub texture_type: TextureType,
    /// Image file path.
    pub image_path: String,
    /// UV channel index.
    pub uv_channel: u32,
    /// Texture strength/influence (0-1).
    pub strength: f32,
    /// Whether this slot is enabled.
    pub enabled: bool,
}

impl Default for TextureSlot {
    fn default() -> Self {
        Self {
            texture_type: TextureType::Diffuse,
            image_path: String::new(),
            uv_channel: 0,
            strength: 1.0,
            enabled: true,
        }
    }
}

/// Render pass type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPass {
    Combined,
    Diffuse,
    Glossy,
    Transmission,
    Emission,
    AO,
    Shadow,
    Normal,
    Depth,
    Mist,
    ObjectIndex,
    MaterialIndex,
}

impl std::fmt::Display for RenderPass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Combined => write!(f, "Combined"),
            Self::Diffuse => write!(f, "Diffuse"),
            Self::Glossy => write!(f, "Glossy"),
            Self::Transmission => write!(f, "Transmission"),
            Self::Emission => write!(f, "Emission"),
            Self::AO => write!(f, "Ambient Occlusion"),
            Self::Shadow => write!(f, "Shadow"),
            Self::Normal => write!(f, "Normal"),
            Self::Depth => write!(f, "Depth"),
            Self::Mist => write!(f, "Mist"),
            Self::ObjectIndex => write!(f, "Object Index"),
            Self::MaterialIndex => write!(f, "Material Index"),
        }
    }
}

/// Render layer (collection of passes).
#[derive(Debug, Clone)]
pub struct RenderLayer {
    /// Layer name.
    pub name: String,
    /// Enabled passes.
    pub passes: Vec<RenderPass>,
    /// Whether this layer is active.
    pub enabled: bool,
    /// Layer samples override (0 = use scene).
    pub samples_override: u32,
}

impl Default for RenderLayer {
    fn default() -> Self {
        Self {
            name: "RenderLayer".to_string(),
            passes: vec![RenderPass::Combined],
            enabled: true,
            samples_override: 0,
        }
    }
}

/// View layer (scene subset for rendering).
#[derive(Debug, Clone)]
pub struct ViewLayer {
    /// Layer name.
    pub name: String,
    /// Object indices included in this view layer.
    #[allow(dead_code)]
    pub object_indices: Vec<usize>,
    /// Whether this layer is the active one.
    pub active: bool,
    /// Use for rendering.
    pub use_for_rendering: bool,
}

impl Default for ViewLayer {
    fn default() -> Self {
        Self {
            name: "ViewLayer".to_string(),
            object_indices: Vec::new(),
            active: true,
            use_for_rendering: true,
        }
    }
}

/// Motion path point for animation trail.
#[derive(Debug, Clone)]
pub struct MotionPathPoint {
    /// World position.
    pub position: [f32; 3],
    /// Frame number.
    pub frame: i32,
}

/// Motion path for an object.
#[derive(Debug, Clone)]
pub struct MotionPath {
    /// Path points (position per frame).
    pub points: Vec<MotionPathPoint>,
    /// Display color.
    pub color: [f32; 3],
    /// Start frame.
    pub start_frame: i32,
    /// End frame.
    pub end_frame: i32,
    /// Whether to show frame numbers.
    #[allow(dead_code)]
    pub show_frame_numbers: bool,
}

/// Proportional editing falloff type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProportionalFalloff {
    #[default]
    Smooth,
    Sphere,
    Root,
    InverseSquare,
    Sharp,
    Linear,
    Constant,
    Random,
}

impl std::fmt::Display for ProportionalFalloff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Smooth => write!(f, "Smooth"),
            Self::Sphere => write!(f, "Sphere"),
            Self::Root => write!(f, "Root"),
            Self::InverseSquare => write!(f, "Inverse Square"),
            Self::Sharp => write!(f, "Sharp"),
            Self::Linear => write!(f, "Linear"),
            Self::Constant => write!(f, "Constant"),
            Self::Random => write!(f, "Random"),
        }
    }
}

/// Custom property (user-defined metadata on objects).
#[derive(Debug, Clone)]
pub struct CustomProperty {
    /// Property name.
    pub name: String,
    /// Property value as string.
    pub value: String,
    /// Property type hint.
    pub prop_type: CustomPropType,
}

/// Custom property type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomPropType {
    String,
    Integer,
    Float,
    Boolean,
}

impl std::fmt::Display for CustomPropType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String => write!(f, "String"),
            Self::Integer => write!(f, "Integer"),
            Self::Float => write!(f, "Float"),
            Self::Boolean => write!(f, "Boolean"),
        }
    }
}

/// Asset browser category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetCategory {
    Materials,
    Objects,
    Worlds,
    Actions,
    NodeGroups,
}

impl std::fmt::Display for AssetCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Materials => write!(f, "Materials"),
            Self::Objects => write!(f, "Objects"),
            Self::Worlds => write!(f, "Worlds"),
            Self::Actions => write!(f, "Actions"),
            Self::NodeGroups => write!(f, "Node Groups"),
        }
    }
}

/// Snap element type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapElement {
    #[default]
    Increment,
    Vertex,
    Edge,
    Face,
    Volume,
    EdgeCenter,
    EdgePerpendicular,
}

impl std::fmt::Display for SnapElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Increment => write!(f, "Increment"),
            Self::Vertex => write!(f, "Vertex"),
            Self::Edge => write!(f, "Edge"),
            Self::Face => write!(f, "Face"),
            Self::Volume => write!(f, "Volume"),
            Self::EdgeCenter => write!(f, "Edge Center"),
            Self::EdgePerpendicular => write!(f, "Edge Perp."),
        }
    }
}

/// Render engine type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderEngine {
    #[default]
    Eevee,
    Cycles,
    Workbench,
    /// Neural Radiance Fields volumetric renderer (Mildenhall et al. 2020).
    NeRF,
    /// Neural Radiance Cache MLP for real-time global illumination (Müller et al. 2021).
    NeuralCache,
}

impl std::fmt::Display for RenderEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eevee => write!(f, "Eevee"),
            Self::Cycles => write!(f, "Cycles"),
            Self::Workbench => write!(f, "Workbench"),
            Self::NeRF => write!(f, "NeRF (Neural Radiance Fields)"),
            Self::NeuralCache => write!(f, "Neural Cache (NRC)"),
        }
    }
}

/// Hair particle advanced settings.
#[derive(Debug, Clone)]
pub struct HairSettings {
    /// Hair strand length.
    pub length: f32,
    /// Number of children per parent.
    pub children: u32,
    /// Clump factor (0-1).
    pub clump: f32,
    /// Roughness (0-1).
    pub roughness: f32,
    /// Random seed.
    pub random_seed: u32,
    /// Radius of root.
    pub root_radius: f32,
    /// Radius of tip.
    pub tip_radius: f32,
    /// Number of render steps (segments per strand).
    pub render_steps: u32,
    /// Use hair dynamics.
    pub dynamics: bool,
}

impl Default for HairSettings {
    fn default() -> Self {
        Self {
            length: 0.5,
            children: 0,
            clump: 0.0,
            roughness: 0.0,
            random_seed: 0,
            root_radius: 0.01,
            tip_radius: 0.001,
            render_steps: 3,
            dynamics: false,
        }
    }
}

/// Fluid simulation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FluidType {
    Domain,
    Inflow,
    Outflow,
    Obstacle,
}

impl std::fmt::Display for FluidType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Domain => write!(f, "Domain"),
            Self::Inflow => write!(f, "Inflow"),
            Self::Outflow => write!(f, "Outflow"),
            Self::Obstacle => write!(f, "Obstacle"),
        }
    }
}

/// Fluid simulation settings.
#[derive(Debug, Clone)]
pub struct FluidSettings {
    /// Fluid type.
    pub fluid_type: FluidType,
    /// Domain resolution.
    pub resolution: u32,
    /// Viscosity.
    pub viscosity: f32,
    /// Time scale.
    pub time_scale: f32,
    /// Diffusion factor.
    #[allow(dead_code)]
    pub diffusion: f32,
    /// Surface tension.
    #[allow(dead_code)]
    pub surface_tension: f32,
    /// Cache baked.
    pub baked: bool,
    /// Enabled.
    pub enabled: bool,
}

impl Default for FluidSettings {
    fn default() -> Self {
        Self {
            fluid_type: FluidType::Domain,
            resolution: 64,
            viscosity: 0.001,
            time_scale: 1.0,
            diffusion: 0.0,
            surface_tension: 0.0,
            baked: false,
            enabled: true,
        }
    }
}

/// Sequencer strip type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SequencerStripType {
    Image,
    Movie,
    Sound,
    Scene,
    Color,
    Text,
    Adjustment,
}

impl std::fmt::Display for SequencerStripType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Image => write!(f, "Image"),
            Self::Movie => write!(f, "Movie"),
            Self::Sound => write!(f, "Sound"),
            Self::Scene => write!(f, "Scene"),
            Self::Color => write!(f, "Color"),
            Self::Text => write!(f, "Text"),
            Self::Adjustment => write!(f, "Adjustment"),
        }
    }
}

/// Sequencer strip.
#[derive(Debug, Clone)]
pub struct SequencerStrip {
    /// Strip name.
    pub name: String,
    /// Strip type.
    pub strip_type: SequencerStripType,
    /// Start frame.
    pub start_frame: i32,
    /// Duration (frames).
    pub duration: i32,
    /// Channel (track).
    pub channel: u32,
    /// Muted.
    pub muted: bool,
    /// Opacity/Volume (0-1).
    pub blend: f32,
}

/// Performance stats.
#[derive(Debug, Clone, Default)]
pub struct PerformanceStats {
    /// FPS history (last 60 values).
    pub fps_history: Vec<f32>,
    /// Total vertices in scene.
    pub total_vertices: u32,
    /// Total faces in scene.
    pub total_faces: u32,
    /// Total edges in scene.
    #[allow(dead_code)]
    pub total_edges: u32,
    /// Estimated memory usage (MB).
    #[allow(dead_code)]
    pub memory_mb: f32,
    /// Draw calls estimate.
    pub draw_calls: u32,
}

/// Viewport overlay configuration.
#[derive(Debug, Clone)]
pub struct ViewportOverlays {
    /// Show floor grid.
    pub show_grid: bool,
    /// Show axis lines (X=red, Y=green, Z=blue).
    pub show_axes: bool,
    /// Show wireframe on top of solid.
    pub wireframe_on_solid: bool,
    /// Grid floor opacity (0-1).
    pub grid_opacity: f32,
    /// Show motion paths for animated objects.
    pub show_motion_paths: bool,
    /// Show armature bones (when implemented).
    #[allow(dead_code)]
    pub show_bones: bool,
    /// Show annotations.
    #[allow(dead_code)]
    pub show_annotations: bool,
}

/// Object collection (named group of objects).
#[derive(Debug, Clone)]
pub struct ObjectCollection {
    pub name: String,
    pub object_indices: Vec<usize>,
    pub visible: bool,
    #[allow(dead_code)]
    pub color: [f32; 3],
}

/// Vertex group for weight painting and deformation.
#[derive(Debug, Clone)]
pub struct VertexGroup {
    pub name: String,
    pub weights: Vec<(usize, f32)>, // (vertex_index, weight)
}

/// Saved camera view bookmark.
#[derive(Debug, Clone)]
pub struct CameraBookmark {
    pub orbit_angles: [f32; 2],
    pub distance: f32,
    pub target: [f32; 3],
}

impl AppState {
    /// Set the simulation visualization mode.
    pub fn set_simulation_mode(&mut self, mode: SimulationMode) {
        self.simulation_mode = mode;
    }

    /// Create a new application state with default scene.
    pub fn new() -> Self {
        let mut state = Self {
            objects: Vec::new(),
            selected_object: None,
            camera: CameraState::new(),
            timeline: TimelineState::new(),
            tool: Tool::default(),
            edit_mode: EditMode::default(),
            shading: ShadingMode::default(),
            simulation_mode: SimulationMode::default(),
            snap_enabled: true,
            snap_increment: 1.0,
            axis_constraint: AxisConstraint::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_object_id: 1,
            physics: Vec::new(),
            physics_running: false,
            multi_selected: Vec::new(),
            wireframe_overlay: false,
            show_viewport_stats: true,
            box_select_active: false,
            box_select_start: None,
            camera_bookmarks: [None, None, None, None, None],
            sculpt_brush: SculptBrush::default(),
            sculpt_radius: 40.0,
            sculpt_strength: 0.5,
            edit_selection: EditSelection::default(),
            proportional_editing: false,
            proportional_radius: 2.0,
            measurements: Vec::new(),
            measuring: false,
            measure_start: None,
            paint_color: [1.0, 0.0, 0.0, 1.0],
            paint_radius: 30.0,
            weight_value: 1.0,
            lasso_active: false,
            lasso_points: Vec::new(),
            cursor_3d: [0.0, 0.0, 0.0],
            pivot_point: PivotPoint::default(),
            show_normals: false,
            show_object_info: false,
            show_orientation_cube: true,
            transform_orientation: TransformOrientation::default(),
            snap_target: SnapTarget::default(),
            show_camera_preview: false,
            selection_history: Vec::new(),
            clipboard: Vec::new(),
            auto_key: false,
            onion_skinning: false,
            onion_frames: 3,
            collections: Vec::new(),
            render_region: None,
            show_face_orientation: false,
            show_edge_lengths: false,
            matcap_index: 0,
            timeline_markers: Vec::new(),
            outliner_filter: OutlinerFilter::default(),
            background_image: None,
            environment_hdri: "Studio".to_string(),
            selection_outline_width: 2.0,
            camera_settings: CameraSettings::default(),
            world: WorldSettings::default(),
            xray_mode: false,
            backface_culling: true,
            clip_near: 0.1,
            clip_far: 1000.0,
            edit_tool: EditTool::default(),
            loop_cut_segments: 1,
            quick_favorites: Vec::new(),
            show_only_render: false,
            show_relationship_lines: true,
            show_cavity: false,
            show_shadows: true,
            show_specular: true,
            scene_props: SceneProperties::default(),
            gizmo_mode: GizmoMode::default(),
            overlays: ViewportOverlays {
                show_grid: true,
                show_axes: true,
                wireframe_on_solid: false,
                grid_opacity: 0.3,
                show_motion_paths: false,
                show_bones: true,
                show_annotations: true,
            },
            select_random_seed: 42,
            select_linked: false,
            workspace: WorkspaceLayout::Modeling,
            color_management: ColorManagement::default(),
            pose_mode: false,
            show_bone_names: false,
            show_bone_axes: false,
            gp_active_layer: "Layer".to_string(),
            gp_color: [0.0, 0.0, 0.0, 1.0],
            gp_size: 3.0,
            render_layers: vec![RenderLayer::default()],
            view_layers: vec![ViewLayer::default()],
            proportional_falloff: ProportionalFalloff::default(),
            asset_category: AssetCategory::Materials,
            asset_search: String::new(),
            snap_element: SnapElement::default(),
            render_engine: RenderEngine::default(),
            film_transparent: false,
            simplify_subdivision: 6,
            show_motion_paths_viewport: false,
            sequencer_strips: Vec::new(),
            perf_stats: PerformanceStats::default(),
            show_perf_overlay: false,
        };
        state.setup_default_scene();
        state
    }

    /// Set up a default scene like professional 3D apps.
    fn setup_default_scene(&mut self) {
        // Default Cube at origin
        self.add_cube();
        self.selected_object = Some(0);

        // Key Light (main light, upper-right)
        let mut light_mat = MaterialState::default();
        light_mat.emissive = 1.0;
        light_mat.base_color = [1.0, 0.98, 0.95, 1.0];
        self.objects.push(SceneObject {
            physiological_signal: 0.0,
            name: "Light".to_string(),
            object_type: ObjectType::Light,
            position: [4.0, 5.0, 3.0],
            rotation: [-37.0, 33.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            material: light_mat,
            modifiers: Vec::new(),
            visible: true,
            smooth_shading: false,
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
        self.next_object_id += 1;

        // Camera
        self.objects.push(SceneObject {
            physiological_signal: 0.0,
            name: "Camera".to_string(),
            object_type: ObjectType::Camera,
            position: [7.0, 5.0, 6.0],
            rotation: [-30.0, 45.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            material: MaterialState::default(),
            modifiers: Vec::new(),
            visible: true,
            smooth_shading: false,
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
        self.next_object_id += 1;

        // Ground plane (subtle, larger)
        let mut ground_mat = MaterialState::default();
        ground_mat.base_color = [0.25, 0.25, 0.28, 1.0];
        ground_mat.roughness = 0.9;
        self.objects.push(SceneObject {
            physiological_signal: 0.0,
            name: "Ground".to_string(),
            object_type: ObjectType::Plane,
            position: [0.0, -0.5, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [10.0, 1.0, 10.0],
            material: ground_mat,
            modifiers: Vec::new(),
            visible: true,
            smooth_shading: false,
            locked: true,
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
        self.next_object_id += 1;

        // Select the cube
        self.selected_object = Some(0);
    }

    /// Create a new scene.
    pub fn new_scene(&mut self) {
        self.objects.clear();
        self.selected_object = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.next_object_id = 1;
    }

    /// Add a cube to the scene.
    pub fn add_cube(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Cube.{:03}", self.next_object_id),
            object_type: ObjectType::Cube,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a sphere to the scene.
    pub fn add_sphere(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Sphere.{:03}", self.next_object_id),
            object_type: ObjectType::Sphere,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a cylinder to the scene.
    pub fn add_cylinder(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Cylinder.{:03}", self.next_object_id),
            object_type: ObjectType::Cylinder,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a plane to the scene.
    pub fn add_plane(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Plane.{:03}", self.next_object_id),
            object_type: ObjectType::Plane,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a torus to the scene.
    pub fn add_torus(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Torus.{:03}", self.next_object_id),
            object_type: ObjectType::Torus,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a cone to the scene.
    pub fn add_cone(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Cone.{:03}", self.next_object_id),
            object_type: ObjectType::Cone,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add an icosphere to the scene.
    pub fn add_icosphere(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("IcoSphere.{:03}", self.next_object_id),
            object_type: ObjectType::IcoSphere,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a grid to the scene.
    pub fn add_grid(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Grid.{:03}", self.next_object_id),
            object_type: ObjectType::Grid,
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [2.0, 1.0, 2.0],
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a circle to the scene.
    pub fn add_circle(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Circle.{:03}", self.next_object_id),
            object_type: ObjectType::Circle,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a bezier curve to the scene.
    pub fn add_bezier_curve(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("BezierCurve.{:03}", self.next_object_id),
            object_type: ObjectType::BezierCurve,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a NURBS curve to the scene.
    pub fn add_nurbs_curve(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("NurbsCurve.{:03}", self.next_object_id),
            object_type: ObjectType::NurbsCurve,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a text object to the scene.
    pub fn add_text(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Text.{:03}", self.next_object_id),
            object_type: ObjectType::Text,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add an empty object to the scene.
    pub fn add_empty(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Empty.{:03}", self.next_object_id),
            object_type: ObjectType::Empty,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a point light to the scene.
    pub fn add_point_light(&mut self) {
        let mut mat = MaterialState::default();
        mat.emissive = 1.0;
        mat.base_color = [1.0, 1.0, 0.9, 1.0];
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("PointLight.{:03}", self.next_object_id),
            object_type: ObjectType::Light,
            position: [0.0, 3.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            material: mat,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a directional light to the scene.
    pub fn add_directional_light(&mut self) {
        let mut mat = MaterialState::default();
        mat.emissive = 1.0;
        mat.base_color = [1.0, 1.0, 0.95, 1.0];
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("DirLight.{:03}", self.next_object_id),
            object_type: ObjectType::Light,
            position: [0.0, 5.0, 0.0],
            rotation: [-45.0, 45.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            material: mat,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add a camera to the scene.
    pub fn add_camera_object(&mut self) {
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Camera.{:03}", self.next_object_id),
            object_type: ObjectType::Camera,
            position: [5.0, 5.0, 5.0],
            rotation: [-30.0, 45.0, 0.0],
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Add an armature to the scene with a default bone chain.
    pub fn add_armature(&mut self) {
        let bones = vec![
            ArmatureBone {
                name: "Root".to_string(),
                head: [0.0, 0.0, 0.0],
                tail: [0.0, 0.5, 0.0],
                parent: None,
                connected: false,
                ..ArmatureBone::default()
            },
            ArmatureBone {
                name: "Spine".to_string(),
                head: [0.0, 0.5, 0.0],
                tail: [0.0, 1.2, 0.0],
                parent: Some("Root".to_string()),
                connected: true,
                ..ArmatureBone::default()
            },
            ArmatureBone {
                name: "Head".to_string(),
                head: [0.0, 1.2, 0.0],
                tail: [0.0, 1.6, 0.0],
                parent: Some("Spine".to_string()),
                connected: true,
                ..ArmatureBone::default()
            },
            ArmatureBone {
                name: "Arm.L".to_string(),
                head: [0.0, 1.1, 0.0],
                tail: [0.5, 0.8, 0.0],
                parent: Some("Spine".to_string()),
                connected: false,
                ..ArmatureBone::default()
            },
            ArmatureBone {
                name: "Arm.R".to_string(),
                head: [0.0, 1.1, 0.0],
                tail: [-0.5, 0.8, 0.0],
                parent: Some("Spine".to_string()),
                connected: false,
                ..ArmatureBone::default()
            },
        ];
        let obj = SceneObject {
            physiological_signal: 0.0,
            name: format!("Armature.{:03}", self.next_object_id),
            object_type: ObjectType::Empty,
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
            bones,
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
        self.next_object_id += 1;
        self.objects.push(obj);
        self.selected_object = Some(self.objects.len() - 1);
    }

    /// Calculate motion path for an object from its keyframes.
    pub fn calculate_motion_path(&mut self, obj_idx: usize) {
        if obj_idx >= self.objects.len() {
            return;
        }
        let obj = &self.objects[obj_idx];
        if obj.keyframes.is_empty() {
            return;
        }
        let start = obj.keyframes.first().map(|k| k.frame).unwrap_or(1);
        let end = obj.keyframes.last().map(|k| k.frame).unwrap_or(250);
        let mut points = Vec::new();
        // Generate points by interpolating between keyframes
        for frame in start..=end {
            // Find surrounding keyframes
            let pos = if let Some(exact) = obj.keyframes.iter().find(|k| k.frame == frame) {
                exact.position
            } else {
                // Linear interpolation between nearest keyframes
                let prev = obj.keyframes.iter().rev().find(|k| k.frame < frame);
                let next = obj.keyframes.iter().find(|k| k.frame > frame);
                match (prev, next) {
                    (Some(p), Some(n)) => {
                        let t = (frame - p.frame) as f32 / (n.frame - p.frame) as f32;
                        [
                            p.position[0] + (n.position[0] - p.position[0]) * t,
                            p.position[1] + (n.position[1] - p.position[1]) * t,
                            p.position[2] + (n.position[2] - p.position[2]) * t,
                        ]
                    }
                    (Some(p), None) => p.position,
                    (None, Some(n)) => n.position,
                    (None, None) => obj.position,
                }
            };
            points.push(MotionPathPoint {
                position: pos,
                frame,
            });
        }
        self.objects[obj_idx].motion_path = Some(MotionPath {
            points,
            color: [1.0, 0.8, 0.0],
            start_frame: start,
            end_frame: end,
            show_frame_numbers: false,
        });
    }

    /// Apply material to all selected objects.
    #[allow(dead_code)]
    pub fn apply_material_to_selected(&mut self, material: MaterialState) {
        let indices: Vec<usize> = self.get_all_selected();
        for idx in indices {
            if idx < self.objects.len() {
                self.objects[idx].material = material.clone();
            }
        }
    }

    /// Get all selected object indices (primary + multi).
    #[allow(dead_code)]
    pub fn get_all_selected(&self) -> Vec<usize> {
        let mut selected = Vec::new();
        if let Some(idx) = self.selected_object {
            selected.push(idx);
        }
        for &idx in &self.multi_selected {
            if !selected.contains(&idx) {
                selected.push(idx);
            }
        }
        selected
    }

    /// Ensure physics vec is synced with objects vec.
    pub fn sync_physics(&mut self) {
        while self.physics.len() < self.objects.len() {
            self.physics.push(PhysicsBody::default());
        }
        self.physics.truncate(self.objects.len());
    }

    /// Enable rigid body on selected object.
    pub fn enable_rigid_body(&mut self) {
        self.sync_physics();
        if let Some(idx) = self.selected_object {
            if idx < self.physics.len() {
                self.physics[idx].is_rigid_body = true;
            }
        }
    }

    /// Enable static collider on selected object (e.g., ground).
    pub fn enable_static_collider(&mut self) {
        self.sync_physics();
        if let Some(idx) = self.selected_object {
            if idx < self.physics.len() {
                self.physics[idx].is_static = true;
            }
        }
    }

    /// Step physics simulation.
    pub fn physics_step(&mut self, dt: f32) {
        if !self.physics_running {
            return;
        }
        self.sync_physics();
        let gravity = -9.81_f32;
        let ground_y = -0.5_f32;

        for i in 0..self.objects.len() {
            if i >= self.physics.len() {
                break;
            }
            if !self.physics[i].is_rigid_body || self.physics[i].is_static {
                continue;
            }

            // Apply gravity
            self.physics[i].velocity[1] += gravity * dt;

            // Integrate position
            self.objects[i].position[0] += self.physics[i].velocity[0] * dt;
            self.objects[i].position[1] += self.physics[i].velocity[1] * dt;
            self.objects[i].position[2] += self.physics[i].velocity[2] * dt;

            // Ground collision (simple plane at ground_y)
            let half_height = self.objects[i].scale[1] * 0.5;
            let bottom = self.objects[i].position[1] - half_height;
            if bottom < ground_y {
                self.objects[i].position[1] = ground_y + half_height;
                // Bounce with restitution
                self.physics[i].velocity[1] =
                    -self.physics[i].velocity[1] * self.physics[i].restitution;
                // Damping: stop tiny bounces
                if self.physics[i].velocity[1].abs() < 0.1 {
                    self.physics[i].velocity[1] = 0.0;
                }
                // Friction
                self.physics[i].velocity[0] *= 0.95;
                self.physics[i].velocity[2] *= 0.95;
            }
        }
    }

    /// Add a modifier to the selected object.
    pub fn add_modifier(&mut self, modifier_name: &str) {
        if let Some(idx) = self.selected_object {
            if let Some(obj) = self.objects.get_mut(idx) {
                obj.modifiers.push(modifier_name.to_string());
            }
        }
    }

    /// Remove a modifier from the selected object.
    #[allow(dead_code)]
    pub fn remove_modifier(&mut self, modifier_idx: usize) {
        if let Some(idx) = self.selected_object {
            if let Some(obj) = self.objects.get_mut(idx) {
                if modifier_idx < obj.modifiers.len() {
                    obj.modifiers.remove(modifier_idx);
                }
            }
        }
    }

    /// Get selected object reference.
    #[allow(dead_code)]
    pub fn get_selected(&self) -> Option<&SceneObject> {
        self.selected_object.and_then(|idx| self.objects.get(idx))
    }

    /// Get selected object mutable reference.
    #[allow(dead_code)]
    pub fn get_selected_mut(&mut self) -> Option<&mut SceneObject> {
        self.selected_object
            .and_then(|idx| self.objects.get_mut(idx))
    }

    /// Delete the selected object.
    pub fn delete_selected(&mut self) {
        if let Some(idx) = self.selected_object {
            if idx < self.objects.len() {
                self.objects.remove(idx);
                self.selected_object = if self.objects.is_empty() {
                    None
                } else {
                    Some(idx.min(self.objects.len() - 1))
                };
            }
        }
    }

    /// Check if an object index is in the selection (primary or multi).
    pub fn is_selected(&self, idx: usize) -> bool {
        self.selected_object == Some(idx) || self.multi_selected.contains(&idx)
    }

    /// Toggle multi-select for an object (Shift+Click).
    pub fn toggle_multi_select(&mut self, idx: usize) {
        if self.selected_object == Some(idx) {
            // Deselect primary - promote first multi-selected if any
            if let Some(first) = self.multi_selected.first().copied() {
                self.selected_object = Some(first);
                self.multi_selected.remove(0);
            } else {
                self.selected_object = None;
            }
        } else if let Some(pos) = self.multi_selected.iter().position(|&i| i == idx) {
            self.multi_selected.remove(pos);
        } else {
            // Add to multi selection
            if self.selected_object.is_none() {
                self.selected_object = Some(idx);
            } else {
                self.multi_selected.push(idx);
            }
        }
    }

    /// Get all selected indices.
    pub fn all_selected(&self) -> Vec<usize> {
        let mut result = Vec::new();
        if let Some(idx) = self.selected_object {
            result.push(idx);
        }
        result.extend_from_slice(&self.multi_selected);
        result
    }

    /// Clear multi-selection.
    pub fn clear_multi_select(&mut self) {
        self.multi_selected.clear();
    }

    /// Insert a keyframe at the current frame for the selected object.
    pub fn insert_keyframe(&mut self) -> bool {
        let frame = self.timeline.current_frame;
        if let Some(idx) = self.selected_object {
            if let Some(obj) = self.objects.get_mut(idx) {
                // Remove existing keyframe at this frame if any
                obj.keyframes.retain(|k| k.frame != frame);
                // Insert new keyframe
                obj.keyframes.push(Keyframe {
                    frame,
                    position: obj.position,
                    rotation: obj.rotation,
                    scale: obj.scale,
                });
                // Sort by frame
                obj.keyframes.sort_by_key(|k| k.frame);
                return true;
            }
        }
        false
    }

    /// Delete keyframe at the current frame for the selected object.
    pub fn delete_keyframe(&mut self) -> bool {
        let frame = self.timeline.current_frame;
        if let Some(idx) = self.selected_object {
            if let Some(obj) = self.objects.get_mut(idx) {
                let before = obj.keyframes.len();
                obj.keyframes.retain(|k| k.frame != frame);
                return obj.keyframes.len() < before;
            }
        }
        false
    }

    /// Evaluate keyframe animation for all objects at the current frame.
    pub fn evaluate_keyframes(&mut self) {
        let frame = self.timeline.current_frame;
        for obj in &mut self.objects {
            if obj.keyframes.len() < 2 {
                continue;
            }

            // Find surrounding keyframes
            let mut prev: Option<&Keyframe> = None;
            let mut next: Option<&Keyframe> = None;
            for k in &obj.keyframes {
                if k.frame <= frame {
                    prev = Some(k);
                } else {
                    next = Some(k);
                    break;
                }
            }

            match (prev, next) {
                (Some(p), Some(n)) => {
                    // Linear interpolation between keyframes
                    let range = (n.frame - p.frame) as f32;
                    if range > 0.0 {
                        let t = (frame - p.frame) as f32 / range;
                        for i in 0..3 {
                            obj.position[i] = p.position[i] + (n.position[i] - p.position[i]) * t;
                            obj.rotation[i] = p.rotation[i] + (n.rotation[i] - p.rotation[i]) * t;
                            obj.scale[i] = p.scale[i] + (n.scale[i] - p.scale[i]) * t;
                        }
                    }
                }
                (Some(p), None) => {
                    // Past last keyframe: hold last value
                    obj.position = p.position;
                    obj.rotation = p.rotation;
                    obj.scale = p.scale;
                }
                (None, Some(n)) => {
                    // Before first keyframe: hold first value
                    obj.position = n.position;
                    obj.rotation = n.rotation;
                    obj.scale = n.scale;
                }
                _ => {}
            }
        }
    }

    /// Set parent for selected object (use last multi-selected as parent).
    pub fn parent_selected(&mut self) -> bool {
        let all = self.all_selected();
        if all.len() >= 2 {
            let parent_idx = *all.last().unwrap();
            for &idx in &all[..all.len() - 1] {
                if idx < self.objects.len() {
                    self.objects[idx].parent = Some(parent_idx);
                }
            }
            true
        } else {
            false
        }
    }

    /// Clear parent for all selected objects.
    pub fn clear_parent_selected(&mut self) {
        for idx in self.all_selected() {
            if idx < self.objects.len() {
                self.objects[idx].parent = None;
            }
        }
    }

    /// Get children of an object.
    pub fn get_children(&self, parent_idx: usize) -> Vec<usize> {
        self.objects
            .iter()
            .enumerate()
            .filter(|(_, obj)| obj.parent == Some(parent_idx))
            .map(|(i, _)| i)
            .collect()
    }

    /// Save a camera bookmark (slot 0-4).
    pub fn save_camera_bookmark(&mut self, slot: usize) {
        if slot < 5 {
            self.camera_bookmarks[slot] = Some(CameraBookmark {
                orbit_angles: self.camera.orbit_angles,
                distance: self.camera.distance,
                target: self.camera.target,
            });
        }
    }

    /// Restore a camera bookmark (slot 0-4).
    pub fn restore_camera_bookmark(&mut self, slot: usize) -> bool {
        if slot < 5 {
            if let Some(bm) = &self.camera_bookmarks[slot] {
                self.camera.orbit_angles = bm.orbit_angles;
                self.camera.distance = bm.distance;
                self.camera.target = bm.target;
                self.camera.update_position();
                return true;
            }
        }
        false
    }

    /// Extract world-space vertices and face indices from a SceneObject for CSG operations.
    fn extract_object_geometry(obj: &SceneObject) -> (Vec<[f32; 3]>, Vec<Vec<usize>>) {
        let [px, py, pz] = obj.position;
        let [sx, sy, sz] = obj.scale;

        if let (Some(cv), Some(cf)) = (&obj.custom_vertices, &obj.custom_faces) {
            let world_v = cv
                .iter()
                .map(|[x, y, z]| [px + x * sx, py + y * sy, pz + z * sz])
                .collect();
            return (world_v, cf.clone());
        }

        match obj.object_type {
            ObjectType::Cube | ObjectType::Mesh => {
                let (hx, hy, hz) = (sx * 0.5, sy * 0.5, sz * 0.5);
                let v = vec![
                    [px - hx, py - hy, pz - hz],
                    [px + hx, py - hy, pz - hz],
                    [px + hx, py + hy, pz - hz],
                    [px - hx, py + hy, pz - hz],
                    [px - hx, py - hy, pz + hz],
                    [px + hx, py - hy, pz + hz],
                    [px + hx, py + hy, pz + hz],
                    [px - hx, py + hy, pz + hz],
                ];
                let f = vec![
                    vec![0, 3, 2, 1],
                    vec![4, 5, 6, 7],
                    vec![0, 1, 5, 4],
                    vec![1, 2, 6, 5],
                    vec![2, 3, 7, 6],
                    vec![3, 0, 4, 7],
                ];
                (v, f)
            }
            ObjectType::Sphere | ObjectType::IcoSphere => {
                let (rx, ry, rz) = (sx * 0.5, sy * 0.5, sz * 0.5);
                let rings = 10usize;
                let segs = 14usize;
                let pi = std::f32::consts::PI;
                let mut verts = vec![[px, py + ry, pz]]; // north pole
                for j in 1..rings {
                    let phi = pi * j as f32 / rings as f32;
                    for i in 0..segs {
                        let theta = 2.0 * pi * i as f32 / segs as f32;
                        verts.push([
                            px + rx * phi.sin() * theta.cos(),
                            py + ry * phi.cos(),
                            pz + rz * phi.sin() * theta.sin(),
                        ]);
                    }
                }
                verts.push([px, py - ry, pz]); // south pole
                let south = verts.len() - 1;
                let mut faces = vec![];
                for i in 0..segs {
                    faces.push(vec![0, 1 + i, 1 + (i + 1) % segs]);
                }
                for j in 0..rings - 2 {
                    for i in 0..segs {
                        let r0 = 1 + j * segs + i;
                        let r1 = 1 + j * segs + (i + 1) % segs;
                        let r2 = 1 + (j + 1) * segs + (i + 1) % segs;
                        let r3 = 1 + (j + 1) * segs + i;
                        faces.push(vec![r0, r1, r2, r3]);
                    }
                }
                let lr = 1 + (rings - 2) * segs;
                for i in 0..segs {
                    faces.push(vec![south, lr + (i + 1) % segs, lr + i]);
                }
                (verts, faces)
            }
            ObjectType::Plane => {
                let (hx, hz) = (sx * 0.5, sz * 0.5);
                let v = vec![
                    [px - hx, py, pz - hz],
                    [px + hx, py, pz - hz],
                    [px + hx, py, pz + hz],
                    [px - hx, py, pz + hz],
                ];
                (v, vec![vec![0, 1, 2, 3]])
            }
            ObjectType::Cylinder => {
                let r = sx * 0.5;
                let h = sy * 0.5;
                let segs = 14usize;
                let pi = std::f32::consts::PI;
                let mut verts = vec![];
                for i in 0..segs {
                    let theta = 2.0 * pi * i as f32 / segs as f32;
                    verts.push([px + r * theta.cos(), py - h, pz + r * theta.sin()]);
                }
                for i in 0..segs {
                    let theta = 2.0 * pi * i as f32 / segs as f32;
                    verts.push([px + r * theta.cos(), py + h, pz + r * theta.sin()]);
                }
                verts.push([px, py - h, pz]);
                verts.push([px, py + h, pz]);
                let bc = 2 * segs;
                let tc = 2 * segs + 1;
                let mut faces = vec![];
                for i in 0..segs {
                    let n = (i + 1) % segs;
                    faces.push(vec![i, n, segs + n, segs + i]);
                    faces.push(vec![bc, n, i]);
                    faces.push(vec![tc, segs + i, segs + n]);
                }
                (verts, faces)
            }
            ObjectType::Cone => {
                let r = sx * 0.5;
                let h = sy * 0.5;
                let segs = 14usize;
                let pi = std::f32::consts::PI;
                let mut verts = vec![];
                for i in 0..segs {
                    let theta = 2.0 * pi * i as f32 / segs as f32;
                    verts.push([px + r * theta.cos(), py - h, pz + r * theta.sin()]);
                }
                let apex = segs;
                let base = segs + 1;
                verts.push([px, py + h, pz]);
                verts.push([px, py - h, pz]);
                let mut faces = vec![];
                for i in 0..segs {
                    let n = (i + 1) % segs;
                    faces.push(vec![i, n, apex]);
                    faces.push(vec![base, n, i]);
                }
                (verts, faces)
            }
            _ => (vec![], vec![]),
        }
    }

    /// Perform a boolean operation between two selected objects.
    /// Returns the name of the result object, or None on failure.
    pub fn boolean_op(&mut self, op: BooleanOp) -> Option<String> {
        use nat3d_modeling::polygon::boolean::{boolean_operation, BooleanOp as ModelingBoolOp};

        let all = self.all_selected();
        if all.len() < 2 {
            return None;
        }
        let a_idx = all[0];
        let b_idx = all[1];
        if a_idx >= self.objects.len() || b_idx >= self.objects.len() {
            return None;
        }

        let (wv_a, wf_a) = Self::extract_object_geometry(&self.objects[a_idx]);
        let (wv_b, wf_b) = Self::extract_object_geometry(&self.objects[b_idx]);
        if wv_a.is_empty() || wv_b.is_empty() {
            return None;
        }

        let pts_a: Vec<nalgebra::Point3<f64>> = wv_a
            .iter()
            .map(|[x, y, z]| nalgebra::Point3::new(*x as f64, *y as f64, *z as f64))
            .collect();
        let pts_b: Vec<nalgebra::Point3<f64>> = wv_b
            .iter()
            .map(|[x, y, z]| nalgebra::Point3::new(*x as f64, *y as f64, *z as f64))
            .collect();

        let modeling_op = match op {
            BooleanOp::Union => ModelingBoolOp::Union,
            BooleanOp::Difference => ModelingBoolOp::Difference,
            BooleanOp::Intersection => ModelingBoolOp::Intersection,
        };

        let (res_pts, res_faces) = boolean_operation(modeling_op, &pts_a, &wf_a, &pts_b, &wf_b);
        if res_pts.is_empty() {
            return None;
        }

        let result_verts: Vec<[f32; 3]> = res_pts
            .iter()
            .map(|p| [p.x as f32, p.y as f32, p.z as f32])
            .collect();

        let name = format!("{}_{}", self.objects[a_idx].name, op);
        let mat = self.objects[a_idx].material.clone();

        let result = SceneObject {
            physiological_signal: 0.0,
            name: name.clone(),
            object_type: ObjectType::Mesh,
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            material: mat,
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
            custom_vertices: Some(result_verts),
            custom_faces: Some(res_faces),
            uv_coords: None,
        };

        self.objects[a_idx].visible = false;
        self.objects[b_idx].visible = false;
        self.next_object_id += 1;
        self.objects.push(result);
        self.selected_object = Some(self.objects.len() - 1);
        self.multi_selected.clear();
        Some(name)
    }

    /// Add a constraint to the selected object.
    pub fn add_constraint(&mut self, constraint: ObjectConstraint) -> bool {
        if let Some(idx) = self.selected_object {
            if idx < self.objects.len() {
                self.objects[idx].constraints.push(constraint);
                return true;
            }
        }
        false
    }

    /// Remove a constraint from the selected object (used by menus).
    #[allow(dead_code)]
    pub fn remove_constraint(&mut self, constraint_idx: usize) -> bool {
        if let Some(idx) = self.selected_object {
            if idx < self.objects.len() && constraint_idx < self.objects[idx].constraints.len() {
                self.objects[idx].constraints.remove(constraint_idx);
                return true;
            }
        }
        false
    }

    /// Evaluate object constraints.
    pub fn evaluate_constraints(&mut self) {
        let obj_count = self.objects.len();
        for i in 0..obj_count {
            if self.objects[i].constraints.is_empty() {
                continue;
            }
            let constraints: Vec<ObjectConstraint> = self.objects[i].constraints.clone();
            for constraint in &constraints {
                match constraint {
                    ObjectConstraint::TrackTo { target_idx } => {
                        if *target_idx < obj_count && *target_idx != i {
                            let target = self.objects[*target_idx].position;
                            let pos = self.objects[i].position;
                            let dx = target[0] - pos[0];
                            let dy = target[1] - pos[1];
                            let dz = target[2] - pos[2];
                            let yaw = dx.atan2(dz).to_degrees();
                            let dist_xz = (dx * dx + dz * dz).sqrt();
                            let pitch = (-dy).atan2(dist_xz).to_degrees();
                            self.objects[i].rotation = [pitch, yaw, 0.0];
                        }
                    }
                    ObjectConstraint::CopyLocation {
                        target_idx,
                        influence,
                    } => {
                        if *target_idx < obj_count && *target_idx != i {
                            let target = self.objects[*target_idx].position;
                            let t = *influence;
                            for axis in 0..3 {
                                self.objects[i].position[axis] =
                                    self.objects[i].position[axis] * (1.0 - t) + target[axis] * t;
                            }
                        }
                    }
                    ObjectConstraint::CopyRotation {
                        target_idx,
                        influence,
                    } => {
                        if *target_idx < obj_count && *target_idx != i {
                            let target = self.objects[*target_idx].rotation;
                            let t = *influence;
                            for axis in 0..3 {
                                self.objects[i].rotation[axis] =
                                    self.objects[i].rotation[axis] * (1.0 - t) + target[axis] * t;
                            }
                        }
                    }
                    ObjectConstraint::LimitLocation { min, max } => {
                        for axis in 0..3 {
                            self.objects[i].position[axis] =
                                self.objects[i].position[axis].clamp(min[axis], max[axis]);
                        }
                    }
                    ObjectConstraint::FollowPath { path_idx, offset } => {
                        if *path_idx < obj_count && *path_idx != i {
                            let path_pos = self.objects[*path_idx].position;
                            self.objects[i].position[0] = path_pos[0] + offset;
                            self.objects[i].position[1] = path_pos[1];
                            self.objects[i].position[2] = path_pos[2];
                        }
                    }
                }
            }
        }
    }

    /// Add a shape key to the selected object.
    pub fn add_shape_key(&mut self, name: &str) -> bool {
        if let Some(idx) = self.selected_object {
            if idx < self.objects.len() {
                self.objects[idx].shape_keys.push(ShapeKey {
                    name: name.to_string(),
                    value: 0.0,
                    positions: Vec::new(),
                });
                return true;
            }
        }
        false
    }

    /// Add a measurement between two points.
    pub fn add_measurement(&mut self, start: [f32; 3], end: [f32; 3]) {
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let dz = end[2] - start[2];
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        self.measurements.push(Measurement {
            start,
            end,
            distance,
        });
    }

    /// Clear all measurements.
    pub fn clear_measurements(&mut self) {
        self.measurements.clear();
    }

    /// Add a timeline marker at the current frame.
    pub fn add_timeline_marker(&mut self, name: &str) {
        self.timeline_markers.push(TimelineMarker {
            frame: self.timeline.current_frame,
            name: name.to_string(),
            color: [0.2, 0.8, 0.3],
        });
    }

    /// Remove timeline marker at a specific frame.
    #[allow(dead_code)]
    pub fn remove_timeline_marker(&mut self, frame: i32) {
        self.timeline_markers.retain(|m| m.frame != frame);
    }

    /// Check if an object matches the current outliner filter.
    pub fn matches_outliner_filter(&self, obj_type: ObjectType) -> bool {
        match self.outliner_filter {
            OutlinerFilter::All => true,
            OutlinerFilter::MeshOnly => matches!(
                obj_type,
                ObjectType::Cube
                    | ObjectType::Sphere
                    | ObjectType::Cylinder
                    | ObjectType::Plane
                    | ObjectType::Torus
                    | ObjectType::Cone
                    | ObjectType::IcoSphere
                    | ObjectType::Grid
                    | ObjectType::Circle
                    | ObjectType::Mesh
            ),
            OutlinerFilter::LightsOnly => obj_type == ObjectType::Light,
            OutlinerFilter::CamerasOnly => obj_type == ObjectType::Camera,
            OutlinerFilter::CurvesOnly => {
                matches!(obj_type, ObjectType::BezierCurve | ObjectType::NurbsCurve)
            }
        }
    }

    /// Perform an align operation on selected objects.
    pub fn align_objects(&mut self, align: AlignAxis) -> bool {
        let selected = self.all_selected();
        if selected.is_empty() {
            return false;
        }

        match align {
            AlignAxis::AlignX | AlignAxis::AlignY | AlignAxis::AlignZ => {
                let axis = match align {
                    AlignAxis::AlignX => 0,
                    AlignAxis::AlignY => 1,
                    AlignAxis::AlignZ => 2,
                    _ => 0,
                };
                // Align to the first selected object's position on this axis
                if let Some(&first) = selected.first() {
                    let target_val = self.objects[first].position[axis];
                    for &idx in &selected[1..] {
                        if idx < self.objects.len() {
                            self.objects[idx].position[axis] = target_val;
                        }
                    }
                }
                true
            }
            AlignAxis::DistributeX | AlignAxis::DistributeY | AlignAxis::DistributeZ => {
                let axis = match align {
                    AlignAxis::DistributeX => 0,
                    AlignAxis::DistributeY => 1,
                    AlignAxis::DistributeZ => 2,
                    _ => 0,
                };
                if selected.len() >= 3 {
                    let mut vals: Vec<(usize, f32)> = selected
                        .iter()
                        .filter(|&&i| i < self.objects.len())
                        .map(|&i| (i, self.objects[i].position[axis]))
                        .collect();
                    vals.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                    let min = vals.first().map_or(0.0, |v| v.1);
                    let max = vals.last().map_or(0.0, |v| v.1);
                    let count = vals.len();
                    if count > 1 {
                        let step = (max - min) / (count - 1) as f32;
                        for (i, (obj_idx, _)) in vals.iter().enumerate() {
                            self.objects[*obj_idx].position[axis] = min + step * i as f32;
                        }
                    }
                }
                true
            }
            AlignAxis::CenterToWorld => {
                for &idx in &selected {
                    if idx < self.objects.len() {
                        self.objects[idx].position = [0.0, 0.0, 0.0];
                    }
                }
                true
            }
            AlignAxis::CenterToActive => {
                if let Some(active_idx) = self.selected_object {
                    let target = self.objects[active_idx].position;
                    for &idx in &self.multi_selected.clone() {
                        if idx < self.objects.len() {
                            self.objects[idx].position = target;
                        }
                    }
                }
                true
            }
            AlignAxis::SnapToGrid => {
                let grid = self.snap_increment;
                for &idx in &selected {
                    if idx < self.objects.len() {
                        for axis in 0..3 {
                            self.objects[idx].position[axis] =
                                (self.objects[idx].position[axis] / grid).round() * grid;
                        }
                    }
                }
                true
            }
            AlignAxis::SnapToGround => {
                for &idx in &selected {
                    if idx < self.objects.len() {
                        let half_h = self.objects[idx].scale[1] * 0.5;
                        self.objects[idx].position[1] = -0.5 + half_h;
                    }
                }
                true
            }
        }
    }

    /// Duplicate the selected object.
    pub fn duplicate_selected(&mut self) {
        if let Some(idx) = self.selected_object {
            if let Some(obj) = self.objects.get(idx) {
                let mut new_obj = obj.clone();
                new_obj.name = format!("{}.{:03}", obj.name, self.next_object_id);
                new_obj.position[0] += 1.0;
                self.next_object_id += 1;
                self.objects.push(new_obj);
                self.selected_object = Some(self.objects.len() - 1);
            }
        }
    }

    /// Save current state for undo.
    pub fn save_undo_state(&mut self) {
        self.undo_stack.push(UndoState {
            objects: self.objects.clone(),
            selected_object: self.selected_object,
        });
        // Limit undo stack size
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        // Clear redo stack when new action is performed
        self.redo_stack.clear();
    }

    /// Undo the last action.
    pub fn undo(&mut self) -> bool {
        if let Some(state) = self.undo_stack.pop() {
            // Save current state to redo stack
            self.redo_stack.push(UndoState {
                objects: self.objects.clone(),
                selected_object: self.selected_object,
            });
            // Restore previous state
            self.objects = state.objects;
            self.selected_object = state.selected_object;
            true
        } else {
            false
        }
    }

    /// Redo the last undone action.
    pub fn redo(&mut self) -> bool {
        if let Some(state) = self.redo_stack.pop() {
            // Save current state to undo stack
            self.undo_stack.push(UndoState {
                objects: self.objects.clone(),
                selected_object: self.selected_object,
            });
            // Restore redo state
            self.objects = state.objects;
            self.selected_object = state.selected_object;
            true
        } else {
            false
        }
    }

    /// Check if undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Compute UV coordinates for the selected mesh object.
    ///
    /// Stores results in `selected_object.uv_coords`. Returns `false` if no
    /// object is selected or the object has no geometry.
    pub fn unwrap_uvs(&mut self, method: nat3d_modeling::uv::UvMethod) -> bool {
        use nat3d_modeling::uv::UvUnwrapper;
        let idx = match self.selected_object {
            Some(i) if i < self.objects.len() => i,
            _ => return false,
        };
        let (verts, faces) = Self::extract_object_geometry(&self.objects[idx]);
        if verts.is_empty() {
            return false;
        }
        let uvs = UvUnwrapper::new(&verts, &faces).unwrap(method);
        self.objects[idx].uv_coords = Some(uvs);
        true
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Scene object.
#[derive(Debug, Clone)]
pub struct SceneObject {
    pub physiological_signal: f32,
    /// Object name.
    pub name: String,
    /// Object type.
    pub object_type: ObjectType,
    /// Position (x, y, z).
    pub position: [f32; 3],
    /// Rotation (x, y, z) in degrees.
    pub rotation: [f32; 3],
    /// Scale (x, y, z).
    pub scale: [f32; 3],
    /// Material.
    pub material: MaterialState,
    /// Modifiers.
    pub modifiers: Vec<String>,
    /// Visibility.
    pub visible: bool,
    /// Smooth shading (interpolate normals vs flat).
    pub smooth_shading: bool,
    /// Locked (prevent transforms).
    pub locked: bool,
    /// Parent object index (for hierarchy).
    pub parent: Option<usize>,
    /// Animation keyframes (frame -> transform snapshot).
    pub keyframes: Vec<Keyframe>,
    /// Shape keys (morph targets).
    pub shape_keys: Vec<ShapeKey>,
    /// Object constraints.
    pub constraints: Vec<ObjectConstraint>,
    /// Vertex colors for texture painting (RGBA per vertex).
    #[allow(dead_code)]
    pub vertex_colors: Vec<[f32; 4]>,
    /// Vertex group weights for weight painting (0.0-1.0 per vertex).
    #[allow(dead_code)]
    pub vertex_weights: Vec<f32>,
    /// Vertex groups for weight painting and deformation.
    pub vertex_groups: Vec<VertexGroup>,
    /// Particle systems attached to this object.
    pub particle_systems: Vec<ParticleSystem>,
    /// Armature bones (for Armature type objects).
    pub bones: Vec<ArmatureBone>,
    /// Animation drivers on this object.
    pub drivers: Vec<AnimationDriver>,
    /// Force field attached to this object.
    pub force_field: Option<ForceFieldSettings>,
    /// Cloth simulation settings.
    pub cloth: Option<ClothSettings>,
    /// Soft body simulation settings.
    pub soft_body: Option<SoftBodySettings>,
    /// NLA tracks for this object.
    pub nla_tracks: Vec<NLATrack>,
    /// Grease pencil strokes on this object.
    #[allow(dead_code)]
    pub gp_strokes: Vec<GreasePencilStroke>,
    /// Texture slots on this object's material.
    pub texture_slots: Vec<TextureSlot>,
    /// Custom properties (user metadata).
    pub custom_properties: Vec<CustomProperty>,
    /// Motion path data (calculated from keyframes).
    pub motion_path: Option<MotionPath>,
    /// Object pass index (for compositing).
    #[allow(dead_code)]
    pub pass_index: u32,
    /// Hair particle settings (for Hair type particles).
    pub hair_settings: Option<HairSettings>,
    /// Fluid simulation settings.
    pub fluid: Option<FluidSettings>,
    /// Linked data source (for linked duplicates, empty = own data).
    #[allow(dead_code)]
    pub linked_data: Option<String>,
    /// Editable mesh data (when in edit mode).
    pub edit_mesh: Option<EditableMesh>,
    /// Edit mode sub-element selection.
    pub edit_selection: EditModeSelection,
    /// Custom mesh vertices (for ObjectType::Mesh after Edit Mode).
    pub custom_vertices: Option<Vec<[f32; 3]>>,
    /// Custom mesh faces (for ObjectType::Mesh after Edit Mode).
    pub custom_faces: Option<Vec<Vec<usize>>>,
    /// UV coordinates computed by UV unwrapping (one per vertex).
    pub uv_coords: Option<Vec<[f32; 2]>>,
}

/// Animation keyframe for an object.
#[derive(Debug, Clone)]
pub struct Keyframe {
    /// Frame number.
    pub frame: i32,
    /// Position at this frame.
    pub position: [f32; 3],
    /// Rotation at this frame (degrees).
    pub rotation: [f32; 3],
    /// Scale at this frame.
    pub scale: [f32; 3],
}

/// Boolean operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    Union,
    Difference,
    Intersection,
}

impl std::fmt::Display for BooleanOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Union => write!(f, "Union"),
            Self::Difference => write!(f, "Difference"),
            Self::Intersection => write!(f, "Intersection"),
        }
    }
}

/// Edit mode sub-selection type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditSelection {
    #[default]
    Vertex,
    Edge,
    Face,
}

impl std::fmt::Display for EditSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Vertex => write!(f, "Vertex"),
            Self::Edge => write!(f, "Edge"),
            Self::Face => write!(f, "Face"),
        }
    }
}

/// Shape key (morph target) for an object.
#[derive(Debug, Clone)]
pub struct ShapeKey {
    pub name: String,
    pub value: f32, // 0.0 to 1.0 blend weight
    #[allow(dead_code)]
    pub positions: Vec<[f32; 3]>, // vertex position offsets
}

/// Object constraint.
#[derive(Debug, Clone)]
pub enum ObjectConstraint {
    TrackTo {
        target_idx: usize,
    },
    CopyLocation {
        target_idx: usize,
        influence: f32,
    },
    CopyRotation {
        target_idx: usize,
        influence: f32,
    },
    LimitLocation {
        min: [f32; 3],
        max: [f32; 3],
    },
    #[allow(dead_code)]
    FollowPath {
        path_idx: usize,
        offset: f32,
    },
}

impl std::fmt::Display for ObjectConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TrackTo { target_idx } => write!(f, "Track To [{}]", target_idx),
            Self::CopyLocation { target_idx, .. } => write!(f, "Copy Location [{}]", target_idx),
            Self::CopyRotation { target_idx, .. } => write!(f, "Copy Rotation [{}]", target_idx),
            Self::LimitLocation { .. } => write!(f, "Limit Location"),
            Self::FollowPath { path_idx, .. } => write!(f, "Follow Path [{}]", path_idx),
        }
    }
}

/// Measurement between two points.
#[derive(Debug, Clone)]
pub struct Measurement {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub distance: f32,
}

/// Sculpt brush type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SculptBrush {
    #[default]
    Draw,
    Smooth,
    Flatten,
    Pinch,
    Inflate,
    Grab,
}

impl std::fmt::Display for SculptBrush {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draw => write!(f, "Draw"),
            Self::Smooth => write!(f, "Smooth"),
            Self::Flatten => write!(f, "Flatten"),
            Self::Pinch => write!(f, "Pinch"),
            Self::Inflate => write!(f, "Inflate"),
            Self::Grab => write!(f, "Grab"),
        }
    }
}

/// Object type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Cube,
    Sphere,
    Cylinder,
    Plane,
    Torus,
    Cone,
    IcoSphere,
    Grid,
    Circle,
    Empty,
    Light,
    Camera,
    /// Imported mesh (custom geometry).
    Mesh,
    /// Bezier curve (control points + handles).
    BezierCurve,
    /// NURBS curve.
    NurbsCurve,
    /// Text object.
    Text,
}

/// Material state.
#[derive(Debug, Clone)]
pub struct MaterialState {
    /// Base color (RGBA).
    pub base_color: [f32; 4],
    /// Metallic factor.
    pub metallic: f32,
    /// Roughness factor.
    pub roughness: f32,
    /// Emissive strength.
    pub emissive: f32,
}

impl Default for MaterialState {
    fn default() -> Self {
        Self {
            base_color: [0.68, 0.68, 0.72, 1.0],
            metallic: 0.0,
            roughness: 0.45,
            emissive: 0.0,
        }
    }
}

/// Camera state.
pub struct CameraState {
    /// Camera position.
    pub position: [f32; 3],
    /// Camera target.
    pub target: [f32; 3],
    /// Orbit angles (yaw, pitch).
    pub orbit_angles: [f32; 2],
    /// Orbit distance.
    pub distance: f32,
    /// Field of view.
    #[allow(dead_code)]
    pub fov: f32,
    /// Near plane.
    #[allow(dead_code)]
    pub near: f32,
    /// Far plane.
    #[allow(dead_code)]
    pub far: f32,
}

impl CameraState {
    /// Create a new camera state.
    pub fn new() -> Self {
        let mut state = Self {
            position: [5.0, 5.0, 5.0],
            target: [0.0, 0.0, 0.0],
            orbit_angles: [45.0, 30.0],
            distance: 10.0,
            fov: 45.0,
            near: 0.1,
            far: 1000.0,
        };
        state.update_position();
        state
    }

    /// Update camera position from orbit angles.
    pub fn update_position(&mut self) {
        let yaw = self.orbit_angles[0].to_radians();
        let pitch = self.orbit_angles[1].to_radians();

        self.position[0] = self.target[0] + self.distance * pitch.cos() * yaw.sin();
        self.position[1] = self.target[1] + self.distance * pitch.sin();
        self.position[2] = self.target[2] + self.distance * pitch.cos() * yaw.cos();
    }

    /// Orbit the camera.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.orbit_angles[0] += dx;
        self.orbit_angles[1] = (self.orbit_angles[1] - dy).clamp(-89.0, 89.0);
        self.update_position();
    }

    /// Pan the camera.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let yaw = self.orbit_angles[0].to_radians();

        let right = [yaw.cos(), 0.0, -yaw.sin()];
        let up = [0.0, 1.0, 0.0];

        let scale = self.distance * 0.1;
        self.target[0] -= (right[0] * dx + up[0] * dy) * scale;
        self.target[1] -= (right[1] * dx + up[1] * dy) * scale;
        self.target[2] -= (right[2] * dx + up[2] * dy) * scale;

        self.update_position();
    }

    /// Zoom the camera.
    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance * (1.0 - delta)).clamp(0.5, 500.0);
        self.update_position();
    }

    /// Set front view.
    pub fn set_view_front(&mut self) {
        self.orbit_angles = [0.0, 0.0];
        self.update_position();
    }

    /// Set back view.
    pub fn set_view_back(&mut self) {
        self.orbit_angles = [180.0, 0.0];
        self.update_position();
    }

    /// Set left view.
    pub fn set_view_left(&mut self) {
        self.orbit_angles = [-90.0, 0.0];
        self.update_position();
    }

    /// Set right view.
    pub fn set_view_right(&mut self) {
        self.orbit_angles = [90.0, 0.0];
        self.update_position();
    }

    /// Set top view.
    pub fn set_view_top(&mut self) {
        self.orbit_angles = [0.0, 89.0];
        self.update_position();
    }

    /// Set bottom view.
    pub fn set_view_bottom(&mut self) {
        self.orbit_angles = [0.0, -89.0];
        self.update_position();
    }

    /// Reset camera to default view.
    pub fn reset(&mut self) {
        self.target = [0.0, 0.0, 0.0];
        self.orbit_angles = [45.0, 30.0];
        self.distance = 10.0;
        self.update_position();
    }

    /// Free-fly translation (drone / first-person style): move the whole camera rig
    /// (`target` plus the derived `position`) through world space along the current
    /// view basis. `forward_amt` moves along the look direction, `right_amt` strafes,
    /// `up_amt` rises. Orbit angles and distance are preserved, so orbit/pan/zoom stay
    /// consistent afterwards — this is a translation of the pivot, not a re-orbit.
    pub fn fly(&mut self, forward_amt: f32, right_amt: f32, up_amt: f32) {
        let yaw = self.orbit_angles[0].to_radians();
        let pitch = self.orbit_angles[1].to_radians();
        // `dir` points target -> position (see `update_position`); the look direction
        // (position -> target) is therefore -dir.
        let dir = [
            pitch.cos() * yaw.sin(),
            pitch.sin(),
            pitch.cos() * yaw.cos(),
        ];
        let forward = [-dir[0], -dir[1], -dir[2]];
        let right = [yaw.cos(), 0.0, -yaw.sin()]; // matches pan()'s right vector
        let up = [0.0, 1.0, 0.0];
        for i in 0..3 {
            self.target[i] += forward[i] * forward_amt + right[i] * right_amt + up[i] * up_amt;
        }
        self.update_position();
    }
}

impl Default for CameraState {
    fn default() -> Self {
        Self::new()
    }
}

/// Timeline state.
pub struct TimelineState {
    /// Current frame.
    pub current_frame: i32,
    /// Start frame.
    pub start_frame: i32,
    /// End frame.
    pub end_frame: i32,
    /// Is playing.
    pub is_playing: bool,
    /// Frame rate.
    pub frame_rate: f32,
}

impl TimelineState {
    /// Create a new timeline state.
    pub fn new() -> Self {
        Self {
            current_frame: 1,
            start_frame: 1,
            end_frame: 250,
            is_playing: false,
            frame_rate: 24.0,
        }
    }

    /// Go to start.
    pub fn goto_start(&mut self) {
        self.current_frame = self.start_frame;
    }

    /// Go to end.
    pub fn goto_end(&mut self) {
        self.current_frame = self.end_frame;
    }

    /// Step forward.
    pub fn step_forward(&mut self) {
        self.current_frame = (self.current_frame + 1).min(self.end_frame);
    }

    /// Step backward.
    pub fn step_backward(&mut self) {
        self.current_frame = (self.current_frame - 1).max(self.start_frame);
    }

    /// Toggle play.
    pub fn toggle_play(&mut self) {
        self.is_playing = !self.is_playing;
    }

    /// Update timeline (call each frame when playing).
    /// Returns true if frame changed.
    pub fn update(&mut self, delta_seconds: f32) -> bool {
        if !self.is_playing {
            return false;
        }

        // Simple frame advance based on delta time
        let frames_per_second = self.frame_rate;
        let frame_delta = (delta_seconds * frames_per_second) as i32;

        if frame_delta > 0 {
            self.current_frame += frame_delta;
            // Loop back to start when reaching end
            if self.current_frame > self.end_frame {
                self.current_frame = self.start_frame;
            }
            return true;
        }
        false
    }

    /// Set the current frame, clamping to valid range.
    pub fn set_frame(&mut self, frame: i32) {
        self.current_frame = frame.clamp(self.start_frame, self.end_frame);
    }

    /// Get progress as a 0.0-1.0 value.
    pub fn progress(&self) -> f32 {
        let range = (self.end_frame - self.start_frame) as f32;
        if range <= 0.0 {
            return 0.0;
        }
        (self.current_frame - self.start_frame) as f32 / range
    }
}

impl Default for TimelineState {
    fn default() -> Self {
        Self::new()
    }
}

/// Physics body state for simulation.
#[derive(Debug, Clone)]
pub struct PhysicsBody {
    /// Linear velocity (x, y, z).
    pub velocity: [f32; 3],
    /// Whether this object participates as a rigid body.
    pub is_rigid_body: bool,
    /// Whether this object is a static collider (ground).
    pub is_static: bool,
    /// Mass (kg).
    pub mass: f32,
    /// Restitution (bounciness).
    pub restitution: f32,
}

impl Default for PhysicsBody {
    fn default() -> Self {
        Self {
            velocity: [0.0, 0.0, 0.0],
            is_rigid_body: false,
            is_static: false,
            mass: 1.0,
            restitution: 0.3,
        }
    }
}

/// Undo state snapshot.
#[derive(Clone)]
struct UndoState {
    objects: Vec<SceneObject>,
    selected_object: Option<usize>,
}

#[cfg(test)]
mod camera_tests {
    use super::*;

    #[test]
    fn fly_translates_rig_preserving_orientation_and_distance() {
        let mut cam = CameraState::new();
        let angles0 = cam.orbit_angles;
        let dist0 = cam.distance;
        let rel0 = [
            cam.position[0] - cam.target[0],
            cam.position[1] - cam.target[1],
            cam.position[2] - cam.target[2],
        ];
        let tgt0 = cam.target;

        cam.fly(3.0, 2.0, 1.0);

        // Orbit orientation and distance are untouched (fly is a pure translation).
        assert_eq!(
            cam.orbit_angles, angles0,
            "fly must not change orbit angles"
        );
        assert!(
            (cam.distance - dist0).abs() < 1e-6,
            "fly must not change distance"
        );

        // Camera and target shift together → position-target vector is preserved.
        let rel1 = [
            cam.position[0] - cam.target[0],
            cam.position[1] - cam.target[1],
            cam.position[2] - cam.target[2],
        ];
        for i in 0..3 {
            assert!(
                (rel0[i] - rel1[i]).abs() < 1e-4,
                "fly must preserve camera->target vector"
            );
        }
        // The rig actually moved.
        let moved: f32 = (0..3).map(|i| (cam.target[i] - tgt0[i]).powi(2)).sum();
        assert!(moved.sqrt() > 1e-3, "fly must move the rig");
    }

    #[test]
    fn fly_forward_moves_toward_look_direction() {
        let mut cam = CameraState::new();
        // Distance from camera to target must shrink when flying forward, because the
        // target moves along the look direction (camera follows by the same offset, but
        // we verify the target advanced toward where the camera was looking).
        let look0 = [
            cam.target[0] - cam.position[0],
            cam.target[1] - cam.position[1],
            cam.target[2] - cam.position[2],
        ];
        let tgt0 = cam.target;
        cam.fly(1.0, 0.0, 0.0);
        // Displacement of the target should be parallel (positive dot) to the look dir.
        let disp = [
            cam.target[0] - tgt0[0],
            cam.target[1] - tgt0[1],
            cam.target[2] - tgt0[2],
        ];
        let dot = look0[0] * disp[0] + look0[1] * disp[1] + look0[2] * disp[2];
        assert!(
            dot > 0.0,
            "flying forward must advance along the look direction"
        );
    }
}
