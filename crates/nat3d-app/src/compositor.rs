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

//! # Batch 29: Compositor
//!
//! Node-based image compositor for post-processing rendered output.
//!
//! ## Node Types
//! - **Input**: Render Layer, Image
//! - **Output**: Composite, Viewer, File Output
//! - **Color**: Brightness/Contrast, RGB Curves, Hue/Saturation
//! - **Mix**: Mix (with multiple blend modes)
//!
//! ## Execution
//! `Compositor::execute()` evaluates the node graph topologically and
//! produces a final `CompositorImage`.

#![allow(clippy::all, dead_code, unused_imports)]

use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Image representation
// ─────────────────────────────────────────────────────────────────────────────

/// RGBA floating-point image used throughout the compositor.
#[derive(Debug, Clone)]
pub struct CompositorImage {
    pub width: u32,
    pub height: u32,
    /// Pixel data in row-major RGBA f32 format.
    pub pixels: Vec<[f32; 4]>,
}

impl CompositorImage {
    /// Create a solid-color image.
    pub fn solid(width: u32, height: u32, color: [f32; 4]) -> Self {
        let pixels = vec![color; (width * height) as usize];
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Create a blank (transparent black) image.
    pub fn blank(width: u32, height: u32) -> Self {
        Self::solid(width, height, [0.0, 0.0, 0.0, 1.0])
    }

    /// Get pixel at (x, y).
    pub fn get(&self, x: u32, y: u32) -> [f32; 4] {
        let idx = (y * self.width + x) as usize;
        if idx < self.pixels.len() {
            self.pixels[idx]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        }
    }

    /// Set pixel at (x, y).
    pub fn set(&mut self, x: u32, y: u32, pixel: [f32; 4]) {
        let idx = (y * self.width + x) as usize;
        if idx < self.pixels.len() {
            self.pixels[idx] = pixel;
        }
    }

    /// Convert to u8 RGBA bytes for display.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for px in &self.pixels {
            out.push((px[0].clamp(0.0, 1.0) * 255.0) as u8);
            out.push((px[1].clamp(0.0, 1.0) * 255.0) as u8);
            out.push((px[2].clamp(0.0, 1.0) * 255.0) as u8);
            out.push((px[3].clamp(0.0, 1.0) * 255.0) as u8);
        }
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Node Definitions
// ─────────────────────────────────────────────────────────────────────────────

/// Unique identifier for a compositor node.
pub type NodeId = u32;

/// Blend mode for the Mix node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MixBlendMode {
    #[default]
    Mix,
    Add,
    Subtract,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Difference,
}

impl MixBlendMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Mix => "Mix",
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::Darken => "Darken",
            Self::Lighten => "Lighten",
            Self::Difference => "Difference",
        }
    }

    pub fn all() -> &'static [MixBlendMode] {
        &[
            Self::Mix,
            Self::Add,
            Self::Subtract,
            Self::Multiply,
            Self::Screen,
            Self::Overlay,
            Self::Darken,
            Self::Lighten,
            Self::Difference,
        ]
    }
}

/// RGB Curves control point.
#[derive(Debug, Clone)]
pub struct CurvePoint {
    pub x: f32,
    pub y: f32,
}

/// RGB Curves for individual channel control.
#[derive(Debug, Clone)]
pub struct RgbCurves {
    /// Control points for R channel.
    pub r: Vec<CurvePoint>,
    /// Control points for G channel.
    pub g: Vec<CurvePoint>,
    /// Control points for B channel.
    pub b: Vec<CurvePoint>,
    /// Control points for combined (master) channel.
    pub master: Vec<CurvePoint>,
}

impl Default for RgbCurves {
    fn default() -> Self {
        let linear = vec![CurvePoint { x: 0.0, y: 0.0 }, CurvePoint { x: 1.0, y: 1.0 }];
        Self {
            r: linear.clone(),
            g: linear.clone(),
            b: linear.clone(),
            master: linear,
        }
    }
}

