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

//! Animation timeline panel.

/// Keyframe data.
#[derive(Debug, Clone, Copy)]
pub struct Keyframe {
    pub frame: i32,
    pub value: f64,
    pub interpolation: Interpolation,
}

/// Interpolation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    Constant,
    Linear,
    Bezier,
}

/// Animation channel (e.g., position.x, rotation.y).
#[derive(Debug, Clone)]
pub struct AnimationChannel {
    pub name: String,
    pub keyframes: Vec<Keyframe>,
}

/// Timeline panel.
#[derive(Debug, Clone)]
pub struct TimelinePanel {
    /// Current frame.
    pub current_frame: i32,
    /// Start frame of playable range.
    pub start_frame: i32,
    /// End frame of playable range.
    pub end_frame: i32,
    /// Frames per second.
    pub fps: f64,
    /// Is animation playing.
    pub playing: bool,
    /// Animation channels with keyframes.
    pub channels: Vec<AnimationChannel>,
    /// Zoom level (pixels per frame).
    pub zoom: f64,
    /// Scroll offset.
    pub scroll_offset: f64,
}

impl TimelinePanel {
    /// Create a new timeline panel.
    pub fn new() -> Self {
        Self {
            current_frame: 0,
            start_frame: 0,
            end_frame: 250,
            fps: 24.0,
            playing: false,
            channels: Vec::new(),
            zoom: 4.0,
            scroll_offset: 0.0,
        }
    }

    /// Show the timeline panel.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Timeline");

            // Playback controls
            if ui.button(if self.playing { "⏸" } else { "▶" }).clicked() {
                self.playing = !self.playing;
            }
            if ui.button("⏹").clicked() {
                self.playing = false;
                self.current_frame = self.start_frame;
            }
            if ui.button("⏮").clicked() {
                self.current_frame = self.start_frame;
            }
            if ui.button("⏭").clicked() {
                self.current_frame = self.end_frame;
            }

            ui.separator();

            // Frame display
            ui.label(format!("Frame: {}", self.current_frame));
            ui.add(egui::DragValue::new(&mut self.current_frame).speed(1.0));

            ui.separator();

            // FPS
            ui.label("FPS:");
            ui.add(
                egui::DragValue::new(&mut self.fps)
                    .speed(1.0)
                    .range(1.0..=120.0),
            );

            ui.separator();

            // Range
            ui.label("Range:");
            ui.add(egui::DragValue::new(&mut self.start_frame).speed(1.0));
            ui.label("-");
            ui.add(egui::DragValue::new(&mut self.end_frame).speed(1.0));
        });

        ui.separator();

        // Timeline scrubber
        self.draw_scrubber(ui);

        ui.separator();

        // Keyframe track
        self.draw_keyframe_track(ui);
    }

    /// Draw timeline scrubber.
    fn draw_scrubber(&mut self, ui: &mut egui::Ui) {
        let available_width = ui.available_width();
        let height = 40.0;

        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(available_width, height),
            egui::Sense::click_and_drag(),
        );

        // Background
        painter.rect_filled(response.rect, 0.0, egui::Color32::from_gray(30));

        // Frame marks
        let frame_range = (self.end_frame - self.start_frame) as f64;
        let pixels_per_frame = available_width / frame_range as f32;

        for i in self.start_frame..=self.end_frame {
            let x = response.rect.left() + (i - self.start_frame) as f32 * pixels_per_frame;
            let y_top = response.rect.top();
            let _y_bottom = response.rect.bottom();

            // Draw frame tick
            let tick_height = if i % 10 == 0 {
                10.0
            } else if i % 5 == 0 {
                5.0
            } else {
                2.0
            };
            painter.line_segment(
                [egui::pos2(x, y_top), egui::pos2(x, y_top + tick_height)],
                egui::Stroke::new(1.0_f32, egui::Color32::from_gray(100)),
            );

            // Draw frame number every 10 frames
            if i % 10 == 0 {
                painter.text(
                    egui::pos2(x, y_top + 15.0),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i),
                    egui::FontId::default(),
                    egui::Color32::from_gray(200),
                );
            }
        }

        // Current frame indicator
        let current_x = response.rect.left()
            + (self.current_frame - self.start_frame) as f32 * pixels_per_frame;
        painter.line_segment(
            [
                egui::pos2(current_x, response.rect.top()),
                egui::pos2(current_x, response.rect.bottom()),
            ],
            egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(255, 200, 0)),
        );

        // Handle click to jump to frame
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let relative_x = pos.x - response.rect.left();
                let frame = self.start_frame + (relative_x / pixels_per_frame) as i32;
                self.current_frame = frame.clamp(self.start_frame, self.end_frame);
            }
        }
    }

    /// Draw keyframe track.
    fn draw_keyframe_track(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for channel in &self.channels {
                ui.horizontal(|ui| {
                    ui.label(&channel.name);

                    // Draw keyframe diamonds
                    for kf in &channel.keyframes {
                        if kf.frame >= self.start_frame
                            && kf.frame <= self.end_frame
                            && ui.small_button("◆").clicked()
                        {
                            self.current_frame = kf.frame;
                        }
                    }
                });
            }

            if self.channels.is_empty() {
                ui.label("No animation channels");
            }
        });
    }

    /// Update animation (call every frame when playing).
    pub fn update(&mut self, delta_time: f64) {
        if self.playing {
            let frames_to_advance = (delta_time * self.fps) as i32;
            self.current_frame += frames_to_advance;

            // Loop at end
            if self.current_frame > self.end_frame {
                self.current_frame = self.start_frame;
            }
        }
    }

    /// Add keyframe at current frame.
    pub fn add_keyframe(&mut self, channel_name: &str, value: f64) {
        if let Some(channel) = self.channels.iter_mut().find(|c| c.name == channel_name) {
            channel.keyframes.push(Keyframe {
                frame: self.current_frame,
                value,
                interpolation: Interpolation::Linear,
            });
            channel.keyframes.sort_by_key(|kf| kf.frame);
        }
    }

    /// Get current time in seconds.
    pub fn get_time(&self) -> f64 {
        self.current_frame as f64 / self.fps
    }
}

impl Default for TimelinePanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeline_panel() {
        let panel = TimelinePanel::new();
        assert_eq!(panel.current_frame, 0);
        assert_eq!(panel.fps, 24.0);
    }

    #[test]
    fn test_playback() {
        let mut panel = TimelinePanel::new();
        panel.end_frame = 100;
        panel.playing = true;

        panel.update(1.0); // Advance 24 frames at 24fps
        assert_eq!(panel.current_frame, 24);
    }

    #[test]
    fn test_get_time() {
        let mut panel = TimelinePanel::new();
        panel.current_frame = 24;
        panel.fps = 24.0;
        assert_eq!(panel.get_time(), 1.0);
    }
}
