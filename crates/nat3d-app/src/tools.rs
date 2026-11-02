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

//! Interactive tools for 3D manipulation.

/// Snap settings.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SnapSettings {
    /// Master switch; when `false`, `snap_value`, `snap_rotation` and `snap_scale` return their input unchanged. Default `true`.
    pub enabled: bool,
    /// Whether values are snapped to the grid increment (used by `snap_value`). Default `true`.
    pub grid: bool,
    /// Vertex snapping flag. Default `false`; not read within this file.
    pub vertex: bool,
    /// Edge snapping flag. Default `false`; not read within this file.
    pub edge: bool,
    /// Face snapping flag. Default `false`; not read within this file.
    pub face: bool,
    /// Grid snap step used by `snap_value`; snapping is skipped if it is not positive. Default `1.0`.
    pub increment: f32,
    /// Step used by `snap_rotation`; snapping is skipped if it is not positive. Default `15.0`.
    pub rotation_increment: f32,
    /// Step used by `snap_scale`; snapping is skipped if it is not positive. Default `0.1`.
    pub scale_increment: f32,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            grid: true,
            vertex: false,
            edge: false,
            face: false,
            increment: 1.0,
            rotation_increment: 15.0,
            scale_increment: 0.1,
        }
    }
}

#[allow(dead_code)]
impl SnapSettings {
    /// Snap a value to the grid.
    pub fn snap_value(&self, value: f32) -> f32 {
        if self.enabled && self.grid && self.increment > 0.0 {
            (value / self.increment).round() * self.increment
        } else {
            value
        }
    }

    /// Snap a position.
    pub fn snap_position(&self, pos: [f32; 3]) -> [f32; 3] {
        [
            self.snap_value(pos[0]),
            self.snap_value(pos[1]),
            self.snap_value(pos[2]),
        ]
    }

    /// Snap rotation.
    pub fn snap_rotation(&self, angle: f32) -> f32 {
        if self.enabled && self.rotation_increment > 0.0 {
            (angle / self.rotation_increment).round() * self.rotation_increment
        } else {
            angle
        }
    }

    /// Snap scale.
    pub fn snap_scale(&self, scale: f32) -> f32 {
        if self.enabled && self.scale_increment > 0.0 {
            (scale / self.scale_increment).round() * self.scale_increment
        } else {
            scale
        }
    }
}

/// Transform pivot point.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PivotPoint {
    #[default]
    /// Median of the selected elements (default).
    MedianPoint,
    /// Each selected element rotates or scales about its own origin.
    IndividualOrigins,
    /// The active element.
    ActiveElement,
    /// The 3D cursor.
    Cursor3D,
    /// Center of the bounding box of the selection.
    BoundingBoxCenter,
}

/// Transform orientation.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransformOrientation {
    #[default]
    /// World axes (default).
    Global,
    /// Local axes of the object.
    Local,
    /// Normal direction of the selection.
    Normal,
    /// View-aligned axes.
    View,
    /// Axes of the 3D cursor.
    Cursor,
}

/// Selection mode for edit mode.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    /// Select vertices (default).
    Vertex,
    /// Select edges.
    Edge,
    /// Select faces.
    Face,
}

/// Selection settings.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SelectionSettings {
    /// Current element selection mode. Default `SelectionMode::Vertex`.
    pub mode: SelectionMode,
    /// X-ray selection flag. Default `false`.
    pub x_ray: bool,
    /// Limit-selection-to-visible flag. Default `true`.
    pub limit_to_visible: bool,
    /// Box-select flag. Default `false`.
    pub box_select: bool,
    /// Radius used for circle select. Default `25.0`.
    pub circle_select_radius: f32,
}

impl Default for SelectionSettings {
    fn default() -> Self {
        Self {
            mode: SelectionMode::Vertex,
            x_ray: false,
            limit_to_visible: true,
            box_select: false,
            circle_select_radius: 25.0,
        }
    }
}

/// Proportional editing settings.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ProportionalEdit {
    /// Whether proportional editing is on. Default `false`.
    pub enabled: bool,
    /// Connected-only flag. Default `false`.
    pub connected: bool,
    /// Falloff curve used by proportional editing. Default `ProportionalFalloff::Smooth`.
    pub falloff: ProportionalFalloff,
    /// Radius of the proportional influence. Default `1.0`.
    pub size: f32,
}

impl Default for ProportionalEdit {
    fn default() -> Self {
        Self {
            enabled: false,
            connected: false,
            falloff: ProportionalFalloff::Smooth,
            size: 1.0,
        }
    }
}

/// Proportional editing falloff curve.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProportionalFalloff {
    /// `1 - 3t^2 + 2t^3`, where `t = distance / radius`.
    Smooth,
    /// `sqrt(1 - t^2)`, where `t = distance / radius`.
    Sphere,
    /// `sqrt(1 - t)`, where `t = distance / radius`.
    Root,
    /// `1 / (1 + t^2) - 0.5`, where `t = distance / radius`.
    InverseSquare,
    /// `(1 - t)^2`, where `t = distance / radius`.
    Sharp,
    /// `1 - t`, where `t = distance / radius`.
    Linear,
    /// Always `1.0` inside the radius.
    Constant,
    /// `1 - t` times a pseudo-random value derived from the distance.
    Random,
}