impl RgbCurves {
    /// Sample a curve at position x using linear interpolation.
    fn sample(curve: &[CurvePoint], x: f32) -> f32 {
        if curve.is_empty() {
            return x;
        }
        if x <= curve[0].x {
            return curve[0].y;
        }
        if x >= curve[curve.len() - 1].x {
            return curve[curve.len() - 1].y;
        }
        for i in 0..curve.len() - 1 {
            let a = &curve[i];
            let b = &curve[i + 1];
            if x >= a.x && x <= b.x {
                let t = (x - a.x) / (b.x - a.x).max(1e-6);
                return a.y + t * (b.y - a.y);
            }
        }
        x
    }

    /// Apply RGB curves to a pixel.
    pub fn apply(&self, pixel: [f32; 4]) -> [f32; 4] {
        let r = Self::sample(&self.master, Self::sample(&self.r, pixel[0]));
        let g = Self::sample(&self.master, Self::sample(&self.g, pixel[1]));
        let b = Self::sample(&self.master, Self::sample(&self.b, pixel[2]));
        [
            r.clamp(0.0, 1.0),
            g.clamp(0.0, 1.0),
            b.clamp(0.0, 1.0),
            pixel[3],
        ]
    }
}

/// A compositor node and its parameters.
#[derive(Debug, Clone)]
pub enum CompositorNodeKind {
    // ── Input nodes ───────────────────────────────────────────────────────
    /// Provides the rendered image from a render layer.
    RenderLayer {
        /// Name of the render layer to use.
        layer_name: String,
    },
    /// Loads a static image from disk.
    Image {
        path: String,
        /// Cached image data (loaded on first execute).
        data: Option<CompositorImage>,
    },

    // ── Output nodes ──────────────────────────────────────────────────────
    /// Final composite output.
    Composite,
    /// Viewer node (shows image in a side panel).
    Viewer,
    /// Save to file.
    FileOutput {
        path: String,
        format: String, // "png", "exr", "jpg"
    },

    // ── Color nodes ───────────────────────────────────────────────────────
    /// Adjust brightness and contrast.
    BrightnessContrast { brightness: f32, contrast: f32 },
    /// Full RGB curves control.
    RgbCurves { curves: RgbCurves },
    /// Adjust hue, saturation, value.
    HueSaturation {
        hue: f32,        // offset in 0.0–1.0 range
        saturation: f32, // multiplier, 1.0 = no change
        value: f32,      // multiplier, 1.0 = no change
    },

    // ── Mix node ──────────────────────────────────────────────────────────
    /// Blend two images together.
    Mix { mode: MixBlendMode, factor: f32 },
}

/// Input socket connection (source_node_id, output_slot_index).
pub type SocketConnection = (NodeId, usize);

/// A single compositor node with position and connections.
#[derive(Debug, Clone)]
pub struct CompositorNode {
    pub id: NodeId,
    pub name: String,
    pub kind: CompositorNodeKind,
    /// Screen position for the node editor UI.
    pub position: [f32; 2],
    pub size: [f32; 2],
    /// Input connections: input_slot → (source_node_id, output_slot).
    pub inputs: HashMap<usize, SocketConnection>,
    /// Cached output images from last execution.
    pub output_cache: Vec<Option<CompositorImage>>,
    pub selected: bool,
}

impl CompositorNode {
    pub fn new(
        id: NodeId,
        name: impl Into<String>,
        kind: CompositorNodeKind,
        pos: [f32; 2],
    ) -> Self {
        let num_outputs = match &kind {
            CompositorNodeKind::RenderLayer { .. } => 1,
            CompositorNodeKind::Image { .. } => 1,
            CompositorNodeKind::Composite => 0,
            CompositorNodeKind::Viewer => 0,
            CompositorNodeKind::FileOutput { .. } => 0,
            CompositorNodeKind::BrightnessContrast { .. } => 1,
            CompositorNodeKind::RgbCurves { .. } => 1,
            CompositorNodeKind::HueSaturation { .. } => 1,
            CompositorNodeKind::Mix { .. } => 1,
        };
        Self {
            id,
            name: name.into(),
            kind,
            position: pos,
            size: [180.0, 100.0],
            inputs: HashMap::new(),
            output_cache: vec![None; num_outputs.max(1)],
            selected: false,
        }
    }

