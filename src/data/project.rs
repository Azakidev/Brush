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
    time::{SystemTime, UNIX_EPOCH},
};

use gtk::glib::WeakRef;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    components::{layer_item::BrushLayerItem, utils::renderer::buffer::LayerBuffer},
    data::{layer::Layer, layer_types::refs::RefLayer},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BrushProject {
    // Data
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

    pub fn find_layer(&self, uuid: Uuid) -> Option<&Layer> {
        Self::search_recursive(&self.layers, uuid)
    }

    pub fn find_layer_mut(&mut self, uuid: Uuid) -> Option<&mut Layer> {
        Self::search_recursive_mut(&mut self.layers, uuid)
    }

    pub fn rename_layer(&mut self, uuid: Uuid, new_name: String) {
        if let Some(layer) = self.find_layer_mut(uuid) {
            *layer.name_mut() = new_name;
        }
    }

    pub fn remove_layer(&mut self, uuid: Uuid) -> Option<()> {
        let layer = self.find_layer(uuid)?.clone();
        if let Some(parent) = self.find_parent_mut(uuid) {
            parent.remove_child(&layer);
        } else if let Some(idx) = self.layers.iter().position(|l| l.id() == uuid) {
            self.layers.remove(idx);
        }

        Some(())
    }

    pub fn move_layer(
        &mut self,
        layer: &Layer,
        index: usize,
        old_parent: Option<Uuid>,
        new_parent: Option<Uuid>,
        widget_cache: &mut HashMap<Uuid, WeakRef<BrushLayerItem>>,
        buf_cache: &mut HashMap<Uuid, LayerBuffer>,
    ) {
        // Remove old
        if let Some(parent_id) = old_parent {
            if let Some(parent) = self.find_layer_mut(parent_id) {
                if let Some(children) = parent.children() {
                    if children.iter().any(|l| l.id() == layer.id()) {
                        parent.remove_child(layer); // Force clear group texture
                        buf_cache.remove(&parent.id());
                        widget_cache.remove(&parent.id());
                        widget_cache.remove(&layer.id());
                    }
                }
            }
        } else if let Some(idx) = self.layers.iter().position(|l| l.id() == layer.id()) {
            self.layers.remove(idx);
            widget_cache.remove(&layer.id());
        }
        // Add on new position
        if let Some(parent_id) = new_parent {
            if let Some(parent) = self.find_layer_mut(parent_id) {
                parent.append(index, layer.clone());
                if let Some(entry) = widget_cache.get(&parent_id) {
                    if let Some(widget) = entry.upgrade() {
                        widget.reveal();
                    }
                }
            }
        } else {
            self.layers.insert(index, layer.clone());
        }
    }

    pub fn find_parent(&self, target_id: Uuid) -> Option<&Layer> {
        Self::search_parent_recursive(&self.layers, target_id)
    }

    pub fn find_parent_mut(&mut self, target_id: Uuid) -> Option<&mut Layer> {
        Self::search_parent_recursive_mut(&mut self.layers, target_id)
    }

    fn search_parent_recursive(layers: &[Layer], target_id: Uuid) -> Option<&Layer> {
        for layer in layers {
            // Layer contains target, return target
            if let Some(children) = layer.children() {
                if children.iter().any(|child| child.id() == target_id) {
                    return Some(layer);
                // Layer doesn't contain target, but has children that must be checked
                } else if let Some(found_parent) =
                    Self::search_parent_recursive(children, target_id)
                {
                    return Some(found_parent);
                }
            }
        }
        // Target is at the project's root
        None
    }

    fn search_parent_recursive_mut(layers: &mut [Layer], target_id: Uuid) -> Option<&mut Layer> {
        for layer in layers {
            // Layer contains target, return target
            if let Some(children) = layer.children() {
                if children.iter().any(|child| child.id() == target_id) {
                    return Some(layer);
                }
            }

            // Layer doesn't contain target, but has children that must be checked
            if let Some(children) = layer.children_mut() {
                if let Some(found_parent) = Self::search_parent_recursive_mut(children, target_id) {
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
