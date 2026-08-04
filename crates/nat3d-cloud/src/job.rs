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

//! Cloud render jobs.
//!
//! Defines render job configurations and status tracking.

use std::time::{Duration, SystemTime};

/// Render job status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    /// Job is waiting in queue
    Queued,
    /// Job is being prepared
    Preparing,
    /// Job is actively rendering
    Rendering,
    /// Post-processing (denoising, compositing)
    PostProcessing,
    /// Job completed successfully
    Completed,
    /// Job failed with error
    Failed,
    /// Job was cancelled
    Cancelled,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "Queued"),
            Self::Preparing => write!(f, "Preparing"),
            Self::Rendering => write!(f, "Rendering"),
            Self::PostProcessing => write!(f, "Post-Processing"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Render output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Jpg,
    Exr,
    Tiff,
    Bmp,
}

impl OutputFormat {
    /// Get file extension.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpg => "jpg",
            Self::Exr => "exr",
            Self::Tiff => "tiff",
            Self::Bmp => "bmp",
        }
    }
}

/// Render engine type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderEngine {
    /// CPU path tracer
    PathTracer,
    /// GPU CUDA renderer
    Cuda,
    /// GPU OptiX renderer
    Optix,
    /// Rasterization (fast preview)
    Raster,
    /// Hybrid ray-tracing
    Hybrid,
}

/// Render quality preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityPreset {
    /// Fast preview quality
    Preview,
    /// Low quality for testing
    Low,
    /// Medium quality balance
    Medium,
    /// High quality
    High,
    /// Ultra quality for final output
    Ultra,
    /// Custom settings
    Custom,
}

/// Render settings.
#[derive(Debug, Clone)]
pub struct RenderSettings {
    /// Output width in pixels
    pub width: u32,
    /// Output height in pixels
    pub height: u32,
    /// Samples per pixel
    pub samples: u32,
    /// Max bounces for path tracing
    pub max_bounces: u32,
    /// Clamp indirect lighting
    pub clamp_indirect: f32,
    /// Use adaptive sampling
    pub adaptive_sampling: bool,
    /// Adaptive threshold
    pub adaptive_threshold: f32,
    /// Output format
    pub output_format: OutputFormat,
    /// Use denoiser
    pub denoise: bool,
    /// Denoiser strength
    pub denoise_strength: f32,
    /// Tile size for rendering
    pub tile_size: u32,
    /// Use GPU acceleration
    pub use_gpu: bool,
    /// Render engine
    pub engine: RenderEngine,
    /// Quality preset
    pub quality: QualityPreset,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            samples: 128,
            max_bounces: 8,
            clamp_indirect: 10.0,
            adaptive_sampling: true,
            adaptive_threshold: 0.01,
            output_format: OutputFormat::Png,
            denoise: true,
            denoise_strength: 0.5,
            tile_size: 256,
            use_gpu: true,
            engine: RenderEngine::PathTracer,
            quality: QualityPreset::Medium,
        }
    }
}

impl RenderSettings {
    /// Create settings from preset.
    #[allow(clippy::field_reassign_with_default)]
    pub fn from_preset(preset: QualityPreset) -> Self {
        let mut settings = Self::default();
        settings.quality = preset;

        match preset {
            QualityPreset::Preview => {
                settings.samples = 16;
                settings.max_bounces = 4;
                settings.denoise = false;
            }
            QualityPreset::Low => {
                settings.samples = 32;
                settings.max_bounces = 6;
            }
            QualityPreset::Medium => {
                settings.samples = 128;
                settings.max_bounces = 8;
            }
            QualityPreset::High => {
                settings.samples = 512;
                settings.max_bounces = 12;
            }
            QualityPreset::Ultra => {
                settings.samples = 2048;
                settings.max_bounces = 16;
                settings.adaptive_threshold = 0.001;
            }
            QualityPreset::Custom => {}
        }

        settings
    }

