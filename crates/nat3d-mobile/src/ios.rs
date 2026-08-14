// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! iOS native integration with Apple Pencil support.
//!
//! This module provides:
//! - FFI callbacks for iOS UIKit to push stylus events
//! - ApplePencilProvider implementing StylusProvider trait
//! - Thread-safe event queue for cross-language communication
//!
//! # Architecture
//!
//! ```text
//! iOS UIKit (Swift/ObjC)
//!     │
//!     ▼ FFI extern "C"
//! nat3d_ios_pencil_* functions
//!     │
//!     ▼ lock-free queue
//! ApplePencilProvider::poll()
//!     │
//!     ▼
//! nat3d_core::StylusEvent
//! ```
//!
//! # Usage from Swift
//!
//! ```swift
//! // In your UIViewController handling Apple Pencil:
//! override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
//!     for touch in touches {
//!         if touch.type == .pencil {
//!             nat3d_ios_pencil_down(
//!                 Float(touch.location(in: view).x / view.bounds.width),
//!                 Float(touch.location(in: view).y / view.bounds.height),
//!                 Float(touch.force / touch.maximumPossibleForce),
//!                 Float(touch.altitudeAngle),
//!                 Float(touch.azimuthAngle(in: view))
//!             )
//!         }
//!     }
//! }
//! ```

use nat3d_core::stylus::{StylusCapabilities, StylusEvent, StylusInput, StylusProvider};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

/// Global event queue for iOS FFI callbacks.
static EVENT_QUEUE: OnceLock<Arc<Mutex<EventQueue>>> = OnceLock::new();

struct EventQueue {
    events: VecDeque<StylusEvent>,
    start_time: Instant,
    connected: bool,
}

