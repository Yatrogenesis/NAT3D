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

use nat3d_core::Document;
use nat3d_math::glam::Vec3;
use rayon::prelude::*;
use std::time::Instant;

const WIDTH: usize = 80;
const HEIGHT: usize = 40;

struct Drone {
    pos: Vec3,
    vel: Vec3,
    is_fallen: bool,
}

fn main() {
    let mut drones: Vec<Drone> = (0..5000).map(|_| Drone {
        pos: Vec3::new(rand::random::<f32>() * 200.0 - 100.0, 0.0, rand::random::<f32>() * 200.0 - 100.0),
        vel: Vec3::new(rand::random::<f32>() - 0.5, 0.0, rand::random::<f32>() - 0.5),
        is_fallen: false,
    }).collect();

    // Loop de Simulación y Renderizado TUI
    for frame in 0..50 {
        // 1. Actualizar Física
        drones.par_iter_mut().for_each(|d| {
            if !d.is_fallen {
                d.pos += d.vel;
                // Simular zona de peligro en los bordes
                if d.pos.length() > 90.0 && rand::random::<f32>() < 0.02 {
                    d.is_fallen = true;
                }
            }
        });

        // 2. Renderizar Frame TUI
        let mut grid = vec![vec![' '; WIDTH]; HEIGHT];
        
        // Dibujar Perímetro
        for i in 0..WIDTH {
            for j in 0..HEIGHT {
                let x = (i as f32 - (WIDTH as f32 / 2.0)) * 5.0;
                let y = (j as f32 - (HEIGHT as f32 / 2.0)) * 5.0;
                if (x*x + y*y).sqrt() > 95.0 && (x*x + y*y).sqrt() < 105.0 {
                    grid[j][i] = '·';
                }
            }
        }

        // Dibujar Drones
        for d in &drones {
            let x = ((d.pos.x / 200.0 + 0.5) * (WIDTH as f32 - 1.0)) as usize;
            let y = ((d.pos.z / 200.0 + 0.5) * (HEIGHT as f32 - 1.0)) as usize;
            if x < WIDTH && y < HEIGHT {
                if d.is_fallen {
                    grid[y][x] = 'X'; // Rojo/Caído
                } else {
                    grid[y][x] = '▲'; // Verde/Activo
                }
            }
        }

        // Imprimir Pantalla
        print!("\x1B[H"); // Reset cursor a inicio
        println!("FRAME: {} | NAT3D TACTICAL VIEW | ^=Drone X=FALLEN .=Perimeter", frame);
        for row in grid {
            let s: String = row.into_iter().collect();
            println!("{}", s);
        }
        
        // Removed sleep
    }
}
