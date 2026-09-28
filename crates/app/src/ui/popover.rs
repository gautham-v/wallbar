//! The popover: a wall label for the wallpaper.
//!
//! Top to bottom, as in the approved mockup: the painting cropped wide, a
//! row of previous / Shuffle / next with the pool position on the right, the
//! title and artist, Year and Where, a few sentences about it, and a footer
//! with the Match appearance box and a link out.
//!
//! Stepping is also ←/→ while the popover has focus, and a sideways swipe
//! over the picture (one swipe, one step; see [`StepGate`]).
//!
//! The height follows the content — titles and notes wrap to different
//! lengths — so the root measures its children after layout and asks the
//! window to fit them.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use gpui::prelude::FluentBuilder;
use gpui::{
    actions, div, img, px, size, svg, App, Context, Div, EventEmitter, FocusHandle, Focusable,
    HighlightStyle, InteractiveElement, IntoElement, KeyBinding, ObjectFit, ParentElement, Render,
    ScrollDelta, ScrollWheelEvent, SharedString, StatefulInteractiveElement, Styled, StyledImage,
    StyledText, TouchPhase, Window,
};
use wallbar_core::gesture::{Phase, Step, StepGate};
use wallbar_core::Painting;

use crate::controller::{Controller, Nav, Snapshot};
use crate::ui::icons;
use crate::ui::theme::{self, Theme};

pub enum PopoverEvent {
    /// Esc: close the window.
    Close,
}

actions!(
    wallbar,
    [NextWallpaper, PrevWallpaper, ShuffleWallpaper, Dismiss]
);

pub const KEY_CONTEXT: &str = "Wallbar";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Dismiss, Some(KEY_CONTEXT)),
        KeyBinding::new("right", NextWallpaper, Some(KEY_CONTEXT)),
        KeyBinding::new("left", PrevWallpaper, Some(KEY_CONTEXT)),
        KeyBinding::new("s", ShuffleWallpaper, Some(KEY_CONTEXT)),
    ]);
}

/// A first guess at the height, used until the first layout has measured it.
pub const INITIAL_HEIGHT: f32 = 460.0;

pub struct Popover {
    focus: FocusHandle,
    controller: Rc<Controller>,
    snapshot: Snapshot,
    /// The header thumbnail for the current painting, once it exists.
    thumb: Option<PathBuf>,
    /// The painting a thumbnail is being made for.
    thumb_pending: Option<PathBuf>,
    /// The last thing that went wrong, as an 11px caption.
    notice: Option<SharedString>,
    swipe: StepGate,
    theme: Theme,
    appearance: Option<gpui::Subscription>,
    /// The content height the last layout measured.
    pub height: Rc<Cell<f32>>,
    /// The most the window may grow to on this display; set on open.
    pub max_height: Rc<Cell<f32>>,
}

impl Popover {
    pub fn new(controller: Rc<Controller>, cx: &mut Context<Self>) -> Self {
        let snapshot = controller.snapshot();
        let mut this = Self {
            focus: cx.focus_handle(),
            controller,
            snapshot,
            thumb: None,
            thumb_pending: None,
            notice: None,
            swipe: StepGate::new(),
            theme: Theme::default(),
            appearance: None,
            height: Rc::new(Cell::new(INITIAL_HEIGHT)),
            max_height: Rc::new(Cell::new(f32::MAX)),
        };
        this.load_thumb(cx);
        this
    }