#[allow(dead_code)]
impl ProportionalFalloff {
    /// Evaluate falloff at distance.
    pub fn evaluate(&self, distance: f32, radius: f32) -> f32 {
        if distance >= radius {
            return 0.0;
        }
        let t = distance / radius;
        match self {
            Self::Smooth => {
                let t2 = t * t;
                let t3 = t2 * t;
                1.0 - 3.0 * t2 + 2.0 * t3
            }
            Self::Sphere => (1.0 - t * t).sqrt(),
            Self::Root => (1.0 - t).sqrt(),
            Self::InverseSquare => 1.0 / (1.0 + t * t) - 0.5,
            Self::Sharp => (1.0 - t) * (1.0 - t),
            Self::Linear => 1.0 - t,
            Self::Constant => 1.0,
            Self::Random => 1.0 - t * rand_simple(distance as u32),
        }
    }
}

#[allow(dead_code)]
fn rand_simple(seed: u32) -> f32 {
    let x = seed.wrapping_mul(1103515245).wrapping_add(12345);
    x as f32 / u32::MAX as f32
}

/// 3D cursor.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Cursor3D {
    /// Cursor position in world space as `[x, y, z]`. Default `[0.0, 0.0, 0.0]`.
    pub position: [f32; 3],
    /// Cursor rotation as a four-component quaternion (the default is `[0.0, 0.0, 0.0, 1.0]`).
    pub rotation: [f32; 4], // quaternion
}

impl Default for Cursor3D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

#[allow(dead_code)]
impl Cursor3D {
    /// Reset cursor to origin.
    pub fn reset(&mut self) {
        self.position = [0.0, 0.0, 0.0];
        self.rotation = [0.0, 0.0, 0.0, 1.0];
    }

    /// Set position from selection center.
    pub fn set_from_selection(&mut self, center: [f32; 3]) {
        self.position = center;
    }
}

/// Transform tool state.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TransformState {
    /// Whether a transform is in progress; set by `begin`, cleared by `end` and `cancel`.
    pub active: bool,
    /// Axis the movement is restricted to, if any; see `constrain_delta`.
    pub axis_constraint: Option<Axis>,
    /// Axis normal to the plane the movement is restricted to, if any; see `constrain_delta`.
    pub plane_constraint: Option<Axis>,
    /// Position stored by `begin` and returned by `cancel`.
    pub start_position: [f32; 3],
    /// Rotation stored by `begin` and returned by `cancel`.
    pub start_rotation: [f32; 3],
    /// Scale stored by `begin` and returned by `cancel`; default `[1.0, 1.0, 1.0]`.
    pub start_scale: [f32; 3],
    /// Accumulated delta; reset to zero by `begin`.
    pub accumulated_delta: [f32; 3],
    /// Pivot point used for the transform. Default `PivotPoint::MedianPoint`.
    pub pivot: PivotPoint,
    /// Orientation used for the transform. Default `TransformOrientation::Global`.
    pub orientation: TransformOrientation,
    /// Snap settings for the transform.
    pub snap: SnapSettings,
    /// Proportional-editing settings for the transform.
    pub proportional: ProportionalEdit,
}

impl Default for TransformState {
    fn default() -> Self {
        Self {
            active: false,
            axis_constraint: None,
            plane_constraint: None,
            start_position: [0.0, 0.0, 0.0],
            start_rotation: [0.0, 0.0, 0.0],
            start_scale: [1.0, 1.0, 1.0],
            accumulated_delta: [0.0, 0.0, 0.0],
            pivot: PivotPoint::default(),
            orientation: TransformOrientation::default(),
            snap: SnapSettings::default(),
            proportional: ProportionalEdit::default(),
        }
    }
}

#[allow(dead_code)]
impl TransformState {
    /// Begin transform.
    pub fn begin(&mut self, pos: [f32; 3], rot: [f32; 3], scale: [f32; 3]) {
        self.active = true;
        self.start_position = pos;
        self.start_rotation = rot;
        self.start_scale = scale;
        self.accumulated_delta = [0.0, 0.0, 0.0];
        self.axis_constraint = None;
        self.plane_constraint = None;
    }

    /// End transform.
    pub fn end(&mut self) {
        self.active = false;
    }

    /// Cancel transform.
    pub fn cancel(&mut self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        self.active = false;
        (self.start_position, self.start_rotation, self.start_scale)
    }

    /// Set axis constraint.
    pub fn set_axis(&mut self, axis: Axis) {
        if self.axis_constraint == Some(axis) {
            self.axis_constraint = None;
        } else {
            self.axis_constraint = Some(axis);
            self.plane_constraint = None;
        }
    }

    /// Set plane constraint.
    pub fn set_plane(&mut self, normal_axis: Axis) {
        if self.plane_constraint == Some(normal_axis) {
            self.plane_constraint = None;
        } else {
            self.plane_constraint = Some(normal_axis);
            self.axis_constraint = None;
        }
    }

