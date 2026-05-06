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
    collections::HashMap,
    fs::File,
    io::{Cursor, ErrorKind, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use url::Url;
use uuid::Uuid;
use zip::{
    ZipArchive, ZipWriter,
    result::{ZipError, ZipResult},
    write::SimpleFileOptions,
};

const LAYER_FOLDER: &str = "layers";
const REFS_FOLDER: &str = "refs";

use crate::data::{layer::Layer, layer_types::refs::RefLayer, project::BrushProject};

pub fn open_project(path: &Path) -> ZipResult<BrushProject> {
    let mut zip = open_zip(path)?;
    // Read and deserialize the structure
    let mut project = open_structure(&mut zip)?;
    // Load up the pixel layers
    open_layers(&mut zip, &mut project.layers)?;
    Ok(project)
}

pub fn save_project(path: &Path, project: BrushProject, preview: &[u8]) -> ZipResult<()> {
    let s = Instant::now();
    let mut zip = prepare_zip(path)?;
    println!("Zip done");
    // Save the main project structure
    save_structure(&mut zip, &project)?;
    println!("Structure done");
    // Walk through each layer and save it
    save_layers(&mut zip, &project.layers)?;
    save_refs(&mut zip, &project.references)?;
    println!("Layers done");
    // Generate a preview
    save_preview(&mut zip, &project, preview)?;
    println!("Preview done");
    // Commit the file
    zip.finish()?;
    println!("File saved in {:?}", s.elapsed());
    Ok(())
}

pub fn save_image(path: &Path, project: BrushProject, img: &[u8]) -> ZipResult<()> {
    let s = Instant::now();

    let format = match path.extension().unwrap().to_str().unwrap() {
        "png" => image::ImageFormat::Png,
        "avif" => image::ImageFormat::Avif,
        "jpeg" => image::ImageFormat::Jpeg,
        "jpg" => image::ImageFormat::Jpeg,
        "bmp" => image::ImageFormat::Bmp,
        "exr" => image::ImageFormat::OpenExr,
        "webp" => image::ImageFormat::WebP,
        "gif" => image::ImageFormat::Gif,
        "jif" => image::ImageFormat::Gif,
        _ => image::ImageFormat::Png,
    };

    let result = image::save_buffer_with_format(
        path,
        img,
        project.width,
        project.height,
        image::ColorType::Rgba8,
        format,
    );

    if let Err(e) = result {
        eprintln!("{}", e);
        return Err(ZipError::FileNotFound);
    }

    println!("File saved in {:?}", s.elapsed());
    Ok(())
}

fn open_structure(zip: &mut ZipArchive<File>) -> ZipResult<BrushProject> {
    let mut structure_file = match zip.by_name("meta.json") {
        Ok(f) => f,
        Err(e) => {
            return Err(e);
        }
    };

    let mut project = String::new();
    structure_file.read_to_string(&mut project).unwrap();
    let project = match serde_json::from_str::<BrushProject>(&project) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return Err(ZipError::FileNotFound);
        }
    };

    Ok(project)
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

            zip.start_file("preview", options)?;
            zip.write_all(&png.into_inner())?;
        }
        Err(e) => {
            eprintln!("{}", e);
            return Err(ZipError::FileNotFound);
        }
    }

    Ok(())
}

fn open_layers(zip: &mut ZipArchive<File>, layers: &mut Vec<Layer>) -> ZipResult<()> {
    // Flatten the layer tree
    let s = Instant::now();
    let mut flattened = Vec::new();
    flatten_tree(&mut flattened, layers);

    // Read the raw pixel data from the zip sequentially
    let mut layer_data: HashMap<Uuid, Vec<u8>> = HashMap::with_capacity(flattened.len());
    for layer in flattened {
        let raw_data = open_pixel_data(zip, LAYER_FOLDER, layer.id())?;
        layer_data.insert(layer.id(), raw_data);
    }

    // Inflate the data in parallel
    let inflated: HashMap<Uuid, Vec<f32>> = layer_data
        .par_iter()
        .map(|(id, buf)| (*id, inflate_pixel_data(buf.as_slice())))
        .collect();

    // Assign it to the layers sequentially
    fill_layer_data(&inflated, layers);

    println!("File opened in {:?}", s.elapsed());
    Ok(())
}

fn fill_layer_data(map: &HashMap<Uuid, Vec<f32>>, layers: &mut Vec<Layer>) {
    for layer in layers {
        match layer {
            Layer::Group(_) => {
                layer.set_dirty(true);
                fill_layer_data(map, layer.children_mut().unwrap());
            }
            Layer::Pixel(_) => {
                let id = layer.id();
                let data = map.get(&id).unwrap();
                layer.replace_pixel_data(data);
            }
            _ => {} // NO OP on data only layers
        }
    }
}

fn save_layers(zip: &mut ZipWriter<File>, layers: &Vec<Layer>) -> ZipResult<()> {
    let s = Instant::now();
    let mut flattened = Vec::new();
    flatten_tree(&mut flattened, layers);

    let data: Vec<(Uuid, Vec<u8>)> = flattened
        .par_iter()
        .filter_map(|layer| {
            let data = layer.pixel_data()?;
            Some((layer.id(), get_pixel_data(data)))
        })
        .collect();

    for (id, data) in data {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o444);

        let loc = format!("{}/{}", LAYER_FOLDER, id);

        zip.start_file(&loc, options)?;
        zip.write_all(&data)?;
    }
    println!("Layers written in {:?}", s.elapsed());
    Ok(())
}

