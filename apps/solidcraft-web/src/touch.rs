//! Fingers and the pen (Apple Pencil) on a touch screen. eframe turns touches into a mouse
//! without saying what touched, so the page listens to the browser's pointer events first (on
//! their way down to the canvas, before eframe) and keeps each touch's kind: a finger or a pen.
//! The app's raw-input hook hands them to `solidcraft_ui_egui::touch_input`, which decides what a
//! finger and the pen do.

use std::cell::RefCell;

use solidcraft_ui_egui::touch_input::{TouchKind, TouchPhase, TouchSample};
use wasm_bindgen::{JsCast as _, closure::Closure};

/// A pointer sample in the page's CSS pixels.
struct Raw {
    id: i32,
    kind: TouchKind,
    phase: TouchPhase,
    x: f32,
    y: f32,
}

thread_local! {
    /// Samples since the last frame.
    static SAMPLES: RefCell<Vec<Raw>> = const { RefCell::new(Vec::new()) };
}

/// Listen for touch and pen pointers for as long as the page lives. Mice and trackpads are left
/// to eframe as before.
pub fn listen(window: &web_sys::Window, ctx: &egui::Context) {
    for (name, phase) in
        [("pointerdown", TouchPhase::Down), ("pointermove", TouchPhase::Move), ("pointerup", TouchPhase::Up), ("pointercancel", TouchPhase::Cancel)]
    {
        let ctx = ctx.clone();
        let on = Closure::<dyn FnMut(web_sys::PointerEvent)>::new(move |e: web_sys::PointerEvent| {
            let kind = match e.pointer_type().as_str() {
                "touch" => TouchKind::Finger,
                "pen" => TouchKind::Pen,
                _ => return,
            };
            let raw = Raw { id: e.pointer_id(), kind, phase, x: e.client_x() as f32, y: e.client_y() as f32 };
            SAMPLES.with(|s| s.borrow_mut().push(raw));
            ctx.request_repaint();
        });
        let _ = window.add_event_listener_with_callback_and_bool(name, on.as_ref().unchecked_ref(), true);
        // The listener lives as long as the page.
        on.forget();
    }
}

/// The samples since the last frame, in egui points over the element `canvas_id`.
pub fn take(canvas_id: &str, ctx: &egui::Context) -> Vec<TouchSample> {
    let raw = SAMPLES.with(|s| std::mem::take(&mut *s.borrow_mut()));
    if raw.is_empty() {
        return Vec::new();
    }
    let canvas = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(canvas_id));
    let (left, top) = canvas.map(|c| c.get_bounding_client_rect()).map_or((0.0, 0.0), |r| (r.left() as f32, r.top() as f32));
    let zoom = ctx.zoom_factor();
    let zoom = if zoom.is_finite() && zoom > 0.0 { zoom } else { 1.0 };
    raw.into_iter()
        .map(|r| TouchSample { id: r.id, kind: r.kind, phase: r.phase, pos: egui::pos2((r.x - left) / zoom, (r.y - top) / zoom) })
        .collect()
}
