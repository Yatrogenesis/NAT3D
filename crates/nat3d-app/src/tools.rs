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
    pub enabled: bool,
    pub grid: bool,
    pub vertex: bool,
    pub edge: bool,
    pub face: bool,
    pub increment: f32,
    pub rotation_increment: f32,
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
    MedianPoint,
    IndividualOrigins,
    ActiveElement,
    Cursor3D,
    BoundingBoxCenter,
}

/// Transform orientation.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransformOrientation {
    #[default]
    Global,
    Local,
    Normal,
    View,
    Cursor,
}

/// Selection mode for edit mode.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    Vertex,
    Edge,
    Face,
}

/// Selection settings.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SelectionSettings {
    pub mode: SelectionMode,
    pub x_ray: bool,
    pub limit_to_visible: bool,
    pub box_select: bool,
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
    pub enabled: bool,
    pub connected: bool,
    pub falloff: ProportionalFalloff,
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
    Smooth,
    Sphere,
    Root,
    InverseSquare,
    Sharp,
    Linear,
    Constant,
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
    pub position: [f32; 3],
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
    pub active: bool,
    pub axis_constraint: Option<Axis>,
    pub plane_constraint: Option<Axis>,
    pub start_position: [f32; 3],
    pub start_rotation: [f32; 3],
    pub start_scale: [f32; 3],
    pub accumulated_delta: [f32; 3],
    pub pivot: PivotPoint,
    pub orientation: TransformOrientation,
    pub snap: SnapSettings,
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
    X,
    Y,
    Z,
}

/// Tool settings collection.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ToolSettings {
    pub snap: SnapSettings,
    pub selection: SelectionSettings,
    pub proportional: ProportionalEdit,
    pub pivot: PivotPoint,
    pub orientation: TransformOrientation,
    pub cursor: Cursor3D,
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
    pub distance: f32,
    pub angle: f32,
    pub area: f32,
    pub volume: f32,
    pub edge_length: f32,
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
