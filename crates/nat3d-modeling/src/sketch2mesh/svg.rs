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

//! SVG import for sketch-to-mesh conversion.

use super::path::{Path2D, Point2D};

/// SVG importer for converting SVG paths to Path2D.
pub struct SvgImporter;

impl SvgImporter {
    /// Parse SVG path data string (d attribute).
    pub fn parse_path_data(data: &str) -> Result<Path2D, SvgError> {
        let mut path = Path2D::new();
        let mut chars = data.chars().peekable();
        let mut current_cmd = ' ';
        let mut current_pos = Point2D::new(0.0, 0.0);
        let mut start_pos = current_pos;

        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ',' {
                chars.next();
                continue;
            }

            if c.is_alphabetic() {
                current_cmd = chars.next().unwrap();
                // Handle Z/z immediately since it takes no arguments
                if current_cmd.eq_ignore_ascii_case(&'Z') {
                    path.close();
                    current_pos = start_pos;
                }
                continue;
            }

            match current_cmd.to_ascii_uppercase() {
                'M' => {
                    let x = parse_number(&mut chars)?;
                    let y = parse_number(&mut chars)?;
                    let (x, y) = if current_cmd.is_uppercase() {
                        (x, y)
                    } else {
                        (current_pos.x + x, current_pos.y + y)
                    };
                    path.move_to(x, y);
                    current_pos = Point2D::new(x, y);
                    start_pos = current_pos;
                    // Subsequent coordinates are line-to
                    current_cmd = if current_cmd.is_uppercase() { 'L' } else { 'l' };
                }
                'L' => {
                    let x = parse_number(&mut chars)?;
                    let y = parse_number(&mut chars)?;
                    let (x, y) = if current_cmd.is_uppercase() {
                        (x, y)
                    } else {
                        (current_pos.x + x, current_pos.y + y)
                    };
                    path.line_to(x, y);
                    current_pos = Point2D::new(x, y);
                }
                'H' => {
                    let x = parse_number(&mut chars)?;
                    let x = if current_cmd.is_uppercase() {
                        x
                    } else {
                        current_pos.x + x
                    };
                    path.line_to(x, current_pos.y);
                    current_pos.x = x;
                }
                'V' => {
                    let y = parse_number(&mut chars)?;
                    let y = if current_cmd.is_uppercase() {
                        y
                    } else {
                        current_pos.y + y
                    };
                    path.line_to(current_pos.x, y);
                    current_pos.y = y;
                }
                'Q' => {
                    let cx = parse_number(&mut chars)?;
                    let cy = parse_number(&mut chars)?;
                    let x = parse_number(&mut chars)?;
                    let y = parse_number(&mut chars)?;
                    let (cx, cy, x, y) = if current_cmd.is_uppercase() {
                        (cx, cy, x, y)
                    } else {
                        (
                            current_pos.x + cx,
                            current_pos.y + cy,
                            current_pos.x + x,
                            current_pos.y + y,
                        )
                    };
                    path.quad_to(cx, cy, x, y);
                    current_pos = Point2D::new(x, y);
                }
                'C' => {
                    let c1x = parse_number(&mut chars)?;
                    let c1y = parse_number(&mut chars)?;
                    let c2x = parse_number(&mut chars)?;
                    let c2y = parse_number(&mut chars)?;
                    let x = parse_number(&mut chars)?;
                    let y = parse_number(&mut chars)?;
                    let (c1x, c1y, c2x, c2y, x, y) = if current_cmd.is_uppercase() {
                        (c1x, c1y, c2x, c2y, x, y)
                    } else {
                        (
                            current_pos.x + c1x,
                            current_pos.y + c1y,
                            current_pos.x + c2x,
                            current_pos.y + c2y,
                            current_pos.x + x,
                            current_pos.y + y,
                        )
                    };
                    path.cubic_to(c1x, c1y, c2x, c2y, x, y);
                    current_pos = Point2D::new(x, y);
                }
                'A' => {
                    let rx = parse_number(&mut chars)?;
                    let ry = parse_number(&mut chars)?;
                    let x_rot = parse_number(&mut chars)?;
                    let large_arc = parse_flag(&mut chars)?;
                    let sweep = parse_flag(&mut chars)?;
                    let x = parse_number(&mut chars)?;
                    let y = parse_number(&mut chars)?;
                    let (x, y) = if current_cmd.is_uppercase() {
                        (x, y)
                    } else {
                        (current_pos.x + x, current_pos.y + y)
                    };
                    path.arc_to(rx, ry, x_rot, large_arc, sweep, x, y);
                    current_pos = Point2D::new(x, y);
                }
                'Z' => {
                    path.close();
                    current_pos = start_pos;
                }
                _ => {
                    // Skip unknown commands
                    chars.next();
                }
            }
        }

