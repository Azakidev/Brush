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
    path::{Path, PathBuf},
};

use ashpd::{
    Uri,
    desktop::file_chooser::{FileFilter, SelectedFiles},
};
use url::Url;
use uuid::Uuid;
use zip::{
    ZipWriter,
    result::{ZipError, ZipResult},
    write::SimpleFileOptions,
};

use crate::data::{layer::Layer, layer_types::refs::RefLayer, project::BrushProject};

pub fn save_project(path: &Path, project: &BrushProject, preview: &[u8]) -> ZipResult<()> {
    let mut zip = prepare_zip(path)?;
    println!("Zip done");
    // Save the main project structure
    save_structure(&mut zip, project)?;
    println!("Structure done");
    // Walk through each layer and save it
    save_layers(&mut zip, &project.layers)?;
    save_refs(&mut zip, &project.references)?;
    println!("Layers done");
    // Generate a preview
    save_preview(&mut zip, project, preview)?;
    println!("Preview done");
    // Commit the file
    zip.finish()?;
    Ok(())
}

fn save_structure(zip: &mut ZipWriter<File>, project: &BrushProject) -> ZipResult<()> {
    if let Ok(structure) = serde_json::to_string(project) {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::DEFLATE)
            .unix_permissions(0o444);

        zip.start_file("meta.json", options)?;
        zip.write_all(structure.as_bytes())?;
    }

    Ok(())
}

fn save_preview(zip: &mut ZipWriter<File>, project: &BrushProject, data: &[u8]) -> ZipResult<()> {
    let mut png = Cursor::new(Vec::new());

    let result = image::write_buffer_with_format(
        &mut png,
        data,
        project.width,
        project.height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    );

    match result {
        Ok(_) => {
            let options = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::DEFLATE)
                .unix_permissions(0o444);

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
                    save_pixel_data(zip, "layers", layer.id(), data)?;
                }
            }
            // Do nothing if it's a data only layer
            _ => {}
        }
    }
    Ok(())
}

fn save_refs(zip: &mut ZipWriter<File>, refs: &Vec<RefLayer>) -> ZipResult<()> {
    for ref_layer in refs {
        save_pixel_data(zip, "refs", ref_layer.id(), ref_layer.pixel_data())?;
    }
    Ok(())
}

fn save_pixel_data(
    zip: &mut ZipWriter<File>,
    destination: &str,
    id: Uuid,
    pixels: &[f32],
) -> zip::result::ZipResult<()> {
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::DEFLATE)
        .unix_permissions(0o444);

    let file = format!("{destination}/{}", id);

    zip.start_file(&file, options)?;
    zip.write_all(bytemuck::cast_slice(pixels))?;

    Ok(())
}

fn prepare_zip(path: &Path) -> zip::result::ZipResult<ZipWriter<File>> {
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

    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o444);

    zip.start_file("mimetype", options)?;
    zip.write_all(b"application/x-brush\n")?;

    Ok(zip)
}

pub async fn request_open() -> ashpd::Result<Vec<Uri>> {
    let files = SelectedFiles::open_file()
        .title("Open a project") // TODO:  i18n
        .accept_label("Open") // TODO: i18n
        .modal(true)
        .multiple(false)
        .filter(FileFilter::new("Brush Project").mimetype("application/x-brush")) // TODO: i18n
        .filter(FileFilter::new("Any file").mimetype("application/octet-stream")) // TODO: i18n
        .send()
        .await?
        .response()?;

    Ok(files.uris().to_vec())
}

pub async fn request_save() -> ashpd::Result<PathBuf> {
    let files = SelectedFiles::save_file()
        .title("Save the project") // TODO:  i18n
        .accept_label("Save") // TODO: i18n
        .modal(true)
        .filter(FileFilter::new("Brush Project").glob("*.bsh")) // TODO: i18n
        .send()
        .await?
        .response()?;

    let uri = files.uris().first().unwrap();
    let url = Url::parse(uri.as_str()).unwrap();

    if url.scheme() == "file" {
        return Ok(url.to_file_path().unwrap());
    }
    Err(ashpd::Error::Portal(ashpd::PortalError::NotFound("Uri is not a file".to_owned())))
}