    /// Apply constraint to delta.
    pub fn constrain_delta(&self, delta: [f32; 3]) -> [f32; 3] {
        if let Some(axis) = self.axis_constraint {
            match axis {
                Axis::X => [delta[0], 0.0, 0.0],
                Axis::Y => [0.0, delta[1], 0.0],
                Axis::Z => [0.0, 0.0, delta[2]],
            }
        } else if let Some(plane) = self.plane_constraint {
            match plane {
                Axis::X => [0.0, delta[1], delta[2]], // YZ plane
                Axis::Y => [delta[0], 0.0, delta[2]], // XZ plane
                Axis::Z => [delta[0], delta[1], 0.0], // XY plane
            }
        } else {
            delta
        }
    }
}

/// Axis.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// The X axis.
    X,
    /// The Y axis.
    Y,
    /// The Z axis.
    Z,
}

/// Tool settings collection.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ToolSettings {
    /// Snap settings.
    pub snap: SnapSettings,
    /// Selection settings.
    pub selection: SelectionSettings,
    /// Proportional-editing settings.
    pub proportional: ProportionalEdit,
    /// Current pivot point; advanced by `cycle_pivot`.
    pub pivot: PivotPoint,
    /// Current transform orientation; advanced by `cycle_orientation`.
    pub orientation: TransformOrientation,
    /// The 3D cursor.
    pub cursor: Cursor3D,
    /// State of the transform tool.
    pub transform: TransformState,
}

#[allow(dead_code)]
impl ToolSettings {
    /// Create new tool settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle snap.
    pub fn toggle_snap(&mut self) {
        self.snap.enabled = !self.snap.enabled;
    }

    /// Toggle proportional editing.
    pub fn toggle_proportional(&mut self) {
        self.proportional.enabled = !self.proportional.enabled;
    }

    /// Cycle selection mode.
    pub fn cycle_selection_mode(&mut self) {
        self.selection.mode = match self.selection.mode {
            SelectionMode::Vertex => SelectionMode::Edge,
            SelectionMode::Edge => SelectionMode::Face,
            SelectionMode::Face => SelectionMode::Vertex,
        };
    }

    /// Cycle pivot point.
    pub fn cycle_pivot(&mut self) {
        self.pivot = match self.pivot {
            PivotPoint::MedianPoint => PivotPoint::IndividualOrigins,
            PivotPoint::IndividualOrigins => PivotPoint::ActiveElement,
            PivotPoint::ActiveElement => PivotPoint::Cursor3D,
            PivotPoint::Cursor3D => PivotPoint::BoundingBoxCenter,
            PivotPoint::BoundingBoxCenter => PivotPoint::MedianPoint,
        };
    }

    /// Cycle orientation.
    pub fn cycle_orientation(&mut self) {
        self.orientation = match self.orientation {
            TransformOrientation::Global => TransformOrientation::Local,
            TransformOrientation::Local => TransformOrientation::Normal,
            TransformOrientation::Normal => TransformOrientation::View,
            TransformOrientation::View => TransformOrientation::Global,
            TransformOrientation::Cursor => TransformOrientation::Global,
        };
    }
}

/// Measure tool results.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct MeasureResult {
    /// Measured distance.
    pub distance: f32,
    /// Measured angle (`MeasureResult::angle` returns degrees).
    pub angle: f32,
    /// Measured area.
    pub area: f32,
    /// Measured volume.
    pub volume: f32,
    /// Measured edge length.
    pub edge_length: f32,
    /// Measured face area.
    pub face_area: f32,
}

#[allow(dead_code)]
impl MeasureResult {
    /// Calculate distance between two points.
    pub fn distance(p1: [f32; 3], p2: [f32; 3]) -> f32 {
        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        let dz = p2[2] - p1[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Calculate angle between three points.
    pub fn angle(p1: [f32; 3], p2: [f32; 3], p3: [f32; 3]) -> f32 {
        let v1 = [p1[0] - p2[0], p1[1] - p2[1], p1[2] - p2[2]];
        let v2 = [p3[0] - p2[0], p3[1] - p2[1], p3[2] - p2[2]];

        let dot = v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2];
        let len1 = (v1[0] * v1[0] + v1[1] * v1[1] + v1[2] * v1[2]).sqrt();
        let len2 = (v2[0] * v2[0] + v2[1] * v2[1] + v2[2] * v2[2]).sqrt();

        if len1 > 0.0 && len2 > 0.0 {
            (dot / (len1 * len2)).clamp(-1.0, 1.0).acos().to_degrees()
        } else {
            0.0
        }
    }

    /// Calculate triangle area.
    pub fn triangle_area(p1: [f32; 3], p2: [f32; 3], p3: [f32; 3]) -> f32 {
        let v1 = [p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]];
        let v2 = [p3[0] - p1[0], p3[1] - p1[1], p3[2] - p1[2]];

        let cross = [
            v1[1] * v2[2] - v1[2] * v2[1],
            v1[2] * v2[0] - v1[0] * v2[2],
            v1[0] * v2[1] - v1[1] * v2[0],
        ];

        0.5 * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt()
    }
}
