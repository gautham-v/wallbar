//! The Cocoa glue: activation policy, the `NSStatusItem`, and the bridge from
//! AppKit events back into gpui.
//!
//! Same shape as the sibling apps: the item's target/action is a tiny `objc2`
//! class that pushes a [`StatusItemEvent`] down an unbounded channel, and the
//! app drains it from a gpui task. Two event monitors ride along:
//!
//! - a global mouse-down monitor, because a non-activating panel gets no
//!   reliable "clicked outside" signal of its own;
//! - a local scroll-wheel monitor. The status item's window belongs to this
//!   process, so scrolls over the icon arrive here; the monitor picks out the
//!   ones aimed at that window and turns them into steps with a
//!   [`StepGate`], passing every event on untouched.

use std::cell::RefCell;
use std::ptr::NonNull;
use std::time::Instant;

use block2::RcBlock;
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSCellImagePosition, NSControl, NSEvent,
    NSEventMask, NSEventPhase, NSScreen, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSPoint, NSRect, NSString};
use wallbar_core::gesture::{Phase, Step, StepGate};

use crate::menu_bar_icon;

/// What the status item tells the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusItemEvent {
    Clicked,
    /// A click in another app: the popover should close.
    ClickedOutside,
    /// A scroll over the icon added up to one step.
    Scrolled(Step),
}

/// Where the menu bar item is, and the display it is on, in gpui's screen
/// coordinates (top-left origin, y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub item: ScreenRect,
    pub screen: ScreenRect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

struct TargetIvars {
    tx: UnboundedSender<StatusItemEvent>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "WallbarStatusItemTarget"]
    #[ivars = TargetIvars]
    struct StatusItemTarget;

    unsafe impl NSObjectProtocol for StatusItemTarget {}

    impl StatusItemTarget {
        #[unsafe(method(wallbarStatusItemClicked:))]
        fn clicked(&self, _sender: *mut AnyObject) {
            let _ = self.ivars().tx.unbounded_send(StatusItemEvent::Clicked);
        }
    }
);

impl StatusItemTarget {
    fn new(tx: UnboundedSender<StatusItemEvent>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(TargetIvars { tx });
        unsafe { msg_send![super(this), init] }
    }
}

/// Owns the menu bar item for the life of the app; dropping it removes it.
pub struct StatusItem {
    item: Retained<NSStatusItem>,
    _target: Retained<StatusItemTarget>,
    monitors: Vec<Retained<AnyObject>>,
}

impl StatusItem {
    pub fn new(mtm: MainThreadMarker) -> (Self, UnboundedReceiver<StatusItemEvent>) {
        let (tx, rx) = mpsc::unbounded();
        let target = StatusItemTarget::new(tx.clone());

        let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = item.button(mtm) {
            button.setImage(Some(&menu_bar_icon::frame_image()));
            button.setImagePosition(NSCellImagePosition::ImageOnly);
            button.setToolTip(Some(&NSString::from_str("Wallpaper")));
            let control: &NSControl = &button;
            unsafe {
                control.setTarget(Some(&*target));
                control.setAction(Some(sel!(wallbarStatusItemClicked:)));
            }
        }

        let mut monitors = Vec::new();
        monitors.extend(install_outside_click_monitor(tx.clone()));
        monitors.extend(install_scroll_monitor(mtm, item.clone(), tx));

        (
            Self {
                item,
                _target: target,
                monitors,
            },
            rx,
        )
    }

    /// Where the item is and which display it is on. AppKit's rects are
    /// bottom-left-origin; gpui wants top-left relative to the primary
    /// display, so both are flipped through the primary screen's height.
    pub fn anchor(&self, mtm: MainThreadMarker) -> Option<Anchor> {
        let button = self.item.button(mtm)?;
        let frame: NSRect = button.window()?.frame();
        let flip = primary_screen_height(mtm)?;
        let screen = screen_containing(mtm, frame)?;
        Some(Anchor {
            item: flipped(frame, flip),
            screen: flipped(screen, flip),
        })
    }
}

