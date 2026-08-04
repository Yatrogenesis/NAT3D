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

//! Blender-style keyboard shortcuts.
//!
//! Familiar keymap for Blender users: G (grab/move), R (rotate), S (scale).

use super::default::{Action, Keymap, Modifiers, Shortcut};

impl Keymap {
    /// Create Blender-style keymap.
    pub fn blender() -> Self {
        let mut keymap = Self::default();

        // Transform operations (Blender style: G/R/S instead of Q/W/E/R)
        keymap.bind(Shortcut::new('G'), Action::MoveMode);
        keymap.bind(Shortcut::new('R'), Action::RotateMode);
        keymap.bind(Shortcut::new('S'), Action::ScaleMode);

        // Blender-specific: X for delete menu (we just do delete)
        keymap.bind(Shortcut::new('X'), Action::Delete);

        // Blender uses numpad for views, but we keep 1/3/7/0
        keymap.bind(Shortcut::new('1'), Action::FrontView);
        keymap.bind(Shortcut::new('3'), Action::RightView);
        keymap.bind(Shortcut::new('7'), Action::TopView);
        keymap.bind(Shortcut::new('0'), Action::CameraView);

        // Blender uses period for frame selected
        keymap.bind(Shortcut::new('.'), Action::FocusSelected);

        // Home key frames all (Blender convention)
        // Note: Using 'H' conflicts with hide, so we keep default

        // Blender uses A for select all (we already have this)
        keymap.bind(Shortcut::new('A'), Action::ToggleSelectAll);
        keymap.bind(
            Shortcut::with_mods('A', Modifiers::ALT),
            Action::DeselectAll,
        );

        // Blender uses Shift+D for duplicate
        keymap.bind(
            Shortcut::with_mods('D', Modifiers::SHIFT),
            Action::Duplicate,
        );

        // Z for shading menu
        keymap.bind(Shortcut::new('Z'), Action::ShadingCycle);

        // Tab for edit mode
        keymap.bind(Shortcut::new('\t'), Action::ToggleEditMode);

        // H for hide, Alt+H for unhide
        keymap.bind(Shortcut::new('H'), Action::HideSelected);
        keymap.bind(Shortcut::with_mods('H', Modifiers::ALT), Action::UnhideAll);

        // Ctrl+I for invert selection
        keymap.bind(
            Shortcut::with_mods('I', Modifiers::CTRL),
            Action::InvertSelection,
        );

        // F12 for render (Blender convention)
        // We'll use Enter for now
        keymap.bind(Shortcut::new('\r'), Action::RenderImage);

        keymap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blender_transforms() {
        let keymap = Keymap::blender();

        // Blender-style transforms
        assert_eq!(
            keymap.get_action(&Shortcut::new('G')),
            Some(Action::MoveMode)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::new('R')),
            Some(Action::RotateMode)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::new('S')),
            Some(Action::ScaleMode)
        );
    }

    #[test]
    fn test_blender_selection() {
        let keymap = Keymap::blender();

        assert_eq!(
            keymap.get_action(&Shortcut::new('A')),
            Some(Action::ToggleSelectAll)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::with_mods('D', Modifiers::SHIFT)),
            Some(Action::Duplicate)
        );
    }

    #[test]
    fn test_blender_views() {
        let keymap = Keymap::blender();

        assert_eq!(
            keymap.get_action(&Shortcut::new('1')),
            Some(Action::FrontView)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::new('7')),
            Some(Action::TopView)
        );
    }
}
