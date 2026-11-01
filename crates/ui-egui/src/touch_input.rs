//! Fingers and the pen (Apple Pencil) on a touch screen, for the browser app on a tablet.
//!
//! The browser reports every touch as a pointer with a kind: a finger or a pen. The pen is the
//! precise one, so it works as the mouse's left button everywhere: it picks, draws sketches,
//! drags geometry and presses buttons. Fingers on the model navigate, the way tablet design apps
//! do: one finger turns the view (pans a flat page), two fingers pan and pinch zoom, a tap is a
//! click (it picks, places a point, presses a button on the model) and a press held still opens
//! the context menu. A finger that starts on a panel, a window or a menu acts as the mouse there,
//! so lists scroll and sliders drag. While the pen is down fingers are ignored, so a palm resting
//! on the screen does nothing. With Preferences › Navigation › "Finger gestures" off, a finger
//! acts as the mouse everywhere, as it did before.
//!
//! The app's raw-input hook hands each frame's pointer samples to [`TouchInput::update`], which
//! replaces the browser's own mouse emulation of touches with these events and leaves the camera
//! motion for [`crate::navigation::handle`] to apply ([`take_camera`]), so the camera still has
//! one path. The state machine ([`TouchInput::step`]) is plain Rust and tested natively.

use egui::{Event, Modifiers, PointerButton, Pos2, Rect, Vec2};

/// What touched the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchKind {
    /// A finger.
    Finger,
    /// A pen or stylus (Apple Pencil).
    Pen,
}

/// A pointer sample's phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Down,
    Move,
    Up,
    /// The system took the touch away (an incoming call, a system gesture).
    Cancel,
}

/// One pointer sample from the browser, in egui points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchSample {
    /// The browser's pointer id: stable from down to up.
    pub id: i32,
    pub kind: TouchKind,
    pub phase: TouchPhase,
    pub pos: Pos2,
}

/// Camera motion from fingers on the model, for [`crate::navigation::handle`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchCamera {
    /// One finger dragging: turn the view (pan on a flat page), in points.
    pub orbit: Vec2,
    /// Where the turning finger first touched (the pivot under it, for Orbit around the cursor).
    pub orbit_from: Option<Pos2>,
    /// Two or more fingers moving together, in points: the model follows them.
    pub pan: Vec2,
    /// Fingers spreading (> 1) or pinching (< 1).
    pub zoom: f32,
    /// Between the fingers: where a pinch zooms to.
    pub centre: Option<Pos2>,
    /// A finger gesture on the model is under way (it keeps its orbit pivot until it ends).
    pub active: bool,
}

impl Default for TouchCamera {
    fn default() -> Self {
        Self { orbit: Vec2::ZERO, orbit_from: None, pan: Vec2::ZERO, zoom: 1.0, centre: None, active: false }
    }
}

impl TouchCamera {
    /// Whether this moves the camera at all.
    pub fn moves(&self) -> bool {
        self.orbit != Vec2::ZERO || self.pan != Vec2::ZERO || self.zoom != 1.0
    }

    /// Add `later` (a later frame's motion) to this.
    fn merge(&mut self, later: TouchCamera) {
        self.orbit += later.orbit;
        self.orbit_from = self.orbit_from.or(later.orbit_from);
        self.pan += later.pan;
        self.zoom *= later.zoom;
        self.centre = later.centre.or(self.centre);
        self.active = later.active;
    }
}

/// How far (points) a finger moves before it is a drag, not a tap.
pub const SLOP: f32 = 10.0;
/// How long (seconds) a finger held still takes to open the context menu.
pub const LONG_PRESS: f64 = 0.5;
/// Fingers closer than this (points) give no pinch: the ratio would jump.
const MIN_SPREAD: f32 = 4.0;

/// What the current run of fingers (first down to last up) is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fingers {
    /// No finger down.
    Idle,
    /// One finger down on the model, not yet a drag, a tap or a hold.
    Pending { id: i32, start: Pos2, since: f64 },
    /// One finger acting as the mouse (it started off the model, or gestures are off).
    Mouse { id: i32 },
    /// One finger turning the view.
    Orbit { id: i32, start: Pos2, last: Pos2 },
    /// Two or more fingers: pan and pinch from their centre and spread.
    Multi { centre: Pos2, spread: f32 },
    /// Finished (a tap, a hold, the pen took over): the rest is ignored until every finger lifts.
    Done,
}