fn flatten_tree(dest: &mut Vec<Layer>, layers: &Vec<Layer>) {
    for layer in layers {
        match layer {
            // Save children if it's a group
            Layer::Group(_) => {
                flatten_tree(dest, layer.children().unwrap());
            }
            // Save data if it's a pixel layer
            Layer::Pixel(_) => {
                dest.push(layer.clone());
            }
            // Do nothing if it's a data only layer
            _ => {}
        }
    }
}

fn save_refs(zip: &mut ZipWriter<File>, refs: &Vec<RefLayer>) -> ZipResult<()> {
    let data: Vec<(Uuid, Vec<u8>)> = refs
        .par_iter()
        .map(|ref_layer| (ref_layer.id(), get_pixel_data(ref_layer.pixel_data())))
        .collect();

    for (id, data) in data {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o444);

        let loc = format!("{}/{}", REFS_FOLDER, id);

        zip.start_file(&loc, options)?;
        zip.write_all(&data)?;
    }
    Ok(())
}

fn open_pixel_data(zip: &mut ZipArchive<File>, destination: &str, id: Uuid) -> ZipResult<Vec<u8>> {
    let loc = format!("{}/{}", destination, id);

    let mut file = match zip.by_name(&loc) {
        Ok(f) => f,
        Err(e) => {
            return Err(e);
        }
    };

    let mut buf: Vec<u8> = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut buf)?;

    Ok(buf)
}

fn inflate_pixel_data(buf: &[u8]) -> Vec<f32> {
    let mut decoder = flate2::read::DeflateDecoder::new(buf);
    let mut decoded: Vec<u8> = Vec::new();

    if decoder.read_to_end(&mut decoded).is_ok() {
        let pixels: Vec<f32> = bytemuck::pod_collect_to_vec(&decoded);
        return pixels;
    }

    // Fallback if decompression fails somehow
    let pixels: Vec<f32> = bytemuck::pod_collect_to_vec(buf);
    pixels
}

fn get_pixel_data(pixels: &[f32]) -> Vec<u8> {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
    if encoder.write_all(bytemuck::cast_slice(pixels)).is_ok() {
        let compressed_bytes = encoder.finish().unwrap();
        return compressed_bytes;
    }

    // Fallback if compression fails somehow
    bytemuck::cast_slice(pixels).to_vec()
}

fn open_zip(path: &Path) -> zip::result::ZipResult<ZipArchive<File>> {
    // Create the file relative to the current working directory
    let base = std::env::current_dir().map_err(|_| {
        zip::result::ZipError::Io(std::io::Error::new(
            ErrorKind::NotFound,
            "Failed to get current directory",
        ))
    })?;
    let safe_path = base.join(path);

    std::fs::File::open(safe_path)
        .map_err(ZipError::from)
        .and_then(ZipArchive::new)
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

pub async fn request_open() -> ashpd::Result<Vec<PathBuf>> {
    let files = SelectedFiles::open_file()
        .title("Open a project") // TODO:  i18n
        .accept_label("Open") // TODO: i18n
        .modal(true)
        .multiple(false)
        .filter(
            FileFilter::new("Brush Project")
                .mimetype("application/x-brush")
                .glob("*.bsh"),
        ) // TODO: i18n
        .filter(FileFilter::new("Any file").mimetype("application/octet-stream")) // TODO: i18n
        .send()
        .await?
        .response()?;

    let paths: Vec<PathBuf> = files
        .uris()
        .iter()
        .map(|uri| Url::parse(uri.as_str()).unwrap())
        .filter_map(|url| {
            if url.scheme() == "file" {
                Some(url.to_file_path().unwrap())
            } else {
                None
            }
        })
        .collect();

    Ok(paths)
}

pub async fn request_save(is_export: bool) -> ashpd::Result<PathBuf> {
    let title = if is_export { "Export" } else { "Save" };

    let files = SelectedFiles::save_file()
        .title(title) // TODO:  i18n
        .accept_label("Save") // TODO: i18n
        .modal(true)
        .filter(FileFilter::new("Brush Project").glob("*.bsh")) // TODO: i18n
        .filter(FileFilter::new("PNG Image").glob("*.png")) // TODO: i18n
        .filter(FileFilter::new("AVIF Image").glob("*.avif")) // TODO: i18n
        .filter(FileFilter::new("JPEG Image").glob("*.jpg")) // TODO: i18n
        .filter(FileFilter::new("Bitmap Image").glob("*.bmp")) // TODO: i18n
        .filter(FileFilter::new("EXR Image").glob("*.exr")) // TODO: i18n
        .filter(FileFilter::new("WebP Image").glob("*.webp")) // TODO: i18n
        .filter(FileFilter::new("GIF Image").glob("*.gif")) // TODO: i18n
        .send()
        .await?
        .response()?;

    let uri = files.uris().first().unwrap();
    let url = Url::parse(uri.as_str()).unwrap();

    if url.scheme() == "file" {
        return Ok(url.to_file_path().unwrap());
    }
    Err(ashpd::Error::Portal(ashpd::PortalError::NotFound(
        "Uri is not a file".to_owned(),
    )))
}
