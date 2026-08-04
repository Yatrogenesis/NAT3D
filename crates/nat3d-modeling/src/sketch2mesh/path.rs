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

//! 2D path representation for sketch-to-mesh conversion.

use std::f32::consts::PI;

/// 2D point.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    /// Create a new point.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Distance to another point.
    pub fn distance(&self, other: &Point2D) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Lerp between two points.
    pub fn lerp(&self, other: &Point2D, t: f32) -> Point2D {
        Point2D {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }

    /// Rotate around origin.
    pub fn rotate(&self, angle: f32) -> Point2D {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        Point2D {
            x: self.x * cos_a - self.y * sin_a,
            y: self.x * sin_a + self.y * cos_a,
        }
    }

    /// Scale.
    pub fn scale(&self, factor: f32) -> Point2D {
        Point2D {
            x: self.x * factor,
            y: self.y * factor,
        }
    }

    /// Add offset.
    pub fn offset(&self, dx: f32, dy: f32) -> Point2D {
        Point2D {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

impl From<[f32; 2]> for Point2D {
    fn from(arr: [f32; 2]) -> Self {
        Self {
            x: arr[0],
            y: arr[1],
        }
    }
}

impl From<(f32, f32)> for Point2D {
    fn from(tuple: (f32, f32)) -> Self {
        Self {
            x: tuple.0,
            y: tuple.1,
        }
    }
}

/// Path command types (similar to SVG).
#[derive(Debug, Clone)]
pub enum PathCommand {
    /// Move to position (start new subpath)
    MoveTo(Point2D),
    /// Line to position
    LineTo(Point2D),
    /// Quadratic bezier curve
    QuadraticTo { control: Point2D, end: Point2D },
    /// Cubic bezier curve
    CubicTo {
        control1: Point2D,
        control2: Point2D,
        end: Point2D,
    },
    /// Arc
    ArcTo {
        radius: Point2D,
        x_rotation: f32,
        large_arc: bool,
        sweep: bool,
        end: Point2D,
    },
    /// Close path (line back to start)
    Close,
}

/// A 2D path made of commands.
#[derive(Debug, Clone, Default)]
pub struct Path2D {
    commands: Vec<PathCommand>,
    current_pos: Point2D,
    start_pos: Point2D,
}

impl Path2D {
    /// Create a new empty path.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            current_pos: Point2D::new(0.0, 0.0),
            start_pos: Point2D::new(0.0, 0.0),
        }
    }

    /// Move to position.
    pub fn move_to(&mut self, x: f32, y: f32) -> &mut Self {
        let p = Point2D::new(x, y);
        self.commands.push(PathCommand::MoveTo(p));
        self.current_pos = p;
        self.start_pos = p;
        self
    }

    /// Line to position.
    pub fn line_to(&mut self, x: f32, y: f32) -> &mut Self {
        let p = Point2D::new(x, y);
        self.commands.push(PathCommand::LineTo(p));
        self.current_pos = p;
        self
    }

    /// Quadratic bezier curve.
    pub fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) -> &mut Self {
        let control = Point2D::new(cx, cy);
        let end = Point2D::new(x, y);
        self.commands
            .push(PathCommand::QuadraticTo { control, end });
        self.current_pos = end;
        self
    }

    /// Cubic bezier curve.
    pub fn cubic_to(
        &mut self,
        c1x: f32,
        c1y: f32,
        c2x: f32,
        c2y: f32,
        x: f32,
        y: f32,
    ) -> &mut Self {
        let control1 = Point2D::new(c1x, c1y);
        let control2 = Point2D::new(c2x, c2y);
        let end = Point2D::new(x, y);
        self.commands.push(PathCommand::CubicTo {
            control1,
            control2,
            end,
        });
        self.current_pos = end;
        self
    }

    /// Arc to position.
    #[allow(clippy::too_many_arguments)]
    pub fn arc_to(&mut self, rx: f32, ry: f32, x_rot: f32, large_arc: bool, sweep: bool, x: f32, y: f32) -> &mut Self {
        let end = Point2D::new(x, y);
        self.commands.push(PathCommand::ArcTo {
            radius: Point2D::new(rx, ry),
            x_rotation: x_rot,
            large_arc,
            sweep,
            end,
        });
        self.current_pos = end;
        self
    }

    /// Close the path.
    pub fn close(&mut self) -> &mut Self {
        self.commands.push(PathCommand::Close);
        self.current_pos = self.start_pos;
        self
    }

    /// Get commands.
    pub fn commands(&self) -> &[PathCommand] {
        &self.commands
    }

    /// Check if path is closed.
    pub fn is_closed(&self) -> bool {
        self.commands
            .last()
            .map(|c| matches!(c, PathCommand::Close))
            .unwrap_or(false)
    }

    /// Tesselate path to points.
    pub fn tesselate(&self, resolution: u32) -> Vec<Point2D> {
        let mut points = Vec::new();
        let mut current = Point2D::new(0.0, 0.0);
        let mut start = current;

        for cmd in &self.commands {
            match cmd {
                PathCommand::MoveTo(p) => {
                    current = *p;
                    start = *p;
                    points.push(*p);
                }
                PathCommand::LineTo(p) => {
                    points.push(*p);
                    current = *p;
                }
                PathCommand::QuadraticTo { control, end } => {
                    // Tesselate quadratic bezier
                    for i in 1..=resolution {
                        let t = i as f32 / resolution as f32;
                        let p = quadratic_bezier(current, *control, *end, t);
                        points.push(p);
                    }
                    current = *end;
                }
                PathCommand::CubicTo {
                    control1,
                    control2,
                    end,
                } => {
                    // Tesselate cubic bezier
                    for i in 1..=resolution {
                        let t = i as f32 / resolution as f32;
                        let p = cubic_bezier(current, *control1, *control2, *end, t);
                        points.push(p);
                    }
                    current = *end;
                }
                PathCommand::ArcTo {
                    radius,
                    x_rotation,
                    large_arc,
                    sweep,
                    end,
                } => {
                    // Tesselate arc
                    let arc_points = tesselate_arc(
                        current,
                        *end,
                        *radius,
                        *x_rotation,
                        *large_arc,
                        *sweep,
                        resolution,
                    );
                    points.extend(arc_points);
                    current = *end;
                }
                PathCommand::Close => {
                    if current.distance(&start) > 0.001 {
                        points.push(start);
                    }
                    current = start;
                }
            }
        }

        points
    }

    /// Get bounding box.
    pub fn bounds(&self) -> (Point2D, Point2D) {
        let points = self.tesselate(8);
        if points.is_empty() {
            return (Point2D::new(0.0, 0.0), Point2D::new(0.0, 0.0));
        }

        let mut min = points[0];
        let mut max = points[0];

        for p in &points {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }

        (min, max)
    }

    /// Create a rectangle path.
    pub fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Self {
        let mut path = Self::new();
        path.move_to(x, y)
            .line_to(x + width, y)
            .line_to(x + width, y + height)
            .line_to(x, y + height)
            .close();
        path
    }

    /// Create a rounded rectangle path.
    pub fn rounded_rectangle(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Self {
        let r = radius.min(width / 2.0).min(height / 2.0);
        let mut path = Self::new();

        path.move_to(x + r, y);
        path.line_to(x + width - r, y);
        path.quad_to(x + width, y, x + width, y + r);
        path.line_to(x + width, y + height - r);
        path.quad_to(x + width, y + height, x + width - r, y + height);
        path.line_to(x + r, y + height);
        path.quad_to(x, y + height, x, y + height - r);
        path.line_to(x, y + r);
        path.quad_to(x, y, x + r, y);
        path.close();

        path
    }

    /// Create a circle path.
    pub fn circle(cx: f32, cy: f32, radius: f32) -> Self {
        Self::ellipse(cx, cy, radius, radius)
    }

    /// Create an ellipse path.
    pub fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Self {
        // Use 4 cubic beziers to approximate ellipse
        let k = 0.552_284_8; // Magic number for cubic bezier circle approximation
        let kx = rx * k;
        let ky = ry * k;

        let mut path = Self::new();
        path.move_to(cx + rx, cy);
        path.cubic_to(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry);
        path.cubic_to(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy);
        path.cubic_to(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry);
        path.cubic_to(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy);
        path.close();
        path
    }

    /// Create a regular polygon path.
    pub fn polygon(cx: f32, cy: f32, radius: f32, sides: u32) -> Self {
        let mut path = Self::new();
        let angle_step = 2.0 * PI / sides as f32;

        for i in 0..sides {
            let angle = angle_step * i as f32 - PI / 2.0; // Start from top
            let x = cx + radius * angle.cos();
            let y = cy + radius * angle.sin();

            if i == 0 {
                path.move_to(x, y);
            } else {
                path.line_to(x, y);
            }
        }
        path.close();
        path
    }

    /// Create a star path.
    pub fn star(cx: f32, cy: f32, outer_radius: f32, inner_radius: f32, points: u32) -> Self {
        let mut path = Self::new();
        let angle_step = PI / points as f32;

        for i in 0..(points * 2) {
            let angle = angle_step * i as f32 - PI / 2.0;
            let r = if i % 2 == 0 {
                outer_radius
            } else {
                inner_radius
            };
            let x = cx + r * angle.cos();
            let y = cy + r * angle.sin();

            if i == 0 {
                path.move_to(x, y);
            } else {
                path.line_to(x, y);
            }
        }
        path.close();
        path
    }

    /// Create a heart shape.
    pub fn heart(cx: f32, cy: f32, size: f32) -> Self {
        let mut path = Self::new();
        let s = size / 2.0;

        path.move_to(cx, cy + s * 0.3);
        path.cubic_to(cx, cy - s * 0.5, cx - s, cy - s * 0.5, cx - s, cy);
        path.cubic_to(cx - s, cy + s * 0.5, cx, cy + s * 0.8, cx, cy + s);
        path.cubic_to(cx, cy + s * 0.8, cx + s, cy + s * 0.5, cx + s, cy);
        path.cubic_to(cx + s, cy - s * 0.5, cx, cy - s * 0.5, cx, cy + s * 0.3);
        path.close();
        path
    }

    /// Transform all points in the path.
    pub fn transform(&mut self, scale: f32, rotate: f32, translate: Point2D) {
        for cmd in &mut self.commands {
            match cmd {
                PathCommand::MoveTo(p) | PathCommand::LineTo(p) => {
                    *p = p
                        .scale(scale)
                        .rotate(rotate)
                        .offset(translate.x, translate.y);
                }
                PathCommand::QuadraticTo { control, end } => {
                    *control = control
                        .scale(scale)
                        .rotate(rotate)
                        .offset(translate.x, translate.y);
                    *end = end
                        .scale(scale)
                        .rotate(rotate)
                        .offset(translate.x, translate.y);
                }
                PathCommand::CubicTo {
                    control1,
                    control2,
                    end,
                } => {
                    *control1 = control1
                        .scale(scale)
                        .rotate(rotate)
                        .offset(translate.x, translate.y);
                    *control2 = control2
                        .scale(scale)
                        .rotate(rotate)
                        .offset(translate.x, translate.y);
                    *end = end
                        .scale(scale)
                        .rotate(rotate)
                        .offset(translate.x, translate.y);
                }
                PathCommand::ArcTo { end, .. } => {
                    *end = end
                        .scale(scale)
                        .rotate(rotate)
                        .offset(translate.x, translate.y);
                }
                PathCommand::Close => {}
            }
        }
    }

    /// Reverse the path direction.
    pub fn reverse(&mut self) {
        // This is complex for beziers, simplified version
        self.commands.reverse();
    }
}

/// Evaluate quadratic bezier at t.
fn quadratic_bezier(p0: Point2D, p1: Point2D, p2: Point2D, t: f32) -> Point2D {
    let t1 = 1.0 - t;
    Point2D {
        x: t1 * t1 * p0.x + 2.0 * t1 * t * p1.x + t * t * p2.x,
        y: t1 * t1 * p0.y + 2.0 * t1 * t * p1.y + t * t * p2.y,
    }
}

/// Evaluate cubic bezier at t.
fn cubic_bezier(p0: Point2D, p1: Point2D, p2: Point2D, p3: Point2D, t: f32) -> Point2D {
    let t1 = 1.0 - t;
    let t1_2 = t1 * t1;
    let t1_3 = t1_2 * t1;
    let t_2 = t * t;
    let t_3 = t_2 * t;

    Point2D {
        x: t1_3 * p0.x + 3.0 * t1_2 * t * p1.x + 3.0 * t1 * t_2 * p2.x + t_3 * p3.x,
        y: t1_3 * p0.y + 3.0 * t1_2 * t * p1.y + 3.0 * t1 * t_2 * p2.y + t_3 * p3.y,
    }
}

/// Tesselate an SVG arc.
fn tesselate_arc(
    start: Point2D,
    end: Point2D,
    radius: Point2D,
    _x_rotation: f32,
    _large_arc: bool,
    _sweep: bool,
    resolution: u32,
) -> Vec<Point2D> {
    // Simplified arc tesselation - treat as line segments
    let mut points = Vec::new();
    let center = Point2D {
        x: (start.x + end.x) / 2.0,
        y: (start.y + end.y) / 2.0,
    };

    // Use actual arc math for better results
    let rx = radius.x.abs().max(0.001);
    let _ry = radius.y.abs().max(0.001);

    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let dist = (dx * dx + dy * dy).sqrt();

    if dist < 0.001 {
        return points;
    }

    // Simple circular interpolation
    for i in 1..=resolution {
        let t = i as f32 / resolution as f32;
        let angle = t * PI;
        let p = Point2D {
            x: center.x
                + (end.x - center.x) * t
                + rx * 0.5 * (1.0 - angle.cos()) * (if dy >= 0.0 { -1.0 } else { 1.0 }),
            y: center.y + (end.y - center.y) * t,
        };
        points.push(p);
    }

    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_operations() {
        let p1 = Point2D::new(0.0, 0.0);
        let p2 = Point2D::new(3.0, 4.0);
        assert!((p1.distance(&p2) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_path_rectangle() {
        let path = Path2D::rectangle(0.0, 0.0, 1.0, 1.0);
        assert!(path.is_closed());
        let points = path.tesselate(1);
        assert_eq!(points.len(), 5); // 4 corners + close
    }

    #[test]
    fn test_path_circle() {
        let path = Path2D::circle(0.0, 0.0, 1.0);
        assert!(path.is_closed());
    }
}