/// Turns touch-screen pointer samples into mouse events and camera motion. Keep one per app.
#[derive(Debug)]
pub struct TouchInput {
    fingers: Vec<(i32, Pos2)>,
    run: Fingers,
    /// The pen's pointer id while it is down.
    pen: Option<i32>,
    /// A touch or pen pointer was down at the end of the last frame.
    busy: bool,
}

impl Default for TouchInput {
    fn default() -> Self {
        Self { fingers: Vec::new(), run: Fingers::Idle, pen: None, busy: false }
    }
}

/// What one [`TouchInput::step`] produced.
#[derive(Debug, Default)]
pub struct Step {
    /// Mouse events to give egui, in order.
    pub events: Vec<Event>,
    /// Camera motion from fingers on the model.
    pub camera: TouchCamera,
    /// Seconds until a held finger becomes a long press (a frame is needed then).
    pub wake_in: Option<f64>,
}

impl TouchInput {
    /// Whether any finger or the pen is down.
    pub fn busy(&self) -> bool {
        self.pen.is_some() || !self.fingers.is_empty()
    }

    /// Process this frame's `samples` at time `now` (seconds). `gestures`: Preferences ›
    /// Navigation › Finger gestures; `on_model(p)`: a finger starting at `p` is on a camera view
    /// (not on a panel, window or menu); `orbit`: that view may turn (else one finger pans);
    /// `modifiers`: the keyboard's, for the mouse events (Shift+Pencil adds to the selection).
    pub fn step(
        &mut self,
        samples: &[TouchSample],
        now: f64,
        gestures: bool,
        on_model: &dyn Fn(Pos2) -> bool,
        orbit: bool,
        modifiers: Modifiers,
    ) -> Step {
        let mut out = Step::default();
        let now = if now.is_finite() { now } else { 0.0 };
        for s in samples {
            if !s.pos.x.is_finite() || !s.pos.y.is_finite() {
                continue;
            }
            match s.kind {
                TouchKind::Pen => self.pen_sample(s, modifiers, &mut out),
                TouchKind::Finger => self.finger_sample(s, now, gestures, on_model, orbit, modifiers, &mut out),
            }
        }
        // A finger held still on the model opens the context menu.
        if let Fingers::Pending { start, since, .. } = self.run {
            let held = now - since;
            if held >= LONG_PRESS {
                click(&mut out.events, start, PointerButton::Secondary, modifiers);
                self.run = Fingers::Done;
            } else {
                out.wake_in = Some((LONG_PRESS - held).max(0.0));
            }
        }
        out.camera.active = matches!(self.run, Fingers::Orbit { .. } | Fingers::Multi { .. });
        out
    }

    fn pen_sample(&mut self, s: &TouchSample, modifiers: Modifiers, out: &mut Step) {
        match s.phase {
            TouchPhase::Down => {
                if self.pen.is_some() {
                    return;
                }
                // The pen takes over from fingers: a finger acting as the mouse lets go first.
                if let Fingers::Mouse { id } = self.run {
                    let at = self.finger(id).unwrap_or(s.pos);
                    out.events.push(button(at, PointerButton::Primary, false, modifiers));
                }
                if !self.fingers.is_empty() {
                    self.run = Fingers::Done;
                }
                self.pen = Some(s.id);
                out.events.push(Event::PointerMoved(s.pos));
                out.events.push(button(s.pos, PointerButton::Primary, true, modifiers));
            }
            TouchPhase::Move if self.pen == Some(s.id) => out.events.push(Event::PointerMoved(s.pos)),
            TouchPhase::Up | TouchPhase::Cancel if self.pen == Some(s.id) => {
                self.pen = None;
                out.events.push(Event::PointerMoved(s.pos));
                out.events.push(button(s.pos, PointerButton::Primary, false, modifiers));
            }
            _ => {}
        }
    }

