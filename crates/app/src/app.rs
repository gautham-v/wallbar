//! The menu bar app's plumbing: the accessory activation policy, the status
//! item, and the popover window — anchored under the item, toggled by a
//! click, closed on Esc, an outside click or focus loss, and kept the height
//! of its content. The view is [`crate::ui::popover`]; what it does is
//! [`crate::controller`].

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use gpui::{
    point, px, size, App, AppContext, Application, Bounds, Entity, Focusable, Pixels, Subscription,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
};
use objc2::MainThreadMarker;
use wallbar_core::gesture::Step;

use crate::controller::{Controller, Nav};
use crate::status_item::{self, Anchor, StatusItem, StatusItemEvent};
use crate::ui::icons::Assets;
use crate::ui::popover::{self, Popover, PopoverEvent};
use crate::ui::theme;

/// Keep the popover this far from the screen edges.
const SCREEN_MARGIN: f32 = 8.0;
/// A status-item click this soon after the popover closed itself on focus
/// loss is the click that closed it; don't reopen.
const TOGGLE_GRACE: Duration = Duration::from_millis(250);
/// While the popover is open, re-read the desktop this often so changes from
/// the CLI or Raycast show up.
const POLL_EVERY: Duration = Duration::from_secs(2);

type WindowSlot = Rc<RefCell<Option<WindowHandle<Popover>>>>;