    /// Estimate render time (very rough).
    pub fn estimate_time(&self) -> Duration {
        let base_time = 10.0; // seconds
        let pixel_factor = (self.width * self.height) as f64 / (1920.0 * 1080.0);
        let sample_factor = self.samples as f64 / 128.0;
        let bounce_factor = self.max_bounces as f64 / 8.0;
        let gpu_factor = if self.use_gpu { 0.2 } else { 1.0 };

        let estimated = base_time * pixel_factor * sample_factor * bounce_factor * gpu_factor;
        Duration::from_secs_f64(estimated)
    }
}

/// Frame range for animation rendering.
#[derive(Debug, Clone)]
pub struct FrameRange {
    /// Start frame
    pub start: i32,
    /// End frame
    pub end: i32,
    /// Frame step
    pub step: i32,
}

impl Default for FrameRange {
    fn default() -> Self {
        Self {
            start: 1,
            end: 1,
            step: 1,
        }
    }
}

impl FrameRange {
    /// Get total frame count.
    pub fn frame_count(&self) -> u32 {
        ((self.end - self.start) / self.step + 1).max(0) as u32
    }

    /// Iterate over frames.
    pub fn iter(&self) -> impl Iterator<Item = i32> {
        let start = self.start;
        let end = self.end;
        let step = self.step;
        (start..=end).step_by(step.max(1) as usize)
    }
}

/// Render job.
#[derive(Debug, Clone)]
pub struct RenderJob {
    /// Unique job ID
    pub id: String,
    /// Job name
    pub name: String,
    /// Scene file path/URL
    pub scene_path: String,
    /// Render settings
    pub settings: RenderSettings,
    /// Frame range
    pub frames: FrameRange,
    /// Output path pattern
    pub output_pattern: String,
    /// Current status
    pub status: JobStatus,
    /// Progress (0.0 - 1.0)
    pub progress: f32,
    /// Current frame being rendered
    pub current_frame: i32,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Instance type used
    pub instance_type: String,
    /// Created timestamp
    pub created_at: SystemTime,
    /// Started timestamp
    pub started_at: Option<SystemTime>,
    /// Completed timestamp
    pub completed_at: Option<SystemTime>,
    /// Estimated cost
    pub estimated_cost: f64,
    /// Actual cost
    pub actual_cost: f64,
    /// Priority (higher = sooner)
    pub priority: i32,
    /// Tags for organization
    pub tags: Vec<String>,
}

impl RenderJob {
    /// Create a new render job.
    pub fn new(name: &str, scene_path: &str) -> Self {
        Self {
            id: generate_job_id(),
            name: name.to_string(),
            scene_path: scene_path.to_string(),
            settings: RenderSettings::default(),
            frames: FrameRange::default(),
            output_pattern: "render_####.png".to_string(),
            status: JobStatus::Queued,
            progress: 0.0,
            current_frame: 0,
            error_message: None,
            instance_type: "standard".to_string(),
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
            estimated_cost: 0.0,
            actual_cost: 0.0,
            priority: 0,
            tags: Vec::new(),
        }
    }

    /// Set render settings.
    pub fn with_settings(mut self, settings: RenderSettings) -> Self {
        self.settings = settings;
        self
    }

    /// Set frame range.
    pub fn with_frames(mut self, start: i32, end: i32) -> Self {
        self.frames.start = start;
        self.frames.end = end;
        self
    }

    /// Set output pattern.
    pub fn with_output(mut self, pattern: &str) -> Self {
        self.output_pattern = pattern.to_string();
        self
    }

    /// Set instance type.
    pub fn with_instance(mut self, instance_type: &str) -> Self {
        self.instance_type = instance_type.to_string();
        self
    }

