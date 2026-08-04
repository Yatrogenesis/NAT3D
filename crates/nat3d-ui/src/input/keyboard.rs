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

//! Keyboard input handling.

use std::collections::HashSet;

/// Keyboard modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    Shift,
    Ctrl,
    Alt,
    Super,
}

/// Key code (simplified for common keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    // Numbers
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    // Special keys
    Space,
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    // Arrow keys
    Left,
    Right,
    Up,
    Down,
    // Modifiers (when used as keys)
    LeftShift,
    RightShift,
    LeftCtrl,
    RightCtrl,
    LeftAlt,
    RightAlt,
    // Other
    Unknown,
}

/// Keyboard state tracker.
#[derive(Debug, Clone)]
pub struct KeyboardState {
    /// Currently pressed keys.
    pressed_keys: HashSet<Key>,
    /// Active modifiers.
    active_modifiers: HashSet<Modifier>,
    /// Just pressed this frame.
    just_pressed: HashSet<Key>,
    /// Just released this frame.
    just_released: HashSet<Key>,
}

impl KeyboardState {
    /// Create a new keyboard state.
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            active_modifiers: HashSet::new(),
            just_pressed: HashSet::new(),
            just_released: HashSet::new(),
        }
    }

    /// Handle key press event.
    pub fn handle_key_press(&mut self, key: Key, modifiers: &[Modifier]) {
        if !self.pressed_keys.contains(&key) {
            self.just_pressed.insert(key);
        }
        self.pressed_keys.insert(key);

        // Update modifiers
        self.active_modifiers.clear();
        for &modifier in modifiers {
            self.active_modifiers.insert(modifier);
        }
    }

    /// Handle key release event.
    pub fn handle_key_release(&mut self, key: Key) {
        self.pressed_keys.remove(&key);
        self.just_released.insert(key);
    }

    /// Clear frame-specific state (call at end of frame).
    pub fn clear_frame_state(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
    }

    /// Check if key is currently pressed.
    pub fn is_pressed(&self, key: Key) -> bool {
        self.pressed_keys.contains(&key)
    }

    /// Check if key was just pressed this frame.
    pub fn is_just_pressed(&self, key: Key) -> bool {
        self.just_pressed.contains(&key)
    }

    /// Check if key was just released this frame.
    pub fn is_just_released(&self, key: Key) -> bool {
        self.just_released.contains(&key)
    }

    /// Check if modifier is active.
    pub fn is_modifier_active(&self, modifier: Modifier) -> bool {
        self.active_modifiers.contains(&modifier)
    }

    /// Get shortcut action for current key combination.
    pub fn get_shortcut_action(&self) -> Option<ShortcutAction> {
        // Check for common shortcuts
        if self.is_modifier_active(Modifier::Ctrl) {
            if self.is_just_pressed(Key::Z) {
                return Some(ShortcutAction::Undo);
            }
            if self.is_just_pressed(Key::Y) {
                return Some(ShortcutAction::Redo);
            }
            if self.is_just_pressed(Key::S) {
                return Some(ShortcutAction::Save);
            }
            if self.is_just_pressed(Key::O) {
                return Some(ShortcutAction::Open);
            }
            if self.is_just_pressed(Key::N) {
                return Some(ShortcutAction::New);
            }
        }

        // Tool shortcuts
        if self.is_just_pressed(Key::G) {
            return Some(ShortcutAction::MoveTool);
        }
        if self.is_just_pressed(Key::R) {
            return Some(ShortcutAction::RotateTool);
        }
        if self.is_just_pressed(Key::S) && !self.is_modifier_active(Modifier::Ctrl) {
            return Some(ShortcutAction::ScaleTool);
        }
        if self.is_just_pressed(Key::E) {
            return Some(ShortcutAction::ExtrudeTool);
        }

        // Delete
        if self.is_just_pressed(Key::Delete) || self.is_just_pressed(Key::X) {
            return Some(ShortcutAction::Delete);
        }

        None
    }
}

/// Shortcut action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    Undo,
    Redo,
    Save,
    Open,
    New,
    Delete,
    MoveTool,
    RotateTool,
    ScaleTool,
    ExtrudeTool,
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_state() {
        let mut state = KeyboardState::new();
        state.handle_key_press(Key::A, &[]);
        assert!(state.is_pressed(Key::A));
        assert!(state.is_just_pressed(Key::A));

        state.clear_frame_state();
        assert!(state.is_pressed(Key::A));
        assert!(!state.is_just_pressed(Key::A));

        state.handle_key_release(Key::A);
        assert!(!state.is_pressed(Key::A));
    }

    #[test]
    fn test_modifiers() {
        let mut state = KeyboardState::new();
        state.handle_key_press(Key::A, &[Modifier::Ctrl]);
        assert!(state.is_modifier_active(Modifier::Ctrl));
    }
}