    /// Number of input sockets for this node.
    pub fn num_inputs(&self) -> usize {
        match &self.kind {
            CompositorNodeKind::RenderLayer { .. } => 0,
            CompositorNodeKind::Image { .. } => 0,
            CompositorNodeKind::Composite => 1,
            CompositorNodeKind::Viewer => 1,
            CompositorNodeKind::FileOutput { .. } => 1,
            CompositorNodeKind::BrightnessContrast { .. } => 1,
            CompositorNodeKind::RgbCurves { .. } => 1,
            CompositorNodeKind::HueSaturation { .. } => 1,
            CompositorNodeKind::Mix { .. } => 2,
        }
    }

    /// Input socket names.
    pub fn input_labels(&self) -> Vec<&'static str> {
        match &self.kind {
            CompositorNodeKind::Mix { .. } => vec!["Image 1", "Image 2"],
            CompositorNodeKind::Composite
            | CompositorNodeKind::Viewer
            | CompositorNodeKind::FileOutput { .. }
            | CompositorNodeKind::BrightnessContrast { .. }
            | CompositorNodeKind::RgbCurves { .. }
            | CompositorNodeKind::HueSaturation { .. } => vec!["Image"],
            _ => vec![],
        }
    }

    /// Output socket names.
    pub fn output_labels(&self) -> Vec<&'static str> {
        match &self.kind {
            CompositorNodeKind::RenderLayer { .. }
            | CompositorNodeKind::Image { .. }
            | CompositorNodeKind::BrightnessContrast { .. }
            | CompositorNodeKind::RgbCurves { .. }
            | CompositorNodeKind::HueSaturation { .. }
            | CompositorNodeKind::Mix { .. } => vec!["Image"],
            _ => vec![],
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Compositor Graph
// ─────────────────────────────────────────────────────────────────────────────

/// The full compositor node graph.
#[derive(Debug, Clone, Default)]
pub struct Compositor {
    pub nodes: HashMap<NodeId, CompositorNode>,
    next_id: NodeId,
    /// The final composite output, produced by execute().
    pub output: Option<CompositorImage>,
    /// ID of the Viewer node (shown in editor).
    pub viewer_node: Option<NodeId>,
}

impl Compositor {
    pub fn new() -> Self {
        let mut c = Self::default();
        // Default graph: RenderLayer → Composite
        let rl_id = c.add_node(
            "Render Layer",
            CompositorNodeKind::RenderLayer {
                layer_name: "View Layer".into(),
            },
            [80.0, 150.0],
        );
        let comp_id = c.add_node("Composite", CompositorNodeKind::Composite, [420.0, 150.0]);
        let viewer_id = c.add_node("Viewer", CompositorNodeKind::Viewer, [420.0, 300.0]);
        // Connect RenderLayer → Composite
        if let Some(comp_node) = c.nodes.get_mut(&comp_id) {
            comp_node.inputs.insert(0, (rl_id, 0));
        }
        // Connect RenderLayer → Viewer
        if let Some(viewer_node) = c.nodes.get_mut(&viewer_id) {
            viewer_node.inputs.insert(0, (rl_id, 0));
        }
        c.viewer_node = Some(viewer_id);
        c
    }

    /// Add a node to the graph.
    pub fn add_node(
        &mut self,
        name: impl Into<String>,
        kind: CompositorNodeKind,
        position: [f32; 2],
    ) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        let node = CompositorNode::new(id, name, kind, position);
        self.nodes.insert(id, node);
        id
    }

    /// Remove a node and all connections to it.
    pub fn remove_node(&mut self, id: NodeId) {
        self.nodes.remove(&id);
        // Remove all connections pointing to removed node
        for node in self.nodes.values_mut() {
            node.inputs.retain(|_, conn| conn.0 != id);
        }
    }

    /// Connect output of `src_node` (slot `src_slot`) to input of `dst_node` (slot `dst_slot`).
    pub fn connect(
        &mut self,
        src_node: NodeId,
        src_slot: usize,
        dst_node: NodeId,
        dst_slot: usize,
    ) {
        if let Some(node) = self.nodes.get_mut(&dst_node) {
            node.inputs.insert(dst_slot, (src_node, src_slot));
        }
    }

    /// Execute the compositor graph. Requires a render layer image to start from.
    pub fn execute(&mut self, render_layer: Option<CompositorImage>) -> Option<CompositorImage> {
        // Topological sort (simple DFS)
        let sorted = self.topological_sort();
        let default_img = CompositorImage::blank(
            render_layer.as_ref().map(|r| r.width).unwrap_or(256),
            render_layer.as_ref().map(|r| r.height).unwrap_or(256),
        );

        // Cache: node_id → output_slot → image
        let mut cache: HashMap<NodeId, Vec<Option<CompositorImage>>> = HashMap::new();

        for node_id in &sorted {
            let node = match self.nodes.get(node_id) {
                Some(n) => n.clone(),
                None => continue,
            };

            let inputs: Vec<Option<CompositorImage>> = (0..node.num_inputs())
                .map(|slot| {
                    node.inputs.get(&slot).and_then(|&(src_id, src_slot)| {
                        cache
                            .get(&src_id)
                            .and_then(|v| v.get(src_slot))
                            .and_then(|o| o.clone())
                    })
                })
                .collect();

            let output_images: Vec<Option<CompositorImage>> = match &node.kind {
                CompositorNodeKind::RenderLayer { .. } => {
                    vec![render_layer.clone().or_else(|| Some(default_img.clone()))]
                }
                CompositorNodeKind::Image { path, data } => {
                    if let Some(img) = data {
                        vec![Some(img.clone())]
                    } else {
                        // Placeholder: return a checkerboard or blank
                        eprintln!("[compositor] Image not loaded: {}", path);
                        vec![Some(default_img.clone())]
                    }
                }
                CompositorNodeKind::Composite => {
                    // Store result as compositor output
                    if let Some(img) = inputs.first().and_then(|o| o.as_ref()) {
                        self.output = Some(img.clone());
                    }
                    vec![]
                }
                CompositorNodeKind::Viewer => {
                    // Viewer just passes through for now
                    vec![]
                }
                CompositorNodeKind::FileOutput { path, .. } => {
                    if let Some(img) = inputs.first().and_then(|o| o.as_ref()) {
                        eprintln!(
                            "[compositor] FileOutput (not yet writing): {} ({}x{})",
                            path, img.width, img.height
                        );
                    }
                    vec![]
                }
                CompositorNodeKind::BrightnessContrast {
                    brightness,
                    contrast,
                } => {
                    let b = *brightness;
                    let c = *contrast;
                    let img = inputs
                        .first()
                        .and_then(|o| o.as_ref())
                        .cloned()
                        .unwrap_or_else(|| default_img.clone());
                    let out = apply_brightness_contrast(&img, b, c);
                    vec![Some(out)]
                }
                CompositorNodeKind::RgbCurves { curves } => {
                    let curves = curves.clone();
                    let img = inputs
                        .first()
                        .and_then(|o| o.as_ref())
                        .cloned()
                        .unwrap_or_else(|| default_img.clone());
                    let out = apply_rgb_curves(&img, &curves);
                    vec![Some(out)]
                }
                CompositorNodeKind::HueSaturation {
                    hue,
                    saturation,
                    value,
                } => {
                    let h = *hue;
                    let s = *saturation;
                    let v = *value;
                    let img = inputs
                        .first()
                        .and_then(|o| o.as_ref())
                        .cloned()
                        .unwrap_or_else(|| default_img.clone());
                    let out = apply_hue_saturation(&img, h, s, v);
                    vec![Some(out)]
                }
                CompositorNodeKind::Mix { mode, factor } => {
                    let mode = *mode;
                    let factor = *factor;
                    let img1 = inputs
                        .get(0)
                        .and_then(|o| o.as_ref())
                        .cloned()
                        .unwrap_or_else(|| default_img.clone());
                    let img2 = inputs
                        .get(1)
                        .and_then(|o| o.as_ref())
                        .cloned()
                        .unwrap_or_else(|| default_img.clone());
                    let out = apply_mix(&img1, &img2, mode, factor);
                    vec![Some(out)]
                }
            };

            cache.insert(*node_id, output_images.clone());
            // Update cache into node
            if let Some(node_mut) = self.nodes.get_mut(node_id) {
                node_mut.output_cache = output_images;
            }
        }

        self.output.clone()
    }

    /// Topological sort of the node graph (returns node IDs in execution order).
    fn topological_sort(&self) -> Vec<NodeId> {
        let mut visited = std::collections::HashSet::new();
        let mut order = Vec::new();

        fn dfs(
            id: NodeId,
            nodes: &HashMap<NodeId, CompositorNode>,
            visited: &mut std::collections::HashSet<NodeId>,
            order: &mut Vec<NodeId>,
        ) {
            if visited.contains(&id) {
                return;
            }
            visited.insert(id);
            if let Some(node) = nodes.get(&id) {
                for &(src_id, _) in node.inputs.values() {
                    dfs(src_id, nodes, visited, order);
                }
            }
            order.push(id);
        }

        for &id in self.nodes.keys() {
            dfs(id, &self.nodes, &mut visited, &mut order);
        }
        order
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Image Processing Operations
// ─────────────────────────────────────────────────────────────────────────────

/// Apply Brightness/Contrast adjustment.
pub fn apply_brightness_contrast(
    img: &CompositorImage,
    brightness: f32,
    contrast: f32,
) -> CompositorImage {
    let mut out = img.clone();
    // Standard formula: out = (in - 0.5) * contrast + 0.5 + brightness
    for px in &mut out.pixels {
        for c in 0..3 {
            let v = (px[c] - 0.5) * (1.0 + contrast) + 0.5 + brightness;
            px[c] = v.clamp(0.0, 1.0);
        }
    }
    out
}

/// Apply RGB Curves.
pub fn apply_rgb_curves(img: &CompositorImage, curves: &RgbCurves) -> CompositorImage {
    let mut out = img.clone();
    for px in &mut out.pixels {
        *px = curves.apply(*px);
    }
    out
}

/// Apply Hue/Saturation/Value adjustment.
pub fn apply_hue_saturation(
    img: &CompositorImage,
    hue_offset: f32,
    sat_scale: f32,
    val_scale: f32,
) -> CompositorImage {
    let mut out = img.clone();
    for px in &mut out.pixels {
        let (h, s, v) = rgb_to_hsv(px[0], px[1], px[2]);
        let h2 = (h + hue_offset).fract();
        let s2 = (s * sat_scale).clamp(0.0, 1.0);
        let v2 = (v * val_scale).clamp(0.0, 1.0);
        let (r, g, b) = hsv_to_rgb(h2, s2, v2);
        *px = [r, g, b, px[3]];
    }
    out
}

/// Apply Mix blend between two images.
pub fn apply_mix(
    img1: &CompositorImage,
    img2: &CompositorImage,
    mode: MixBlendMode,
    factor: f32,
) -> CompositorImage {
    let w = img1.width.max(img2.width);
    let h = img1.height.max(img2.height);
    let mut out = CompositorImage::blank(w, h);
    for y in 0..h {
        for x in 0..w {
            let a = img1.get(x.min(img1.width - 1), y.min(img1.height - 1));
            let b = img2.get(x.min(img2.width - 1), y.min(img2.height - 1));
            let blended = blend_pixels(a, b, mode, factor);
            out.set(x, y, blended);
        }
    }
    out
}

fn blend_pixels(a: [f32; 4], b: [f32; 4], mode: MixBlendMode, factor: f32) -> [f32; 4] {
    let mut result = [0.0f32; 4];
    for i in 0..3 {
        let blended = match mode {
            MixBlendMode::Mix => a[i] + factor * (b[i] - a[i]),
            MixBlendMode::Add => (a[i] + b[i] * factor).clamp(0.0, 1.0),
            MixBlendMode::Subtract => (a[i] - b[i] * factor).clamp(0.0, 1.0),
            MixBlendMode::Multiply => a[i] * (1.0 - factor + factor * b[i]),
            MixBlendMode::Screen => 1.0 - (1.0 - a[i]) * (1.0 - b[i] * factor),
            MixBlendMode::Overlay => {
                if a[i] < 0.5 {
                    2.0 * a[i] * (b[i] * factor + (1.0 - factor) * 0.5)
                } else {
                    1.0 - 2.0 * (1.0 - a[i]) * (1.0 - b[i] * factor)
                }
            }
            MixBlendMode::Darken => a[i].min(b[i]).max(a[i] * (1.0 - factor)),
            MixBlendMode::Lighten => a[i].max(b[i] * factor),
            MixBlendMode::Difference => (a[i] - b[i] * factor).abs(),
        };
        result[i] = blended.clamp(0.0, 1.0);
    }
    result[3] = a[3]; // preserve alpha from input 1
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Color Space Conversions
// ─────────────────────────────────────────────────────────────────────────────

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let v = max;
    let s = if max > 1e-6 { delta / max } else { 0.0 };
    let h = if delta < 1e-6 {
        0.0
    } else if max == r {
        (g - b) / delta % 6.0
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    let h = (h / 6.0).rem_euclid(1.0);
    (h, s, v)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s < 1e-6 {
        return (v, v, v);
    }
    let i = (h * 6.0).floor() as i32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Compositor UI Panel
// ─────────────────────────────────────────────────────────────────────────────

use eframe::egui;

/// Show the compositor node graph editor.
pub fn show_compositor_panel(
    ui: &mut egui::Ui,
    compositor: &mut Compositor,
    render_layer: Option<&CompositorImage>,
) {
    ui.heading("Compositor");
    ui.separator();

    // Execute button
    if ui.button("▶ Execute Compositor").clicked() {
        compositor.execute(render_layer.cloned());
    }

    ui.separator();

    // Add node buttons
    ui.label("Add Node:");
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("Render Layer").clicked() {
            compositor.add_node(
                "Render Layer",
                CompositorNodeKind::RenderLayer {
                    layer_name: "View Layer".into(),
                },
                [200.0, 100.0],
            );
        }
        if ui.small_button("Image").clicked() {
            compositor.add_node(
                "Image",
                CompositorNodeKind::Image {
                    path: "".into(),
                    data: None,
                },
                [200.0, 150.0],
            );
        }
        if ui.small_button("Brightness/Contrast").clicked() {
            compositor.add_node(
                "Brightness/Contrast",
                CompositorNodeKind::BrightnessContrast {
                    brightness: 0.0,
                    contrast: 0.0,
                },
                [300.0, 100.0],
            );
        }
        if ui.small_button("RGB Curves").clicked() {
            compositor.add_node(
                "RGB Curves",
                CompositorNodeKind::RgbCurves {
                    curves: RgbCurves::default(),
                },
                [300.0, 150.0],
            );
        }
        if ui.small_button("Hue/Saturation").clicked() {
            compositor.add_node(
                "Hue/Saturation",
                CompositorNodeKind::HueSaturation {
                    hue: 0.0,
                    saturation: 1.0,
                    value: 1.0,
                },
                [300.0, 200.0],
            );
        }
        if ui.small_button("Mix").clicked() {
            compositor.add_node(
                "Mix",
                CompositorNodeKind::Mix {
                    mode: MixBlendMode::Mix,
                    factor: 0.5,
                },
                [300.0, 250.0],
            );
        }
        if ui.small_button("File Output").clicked() {
            compositor.add_node(
                "File Output",
                CompositorNodeKind::FileOutput {
                    path: "output.png".into(),
                    format: "png".into(),
                },
                [500.0, 100.0],
            );
        }
    });

    ui.separator();

    // Node list and parameters
    ui.label("Nodes:");
    let node_ids: Vec<NodeId> = compositor.nodes.keys().copied().collect();
    let mut to_remove: Option<NodeId> = None;

    for id in node_ids {
        if let Some(node) = compositor.nodes.get_mut(&id) {
            let label = format!("[{}] {}", id, node.name);
            ui.horizontal(|ui| {
                let header = egui::CollapsingHeader::new(&label).id_salt(id);
                header.show(ui, |ui| {
                    show_node_params(ui, node);
                });
                if ui.small_button("Remove").clicked() {
                    to_remove = Some(id);
                }
            });
        }
    }

    if let Some(id) = to_remove {
        compositor.remove_node(id);
    }

    // Show output thumbnail
    ui.separator();
    if let Some(out) = &compositor.output {
        ui.label(format!("Output: {}×{} px", out.width, out.height));
        // Draw a small color preview (average color of output)
        let avg = out.pixels.iter().fold([0.0f32; 4], |mut acc, px| {
            acc[0] += px[0];
            acc[1] += px[1];
            acc[2] += px[2];
            acc[3] += px[3];
            acc
        });
        let n = out.pixels.len() as f32;
        let avg_color = egui::Color32::from_rgb(
            ((avg[0] / n) * 255.0) as u8,
            ((avg[1] / n) * 255.0) as u8,
            ((avg[2] / n) * 255.0) as u8,
        );
        let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 80.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 4.0, avg_color);
        ui.label("(avg color preview)");
    } else {
        ui.label("(no output yet — press Execute)");
    }
}

fn show_node_params(ui: &mut egui::Ui, node: &mut CompositorNode) {
    ui.label(format!(
        "Inputs: {}  Outputs: {}",
        node.num_inputs(),
        node.output_labels().len()
    ));

    match &mut node.kind {
        CompositorNodeKind::RenderLayer { layer_name } => {
            ui.horizontal(|ui| {
                ui.label("Layer:");
                ui.text_edit_singleline(layer_name);
            });
        }
        CompositorNodeKind::Image { path, .. } => {
            ui.horizontal(|ui| {
                ui.label("Path:");
                ui.text_edit_singleline(path);
            });
        }
        CompositorNodeKind::BrightnessContrast {
            brightness,
            contrast,
        } => {
            ui.add(egui::Slider::new(brightness, -1.0..=1.0).text("Brightness"));
            ui.add(egui::Slider::new(contrast, -1.0..=1.0).text("Contrast"));
        }
        CompositorNodeKind::RgbCurves { curves: _ } => {
            ui.label("(RGB Curves — use graph editor for full control)");
            // Master gamma shortcut
            let mut master_gamma = 1.0f32;
            ui.add(egui::Slider::new(&mut master_gamma, 0.1..=3.0).text("Master Gamma"));
            // In full implementation, this would modify the curve control points
            let _ = master_gamma;
        }
        CompositorNodeKind::HueSaturation {
            hue,
            saturation,
            value,
        } => {
            ui.add(egui::Slider::new(hue, 0.0..=1.0).text("Hue"));
            ui.add(egui::Slider::new(saturation, 0.0..=2.0).text("Saturation"));
            ui.add(egui::Slider::new(value, 0.0..=2.0).text("Value"));
        }
        CompositorNodeKind::Mix { mode, factor } => {
            ui.horizontal(|ui| {
                ui.label("Mode:");
                egui::ComboBox::from_id_salt(format!("mix_mode_{}", node.id))
                    .selected_text(mode.label())
                    .show_ui(ui, |ui| {
                        for m in MixBlendMode::all() {
                            ui.selectable_value(mode, *m, m.label());
                        }
                    });
            });
            ui.add(egui::Slider::new(factor, 0.0..=1.0).text("Factor"));
        }
        CompositorNodeKind::FileOutput { path, format } => {
            ui.horizontal(|ui| {
                ui.label("Path:");
                ui.text_edit_singleline(path);
            });
            ui.horizontal(|ui| {
                ui.label("Format:");
                ui.text_edit_singleline(format);
            });
        }
        _ => {}
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compositor_image_solid() {
        let img = CompositorImage::solid(4, 4, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(img.pixels.len(), 16);
        assert_eq!(img.get(0, 0), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(img.get(3, 3), [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_brightness_contrast() {
        let img = CompositorImage::solid(2, 2, [0.5, 0.5, 0.5, 1.0]);
        let out = apply_brightness_contrast(&img, 0.1, 0.0);
        assert!(
            out.get(0, 0)[0] > 0.5,
            "Brightness should increase channel value"
        );
    }

    #[test]
    fn test_brightness_clamps() {
        let img = CompositorImage::solid(2, 2, [0.9, 0.9, 0.9, 1.0]);
        let out = apply_brightness_contrast(&img, 1.0, 1.0);
        assert!(out.get(0, 0)[0] <= 1.0, "Should clamp to 1.0");
    }

    #[test]
    fn test_hue_saturation_identity() {
        let img = CompositorImage::solid(4, 4, [0.8, 0.3, 0.1, 1.0]);
        let out = apply_hue_saturation(&img, 0.0, 1.0, 1.0);
        let px = out.get(0, 0);
        assert!(
            (px[0] - 0.8).abs() < 0.01,
            "Identity hue/sat should preserve red"
        );
    }

    #[test]
    fn test_mix_add() {
        let img1 = CompositorImage::solid(4, 4, [0.3, 0.3, 0.3, 1.0]);
        let img2 = CompositorImage::solid(4, 4, [0.3, 0.3, 0.3, 1.0]);
        let out = apply_mix(&img1, &img2, MixBlendMode::Add, 1.0);
        let px = out.get(0, 0);
        assert!(px[0] >= 0.59 && px[0] <= 0.61, "Add blend 0.3+0.3=0.6");
    }

    #[test]
    fn test_mix_darken() {
        let img1 = CompositorImage::solid(4, 4, [0.8, 0.8, 0.8, 1.0]);
        let img2 = CompositorImage::solid(4, 4, [0.2, 0.2, 0.2, 1.0]);
        let out = apply_mix(&img1, &img2, MixBlendMode::Darken, 1.0);
        let px = out.get(0, 0);
        assert!(px[0] <= 0.8, "Darken should produce a value <= brightest");
    }

    #[test]
    fn test_rgb_curves_identity() {
        let img = CompositorImage::solid(4, 4, [0.7, 0.4, 0.2, 1.0]);
        let out = apply_rgb_curves(&img, &RgbCurves::default());
        let px = out.get(0, 0);
        assert!(
            (px[0] - 0.7).abs() < 0.01,
            "Identity curves should preserve red"
        );
        assert!(
            (px[1] - 0.4).abs() < 0.01,
            "Identity curves should preserve green"
        );
    }

    #[test]
    fn test_compositor_execute_basic() {
        let mut comp = Compositor::new();
        let render_input = CompositorImage::solid(16, 16, [0.5, 0.5, 0.5, 1.0]);
        let result = comp.execute(Some(render_input));
        assert!(result.is_some(), "Compositor should produce output");
        let out = result.unwrap();
        assert_eq!(out.width, 16);
        assert_eq!(out.height, 16);
    }

    #[test]
    fn test_compositor_add_remove_node() {
        let mut comp = Compositor::new();
        let n_before = comp.nodes.len();
        let id = comp.add_node(
            "Test Mix",
            CompositorNodeKind::Mix {
                mode: MixBlendMode::Mix,
                factor: 0.5,
            },
            [100.0, 100.0],
        );
        assert_eq!(comp.nodes.len(), n_before + 1);
        comp.remove_node(id);
        assert_eq!(comp.nodes.len(), n_before);
    }

    #[test]
    fn test_rgb_to_hsv_roundtrip() {
        let (h, s, v) = rgb_to_hsv(0.8, 0.3, 0.1);
        let (r, g, b) = hsv_to_rgb(h, s, v);
        assert!((r - 0.8).abs() < 0.01, "RGB round-trip r");
        assert!((g - 0.3).abs() < 0.01, "RGB round-trip g");
        assert!((b - 0.1).abs() < 0.01, "RGB round-trip b");
    }
}
