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

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! 3ds Max-style keyboard shortcuts.
//!
//! Familiar keymap for 3ds Max users: Q/W/E/R for select/move/rotate/scale.

use super::default::{Action, Keymap, Modifiers, Shortcut};

impl Keymap {
    /// Create 3ds Max-style keymap.
    pub fn max() -> Self {
        let mut keymap = Self::default();

        // Transform operations (3ds Max style: Q/W/E/R)
        // These are already in default, but we make it explicit
        keymap.bind(Shortcut::new('Q'), Action::SelectMode);
        keymap.bind(Shortcut::new('W'), Action::MoveMode);
        keymap.bind(Shortcut::new('E'), Action::RotateMode);
        keymap.bind(Shortcut::new('R'), Action::ScaleMode);

        // 3ds Max uses Delete for delete (already default)
        keymap.bind(Shortcut::new('\x7F'), Action::Delete);

        // 3ds Max viewport navigation
        // V = Bottom view
        // F = Front view
        // T = Top view
        // L = Left view
        // (We adapt these to our numpad-based system)
        keymap.bind(Shortcut::new('F'), Action::FrontView);
        keymap.bind(Shortcut::new('T'), Action::TopView);
        keymap.bind(Shortcut::new('L'), Action::RightView); // Left view

        // Z = Zoom extents selected
        keymap.bind(Shortcut::new('Z'), Action::FocusSelected);

        // Alt+W = Maximize viewport (we don't have this, use fullscreen)
        keymap.bind(
            Shortcut::with_mods('W', Modifiers::ALT),
            Action::ToggleFullscreen,
        );

        // H = Select by name (we use search)
        keymap.bind(Shortcut::new('H'), Action::Search);

        // Ctrl+A = Select all
        keymap.bind(Shortcut::with_mods('A', Modifiers::CTRL), Action::SelectAll);
        // Ctrl+D = Deselect all
        keymap.bind(
            Shortcut::with_mods('D', Modifiers::CTRL),
            Action::DeselectAll,
        );
        // Ctrl+I = Invert selection
        keymap.bind(
            Shortcut::with_mods('I', Modifiers::CTRL),
            Action::InvertSelection,
        );

        // Alt+Q = Isolate selection (we use hide/unhide)
        keymap.bind(
            Shortcut::with_mods('Q', Modifiers::ALT),
            Action::HideSelected,
        );
        keymap.bind(
            Shortcut::with_mods('Q', Modifiers::ALT_SHIFT),
            Action::UnhideAll,
        );

        // Shift+Ctrl+Z = Redo (Max uses Ctrl+Y)
        keymap.bind(Shortcut::with_mods('Y', Modifiers::CTRL), Action::Redo);

        // F9 = Render (we use Enter)
        keymap.bind(Shortcut::new('\r'), Action::RenderImage);

        keymap
    }
}

/// Additional modifiers for Max shortcuts.
impl Modifiers {
    /// Alt+Shift.
    pub const ALT_SHIFT: Self = Self {
        ctrl: false,
        shift: true,
        alt: true,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_transforms() {
        let keymap = Keymap::max();

        // 3ds Max-style transforms
        assert_eq!(
            keymap.get_action(&Shortcut::new('Q')),
            Some(Action::SelectMode)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::new('W')),
            Some(Action::MoveMode)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::new('E')),
            Some(Action::RotateMode)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::new('R')),
            Some(Action::ScaleMode)
        );
    }

    #[test]
    fn test_max_views() {
        let keymap = Keymap::max();

        assert_eq!(
            keymap.get_action(&Shortcut::new('F')),
            Some(Action::FrontView)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::new('T')),
            Some(Action::TopView)
        );
    }

    #[test]
    fn test_max_selection() {
        let keymap = Keymap::max();

        assert_eq!(
            keymap.get_action(&Shortcut::with_mods('A', Modifiers::CTRL)),
            Some(Action::SelectAll)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::with_mods('I', Modifiers::CTRL)),
            Some(Action::InvertSelection)
        );
    }
}