        Ok(path)
    }

    /// Parse a simple SVG file and extract paths.
    pub fn parse_svg(content: &str) -> Result<Vec<Path2D>, SvgError> {
        let mut paths = Vec::new();

        // Simple regex-free SVG parsing
        let mut pos = 0;
        while let Some(path_start) = content[pos..].find("<path") {
            let abs_start = pos + path_start;
            if let Some(path_end) = content[abs_start..]
                .find("/>")
                .or_else(|| content[abs_start..].find("</path>"))
            {
                let path_element = &content[abs_start..abs_start + path_end];

                // Extract d attribute
                if let Some(d_start) = path_element.find("d=\"") {
                    let d_content_start = d_start + 3;
                    if let Some(d_end) = path_element[d_content_start..].find('"') {
                        let d_value = &path_element[d_content_start..d_content_start + d_end];
                        if let Ok(path) = Self::parse_path_data(d_value) {
                            paths.push(path);
                        }
                    }
                }

                pos = abs_start + path_end;
            } else {
                break;
            }
        }

        // Also look for basic shapes and convert them
        // Rectangles
        pos = 0;
        while let Some(rect_start) = content[pos..].find("<rect") {
            let abs_start = pos + rect_start;
            if let Some(rect_end) = content[abs_start..]
                .find("/>")
                .or_else(|| content[abs_start..].find("</rect>"))
            {
                let rect_element = &content[abs_start..abs_start + rect_end];

                let x = extract_attr(rect_element, "x").unwrap_or(0.0);
                let y = extract_attr(rect_element, "y").unwrap_or(0.0);
                let width = extract_attr(rect_element, "width").unwrap_or(100.0);
                let height = extract_attr(rect_element, "height").unwrap_or(100.0);
                let rx = extract_attr(rect_element, "rx").unwrap_or(0.0);

                let path = if rx > 0.0 {
                    Path2D::rounded_rectangle(x, y, width, height, rx)
                } else {
                    Path2D::rectangle(x, y, width, height)
                };
                paths.push(path);

                pos = abs_start + rect_end;
            } else {
                break;
            }
        }

        // Circles
        pos = 0;
        while let Some(circle_start) = content[pos..].find("<circle") {
            let abs_start = pos + circle_start;
            if let Some(circle_end) = content[abs_start..]
                .find("/>")
                .or_else(|| content[abs_start..].find("</circle>"))
            {
                let circle_element = &content[abs_start..abs_start + circle_end];

                let cx = extract_attr(circle_element, "cx").unwrap_or(0.0);
                let cy = extract_attr(circle_element, "cy").unwrap_or(0.0);
                let r = extract_attr(circle_element, "r").unwrap_or(50.0);

                paths.push(Path2D::circle(cx, cy, r));
                pos = abs_start + circle_end;
            } else {
                break;
            }
        }

        // Ellipses
        pos = 0;
        while let Some(ellipse_start) = content[pos..].find("<ellipse") {
            let abs_start = pos + ellipse_start;
            if let Some(ellipse_end) = content[abs_start..]
                .find("/>")
                .or_else(|| content[abs_start..].find("</ellipse>"))
            {
                let ellipse_element = &content[abs_start..abs_start + ellipse_end];

                let cx = extract_attr(ellipse_element, "cx").unwrap_or(0.0);
                let cy = extract_attr(ellipse_element, "cy").unwrap_or(0.0);
                let rx = extract_attr(ellipse_element, "rx").unwrap_or(50.0);
                let ry = extract_attr(ellipse_element, "ry").unwrap_or(50.0);

                paths.push(Path2D::ellipse(cx, cy, rx, ry));
                pos = abs_start + ellipse_end;
            } else {
                break;
            }
        }

        Ok(paths)
    }

    /// Import SVG from file.
    pub fn import_file(path: &std::path::Path) -> Result<Vec<Path2D>, SvgError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| SvgError::IoError(e.to_string()))?;
        Self::parse_svg(&content)
    }
}