pub fn run() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        let mtm = MainThreadMarker::new().expect("gpui runs its callbacks on the main thread");
        status_item::set_accessory_activation_policy(mtm);
        popover::bind_keys(cx);
        eprintln!("wallbar: {}", crate::login::register_once());

        let controller = Rc::new(Controller::new(mtm));
        warm_thumbnails(&controller);

        let (item, mut events) = StatusItem::new(mtm);
        let item = Rc::new(item);

        let popover = cx.new({
            let controller = controller.clone();
            |cx| Popover::new(controller, cx)
        });

        let window: WindowSlot = Rc::new(RefCell::new(None));
        let closed_at: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
        let activation: Rc<RefCell<Option<Subscription>>> = Rc::new(RefCell::new(None));

        cx.subscribe(&popover, {
            let window = window.clone();
            move |_popover, event, cx| match event {
                PopoverEvent::Close => {
                    close_popover(&window, cx);
                }
            }
        })
        .detach();

        // Poll while open.
        cx.spawn({
            let window = window.clone();
            let popover = popover.clone();
            async move |cx| loop {
                cx.background_executor().timer(POLL_EVERY).await;
                let alive = cx.update(|cx| {
                    if window.borrow().is_some() {
                        popover.update(cx, |this, cx| this.reload(cx));
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();

        cx.spawn({
            let item = item.clone();
            let popover = popover.clone();
            async move |cx| {
                while let Some(event) = events.next().await {
                    let result = cx.update(|cx| match event {
                        StatusItemEvent::ClickedOutside => {
                            close_popover(&window, cx);
                        }
                        StatusItemEvent::Scrolled(step) => {
                            let nav = match step {
                                Step::Next => Nav::Next,
                                Step::Prev => Nav::Prev,
                            };
                            popover.update(cx, |this, cx| this.step(nav, cx));
                        }
                        StatusItemEvent::Clicked => {
                            if close_popover(&window, cx) {
                                closed_at.set(None);
                                return;
                            }
                            if closed_at.take().is_some_and(|t| t.elapsed() < TOGGLE_GRACE) {
                                return;
                            }
                            let placed = placement(item.anchor(mtm), primary_bounds(cx));
                            popover.update(cx, |this, cx| {
                                this.max_height.set(placed.max_height);
                                this.reset(cx);
                            });
                            match open_popover(
                                cx,
                                placed,
                                popover.clone(),
                                &activation,
                                &window,
                                &closed_at,
                            ) {
                                Ok(handle) => *window.borrow_mut() = Some(handle),
                                Err(err) => eprintln!("wallbar: could not open popover: {err}"),
                            }
                        }
                    });
                    if result.is_err() {
                        break;
                    }
                }
            }
        })
        .detach();

        // Held for the life of the process; dropping it removes the item.
        std::mem::forget(item);
    });
}

/// Make every thumbnail in the background so the first open of each
/// painting is already fast.
fn warm_thumbnails(controller: &Controller) {
    let thumbs = controller.thumbs.clone();
    let images = controller.all_images();
    std::thread::spawn(move || {
        for image in images {
            let _ = thumbs.ensure(&image);
        }
    });
}

fn close_popover(window: &WindowSlot, cx: &mut App) -> bool {
    let handle = window.borrow_mut().take();
    match handle {
        Some(handle) => handle.update(cx, |_, w, _| w.remove_window()).is_ok(),
        None => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Placement {
    x: f32,
    top: f32,
    max_height: f32,
}

fn open_popover(
    cx: &mut App,
    placed: Placement,
    popover: Entity<Popover>,
    activation: &Rc<RefCell<Option<Subscription>>>,
    window_slot: &WindowSlot,
    closed_at: &Rc<Cell<Option<Instant>>>,
) -> anyhow::Result<WindowHandle<Popover>> {
    let height = popover.read(cx).height.get().min(placed.max_height);
    let bounds = Bounds {
        origin: point(px(placed.x), px(placed.top)),
        size: size(theme::POPOVER_WIDTH, px(height)),
    };
    let activation = activation.clone();
    let window_slot = window_slot.clone();
    let closed_at = closed_at.clone();
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: true,
            show: true,
            // A borderless, non-activating NSPanel: the menu-bar popover.
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Blurred,
            window_min_size: None,
            display_id: None,
            app_id: None,
            window_decorations: None,
            tabbing_identifier: None,
        },
        |window, cx| {
            let subscription = popover.update(cx, |_, cx| {
                // The observer fires once at registration, before the panel
                // is key; only a deactivation after it was active dismisses.
                let was_active = Cell::new(false);
                cx.observe_window_activation(window, move |_, window, _| {
                    if window.is_window_active() {
                        was_active.set(true);
                    } else if was_active.get() {
                        *window_slot.borrow_mut() = None;
                        closed_at.set(Some(Instant::now()));
                        window.remove_window();
                    }
                })
            });
            *activation.borrow_mut() = Some(subscription);
            window.focus(&popover.focus_handle(cx));
            popover.clone()
        },
    )?;
    // The panel is non-activating, so ask AppKit to make it key.
    handle.update(cx, |_, window, _| window.activate_window())?;
    Ok(handle)
}

fn primary_bounds(cx: &App) -> Bounds<Pixels> {
    cx.primary_display().map(|d| d.bounds()).unwrap_or(Bounds {
        origin: point(px(0.), px(0.)),
        size: size(px(1440.), px(900.)),
    })
}

/// Centred under the item, top edge under the menu bar, clamped to the
/// display the item is on.
fn placement(anchor: Option<Anchor>, primary: Bounds<Pixels>) -> Placement {
    let width = theme::POPOVER_WIDTH_PX;
    let gap: f32 = theme::POPOVER_TOP_GAP.into();
    let screen = anchor.map(|a| a.screen).unwrap_or(status_item::ScreenRect {
        x: primary.origin.x.into(),
        y: primary.origin.y.into(),
        width: primary.size.width.into(),
        height: primary.size.height.into(),
    });
    let left = screen.x + SCREEN_MARGIN;
    let right = (screen.x + screen.width - width - SCREEN_MARGIN).max(left);
    let (x, top) = match anchor {
        Some(a) => (
            (a.item.x + a.item.width / 2.0 - width / 2.0).clamp(left, right),
            a.item.y + a.item.height + gap,
        ),
        None => (right, screen.y + 28.0 + gap),
    };
    let max_height = (screen.y + screen.height - top - SCREEN_MARGIN).max(200.0);
    Placement { x, top, max_height }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status_item::ScreenRect;

    fn primary() -> Bounds<Pixels> {
        Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(1440.), px(900.)),
        }
    }

    fn anchor(x: f32) -> Option<Anchor> {
        Some(Anchor {
            item: ScreenRect {
                x,
                y: 0.0,
                width: 24.0,
                height: 24.0,
            },
            screen: ScreenRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
        })
    }

    #[test]
    fn it_centres_under_the_item_and_hangs_off_the_bar() {
        let p = placement(anchor(1000.0), primary());
        assert_eq!(p.x, 1012.0 - theme::POPOVER_WIDTH_PX / 2.0);
        assert_eq!(p.top, 24.0);
        assert_eq!(p.max_height, 900.0 - 24.0 - SCREEN_MARGIN);
    }

    #[test]
    fn it_stays_on_screen_at_the_right_edge() {
        let p = placement(anchor(1430.0), primary());
        assert_eq!(p.x, 1440.0 - theme::POPOVER_WIDTH_PX - SCREEN_MARGIN);
    }
}
