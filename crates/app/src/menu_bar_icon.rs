//! The menu bar glyph: a thin picture frame with a line of hills in it.
//!
//! Drawn with Core Graphics into a template image, so AppKit keeps only the
//! alpha and tints it for the menu bar (light, dark, reduced transparency).
//! Everything is therefore drawn in opaque black on clear. The mark is the
//! mockup's 16-unit frame icon, scaled into the glyph box.

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_app_kit::{NSGraphicsContext, NSImage};
use objc2_core_graphics::{CGContext, CGLineCap, CGLineJoin};
use objc2_foundation::{NSRect, NSSize};

/// Glyph box, in points: the optical size of the system glyphs (Wi-Fi,
/// battery), so the item stays as narrow as they are.
pub const WIDTH: f64 = 16.0;
pub const HEIGHT: f64 = 15.0;
/// Stroke width, a touch thinner than the system glyphs' so the frame reads
/// as a frame and not a box.
const STROKE: f64 = 1.1;
const RADIUS: f64 = 1.6;

/// The template image for the status item button.
pub fn frame_image() -> Retained<NSImage> {
    let handler = RcBlock::new(|_dirty: NSRect| -> Bool {
        draw();
        Bool::YES
    });
    let image =
        NSImage::imageWithSize_flipped_drawingHandler(NSSize::new(WIDTH, HEIGHT), false, &handler);
    image.setTemplate(true);
    image
}

fn draw() {
    let Some(ctx) = NSGraphicsContext::currentContext() else {
        return;
    };
    let cg = ctx.CGContext();
    let cg = Some(&*cg);
    CGContext::set_should_antialias(cg, true);
    CGContext::set_line_width(cg, STROKE);
    CGContext::set_line_cap(cg, CGLineCap::Round);
    CGContext::set_line_join(cg, CGLineJoin::Round);
    CGContext::set_gray_stroke_color(cg, 0.0, 1.0);

    // The frame: the full box less half a stroke, so the outline lands inside.
    let h = STROKE / 2.0;
    let (left, right, bottom, top) = (h + 0.5, WIDTH - h - 0.5, h + 1.0, HEIGHT - h - 1.0);
    CGContext::begin_path(cg);
    CGContext::move_to_point(cg, left + RADIUS, bottom);
    CGContext::add_arc_to_point(cg, right, bottom, right, top, RADIUS);
    CGContext::add_arc_to_point(cg, right, top, left, top, RADIUS);
    CGContext::add_arc_to_point(cg, left, top, left, bottom, RADIUS);
    CGContext::add_arc_to_point(cg, left, bottom, right, bottom, RADIUS);
    CGContext::close_path(cg);
    CGContext::stroke_path(cg);

    // The hills, in the frame's own coordinates (y up from its bottom edge).
    let w = right - left;
    let hgt = top - bottom;
    let p = |fx: f64, fy: f64| (left + w * fx, bottom + hgt * fy);
    let points = [
        p(0.06, 0.22),
        p(0.33, 0.55),
        p(0.54, 0.33),
        p(0.70, 0.50),
        p(0.94, 0.24),
    ];
    CGContext::begin_path(cg);
    CGContext::move_to_point(cg, points[0].0, points[0].1);
    for (x, y) in &points[1..] {
        CGContext::add_line_to_point(cg, *x, *y);
    }
    CGContext::stroke_path(cg);
}