    /// Called on every open: clear the notice and re-read everything.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        self.reload(cx);
    }

    /// Re-read the desktop, the folder and the state, and redraw if any of
    /// it changed — how changes made from the CLI or Raycast show up.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.controller.snapshot();
        if snapshot != self.snapshot {
            self.snapshot = snapshot;
            self.load_thumb(cx);
            cx.notify();
        }
    }

    pub fn step(&mut self, nav: Nav, cx: &mut Context<Self>) {
        self.notice = match self.controller.step(nav) {
            Ok(_) => None,
            Err(err) => Some(format!("{err:#}").into()),
        };
        self.snapshot = self.controller.snapshot();
        self.load_thumb(cx);
        cx.notify();
    }

    fn toggle_match(&mut self, cx: &mut Context<Self>) {
        let on = !self.snapshot.match_appearance;
        if let Err(err) = self.controller.set_match_appearance(on) {
            self.notice = Some(format!("{err:#}").into());
        }
        self.snapshot = self.controller.snapshot();
        cx.notify();
    }

    fn open_link(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = &self.snapshot.painting {
            cx.open_url(&p.link_url());
        }
    }

    /// Point the header at the current painting's thumbnail, making it in
    /// the background if it is not cached yet.
    fn load_thumb(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self
            .snapshot
            .painting
            .as_ref()
            .map(|p| PathBuf::from(&p.path))
        else {
            self.thumb = None;
            return;
        };
        if let Some(cached) = self.controller.thumbs.cached(&source) {
            self.thumb = Some(cached);
            return;
        }
        self.thumb = None;
        if self.thumb_pending.as_ref() == Some(&source) {
            return;
        }
        self.thumb_pending = Some(source.clone());
        let thumbs = self.controller.thumbs.clone();
        cx.spawn(async move |this, cx| {
            let made = {
                let source = source.clone();
                cx.background_executor()
                    .spawn(async move { thumbs.ensure(&source) })
                    .await
            };
            let _ = this.update(cx, |this, cx| {
                if this.thumb_pending.as_ref() == Some(&source) {
                    this.thumb_pending = None;
                }
                let showing = this
                    .snapshot
                    .painting
                    .as_ref()
                    .map(|p| PathBuf::from(&p.path));
                if showing.as_ref() == Some(&source) {
                    this.thumb = made.ok();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn on_header_scroll(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let (dx, dy, precise) = match event.delta {
            ScrollDelta::Pixels(p) => (f32::from(p.x), f32::from(p.y), true),
            ScrollDelta::Lines(p) => (p.x, p.y, false),
        };
        // Sideways only: a vertical scroll over the picture is not a swipe.
        if dx.abs() <= dy.abs() {
            return;
        }
        let phase = if matches!(event.touch_phase, TouchPhase::Started) {
            Phase::Began
        } else {
            Phase::Changed
        };
        match self.swipe.feed(dx, phase, precise, Instant::now()) {
            Some(Step::Next) => self.step(Nav::Next, cx),
            Some(Step::Prev) => self.step(Nav::Prev, cx),
            None => {}
        }
    }

    // ── Layout ──────────────────────────────────────────────────────────────

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        // The popover's own corner, less its 1px rim.
        let radius = px(theme::POPOVER_RADIUS_PX - 1.0);
        div()
            .id("header")
            .flex_none()
            .w_full()
            .h(theme::HEADER_HEIGHT)
            .rounded_t(radius)
            // Cover scales the image past the header's edges and gpui
            // paints all of it, so the header has to do the cropping.
            .overflow_hidden()
            .bg(theme.placeholder)
            .on_scroll_wheel(
                cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                    this.on_header_scroll(event, cx)
                }),
            )
            .children(self.thumb.clone().map(|path| {
                // Both edges absolute: with a relative width gpui lets the
                // image's own aspect ratio pick the height.
                img(path)
                    .w(px(theme::POPOVER_WIDTH_PX - 2.0))
                    .h(theme::HEADER_HEIGHT)
                    .rounded_t(radius)
                    .object_fit(ObjectFit::Cover)
            }))
    }

    fn nav_button(&self, id: &'static str, enabled: bool) -> gpui::Stateful<Div> {
        let theme = self.theme;
        div()
            .id(id)
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(5.))
            .h(theme::BUTTON_HEIGHT)
            .min_w(theme::BUTTON_MIN_WIDTH)
            .px(px(8.))
            .rounded(theme::BUTTON_RADIUS)
            .bg(theme.button)
            .text_color(if enabled { theme.text } else { theme.tertiary })
            .when(enabled, |el| el.hover(|s| s.bg(theme.button_hover)))
    }

    fn nav(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let painting = self.snapshot.painting.as_ref();
        let count = painting.map_or(0, |p| p.count);
        let enabled = count > 0;
        let position: SharedString = match painting {
            Some(p) if p.count > 0 && p.index > 0 => format!("{} of {}", p.index, p.count).into(),
            Some(p) if p.count > 0 => format!("\u{2013} of {}", p.count).into(),
            _ => SharedString::default(),
        };
        let icon = |path: &'static str, size: gpui::Pixels| {
            svg()
                .path(path)
                .size(size)
                .flex_none()
                .text_color(if enabled { theme.text } else { theme.tertiary })
        };
        div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .px(theme::PAD_X)
            .pt(px(10.))
            .child(
                self.nav_button("prev", enabled)
                    .child(icon(icons::CHEVRON_LEFT, theme::ICON))
                    .when(enabled, |el| {
                        el.on_click(cx.listener(|this, _, _, cx| this.step(Nav::Prev, cx)))
                    }),
            )
            .child(
                self.nav_button("shuffle", enabled)
                    .child(icon(icons::SHUFFLE, theme::SHUFFLE_ICON))
                    .child("Shuffle")
                    .when(enabled, |el| {
                        el.on_click(cx.listener(|this, _, _, cx| this.step(Nav::Shuffle, cx)))
                    }),
            )
            .child(
                self.nav_button("next", enabled)
                    .child(icon(icons::CHEVRON_RIGHT, theme::ICON))
                    .when(enabled, |el| {
                        el.on_click(cx.listener(|this, _, _, cx| this.step(Nav::Next, cx)))
                    }),
            )
            .child(div().flex_1())
            .child(div().text_color(theme.secondary).child(position))
    }

    fn notice_line(&self) -> Option<impl IntoElement> {
        let message = self
            .notice
            .clone()
            .or_else(|| self.snapshot.problem.clone().map(Into::into))?;
        Some(
            div()
                .flex_none()
                .px(theme::PAD_X)
                .pt(px(6.))
                .text_size(theme::CAPTION)
                .line_height(theme::LINE_CAPTION)
                .text_color(self.theme.secondary)
                .child(message),
        )
    }

    /// "Artist · Nationality, dates" as one paragraph, so it wraps as one.
    fn artist_line(&self, p: &Painting) -> Option<impl IntoElement> {
        if p.artist.is_empty() {
            return None;
        }
        let (text, grey) = match &p.life {
            Some(life) => {
                let text = format!("{} \u{00b7} {}", p.artist, life);
                let start = p.artist.len();
                (text.clone(), Some(start..text.len()))
            }
            None => (p.artist.clone(), None),
        };
        let highlights: Vec<_> = grey
            .into_iter()
            .map(|range| {
                (
                    range,
                    // A highlight colour is blended over the ink, which
                    // leaves grey-over-ink looking like ink; fading the ink
                    // to the secondary alpha gives the real grey.
                    HighlightStyle {
                        fade_out: Some(1.0 - self.theme.secondary.a / self.theme.text.a),
                        ..Default::default()
                    },
                )
            })
            .collect();
        Some(
            div()
                .pt(px(1.))
                .child(StyledText::new(text).with_highlights(highlights)),
        )
    }

    fn fact(&self, label: &'static str, value: &Option<String>) -> Option<impl IntoElement> {
        let value = value.clone()?;
        Some(
            div()
                .flex()
                .flex_row()
                .child(
                    div()
                        .flex_none()
                        .w(theme::LABEL_WIDTH)
                        .text_color(self.theme.secondary)
                        .child(label),
                )
                .child(div().flex_1().child(value)),
        )
    }

    fn body(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let painting = self.snapshot.painting.as_ref();
        let title: SharedString = painting
            .map(|p| p.title.clone())
            .unwrap_or_else(|| "No wallpaper".to_owned())
            .into();
        let facts: Vec<gpui::AnyElement> = painting
            .map(|p| {
                [self.fact("Year", &p.year), self.fact("Where", &p.where_)]
                    .into_iter()
                    .flatten()
                    .map(IntoElement::into_any_element)
                    .collect()
            })
            .unwrap_or_default();
        let about = painting.and_then(|p| p.about.clone());
        let link_label = painting.map(|p| p.link_label());

        div()
            .flex_none()
            .flex()
            .flex_col()
            .px(theme::PAD_X)
            .pt(px(10.))
            .pb(px(10.))
            .child(div().font_weight(theme::WEIGHT_EMPHASIS).child(title))
            .children(painting.and_then(|p| self.artist_line(p)))
            .when(!facts.is_empty(), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .pt(px(8.))
                        .children(facts),
                )
            })
            .children(
                about.map(|about| div().pt(px(8.)).line_height(theme::LINE_PROSE).child(about)),
            )
            .child(div().mt(px(10.)).h(theme::HAIRLINE).bg(theme.separator))
            .child(self.footer(link_label, cx))
    }

    fn footer(&self, link_label: Option<&'static str>, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let on = self.snapshot.match_appearance;
        let checkbox = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(theme::CHECKBOX)
            .rounded(theme::CHECKBOX_RADIUS)
            .map(|el| {
                if on {
                    el.bg(theme.text).child(
                        svg()
                            .path(icons::CHECK)
                            .size(theme::CHECK_ICON)
                            .text_color(theme.on_ink),
                    )
                } else {
                    el.border_1().border_color(theme.tertiary)
                }
            });
        div()
            .flex()
            .flex_row()
            .items_center()
            .pt(px(8.))
            .child(
                div()
                    .id("match")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(checkbox)
                    .child("Match appearance")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_match(cx))),
            )
            .child(div().flex_1())
            .children(link_label.map(|label| {
                div()
                    .id("link")
                    .child(label)
                    .hover(|s| s.underline())
                    .on_click(cx.listener(|this, _, _, cx| this.open_link(cx)))
            }))
    }
}