    fn finger_sample(
        &mut self,
        s: &TouchSample,
        now: f64,
        gestures: bool,
        on_model: &dyn Fn(Pos2) -> bool,
        orbit: bool,
        modifiers: Modifiers,
        out: &mut Step,
    ) {
        match s.phase {
            TouchPhase::Down => {
                if self.finger(s.id).is_some() {
                    return;
                }
                self.fingers.push((s.id, s.pos));
                if self.pen.is_some() {
                    // A palm under the pen.
                    self.run = Fingers::Done;
                    return;
                }
                self.run = match self.run {
                    Fingers::Idle if gestures && on_model(s.pos) => Fingers::Pending { id: s.id, start: s.pos, since: now },
                    Fingers::Idle => {
                        out.events.push(Event::PointerMoved(s.pos));
                        out.events.push(button(s.pos, PointerButton::Primary, true, modifiers));
                        Fingers::Mouse { id: s.id }
                    }
                    // A second finger on the model: pan and pinch from here on.
                    Fingers::Pending { .. } | Fingers::Orbit { .. } | Fingers::Multi { .. } => self.multi(),
                    run @ (Fingers::Mouse { .. } | Fingers::Done) => run,
                };
            }
            TouchPhase::Move => {
                let Some(slot) = self.fingers.iter_mut().find(|(id, _)| *id == s.id) else {
                    return;
                };
                slot.1 = s.pos;
                match self.run {
                    Fingers::Mouse { id } if id == s.id => out.events.push(Event::PointerMoved(s.pos)),
                    Fingers::Pending { id, start, .. } if id == s.id && (s.pos - start).length() > SLOP => {
                        // A drag: turn from where the finger first touched, so no motion is lost.
                        self.drag(&mut out.camera, s.pos - start, start, orbit);
                        self.run = Fingers::Orbit { id, start, last: s.pos };
                    }
                    Fingers::Orbit { id, start, last } if id == s.id => {
                        self.drag(&mut out.camera, s.pos - last, start, orbit);
                        self.run = Fingers::Orbit { id, start, last: s.pos };
                    }
                    Fingers::Multi { centre, spread } => {
                        let Fingers::Multi { centre: c, spread: d } = self.multi() else {
                            return;
                        };
                        out.camera.pan += c - centre;
                        if spread >= MIN_SPREAD && d >= MIN_SPREAD {
                            out.camera.zoom *= d / spread;
                        }
                        out.camera.centre = Some(c);
                        self.run = Fingers::Multi { centre: c, spread: d };
                    }
                    _ => {}
                }
            }
            TouchPhase::Up | TouchPhase::Cancel => {
                let Some(at) = self.finger(s.id) else {
                    return;
                };
                let at = if s.phase == TouchPhase::Up { s.pos } else { at };
                self.fingers.retain(|(id, _)| *id != s.id);
                match self.run {
                    Fingers::Mouse { id } if id == s.id => {
                        out.events.push(Event::PointerMoved(at));
                        out.events.push(button(at, PointerButton::Primary, false, modifiers));
                        self.run = Fingers::Done;
                    }
                    // A tap: a click where the finger first touched.
                    Fingers::Pending { id, start, .. } if id == s.id => {
                        if s.phase == TouchPhase::Up {
                            click(&mut out.events, start, PointerButton::Primary, modifiers);
                        }
                        self.run = Fingers::Done;
                    }
                    Fingers::Orbit { id, .. } if id == s.id => self.run = Fingers::Done,
                    // One finger left after a pinch keeps panning, without a jump.
                    Fingers::Multi { .. } if !self.fingers.is_empty() => self.run = self.multi(),
                    _ => {}
                }
                if self.fingers.is_empty() {
                    self.run = Fingers::Idle;
                }
            }
        }
    }

    fn drag(&self, camera: &mut TouchCamera, delta: Vec2, start: Pos2, orbit: bool) {
        if orbit {
            camera.orbit += delta;
            camera.orbit_from = camera.orbit_from.or(Some(start));
        } else {
            camera.pan += delta;
        }
    }

    fn finger(&self, id: i32) -> Option<Pos2> {
        self.fingers.iter().find(|(f, _)| *f == id).map(|(_, p)| *p)
    }

    /// The fingers' centre and mean distance from it.
    fn multi(&self) -> Fingers {
        let n = self.fingers.len().max(1) as f32;
        let sum = self.fingers.iter().fold(Vec2::ZERO, |a, (_, p)| a + p.to_vec2());
        let centre = (sum / n).to_pos2();
        let spread = self.fingers.iter().map(|(_, p)| (*p - centre).length()).sum::<f32>() / n;
        Fingers::Multi { centre, spread }
    }

