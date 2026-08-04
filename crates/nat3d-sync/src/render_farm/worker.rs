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

//! Worker node implementation.

use crate::protocol::{RenderResult, SyncMessage, TileSpec};
use nalgebra::{Matrix4, Point3, Vector3};
use nat3d_render::backend::cpu::CpuRenderer;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub struct RenderFarmWorker {
    pub name: String,
}

impl RenderFarmWorker {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub async fn run(&mut self, master_addr: SocketAddr) -> anyhow::Result<()> {
        let mut stream = TcpStream::connect(master_addr).await?;
        tracing::info!("Worker {} connected to master", self.name);

        loop {
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await?;
            if n == 0 {
                break;
            }

            if let Ok(msg) = serde_json::from_slice::<SyncMessage>(&buf[..n]) {
                match msg {
                    SyncMessage::AssignTile(tile, frame) => {
                        let data = self.render_tile(&tile, frame).await?;
                        let result = SyncMessage::SubmitResult(RenderResult {
                            tile_id: 0, // Simplified for BATCH 24
                            frame,
                            data,
                        });
                        let encoded = serde_json::to_vec(&result)?;
                        stream.write_all(&encoded).await?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub async fn render_tile(&self, tile: &TileSpec, frame: u32) -> anyhow::Result<Vec<u8>> {
        tracing::debug!(
            "REAL Rendering tile {}x{} for frame {}",
            tile.width,
            tile.height,
            frame
        );

        let mut renderer = CpuRenderer::new(tile.width as usize, tile.height as usize);
        renderer.clear_color(20, 22, 25, 255);

        // BATCH 24: Procedural Pyramid - REAL Geometry
        let angle = (frame as f32) * 0.05;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        // Manual projection NDC -> pixel for a simple pyramid
        let project = |p: [f32; 3]| -> Point3<f32> {
            let px = p[0] * cos_a - p[2] * sin_a;
            let pz = p[0] * sin_a + p[2] * cos_a + 4.0;
            let sx = (px / pz * 1.5 + 1.0) * 0.5 * (tile.width as f32);
            let sy = (1.0 - (p[1] / pz * 1.5)) * 0.5 * (tile.height as f32);
            Point3::new(sx, sy, pz)
        };

        let apex = project([0.0, 1.0, 0.0]);
        let base0 = project([-1.0, -1.0, -1.0]);
        let base1 = project([1.0, -1.0, -1.0]);
        let base2 = project([1.0, -1.0, 1.0]);
        let base3 = project([-1.0, -1.0, 1.0]);

        let fb = renderer.framebuffer_mut();
        fb.draw_triangle_screen(apex, base0, base1, [220, 120, 40]);
        fb.draw_triangle_screen(apex, base1, base2, [180, 200, 60]);
        fb.draw_triangle_screen(apex, base2, base3, [100, 180, 220]);
        fb.draw_triangle_screen(apex, base3, base0, [200, 80, 160]);
        fb.draw_triangle_screen(base0, base1, base2, [160, 160, 160]);
        fb.draw_triangle_screen(base0, base2, base3, [140, 140, 140]);

        let buffer = renderer.color_buffer().to_vec();
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::TileSpec;

    #[tokio::test]
    async fn test_worker_frame_variation() {
        let worker = RenderFarmWorker::new("test".to_string());
        let spec = TileSpec {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        };

        let t0 = worker.render_tile(&spec, 0).await.unwrap();
        let t1 = worker.render_tile(&spec, 30).await.unwrap();

        assert_ne!(
            t0, t1,
            "Tiles with different frames must produce different output"
        );
    }
}
