/* triple_buffer.rs
 *
 * Copyright 2026 FatDawlf
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use uuid::Uuid;

use crate::data::rect::Rect;

#[derive(Debug, Clone)]
pub struct FrameUpdate {
    pub dirty_layer_id: Uuid,
    pub rect: Rect,
}
