/* file.rs
 *
 * Copyright 2026 FatDawlf
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use std::{
    fs::File,
    io::{Cursor, ErrorKind, Write},
    path::Path,
};

use uuid::Uuid;
use zip::{
    ZipWriter,
    result::{ZipError, ZipResult},
    write::SimpleFileOptions,
};

use crate::data::{layer::Layer, project::BrushProject};

pub fn save_project(path: &Path, project: &BrushProject, preview: &[u8]) -> ZipResult<()> {
    println!("Making the zip");
    let mut zip = prepare_zip(path)?;
    println!("Zip made");
    // Save the main project structure
    save_structure(&mut zip, project)?;
    println!("Structure saved");
    // Walk through each layer and save it
    save_layers(&mut zip, &project.layers)?;
    println!("Layers saved");
    // Generate a preview
    save_preview(&mut zip, project, preview)?;
    println!("Preview saved");
    // Commit the file
    zip.finish()?;
    println!("Save finished");
    Ok(())
}

fn save_structure(zip: &mut ZipWriter<File>, project: &BrushProject) -> ZipResult<()> {
    if let Ok(structure) = serde_json::to_string(project) {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o666);

        zip.start_file("meta.json", options)?;
        zip.write_all(structure.as_bytes())?;
    }

    Ok(())
}

fn save_preview(zip: &mut ZipWriter<File>, project: &BrushProject, data: &[u8]) -> ZipResult<()> {

    let mut png = Cursor::new(Vec::new());

    let result = image::write_buffer_with_format(
        &mut png,
        &data,
        project.width,
        project.height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    );

    match result {
        Ok(_) => {
            let options = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .unix_permissions(0o666);

            zip.start_file("preview.png", options)?;
            zip.write_all(&png.into_inner())?;
        }
        Err(e) => {
            eprintln!("{}", e);
            return Err(ZipError::FileNotFound);
        }
    }

    Ok(())
}

fn save_layers(zip: &mut ZipWriter<File>, layers: &Vec<Layer>) -> ZipResult<()> {
    for layer in layers {
        match layer {
            // Save children if it's a group
            Layer::Group(_) => {
                if let Some(children) = layer.children() {
                    save_layers(zip, children)?;
                }
            }
            // Save data if it's a pixel layer
            Layer::Pixel(_) => {
                if let Some(data) = layer.pixel_data() {
                    save_pixel_data(zip, layer.id(), data)?;
                }
            }
            // Do nothing if it's a data only layer
            _ => {}
        }
    }
    Ok(())
}

fn save_pixel_data(
    zip: &mut ZipWriter<File>,
    id: Uuid,
    pixels: &Vec<f32>,
) -> zip::result::ZipResult<()> {
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o666);

    let file = format!("layers/{}", id.to_string());

    zip.start_file(&file, options)?;
    zip.write_all(bytemuck::cast_slice(pixels))?;

    Ok(())
}

fn prepare_zip(path: &Path) -> zip::result::ZipResult<ZipWriter<File>> {
    // Validate that the provided filename does not escape the current directory
    if path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        // Return an error instead of writing to an arbitrary location
        return Err(ZipError::InvalidArchive(
            "Unsafe output path: Attempted directory traversal or absolute path".into(),
        ));
    }
    // Create the file relative to the current working directory
    let base = std::env::current_dir().map_err(|_| {
        zip::result::ZipError::Io(std::io::Error::new(
            ErrorKind::NotFound,
            "Failed to get current directory",
        ))
    })?;
    let safe_path = base.join(path);

    let file = std::fs::File::create(safe_path)?;

    let mut zip = zip::ZipWriter::new(file);

    // Prepare folders and metadata
    zip.add_directory("layers/", SimpleFileOptions::default())?;
    zip.add_directory("refs/", SimpleFileOptions::default())?;

    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o444);

    zip.start_file("mimetype", options)?;
    zip.write_all(b"application/x-brush\n")?;

    Ok(zip)
}
