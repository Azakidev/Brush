/* config.rs
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

pub static APP_ID: &str = default_env(option_env!("APP_ID"), "art.FatDawlf.Brush");
pub static VERSION: &str = default_env(option_env!("VERSION"), "unknown");
pub static GETTEXT_PACKAGE: &str = default_env(option_env!("GETTEXT_PACKAGE"), "brush");
pub static LOCALEDIR: &str = default_env(option_env!("LOCALEDIR"), "/ap/share/locale");
pub static PKGDATADIR: &str = default_env(option_env!("PKGDATADIR"), "/app/share/brush");

const fn default_env(v: Option<&'static str>, default: &'static str) -> &'static str {
    match v {
        Some(v) => v,
        None => default,
    }
}
