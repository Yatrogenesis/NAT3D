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

//! Default keyboard shortcuts for NAT3D.
//!
//! Provides a balanced keymap combining common conventions.

use std::collections::HashMap;

/// Keyboard modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Ctrl (Command on macOS).
    pub ctrl: bool,
    /// Shift key.
    pub shift: bool,
    /// Alt (Option on macOS).
    pub alt: bool,
}

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Self {
        ctrl: false,
        shift: false,
        alt: false,
    };
    /// Ctrl only.
    pub const CTRL: Self = Self {
        ctrl: true,
        shift: false,
        alt: false,
    };
    /// Shift only.
    pub const SHIFT: Self = Self {
        ctrl: false,
        shift: true,
        alt: false,
    };
    /// Alt only.
    pub const ALT: Self = Self {
        ctrl: false,
        shift: false,
        alt: true,
    };
    /// Ctrl+Shift.
    pub const CTRL_SHIFT: Self = Self {
        ctrl: true,
        shift: true,
        alt: false,
    };
}

/// Shortcut key with modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shortcut {
    /// Key code (A-Z, 0-9, F1-F12, etc).
    pub key: char,
    /// Modifier keys.
    pub modifiers: Modifiers,
}

impl Shortcut {
    /// Create shortcut without modifiers.
    pub const fn new(key: char) -> Self {
        Self {
            key,
            modifiers: Modifiers::NONE,
        }
    }

    /// Create shortcut with modifiers.
    pub const fn with_mods(key: char, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
}

/// Action that can be triggered by shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    // Selection modes
    SelectMode,
    MoveMode,
    RotateMode,
    ScaleMode,

    // Selection operations
    SelectAll,
    DeselectAll,
    InvertSelection,
    HideSelected,
    UnhideAll,

    // Object operations
    Delete,
    Duplicate,
    FocusSelected,

    // Edit modes
    ToggleEditMode,
    ToggleObjectMode,

    // View controls
    FrontView,
    RightView,
    TopView,
    CameraView,
    ResetView,
    FrameAll,

    // Shading modes
    ShadingWireframe,
    ShadingSolid,
    ShadingMaterial,
    ShadingRendered,

    // Transform constraints
    ConstrainX,
    ConstrainY,
    ConstrainZ,

    // Timeline
    PlayPause,
    StepForward,
    StepBackward,
    JumpStart,
    JumpEnd,

    // Panels
    ToggleProperties,
    ToggleToolbar,
    ToggleConsole,
    ToggleTimeline,

    // File operations
    NewScene,
    OpenFile,
    SaveFile,
    SaveAs,
    Import,
    Export,

    // Edit operations
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,

    // Rendering
    RenderImage,
    RenderAnimation,

    // Misc
    Search,
    Preferences,
    ToggleFullscreen,
    Quit,
}

/// Keymap defining shortcuts for all actions.
#[derive(Debug, Clone)]
pub struct Keymap {
    /// Map from shortcut to action.
    shortcuts: HashMap<Shortcut, Action>,
    /// Map from action to primary shortcut.
    reverse: HashMap<Action, Shortcut>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new()
    }
}

