/* blend_modes.rs
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
use strum_macros::{Display, EnumIter, VariantNames};

#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy, Display, PartialEq, EnumIter, VariantNames)]
pub enum BlendMode {
    #[default]
    Normal,
    #[strum(to_string = "Hard Light")]
    HardLight,
    #[strum(to_string = "Soft Light")]
    SoftLight,
}

