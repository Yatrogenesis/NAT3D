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
    /// The Shift modifier.
    Shift,
    /// The Ctrl modifier.
    Ctrl,
    /// The Alt modifier.
    Alt,
    /// The Super modifier.
    Super,
}

/// Key code (simplified for common keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    // Letters
    /// The `A` key.
    A,
    /// The `B` key.
    B,
    /// The `C` key.
    C,
    /// The `D` key.
    D,
    /// The `E` key.
    E,
    /// The `F` key.
    F,
    /// The `G` key.
    G,
    /// The `H` key.
    H,
    /// The `I` key.
    I,
    /// The `J` key.
    J,
    /// The `K` key.
    K,
    /// The `L` key.
    L,
    /// The `M` key.
    M,
    /// The `N` key.
    N,
    /// The `O` key.
    O,
    /// The `P` key.
    P,
    /// The `Q` key.
    Q,
    /// The `R` key.
    R,
    /// The `S` key.
    S,
    /// The `T` key.
    T,
    /// The `U` key.
    U,
    /// The `V` key.
    V,
    /// The `W` key.
    W,
    /// The `X` key.
    X,
    /// The `Y` key.
    Y,
    /// The `Z` key.
    Z,
    // Numbers
    /// The `0` key on the number row.
    Num0,
    /// The `1` key on the number row.
    Num1,
    /// The `2` key on the number row.
    Num2,
    /// The `3` key on the number row.
    Num3,
    /// The `4` key on the number row.
    Num4,
    /// The `5` key on the number row.
    Num5,
    /// The `6` key on the number row.
    Num6,
    /// The `7` key on the number row.
    Num7,
    /// The `8` key on the number row.
    Num8,
    /// The `9` key on the number row.
    Num9,
    // Function keys
    /// The `F1` function key.
    F1,
    /// The `F2` function key.
    F2,
    /// The `F3` function key.
    F3,
    /// The `F4` function key.
    F4,
    /// The `F5` function key.
    F5,
    /// The `F6` function key.
    F6,
    /// The `F7` function key.
    F7,
    /// The `F8` function key.
    F8,
    /// The `F9` function key.
    F9,
    /// The `F10` function key.
    F10,
    /// The `F11` function key.
    F11,
    /// The `F12` function key.
    F12,
    // Special keys
    /// The space bar.
    Space,
    /// The Enter key.
    Enter,
    /// The Escape key.
    Escape,
    /// The Tab key.
    Tab,
    /// The Backspace key.
    Backspace,
    /// The Delete key.
    Delete,
    // Arrow keys
    /// The left arrow key.
    Left,
    /// The right arrow key.
    Right,
    /// The up arrow key.
    Up,
    /// The down arrow key.
    Down,
    // Modifiers (when used as keys)
    /// The left Shift key.
    LeftShift,
    /// The right Shift key.
    RightShift,
    /// The left Ctrl key.
    LeftCtrl,
    /// The right Ctrl key.
    RightCtrl,
    /// The left Alt key.
    LeftAlt,
    /// The right Alt key.
    RightAlt,
    // Other
    /// A key that is not represented by any other variant.
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
    /// Undo action (produced by Ctrl+Z).
    Undo,
    /// Redo action (produced by Ctrl+Y).
    Redo,
    /// Save action (produced by Ctrl+S).
    Save,
    /// Open action (produced by Ctrl+O).
    Open,
    /// New action (produced by Ctrl+N).
    New,
    /// Delete action (produced by the Delete or X key).
    Delete,
    /// Move tool action (produced by the G key).
    MoveTool,
    /// Rotate tool action (produced by the R key).
    RotateTool,
    /// Scale tool action (produced by S without Ctrl).
    ScaleTool,
    /// Extrude tool action (produced by the E key).
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