    /// Set priority.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Add a tag.
    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    /// Check if job is finished (completed, failed, or cancelled).
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        )
    }

    /// Check if job is active (rendering or post-processing).
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            JobStatus::Rendering | JobStatus::PostProcessing
        )
    }

    /// Get elapsed time.
    pub fn elapsed(&self) -> Option<Duration> {
        let start = self.started_at?;
        let end = self.completed_at.unwrap_or_else(SystemTime::now);
        end.duration_since(start).ok()
    }

    /// Calculate estimated completion time.
    pub fn estimated_remaining(&self) -> Option<Duration> {
        if self.progress <= 0.0 || self.progress >= 1.0 {
            return None;
        }

        let elapsed = self.elapsed()?.as_secs_f64();
        let remaining = elapsed * (1.0 - self.progress as f64) / self.progress as f64;
        Some(Duration::from_secs_f64(remaining))
    }

    /// Get output filename for a frame.
    pub fn output_filename(&self, frame: i32) -> String {
        self.output_pattern
            .replace("####", &format!("{:04}", frame))
    }

    /// Update cost estimate.
    pub fn update_cost_estimate(&mut self, cost_per_hour: f64) {
        let estimated_time = self.settings.estimate_time().as_secs_f64() / 3600.0;
        let frame_count = self.frames.frame_count() as f64;
        self.estimated_cost = estimated_time * frame_count * cost_per_hour;
    }
}

/// Job queue for managing multiple jobs.
pub struct JobQueue {
    jobs: Vec<RenderJob>,
    max_concurrent: u32,
    active_count: u32,
}

impl JobQueue {
    /// Create a new job queue.
    pub fn new(max_concurrent: u32) -> Self {
        Self {
            jobs: Vec::new(),
            max_concurrent,
            active_count: 0,
        }
    }

    /// Add a job to the queue.
    pub fn add(&mut self, job: RenderJob) {
        self.jobs.push(job);
        self.sort_by_priority();
    }

    /// Get next job to start.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&mut RenderJob> {
        if self.active_count >= self.max_concurrent {
            return None;
        }

        self.jobs.iter_mut().find(|j| j.status == JobStatus::Queued)
    }

    /// Get job by ID.
    pub fn get(&self, id: &str) -> Option<&RenderJob> {
        self.jobs.iter().find(|j| j.id == id)
    }

    /// Get job by ID mutably.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut RenderJob> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    /// Remove completed jobs.
    pub fn clear_completed(&mut self) {
        self.jobs
            .retain(|j| !matches!(j.status, JobStatus::Completed | JobStatus::Cancelled));
    }

    /// Get queue length.
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    /// Check if queue is empty.
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    /// Get all jobs.
    pub fn jobs(&self) -> &[RenderJob] {
        &self.jobs
    }

    /// Sort jobs by priority.
    fn sort_by_priority(&mut self) {
        self.jobs.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Update active count.
    pub fn update_active_count(&mut self) {
        self.active_count = self.jobs.iter().filter(|j| j.is_active()).count() as u32;
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new(4)
    }
}

fn generate_job_id() -> String {
    use std::time::SystemTime;
    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("job-{:x}", time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_settings_preset() {
        let settings = RenderSettings::from_preset(QualityPreset::High);
        assert_eq!(settings.samples, 512);
        assert_eq!(settings.max_bounces, 12);
    }

    #[test]
    fn test_frame_range() {
        let range = FrameRange {
            start: 1,
            end: 10,
            step: 2,
        };
        assert_eq!(range.frame_count(), 5);
        let frames: Vec<_> = range.iter().collect();
        assert_eq!(frames, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn test_job_creation() {
        let job = RenderJob::new("Test Job", "/path/to/scene.nat3d")
            .with_frames(1, 100)
            .with_priority(5);

        assert_eq!(job.frames.start, 1);
        assert_eq!(job.frames.end, 100);
        assert_eq!(job.priority, 5);
        assert_eq!(job.status, JobStatus::Queued);
    }

    #[test]
    fn test_job_queue() {
        let mut queue = JobQueue::new(2);
        queue.add(RenderJob::new("Job 1", "scene1.nat3d"));
        queue.add(RenderJob::new("Job 2", "scene2.nat3d").with_priority(1));

        assert_eq!(queue.len(), 2);
        // Higher priority job should be first
        assert_eq!(queue.jobs()[0].name, "Job 2");
    }
}
