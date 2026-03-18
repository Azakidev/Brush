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

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data::{layer::Layer, layers::refs::RefLayer};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BrushProject {
    pub version: u32,
    pub created: u64, // UNIX timestamp

    pub width: u32,
    pub height: u32,

    pub layers: Vec<Layer>,
    pub references: Vec<RefLayer>,
}

impl Default for BrushProject {
    fn default() -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH);

        match now {
            Ok(n) => Self {
                version: 1,
                created: n.as_secs(),

                width: 1920,
                height: 1080,

                layers: Vec::new(),
                references: Vec::new(),
            },
            Err(_) => panic!("Time before UNIX Epoch"),
        }
    }
}

impl BrushProject {
    pub fn new(
        version: u32,
        created: u64,
        width: u32,
        height: u32,
        layers: Vec<Layer>,
        references: Vec<RefLayer>,
    ) -> Self {
        Self {
            version,
            created,

            width,
            height,

            layers,
            references,
        }
    }

    pub fn find_layer(&self, uuid: &str) -> Option<&Layer> {
        let target_id = Uuid::parse_str(uuid).ok()?;

        Self::search_recursive(&self.layers, target_id)
    }

    pub fn find_parent(&mut self, layer: &Layer) -> Option<&mut Layer> {
        Self::find_parent_mut(&mut self.layers, layer.id())
    }

    pub fn find_layer_mut(&mut self, uuid: &str) -> Option<&mut Layer> {
        let target_id = Uuid::parse_str(uuid).ok()?;
        Self::search_recursive_mut(&mut self.layers, target_id)
    }

    pub fn rename_layer(&mut self, uuid: &str, new_name: String) {
        if let Some(layer) = self.find_layer_mut(uuid) {
            *layer.name_mut() = new_name;
        }
    }

    fn find_parent_mut(layers: &mut [Layer], target_id: Uuid) -> Option<&mut Layer> {
    for layer in layers {
        // Layer contains target, return target
        if let Some(children) = layer.children() {
            if children.iter().any(|child| child.id() == target_id) {
                return Some(layer);
            }
        }

        // Layer doesn't contain target, but has children that must be checked
        if let Some(children) = layer.children_mut() {
            if let Some(found_parent) = Self::find_parent_mut(children, target_id) {
                return Some(found_parent);
            }
        }
    }

    // Target is at the project's root
    None
}

    fn search_recursive(layers: &[Layer], target_id: Uuid) -> Option<&Layer> {
        for layer in layers {
            // Check if this layer is the one
            if layer.id() == target_id {
                return Some(layer);
            }

            // If it's a group, search its children
            if let Some(children) = layer.children() {
                if let Some(found) = Self::search_recursive(children, target_id) {
                    return Some(found);
                }
            }
        }
        // Target doesn't exist
        None
    }

    fn search_recursive_mut(layers: &mut [Layer], target_id: Uuid) -> Option<&mut Layer> {
        for layer in layers {
            // Check if this layer is the one
            if layer.id() == target_id {
                return Some(layer);
            }
            // If it's a group, search its children
            if let Some(children) = layer.children_mut() {
                if let Some(found) = Self::search_recursive_mut(children, target_id) {
                    return Some(found);
                }
            }
        }
        // Target doesn't exist
        None
    }
}
