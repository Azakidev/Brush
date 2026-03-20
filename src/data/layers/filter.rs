/* filter.rs
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

use serde::{Deserialize, Serialize};

use crate::data::layer::{LayerData};

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Filter {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FilterParams {}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilterData {
    mask: String, // TODO: Replace with data or reference to a mask
    params: FilterParams,
}

impl LayerData for FilterData {}