impl Keymap {
    /// Create empty keymap.
    pub fn new() -> Self {
        Self {
            shortcuts: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    /// Bind a shortcut to an action.
    pub fn bind(&mut self, shortcut: Shortcut, action: Action) {
        self.shortcuts.insert(shortcut, action);
        self.reverse.insert(action, shortcut);
    }

    /// Get action for shortcut.
    pub fn get_action(&self, shortcut: &Shortcut) -> Option<Action> {
        self.shortcuts.get(shortcut).copied()
    }

    /// Get primary shortcut for action.
    pub fn get_shortcut(&self, action: Action) -> Option<Shortcut> {
        self.reverse.get(&action).copied()
    }

    /// Create default keymap.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        let mut keymap = Self::new();

        // Selection modes (3ds Max style)
        keymap.bind(Shortcut::new('Q'), Action::SelectMode);
        keymap.bind(Shortcut::new('W'), Action::MoveMode);
        keymap.bind(Shortcut::new('E'), Action::RotateMode);
        keymap.bind(Shortcut::new('R'), Action::ScaleMode);

        // Selection operations
        keymap.bind(Shortcut::with_mods('A', Modifiers::CTRL), Action::SelectAll);
        keymap.bind(Shortcut::new('A'), Action::ToggleSelectAll);
        keymap.bind(
            Shortcut::with_mods('I', Modifiers::CTRL),
            Action::InvertSelection,
        );
        keymap.bind(Shortcut::new('H'), Action::HideSelected);
        keymap.bind(Shortcut::with_mods('H', Modifiers::ALT), Action::UnhideAll);
        keymap.bind(Shortcut::new('\x1B'), Action::DeselectAll); // Escape

        // Object operations
        keymap.bind(Shortcut::new('\x7F'), Action::Delete); // Delete key
        keymap.bind(
            Shortcut::with_mods('D', Modifiers::SHIFT),
            Action::Duplicate,
        );
        keymap.bind(Shortcut::new('F'), Action::FocusSelected);

        // Edit modes
        keymap.bind(Shortcut::new('\t'), Action::ToggleEditMode); // Tab

        // View controls
        keymap.bind(Shortcut::new('1'), Action::FrontView);
        keymap.bind(Shortcut::new('3'), Action::RightView);
        keymap.bind(Shortcut::new('7'), Action::TopView);
        keymap.bind(Shortcut::new('0'), Action::CameraView);
        keymap.bind(Shortcut::new('.'), Action::FrameAll);

        // Shading modes (when in select mode)
        keymap.bind(Shortcut::new('Z'), Action::ShadingCycle);

        // Transform constraints
        keymap.bind(Shortcut::new('X'), Action::ConstrainX);
        keymap.bind(Shortcut::new('Y'), Action::ConstrainY);
        keymap.bind(Shortcut::new('Z'), Action::ConstrainZ);

        // Timeline
        keymap.bind(Shortcut::new(' '), Action::PlayPause);

        // Panels
        keymap.bind(Shortcut::new('N'), Action::ToggleProperties);
        keymap.bind(Shortcut::new('T'), Action::ToggleToolbar);

        // File operations
        keymap.bind(Shortcut::with_mods('N', Modifiers::CTRL), Action::NewScene);
        keymap.bind(Shortcut::with_mods('O', Modifiers::CTRL), Action::OpenFile);
        keymap.bind(Shortcut::with_mods('S', Modifiers::CTRL), Action::SaveFile);
        keymap.bind(
            Shortcut::with_mods('S', Modifiers::CTRL_SHIFT),
            Action::SaveAs,
        );
        keymap.bind(Shortcut::with_mods('I', Modifiers::CTRL), Action::Import);
        keymap.bind(Shortcut::with_mods('E', Modifiers::CTRL), Action::Export);

        // Edit operations
        keymap.bind(Shortcut::with_mods('Z', Modifiers::CTRL), Action::Undo);
        keymap.bind(
            Shortcut::with_mods('Z', Modifiers::CTRL_SHIFT),
            Action::Redo,
        );
        keymap.bind(Shortcut::with_mods('X', Modifiers::CTRL), Action::Cut);
        keymap.bind(Shortcut::with_mods('C', Modifiers::CTRL), Action::Copy);
        keymap.bind(Shortcut::with_mods('V', Modifiers::CTRL), Action::Paste);

        // Rendering
        keymap.bind(Shortcut::new('\r'), Action::RenderImage); // Enter
        keymap.bind(
            Shortcut::with_mods('\r', Modifiers::CTRL),
            Action::RenderAnimation,
        );

        // Misc
        keymap.bind(Shortcut::with_mods('F', Modifiers::CTRL), Action::Search);
        keymap.bind(
            Shortcut::with_mods(',', Modifiers::CTRL),
            Action::Preferences,
        );
        keymap.bind(Shortcut::with_mods('Q', Modifiers::CTRL), Action::Quit);

        keymap
    }
}

// Additional action aliases
impl Action {
    /// Shading cycle (wireframe -> solid -> material -> rendered).
    #[allow(non_upper_case_globals)]
    pub const ShadingCycle: Self = Self::ShadingWireframe;
    /// Toggle select all / deselect all.
    #[allow(non_upper_case_globals)]
    pub const ToggleSelectAll: Self = Self::SelectAll;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_creation() {
        let s1 = Shortcut::new('A');
        assert_eq!(s1.key, 'A');
        assert!(!s1.modifiers.ctrl);

        let s2 = Shortcut::with_mods('B', Modifiers::CTRL);
        assert_eq!(s2.key, 'B');
        assert!(s2.modifiers.ctrl);
    }

    #[test]
    fn test_keymap_binding() {
        let mut keymap = Keymap::new();
        keymap.bind(Shortcut::new('A'), Action::SelectAll);

        assert_eq!(
            keymap.get_action(&Shortcut::new('A')),
            Some(Action::SelectAll)
        );
        assert_eq!(
            keymap.get_shortcut(Action::SelectAll),
            Some(Shortcut::new('A'))
        );
    }

    #[test]
    fn test_default_keymap() {
        let keymap = Keymap::default();

        // Test a few bindings
        assert_eq!(
            keymap.get_action(&Shortcut::new('Q')),
            Some(Action::SelectMode)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::new('W')),
            Some(Action::MoveMode)
        );
        assert_eq!(
            keymap.get_action(&Shortcut::with_mods('Z', Modifiers::CTRL)),
            Some(Action::Undo)
        );
    }

    #[test]
    fn test_modifiers() {
        assert!(!Modifiers::NONE.ctrl);
        assert!(!Modifiers::NONE.shift);
        assert!(!Modifiers::NONE.alt);

        assert!(Modifiers::CTRL.ctrl);
        assert!(!Modifiers::CTRL.shift);

        assert!(Modifiers::CTRL_SHIFT.ctrl);
        assert!(Modifiers::CTRL_SHIFT.shift);
    }
}
