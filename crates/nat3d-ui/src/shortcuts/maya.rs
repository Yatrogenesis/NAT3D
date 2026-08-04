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

//! Maya-style keyboard shortcuts.
//!
//! Familiar keymap for Maya users: W/E/R for move/rotate/scale, Q for select.

use super::default::{Action, Keymap, Modifiers, Shortcut};

impl Keymap {
    /// Create Maya-style keymap.
    pub fn maya() -> Self {
        let mut keymap = Self::default();

        // Transform operations (Maya style: Q/W/E/R like Max)
        keymap.bind(Shortcut::new('Q'), Action::SelectMode);
        keymap.bind(Shortcut::new('W'), Action::MoveMode);
        keymap.bind(Shortcut::new('E'), Action::RotateMode);
        keymap.bind(Shortcut::new('R'), Action::ScaleMode);

        // Maya uses Delete/Backspace for delete
        keymap.bind(Shortcut::new('\x7F'), Action::Delete);
        keymap.bind(Shortcut::new('\x08'), Action::Delete); // Backspace

        // Maya viewport navigation (numpad)
        // 4 = Left, 6 = Right, 8 = Up, 2 = Down (we adapt to 1/3/7)
        keymap.bind(Shortcut::new('1'), Action::FrontView);
        keymap.bind(Shortcut::new('3'), Action::RightView);
        keymap.bind(Shortcut::new('7'), Action::TopView);

        // F = Frame selected
        keymap.bind(Shortcut::new('F'), Action::FocusSelected);
        // A = Frame all
        keymap.bind(Shortcut::new('A'), Action::FrameAll);

        // Maya selection
        // F8 = Object/component mode toggle (we use Tab for edit mode)
        keymap.bind(Shortcut::new('\t'), Action::ToggleEditMode);

        // Ctrl+A = Select all (already default)
        keymap.bind(Shortcut::with_mods('A', Modifiers::CTRL), Action::SelectAll);
        // Ctrl+Shift+I = Invert selection
        keymap.bind(
            Shortcut::with_mods('I', Modifiers::CTRL_SHIFT),
            Action::InvertSelection,
        );
        // Ctrl+D = Duplicate
        keymap.bind(Shortcut::with_mods('D', Modifiers::CTRL), Action::Duplicate);

        // H = Hide selected
        keymap.bind(Shortcut::new('H'), Action::HideSelected);
        // Ctrl+H = Unhide all
        keymap.bind(Shortcut::with_mods('H', Modifiers::CTRL), Action::UnhideAll);

        // Shading modes (hotbox style)
        // 4 = Wireframe, 5 = Shaded, 6 = Textured, 7 = Lighted
        // We use Z for shading cycle
        keymap.bind(Shortcut::new('Z'), Action::ShadingCycle);

        // X-ray mode (we don't have this yet)
        // Alt+X = Toggle X-ray

        // Playback
        // Alt+V = Play/pause
        keymap.bind(Shortcut::with_mods('V', Modifiers::ALT), Action::PlayPause);
        // Space also works
        keymap.bind(Shortcut::new(' '), Action::PlayPause);

        // File operations (standard)
        keymap.bind(Shortcut::with_mods('N', Modifiers::CTRL), Action::NewScene);
        keymap.bind(Shortcut::with_mods('O', Modifiers::CTRL), Action::OpenFile);
        keymap.bind(Shortcut::with_mods('S', Modifiers::CTRL), Action::SaveFile);
        keymap.bind(
            Shortcut::with_mods('S', Modifiers::CTRL_SHIFT),
            Action::SaveAs,
        );

        // Edit operations
        keymap.bind(Shortcut::with_mods('Z', Modifiers::CTRL), Action::Undo);
        keymap.bind(Shortcut::with_mods('Y', Modifiers::CTRL), Action::Redo);
        keymap.bind(Shortcut::with_mods('X', Modifiers::CTRL), Action::Cut);
        keymap.bind(Shortcut::with_mods('C', Modifiers::CTRL), Action::Copy);
        keymap.bind(Shortcut::with_mods('V', Modifiers::CTRL), Action::Paste);

        // Rendering
        keymap.bind(Shortcut::new('\r'), Action::RenderImage);

        keymap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maya_transforms() {
        let keymap = Keymap::maya();

        // Maya-style transforms (same as Max: Q/W/E/R)
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
    fn test_maya_frame_operations() {
        let keymap = Keymap::maya();

        assert_eq!(
            keymap.get_action(&Shortcut::new('F')),
            Some(Action::FocusSelected)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::new('A')),
            Some(Action::FrameAll)
        );
    }

    #[test]
    fn test_maya_duplicate() {
        let keymap = Keymap::maya();

        assert_eq!(
            keymap.get_action(&Shortcut::with_mods('D', Modifiers::CTRL)),
            Some(Action::Duplicate)
        );
    }

    #[test]
    fn test_maya_playback() {
        let keymap = Keymap::maya();

        assert_eq!(
            keymap.get_action(&Shortcut::with_mods('V', Modifiers::ALT)),
            Some(Action::PlayPause)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::new(' ')),
            Some(Action::PlayPause)
        );
    }
}
