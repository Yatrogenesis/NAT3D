//! Raytracing module for NAT3D.
//!
//! Provides neural radiance caching (NRC) and neural radiance fields (NeRF)
//! for high-performance global illumination.

pub mod bvh;
pub mod intersection;
/// Neural Radiance Fields volumetric ray marching (NeRF).
pub mod nerf;
/// Neural Radiance Cache MLP for real-time global illumination (NRC).
pub mod nrc;
pub mod pathtracer;
pub mod ray;