    /// One frame of the browser app: `samples` replace the browser's mouse emulation of touches
    /// in `raw` (its pointer events while a touch or pen is down, and the raw touch events), the
    /// camera motion is left for [`crate::navigation::handle`] and a frame is asked for when a
    /// held finger becomes a long press.
    pub fn update(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput, samples: &[TouchSample], now: f64, gestures: bool) {
        let touch_frame = self.busy || !samples.is_empty() || raw.events.iter().any(|e| matches!(e, Event::Touch { .. }));
        raw.events.retain(|e| {
            !matches!(e, Event::Touch { .. })
                && !(touch_frame && matches!(e, Event::PointerMoved(_) | Event::PointerButton { .. } | Event::PointerGone))
        });
        let target = ctx.data(|d| d.get_temp::<Target>(egui::Id::new(TARGET_ID)));
        let pass = ctx.cumulative_pass_nr();
        let target = target.filter(|t| t.pass + 2 >= pass);
        // The keyboard's modifiers (an iPad keyboard's Shift): as of the last frame.
        let modifiers = ctx.input(|i| i.modifiers);
        let on_model = |p: Pos2| target.is_some_and(|t| t.rect.contains(p) && ctx.layer_id_at(p) == Some(t.layer));
        let step = self.step(samples, now, gestures, &on_model, target.is_none_or(|t| t.orbit), modifiers);
        self.busy = self.busy();
        raw.events.extend(step.events);
        if step.camera.moves() || step.camera.active || self.busy {
            ctx.data_mut(|d| {
                let slot = d.get_temp_mut_or_default::<TouchCamera>(egui::Id::new(CAMERA_ID));
                slot.merge(step.camera);
            });
        }
        if let Some(wait) = step.wake_in.and_then(|s| std::time::Duration::try_from_secs_f64(s).ok()) {
            ctx.request_repaint_after(wait);
        }
    }
}

const TARGET_ID: &str = "solidcraft.touch.target";
const CAMERA_ID: &str = "solidcraft.touch.camera";

/// The camera view fingers navigate: where it was drawn last frame.
#[derive(Clone, Copy, Debug)]
struct Target {
    rect: Rect,
    layer: egui::LayerId,
    orbit: bool,
    pass: u64,
}

/// Record the view `rect` (on `layer`) whose camera [`crate::navigation::handle`] moves this frame.
pub(crate) fn set_target(ctx: &egui::Context, rect: Rect, layer: egui::LayerId, orbit: bool) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(TARGET_ID), Target { rect, layer, orbit, pass }));
}

/// The finger motion waiting for the camera, taken once.
pub(crate) fn take_camera(ctx: &egui::Context) -> Option<TouchCamera> {
    ctx.data_mut(|d| d.remove_temp::<TouchCamera>(egui::Id::new(CAMERA_ID)))
}

fn button(pos: Pos2, button: PointerButton, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton { pos, button, pressed, modifiers }
}