fn parse_number<I: Iterator<Item = char>>(
    chars: &mut std::iter::Peekable<I>,
) -> Result<f32, SvgError> {
    // Skip whitespace and commas
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == ',' {
            chars.next();
        } else {
            break;
        }
    }

    let mut num_str = String::new();
    let mut has_dot = false;
    let mut has_exp = false;

    // Handle sign
    if let Some(&c) = chars.peek() {
        if c == '-' || c == '+' {
            num_str.push(chars.next().unwrap());
        }
    }

    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            num_str.push(chars.next().unwrap());
        } else if c == '.' && !has_dot && !has_exp {
            has_dot = true;
            num_str.push(chars.next().unwrap());
        } else if (c == 'e' || c == 'E') && !has_exp {
            has_exp = true;
            num_str.push(chars.next().unwrap());
            if let Some(&sign) = chars.peek() {
                if sign == '-' || sign == '+' {
                    num_str.push(chars.next().unwrap());
                }
            }
        } else {
            break;
        }
    }

    num_str
        .parse()
        .map_err(|_| SvgError::ParseError(format!("Invalid number: {}", num_str)))
}

fn parse_flag<I: Iterator<Item = char>>(
    chars: &mut std::iter::Peekable<I>,
) -> Result<bool, SvgError> {
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == ',' {
            chars.next();
        } else {
            break;
        }
    }

    match chars.next() {
        Some('0') => Ok(false),
        Some('1') => Ok(true),
        other => Err(SvgError::ParseError(format!(
            "Expected flag (0 or 1), got {:?}",
            other
        ))),
    }
}

fn extract_attr(element: &str, attr_name: &str) -> Option<f32> {
    let search = format!("{}=\"", attr_name);
    if let Some(start) = element.find(&search) {
        let value_start = start + search.len();
        if let Some(end) = element[value_start..].find('"') {
            let value = &element[value_start..value_start + end];
            // Remove units like px, pt, etc
            let value = value.trim_end_matches(|c: char| c.is_alphabetic() || c == '%');
            return value.parse().ok();
        }
    }
    None
}

/// SVG parsing errors.
#[derive(Debug, Clone)]
pub enum SvgError {
    IoError(String),
    ParseError(String),
    InvalidFormat(String),
}

impl std::fmt::Display for SvgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::ParseError(e) => write!(f, "Parse error: {}", e),
            Self::InvalidFormat(e) => write!(f, "Invalid format: {}", e),
        }
    }
}

impl std::error::Error for SvgError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_path() {
        let path = SvgImporter::parse_path_data("M 0 0 L 100 0 L 100 100 L 0 100 Z").unwrap();
        assert!(path.is_closed());
    }

    #[test]
    fn test_parse_curve() {
        let path = SvgImporter::parse_path_data("M 0 0 Q 50 50 100 0").unwrap();
        assert!(!path.is_closed());
    }

    #[test]
    fn test_parse_relative() {
        let path = SvgImporter::parse_path_data("m 0 0 l 100 0 l 0 100 z").unwrap();
        assert!(path.is_closed());
    }
}
