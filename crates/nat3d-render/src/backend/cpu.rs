// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! CPU fallback renderer.

use nalgebra::{Matrix4, Point3, Vector3};

/// CPU framebuffer.
pub struct CpuFramebuffer {
    /// Width of the framebuffer in pixels.
    pub width: usize,
    /// Height of the framebuffer in pixels.
    pub height: usize,
    /// Color buffer data (RGBA8).
    pub color: Vec<u8>,
    /// Depth buffer data (F32).
    pub depth: Vec<f32>,
}

impl CpuFramebuffer {
    /// Creates a new CPU framebuffer with the specified dimensions.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            color: vec![0; width * height * 4],
            depth: vec![f32::INFINITY; width * height],
        }
    }

    /// Clears the color buffer with the specified RGBA values and resets the depth buffer.
    pub fn clear_color(&mut self, r: u8, g: u8, b: u8, a: u8) {
        for i in 0..self.width * self.height {
            self.color[i * 4] = r;
            self.color[i * 4 + 1] = g;
            self.color[i * 4 + 2] = b;
            self.color[i * 4 + 3] = a;
        }
        self.depth.fill(f32::INFINITY);
    }

    /// Returns a reference to the color buffer data.
    pub fn color_buffer(&self) -> &[u8] {
        &self.color
    }

    /// Draws a triangle in screen space (simplified rasterizer).
    pub fn draw_triangle_screen(
        &mut self,
        p0: Point3<f32>,
        p1: Point3<f32>,
        p2: Point3<f32>,
        color: [u8; 3],
    ) {
        let min_x = p0.x.min(p1.x).min(p2.x).max(0.0) as usize;
        let max_x = p0.x.max(p1.x).max(p2.x).min((self.width - 1) as f32) as usize;
        let min_y = p0.y.min(p1.y).min(p2.y).max(0.0) as usize;
        let max_y = p0.y.max(p1.y).max(p2.y).min((self.height - 1) as f32) as usize;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;

                let w0 = edge_function(p1.x, p1.y, p2.x, p2.y, px, py);
                let w1 = edge_function(p2.x, p2.y, p0.x, p0.y, px, py);
                let w2 = edge_function(p0.x, p0.y, p1.x, p1.y, px, py);

                if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0) {
                    let idx = (y * self.width + x) * 4;
                    self.color[idx] = color[0];
                    self.color[idx + 1] = color[1];
                    self.color[idx + 2] = color[2];
                    self.color[idx + 3] = 255;
                }
            }
        }
    }
}

#[inline]
fn edge_function(ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32) -> f32 {
    (cx - ax) * (by - ay) - (cy - ay) * (bx - ax)
}

/// Simple directional light for CPU rendering.
pub struct CpuLight {
    /// Direction of the light.
    pub direction: Vector3<f32>,
    /// Color of the light (RGB).
    pub color: [f32; 3],
    /// Intensity of the light.
    pub intensity: f32,
}

/// CPU-based software renderer.
pub struct CpuRenderer {
    framebuffer: CpuFramebuffer,
    view: Matrix4<f32>,
    projection: Matrix4<f32>,
    light: CpuLight,
    ambient: [f32; 3],
}

impl CpuRenderer {
    /// Creates a new CPU renderer with the specified dimensions.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            framebuffer: CpuFramebuffer::new(width, height),
            view: Matrix4::identity(),
            projection: Matrix4::identity(),
            light: CpuLight {
                direction: Vector3::new(0.0, -1.0, 0.0),
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
            },
            ambient: [0.1, 0.1, 0.1],
        }
    }

    /// Returns a mutable reference to the underlying framebuffer.
    pub fn framebuffer_mut(&mut self) -> &mut CpuFramebuffer {
        &mut self.framebuffer
    }

    /// Clears the color buffer.
    pub fn clear_color(&mut self, r: u8, g: u8, b: u8, a: u8) {
        self.framebuffer.clear_color(r, g, b, a);
    }

    /// Returns the color buffer data.
    pub fn color_buffer(&self) -> &[u8] {
        self.framebuffer.color_buffer()
    }

    /// Sets the view matrix.
    pub fn set_view(&mut self, view: Matrix4<f32>, _camera_pos: Point3<f32>) {
        self.view = view;
    }

    /// Sets the projection matrix.
    pub fn set_projection(&mut self, projection: Matrix4<f32>) {
        self.projection = projection;
    }

    /// Sets the light source.
    pub fn set_light(&mut self, light: CpuLight) {
        self.light = light;
    }
}