impl Drop for StatusItem {
    fn drop(&mut self) {
        for monitor in self.monitors.drain(..) {
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
    }
}

/// Run as a menu bar accessory: no Dock icon, no app menus.
pub fn set_accessory_activation_policy(mtm: MainThreadMarker) {
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
}

fn install_outside_click_monitor(
    tx: UnboundedSender<StatusItemEvent>,
) -> Option<Retained<AnyObject>> {
    let mask =
        NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown | NSEventMask::OtherMouseDown;
    let handler = RcBlock::new(move |_event: NonNull<NSEvent>| {
        let _ = tx.unbounded_send(StatusItemEvent::ClickedOutside);
    });
    let monitor = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &handler);
    if monitor.is_none() {
        eprintln!("wallbar: global mouse monitor unavailable; outside clicks will not dismiss");
    }
    monitor
}

/// Scrolls over the status item's own window become steps.
///
/// The raw deltas already follow the user's scroll-direction setting, and
/// negative means "towards what comes after" on either axis: scrolling down
/// the way a page scrolls down, or swiping left the way photos page. That is
/// the sign [`StepGate`] reads as next. Momentum events are dropped here, so
/// a flick does not keep stepping after the fingers lift.
fn install_scroll_monitor(
    mtm: MainThreadMarker,
    item: Retained<NSStatusItem>,
    tx: UnboundedSender<StatusItemEvent>,
) -> Option<Retained<AnyObject>> {
    let gate = RefCell::new(StepGate::new());
    let handler = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        let ev = unsafe { event.as_ref() };
        let ours = match (ev.window(mtm), item.button(mtm).and_then(|b| b.window())) {
            (Some(w), Some(bw)) => Retained::as_ptr(&w) == Retained::as_ptr(&bw),
            _ => false,
        };
        if ours && ev.momentumPhase() == NSEventPhase::None {
            let (dx, dy) = (ev.scrollingDeltaX() as f32, ev.scrollingDeltaY() as f32);
            let delta = if dx.abs() > dy.abs() { dx } else { dy };
            let phase = if ev.phase().contains(NSEventPhase::Began)
                || ev.phase().contains(NSEventPhase::MayBegin)
            {
                Phase::Began
            } else {
                Phase::Changed
            };
            let step = gate.borrow_mut().feed(
                delta,
                phase,
                ev.hasPreciseScrollingDeltas(),
                Instant::now(),
            );
            if let Some(step) = step {
                let _ = tx.unbounded_send(StatusItemEvent::Scrolled(step));
            }
        }
        event.as_ptr()
    });
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::ScrollWheel, &handler)
    };
    if monitor.is_none() {
        eprintln!("wallbar: scroll monitor unavailable; scrolling over the icon will not step");
    }
    monitor
}

fn flipped(frame: NSRect, flip_height: f64) -> ScreenRect {
    ScreenRect {
        x: frame.origin.x as f32,
        y: (flip_height - (frame.origin.y + frame.size.height)) as f32,
        width: frame.size.width as f32,
        height: frame.size.height as f32,
    }
}

fn screen_containing(mtm: MainThreadMarker, rect: NSRect) -> Option<NSRect> {
    let mid = NSPoint::new(
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    );
    let mut primary: Option<NSRect> = None;
    for screen in NSScreen::screens(mtm).iter() {
        let f = screen.frame();
        if primary.is_none() || f.origin == NSPoint::new(0.0, 0.0) {
            primary = Some(f);
        }
        if mid.x >= f.origin.x
            && mid.x <= f.origin.x + f.size.width
            && mid.y >= f.origin.y
            && mid.y <= f.origin.y + f.size.height
        {
            return Some(f);
        }
    }
    primary
}

fn primary_screen_height(mtm: MainThreadMarker) -> Option<f64> {
    let mut fallback = None;
    for screen in NSScreen::screens(mtm).iter() {
        let f = screen.frame();
        if fallback.is_none() {
            fallback = Some(f.size.height);
        }
        if f.origin == NSPoint::new(0.0, 0.0) {
            return Some(f.size.height);
        }
    }
    fallback
}
