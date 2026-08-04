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

use clap::Parser;
use image::{Rgb, RgbImage};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "NAT3D - Industrial Particle Plotter")]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    Render {
        input: String,
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() {
    let args = Args::parse();
    if let Some(Commands::Render { input, output }) = args.command {
        if let Err(e) = render_industrial_atoms(&input, output.as_deref()) {
            eprintln!("Error: {}", e);
        }
    }
}

fn render_industrial_atoms(input: &str, output: Option<&str>) -> Result<(), String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output.unwrap_or("G:\\FINAL_ATOM_VIEW.png"));

    let file = File::open(&input_path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    let mut vertices = Vec::new();
    let (mut min_x, mut max_x) = (f64::MAX, f64::MIN);
    let (mut min_y, mut max_y) = (f64::MAX, f64::MIN);

    // 1. LEER Y MEDIR (Para el Auto-Escalado)
    for line in reader.lines() {
        let l = line.map_err(|e| e.to_string())?;
        let parts: Vec<&str> = l.split_whitespace().collect();
        if parts.len() >= 4 && parts[0] == "v" {
            let x: f64 = parts[1].parse().unwrap_or(0.0);
            let y: f64 = parts[2].parse().unwrap_or(0.0);
            let z: f64 = parts[3].parse().unwrap_or(0.0);

            if x < min_x {
                min_x = x;
            } else if x > max_x {
                max_x = x;
            }
            if y < min_y {
                min_y = y;
            } else if y > max_y {
                max_y = y;
            }
            vertices.push((x, y, z));
        }
    }

    if vertices.is_empty() {
        return Err("No se encontraron átomos en el archivo".into());
    }

    // 2. CONFIGURAR CANVAS
    let size = 1024u32;
    let margin = 50.0;
    let mut img = RgbImage::new(size, size);
    for p in img.pixels_mut() {
        *p = Rgb([10, 10, 20]);
    } // Fondo azul profundo

    // Calcular escala para ajustar los 800 átomos al 1024x1024
    let range_x = max_x - min_x;
    let range_y = max_y - min_y;
    let scale = ((size as f64 - 2.0 * margin) / range_x.max(range_y)).min(100.0);

    println!("RENDERIZANDO: {} átomos.", vertices.len());
    println!(
        "ESCALA: {:.2} px/A. RANGO: ({:.1}, {:.1})",
        scale, range_x, range_y
    );

    // 3. DIBUJAR ÁTOMOS COMO ESFERAS
    for (vx, vy, vz) in vertices {
        // Proyectar coordenadas al centro del canvas
        let px = ((vx - min_x) * scale + margin) as i32;
        let py = ((vy - min_y) * scale + margin) as i32;

        // Dibujar un círculo de 4x4 píxeles por átomo
        for dx in -2..=2 {
            for dy in -2..=2 {
                let x = (px + dx) as u32;
                let y = (py + dy) as u32;
                if x < size && y < size {
                    // Color basado en profundidad Z para efecto 3D real
                    let z_norm = (vz + 5.0) / 15.0; // Normalizar profundidad
                    let r = (50.0 * z_norm) as u8;
                    let g = (200.0 * z_norm).clamp(100.0, 255.0) as u8;
                    let b = (255.0 * (1.0 - z_norm)) as u8;
                    img.put_pixel(x, y, Rgb([r, g, b]));
                }
            }
        }
    }

    img.save(&output_path).map_err(|e| e.to_string())?;
    println!(
        "ARCHIVO GUARDADO: {} (Tamaño: {} bytes)",
        output_path.display(),
        std::fs::metadata(&output_path).unwrap().len()
    );
    Ok(())
}