impl EventQueue {
    fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(64),
            start_time: Instant::now(),
            connected: false,
        }
    }

    fn push(&mut self, event: StylusEvent) {
        if self.events.len() >= 256 {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    fn pop(&mut self) -> Option<StylusEvent> {
        self.events.pop_front()
    }

    fn timestamp_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

fn get_queue() -> &'static Arc<Mutex<EventQueue>> {
    EVENT_QUEUE.get_or_init(|| Arc::new(Mutex::new(EventQueue::new())))
}

// =============================================================================
// FFI Functions - Called from iOS Swift/ObjC
// =============================================================================

/// Initialize the Apple Pencil subsystem. Call once at app startup.
#[no_mangle]
pub extern "C" fn nat3d_ios_pencil_init() {
    let queue = get_queue();
    if let Ok(mut q) = queue.lock() {
        q.connected = true;
        q.start_time = Instant::now();
    }
}

/// Shutdown the Apple Pencil subsystem.
#[no_mangle]
pub extern "C" fn nat3d_ios_pencil_shutdown() {
    let queue = get_queue();
    if let Ok(mut q) = queue.lock() {
        q.connected = false;
        q.events.clear();
    }
}

/// Report pencil touch down.
///
/// # Parameters
/// - x, y: Normalized position (0.0-1.0)
/// - pressure: Force (0.0-1.0)
/// - altitude: Angle from surface in radians (0 = flat, π/2 = perpendicular)
/// - azimuth: Rotation around perpendicular axis in radians (0-2π)
#[no_mangle]
pub extern "C" fn nat3d_ios_pencil_down(
    x: f32,
    y: f32,
    pressure: f32,
    altitude: f32,
    azimuth: f32,
) {
    let queue = get_queue();
    if let Ok(mut q) = queue.lock() {
        let input = StylusInput::new(x, y, pressure)
            .with_tilt(altitude, azimuth)
            .with_timestamp(q.timestamp_ms());
        q.push(StylusEvent::Down(input));
    }
}

/// Report pencil movement while in contact.
#[no_mangle]
pub extern "C" fn nat3d_ios_pencil_move(
    x: f32,
    y: f32,
    pressure: f32,
    altitude: f32,
    azimuth: f32,
) {
    let queue = get_queue();
    if let Ok(mut q) = queue.lock() {
        let input = StylusInput::new(x, y, pressure)
            .with_tilt(altitude, azimuth)
            .with_timestamp(q.timestamp_ms());
        q.push(StylusEvent::Move(input));
    }
}

/// Report pencil lift off.
#[no_mangle]
pub extern "C" fn nat3d_ios_pencil_up(x: f32, y: f32) {
    let queue = get_queue();
    if let Ok(mut q) = queue.lock() {
        let input = StylusInput::new(x, y, 0.0).with_timestamp(q.timestamp_ms());
        q.push(StylusEvent::Up(input));
    }
}

/// Report pencil hover (proximity without contact).
#[no_mangle]
pub extern "C" fn nat3d_ios_pencil_hover(x: f32, y: f32, altitude: f32, azimuth: f32) {
    let queue = get_queue();
    if let Ok(mut q) = queue.lock() {
        let input = StylusInput::new(x, y, 0.0)
            .with_tilt(altitude, azimuth)
            .with_timestamp(q.timestamp_ms());
        q.push(StylusEvent::Hover(input));
    }
}

/// Report pencil left proximity range.
#[no_mangle]
pub extern "C" fn nat3d_ios_pencil_proximity_out() {
    let queue = get_queue();
    if let Ok(mut q) = queue.lock() {
        q.push(StylusEvent::ProximityOut);
    }
}

/// Report double-tap on Apple Pencil 2 barrel (tool switch gesture).
/// This is exposed as a barrel button press on StylusInput.
#[no_mangle]
pub extern "C" fn nat3d_ios_pencil_double_tap(x: f32, y: f32) {
    let queue = get_queue();
    if let Ok(mut q) = queue.lock() {
        let input = StylusInput::new(x, y, 0.0)
            .with_barrel_button(true)
            .with_timestamp(q.timestamp_ms());
        q.push(StylusEvent::Down(input));
    }
}

// =============================================================================
// StylusProvider Implementation
// =============================================================================

/// Apple Pencil input provider for iOS.
///
/// Implements `StylusProvider` trait from nat3d-core, consuming events
/// pushed by iOS via FFI callbacks.
pub struct ApplePencilProvider {
    _private: (),
}

impl ApplePencilProvider {
    /// Create a new Apple Pencil provider.
    ///
    /// Automatically initializes the FFI subsystem.
    pub fn new() -> Self {
        // Initialize on first provider creation
        if let Ok(mut q) = get_queue().lock() {
            if !q.connected {
                q.connected = true;
                q.start_time = Instant::now();
            }
        }
        Self { _private: () }
    }
}

impl Default for ApplePencilProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl StylusProvider for ApplePencilProvider {
    fn poll(&mut self) -> Option<StylusEvent> {
        if let Ok(mut q) = get_queue().lock() {
            q.pop()
        } else {
            None
        }
    }

    fn capabilities(&self) -> StylusCapabilities {
        StylusCapabilities::apple_pencil()
    }

    fn device_name(&self) -> &str {
        "Apple Pencil"
    }

    fn is_connected(&self) -> bool {
        if let Ok(q) = get_queue().lock() {
            q.connected
        } else {
            false
        }
    }
}

// =============================================================================
// iOS App Entry Point
// =============================================================================

/// iOS application entry point.
///
/// Called by the iOS runtime when the app launches.
/// Sets up eframe with Metal backend and Apple Pencil integration.
#[cfg(target_os = "ios")]
pub fn ios_main() {
    use nat3d_app::Nat3DApp;

    // Initialize Apple Pencil subsystem
    nat3d_ios_pencil_init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NAT3D")
            .with_fullscreen(true),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "nat3d_ios",
        options,
        Box::new(|cc| Ok(Box::new(Nat3DApp::new(cc)))),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialises the tests that touch the process-wide event queue.
    ///
    /// The queue is a single `static` shared by the whole process, and cargo
    /// runs tests in parallel threads inside one process, so without this the
    /// three tests below interleave: the overflow test pushes three hundred
    /// events while the event test is trying to read the three it queued, and
    /// the init test clears `connected` underneath both. That is what made them
    /// flaky. The global is not a defect in the code under test, it is how an
    /// FFI callback surface has to work, so the tests are serialised rather
    /// than the design changed.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Takes the lock and returns the queue to a known empty, disconnected
    /// state, so each test starts from the same place regardless of order.
    ///
    /// A poisoned lock means an earlier test panicked while holding it. The
    /// state it left behind does not matter because this function resets it, so
    /// the guard is recovered instead of turning one failure into four.
    fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let queue = get_queue();
        let mut q = queue.lock().unwrap_or_else(|e| e.into_inner());
        q.events.clear();
        q.connected = false;
        drop(q);
        guard
    }

    #[test]
    fn test_pencil_init_shutdown() {
        let _guard = exclusive();
        nat3d_ios_pencil_init();

        let queue = get_queue();
        assert!(queue.lock().unwrap().connected);

        nat3d_ios_pencil_shutdown();
        assert!(!queue.lock().unwrap().connected);
    }

    #[test]
    fn test_pencil_events() {
        let _guard = exclusive();
        nat3d_ios_pencil_init();

        nat3d_ios_pencil_down(0.5, 0.5, 0.8, 1.2, 0.5);
        nat3d_ios_pencil_move(0.6, 0.6, 0.9, 1.1, 0.6);
        nat3d_ios_pencil_up(0.7, 0.7);

        let mut provider = ApplePencilProvider::new();

        let e1 = provider.poll().expect("Should have down event");
        assert!(matches!(e1, StylusEvent::Down(_)));

        let e2 = provider.poll().expect("Should have move event");
        assert!(matches!(e2, StylusEvent::Move(_)));

        let e3 = provider.poll().expect("Should have up event");
        assert!(matches!(e3, StylusEvent::Up(_)));

        assert!(provider.poll().is_none());

        nat3d_ios_pencil_shutdown();
    }

    // Not serialised: this one reads nothing from the shared queue, so it is
    // free to run alongside the others.
    #[test]
    fn test_provider_capabilities() {
        let provider = ApplePencilProvider::new();
        let caps = provider.capabilities();

        assert!(caps.pressure);
        assert!(caps.tilt);
        assert_eq!(caps.pressure_levels, 4096);
        assert_eq!(provider.device_name(), "Apple Pencil");
    }

    #[test]
    fn test_event_queue_overflow() {
        let _guard = exclusive();
        nat3d_ios_pencil_init();

        // Push more than queue capacity
        for i in 0..300 {
            nat3d_ios_pencil_move(i as f32 / 300.0, 0.5, 0.5, 1.0, 0.0);
        }

        let queue = get_queue();
        let len = queue.lock().unwrap().events.len();
        assert!(len <= 256, "Queue should not exceed max capacity");

        nat3d_ios_pencil_shutdown();
    }
}