/// A press and release at `pos` (egui reads both in one frame as a click).
fn click(events: &mut Vec<Event>, pos: Pos2, b: PointerButton, modifiers: Modifiers) {
    events.push(Event::PointerMoved(pos));
    events.push(button(pos, b, true, modifiers));
    events.push(button(pos, b, false, modifiers));
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn s(id: i32, kind: TouchKind, phase: TouchPhase, x: f32, y: f32) -> TouchSample {
        TouchSample { id, kind, phase, pos: pos2(x, y) }
    }
    fn f(id: i32, phase: TouchPhase, x: f32, y: f32) -> TouchSample {
        s(id, TouchKind::Finger, phase, x, y)
    }
    fn pen(phase: TouchPhase, x: f32, y: f32) -> TouchSample {
        s(99, TouchKind::Pen, phase, x, y)
    }
    fn run(t: &mut TouchInput, samples: &[TouchSample], now: f64) -> Step {
        t.step(samples, now, true, &|p| p.x < 500.0, true, Modifiers::NONE)
    }
    fn presses(events: &[Event], which: PointerButton) -> (usize, usize) {
        let count = |down| {
            events.iter().filter(|e| matches!(e, Event::PointerButton { button, pressed, .. } if *button == which && *pressed == down)).count()
        };
        (count(true), count(false))
    }

    #[test]
    fn a_finger_tap_on_the_model_clicks_where_it_touched() {
        let mut t = TouchInput::default();
        let down = run(&mut t, &[f(1, TouchPhase::Down, 100.0, 100.0)], 0.0);
        assert!(down.events.is_empty(), "nothing until it is a tap or a drag");
        let up = run(&mut t, &[f(1, TouchPhase::Move, 103.0, 102.0), f(1, TouchPhase::Up, 103.0, 102.0)], 0.1);
        assert_eq!(presses(&up.events, PointerButton::Primary), (1, 1));
        assert!(up.events.iter().any(|e| matches!(e, Event::PointerButton { pos, .. } if *pos == pos2(100.0, 100.0))));
        assert!(!up.camera.moves());
        assert!(!t.busy());
    }

    #[test]
    fn one_finger_drag_on_the_model_turns_the_view_without_clicking() {
        let mut t = TouchInput::default();
        run(&mut t, &[f(1, TouchPhase::Down, 100.0, 100.0)], 0.0);
        let a = run(&mut t, &[f(1, TouchPhase::Move, 130.0, 100.0)], 0.05);
        assert_eq!(a.camera.orbit, vec2(30.0, 0.0), "the travel within the slop is not lost");
        assert_eq!(a.camera.orbit_from, Some(pos2(100.0, 100.0)));
        assert!(a.camera.active);
        let b = run(&mut t, &[f(1, TouchPhase::Move, 140.0, 95.0)], 0.1);
        assert_eq!(b.camera.orbit, vec2(10.0, -5.0));
        let c = run(&mut t, &[f(1, TouchPhase::Up, 140.0, 95.0)], 0.2);
        assert!(a.events.is_empty() && b.events.is_empty() && c.events.is_empty(), "a drag never reaches egui as the mouse");
        assert!(!c.camera.active);
    }

    #[test]
    fn one_finger_drag_pans_a_flat_page() {
        let mut t = TouchInput::default();
        t.step(&[f(1, TouchPhase::Down, 100.0, 100.0)], 0.0, true, &|_| true, false, Modifiers::NONE);
        let a = t.step(&[f(1, TouchPhase::Move, 100.0, 140.0)], 0.1, true, &|_| true, false, Modifiers::NONE);
        assert_eq!(a.camera.pan, vec2(0.0, 40.0));
        assert_eq!(a.camera.orbit, Vec2::ZERO);
    }

    #[test]
    fn two_fingers_pan_and_pinch() {
        let mut t = TouchInput::default();
        run(&mut t, &[f(1, TouchPhase::Down, 100.0, 100.0), f(2, TouchPhase::Down, 200.0, 100.0)], 0.0);
        // Both move right by 10: a pan.
        let a = run(&mut t, &[f(1, TouchPhase::Move, 110.0, 100.0), f(2, TouchPhase::Move, 210.0, 100.0)], 0.05);
        assert!((a.camera.pan - vec2(10.0, 0.0)).length() < 1e-4, "{:?}", a.camera.pan);
        assert!((a.camera.zoom - 1.0).abs() < 1e-5);
        // Spread to twice the distance about the same centre: zoom in by 2.
        let b = run(&mut t, &[f(1, TouchPhase::Move, 60.0, 100.0), f(2, TouchPhase::Move, 260.0, 100.0)], 0.1);
        assert!((b.camera.zoom - 2.0).abs() < 1e-4, "{}", b.camera.zoom);
        assert_eq!(b.camera.centre, Some(pos2(160.0, 100.0)));
        assert!(a.events.is_empty() && b.events.is_empty());
        // Lifting one keeps panning with the other, without a jump or a click.
        let c = run(&mut t, &[f(1, TouchPhase::Up, 60.0, 100.0)], 0.15);
        assert_eq!(c.camera.pan, Vec2::ZERO);
        let d = run(&mut t, &[f(2, TouchPhase::Move, 270.0, 100.0)], 0.2);
        assert_eq!(d.camera.pan, vec2(10.0, 0.0));
        let e = run(&mut t, &[f(2, TouchPhase::Up, 270.0, 100.0)], 0.25);
        assert!(c.events.is_empty() && d.events.is_empty() && e.events.is_empty());
        assert!(!t.busy());
    }

    #[test]
    fn a_finger_held_still_opens_the_context_menu_once() {
        let mut t = TouchInput::default();
        let a = run(&mut t, &[f(1, TouchPhase::Down, 100.0, 100.0)], 0.0);
        assert_eq!(a.wake_in, Some(LONG_PRESS));
        let b = run(&mut t, &[], LONG_PRESS + 0.01);
        assert_eq!(presses(&b.events, PointerButton::Secondary), (1, 1));
        let c = run(&mut t, &[f(1, TouchPhase::Up, 100.0, 100.0)], 1.0);
        assert!(c.events.is_empty(), "the lift after a hold is not a tap");
    }

    #[test]
    fn a_finger_off_the_model_is_the_mouse() {
        let mut t = TouchInput::default();
        let a = run(&mut t, &[f(1, TouchPhase::Down, 600.0, 100.0)], 0.0);
        assert_eq!(presses(&a.events, PointerButton::Primary), (1, 0));
        let b = run(&mut t, &[f(1, TouchPhase::Move, 600.0, 160.0)], 0.1);
        assert_eq!(b.events, vec![Event::PointerMoved(pos2(600.0, 160.0))], "a list scrolls, a slider drags");
        assert!(!b.camera.moves());
        let c = run(&mut t, &[f(1, TouchPhase::Up, 600.0, 160.0)], 0.2);
        assert_eq!(presses(&c.events, PointerButton::Primary), (0, 1));
    }

    #[test]
    fn with_gestures_off_a_finger_is_the_mouse_on_the_model_too() {
        let mut t = TouchInput::default();
        let a = t.step(&[f(1, TouchPhase::Down, 100.0, 100.0), f(1, TouchPhase::Move, 150.0, 100.0)], 0.0, false, &|_| true, true, Modifiers::NONE);
        assert_eq!(presses(&a.events, PointerButton::Primary), (1, 0));
        assert!(!a.camera.moves());
    }

    #[test]
    fn the_pen_is_the_left_button_everywhere_and_a_palm_does_nothing() {
        let mut t = TouchInput::default();
        let a = run(&mut t, &[pen(TouchPhase::Down, 100.0, 100.0)], 0.0);
        assert_eq!(presses(&a.events, PointerButton::Primary), (1, 0), "on the model the pen presses at once: it draws and picks");
        // A palm lands and moves while drawing.
        let b = run(&mut t, &[f(1, TouchPhase::Down, 300.0, 300.0), pen(TouchPhase::Move, 120.0, 100.0), f(1, TouchPhase::Move, 360.0, 300.0)], 0.1);
        assert_eq!(b.events, vec![Event::PointerMoved(pos2(120.0, 100.0))]);
        assert!(!b.camera.moves());
        let c = run(&mut t, &[pen(TouchPhase::Up, 120.0, 100.0)], 0.2);
        assert_eq!(presses(&c.events, PointerButton::Primary), (0, 1));
        // The palm lifts later: still nothing, and no long press either.
        let d = run(&mut t, &[f(1, TouchPhase::Up, 360.0, 300.0)], 2.0);
        assert!(d.events.is_empty());
        assert!(!t.busy());
    }

    #[test]
    fn the_pen_takes_over_from_a_finger_holding_the_mouse() {
        let mut t = TouchInput::default();
        run(&mut t, &[f(1, TouchPhase::Down, 600.0, 100.0)], 0.0);
        let a = run(&mut t, &[pen(TouchPhase::Down, 100.0, 100.0)], 0.1);
        assert_eq!(presses(&a.events, PointerButton::Primary), (1, 1), "the finger lets go before the pen presses");
        assert!(matches!(a.events.last(), Some(Event::PointerButton { pressed: true, .. })));
    }

    #[test]
    fn hostile_samples_never_panic() {
        let mut t = TouchInput::default();
        let odd = [
            f(1, TouchPhase::Up, 0.0, 0.0),
            f(1, TouchPhase::Move, f32::NAN, 1.0),
            f(1, TouchPhase::Down, f32::INFINITY, 0.0),
            pen(TouchPhase::Move, 1.0, 1.0),
            pen(TouchPhase::Up, 1.0, 1.0),
            f(2, TouchPhase::Down, 1.0, 1.0),
            f(2, TouchPhase::Down, 1.0, 1.0),
            f(3, TouchPhase::Down, 1.0, 1.0),
            f(3, TouchPhase::Move, 1.0, 1.0),
            f(2, TouchPhase::Cancel, 1.0, 1.0),
            f(3, TouchPhase::Cancel, 1.0, 1.0),
        ];
        let step = run(&mut t, &odd, f64::NAN);
        assert!(step.camera.zoom.is_finite() && step.camera.pan.is_finite());
        assert!(!t.busy());
    }
}