impl EventEmitter<PopoverEvent> for Popover {}

impl Focusable for Popover {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Popover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.appearance.is_none() {
            let this = cx.entity();
            self.appearance = Some(window.observe_window_appearance(move |_window, cx| {
                this.update(cx, |this, cx| {
                    // Appearance picks the pool, so re-read as well as repaint.
                    this.reload(cx);
                    cx.notify();
                });
            }));
        }
        self.theme = Theme::for_appearance(window.appearance());
        let theme = self.theme;

        // Fit the window to the content once it is laid out. The resize is
        // deferred: changing the window's size mid-frame is not allowed.
        let handle = window.window_handle();
        let measured = self.height.clone();
        let max_height = self.max_height.clone();
        let fit =
            move |bounds: Vec<gpui::Bounds<gpui::Pixels>>, window: &mut Window, cx: &mut App| {
                let Some(last) = bounds.last() else {
                    return;
                };
                // The last child's bottom edge, plus the rim under it.
                let wanted = (f32::from(last.origin.y + last.size.height) + 1.0).ceil();
                let wanted = wanted.min(max_height.get());
                measured.set(wanted);
                let have = f32::from(window.viewport_size().height);
                if (wanted - have).abs() > 0.5 {
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, _| {
                            window.resize(size(theme::POPOVER_WIDTH, px(wanted)))
                        });
                    });
                }
            };

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &NextWallpaper, _, cx| this.step(Nav::Next, cx)))
            .on_action(cx.listener(|this, _: &PrevWallpaper, _, cx| this.step(Nav::Prev, cx)))
            .on_action(cx.listener(|this, _: &ShuffleWallpaper, _, cx| this.step(Nav::Shuffle, cx)))
            .on_action(cx.listener(|_, _: &Dismiss, _, cx| cx.emit(PopoverEvent::Close)))
            .on_children_prepainted(fit)
            .flex()
            .flex_col()
            .w(theme::POPOVER_WIDTH)
            .h_full()
            .bg(theme.bg)
            .rounded(theme::POPOVER_RADIUS)
            .border_1()
            .border_color(theme.border)
            .overflow_hidden()
            .font_family(theme::UI_FAMILY)
            .text_size(theme::TEXT)
            .line_height(theme::LINE)
            .text_color(theme.text)
            .child(self.header(cx))
            .child(self.nav(cx))
            .children(self.notice_line())
            .child(self.body(cx))
    }
}
