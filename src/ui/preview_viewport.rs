use gpui::{Bounds, DevicePixels, ObjectFit, Pixels, Point, Size, point, px};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
};

impl super::Studio {
    pub(super) fn preview_zoom_slider(&self, cx: &gpui::Context<Self>) -> impl gpui::IntoElement {
        use gpui::{
            InteractiveElement, MouseButton, MouseDownEvent, ParentElement, Styled, canvas, div,
            rgb,
        };

        let fraction = (self.preview.viewport.transform.zoom - 1.) / 3.;
        let bounds = self.preview.viewport.slider_bounds.clone();
        div()
            .id("preview-zoom-slider")
            .relative()
            .flex_shrink_0()
            .w(px(112.))
            .h(px(22.))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    if let Some(image) = this.preview.display.image() {
                        this.preview
                            .viewport
                            .press_slider(event.position, image.size(0));
                        cx.notify();
                        cx.stop_propagation();
                    }
                }),
            )
            .child(
                canvas(move |track, _, _| bounds.set(Some(track)), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            )
            .child(
                div()
                    .absolute()
                    .left(px(6.))
                    .top(px(9.))
                    .w(px(100.))
                    .h(px(4.))
                    .rounded_full()
                    .bg(rgb(super::LINE))
                    .child(
                        div()
                            .w(px(100. * fraction))
                            .h_full()
                            .rounded_full()
                            .bg(rgb(super::ACCENT)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left(px(100. * fraction))
                    .top(px(5.))
                    .size(px(12.))
                    .rounded_full()
                    .bg(rgb(super::ACCENT)),
            )
    }
}

/// View-only navigation, independent of preview generation and export settings.
#[derive(Default)]
pub(super) struct PreviewViewport {
    pub bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    slider_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    source: Option<PathBuf>,
    pub transform: ViewTransform,
    pub previous_transform: ViewTransform,
    drag: Option<Drag>,
}

impl PreviewViewport {
    /// Follow the displayed frame, not the requested selection: loading keeps the old view intact.
    pub fn sync_source(&mut self, source: Option<&Path>) {
        if self.source.as_deref() != source {
            self.source = source.map(Path::to_owned);
            self.previous_transform = self.transform;
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        self.transform = ViewTransform::default();
        self.drag = None;
    }

    pub fn zoom(&mut self, position: Point<Pixels>, delta: f32, image: Size<DevicePixels>) {
        if let Some(bounds) = self.bounds.get().filter(|b| b.contains(&position)) {
            self.transform.zoom_at(bounds, image, position, delta);
            self.drag = None;
        }
    }

    pub fn press_slider(&mut self, position: Point<Pixels>, image: Size<DevicePixels>) {
        self.drag = Some(Drag::Zoom);
        self.slide(position, image);
    }

    fn slide(&mut self, position: Point<Pixels>, image: Size<DevicePixels>) {
        if let Some(track) = self.slider_bounds.get()
            && let Some(viewport) = self.bounds.get()
        {
            // Leave half a thumb at each end so its center reaches the scale endpoints.
            let width = f32::from(track.size.width) - 12.;
            if width > 0. {
                let fraction = ((f32::from(position.x - track.left()) - 6.) / width).clamp(0., 1.);
                self.transform
                    .set_zoom(viewport, image, viewport.center(), 1. + fraction * 3.);
            }
        }
    }

    pub fn press(&mut self, position: Point<Pixels>) {
        if self.transform.zoom > 1. && self.bounds.get().is_some_and(|b| b.contains(&position)) {
            self.drag = Some(Drag::Pan(position));
        }
    }

    pub fn release(&mut self) {
        self.drag = None;
    }

    pub fn move_pointer(
        &mut self,
        position: Point<Pixels>,
        dragging: bool,
        image: Size<DevicePixels>,
    ) -> bool {
        if !dragging {
            self.release();
            return false;
        }
        let Some(Drag::Pan(previous)) = self.drag else {
            if matches!(self.drag, Some(Drag::Zoom)) {
                self.slide(position, image);
                return true;
            }
            return false;
        };
        self.drag = Some(Drag::Pan(position));
        if let Some(bounds) = self.bounds.get() {
            self.transform.offset = self.transform.clamped_offset(bounds, image);
            self.transform.offset += position - previous;
            self.transform.offset = self.transform.clamped_offset(bounds, image);
        }
        true
    }
}

#[derive(Clone, Copy)]
enum Drag {
    Pan(Point<Pixels>),
    Zoom,
}

#[derive(Clone, Copy)]
pub(super) struct ViewTransform {
    pub zoom: f32,
    offset: Point<Pixels>,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            zoom: 1.,
            offset: point(px(0.), px(0.)),
        }
    }
}

impl ViewTransform {
    pub fn image_bounds(
        self,
        viewport: Bounds<Pixels>,
        image: Size<DevicePixels>,
    ) -> Option<Bounds<Pixels>> {
        if viewport.size.width <= px(0.) || viewport.size.height <= px(0.) {
            return None;
        }
        let fitted = ObjectFit::Contain.get_bounds(viewport, image);
        let size = gpui::size(
            fitted.size.width * self.zoom,
            fitted.size.height * self.zoom,
        );
        Some(Bounds::new(
            viewport.center() - point(size.width / 2., size.height / 2.)
                + self.clamped_offset(viewport, image),
            size,
        ))
    }

    fn clamped_offset(self, viewport: Bounds<Pixels>, image: Size<DevicePixels>) -> Point<Pixels> {
        let fitted = ObjectFit::Contain.get_bounds(viewport, image);
        let limit_x = ((fitted.size.width * self.zoom - viewport.size.width) / 2.).max(px(0.));
        let limit_y = ((fitted.size.height * self.zoom - viewport.size.height) / 2.).max(px(0.));
        point(
            self.offset.x.clamp(-limit_x, limit_x),
            self.offset.y.clamp(-limit_y, limit_y),
        )
    }

    fn zoom_at(
        &mut self,
        viewport: Bounds<Pixels>,
        image: Size<DevicePixels>,
        position: Point<Pixels>,
        delta: f32,
    ) {
        if !delta.is_finite() || viewport.size.width <= px(0.) || viewport.size.height <= px(0.) {
            return;
        }
        let zoom = (self.zoom * (delta * 0.0025).clamp(-4., 4.).exp()).clamp(1., 4.);
        self.set_zoom(viewport, image, position, zoom);
    }

    fn set_zoom(
        &mut self,
        viewport: Bounds<Pixels>,
        image: Size<DevicePixels>,
        position: Point<Pixels>,
        zoom: f32,
    ) {
        let ratio = zoom / self.zoom;
        let anchor = position - viewport.center();
        self.offset = anchor - (anchor - self.clamped_offset(viewport, image)) * ratio;
        self.zoom = zoom;
        self.offset = self.clamped_offset(viewport, image);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    fn geometry() -> (Bounds<Pixels>, Size<DevicePixels>) {
        (
            Bounds::new(point(px(40.), px(60.)), size(px(800.), px(600.))),
            size(1200u32.into(), 900u32.into()),
        )
    }

    #[test]
    fn zoom_keeps_the_image_point_under_the_cursor_and_limits_the_scale() {
        let (bounds, image) = geometry();
        let cursor = point(px(540.), px(410.));
        let mut view = ViewTransform::default();
        let before = view.image_bounds(bounds, image).unwrap();
        view.zoom_at(bounds, image, cursor, 200.);
        let after = view.image_bounds(bounds, image).unwrap();
        let expected = (cursor - before.origin) * view.zoom;
        let actual = cursor - after.origin;
        assert!((expected.x - actual.x).abs() < px(0.001));
        assert!((expected.y - actual.y).abs() < px(0.001));
        view.zoom_at(bounds, image, cursor, 10000.);
        assert_eq!(view.zoom, 4.);
        view.zoom_at(bounds, image, cursor, -10000.);
        assert_eq!(view.zoom, 1.);
        assert_eq!(view.image_bounds(bounds, image).unwrap(), before);
    }

    #[test]
    fn pan_is_clamped_after_drag_resize_and_aspect_changes() {
        let (bounds, image) = geometry();
        let mut view = PreviewViewport::default();
        view.bounds.set(Some(bounds));
        assert!(!view.move_pointer(bounds.center(), true, image));
        view.zoom(bounds.center(), 400., image);
        view.press(bounds.center());
        assert!(view.move_pointer(point(px(10000.), px(-10000.)), true, image));
        let moved = view.transform.image_bounds(bounds, image).unwrap();
        assert!((moved.left() - bounds.left()).abs() < px(0.001));
        assert!((moved.bottom() - bounds.bottom()).abs() < px(0.001));
        let wide = size(4000u32.into(), 100u32.into());
        let resized = Bounds::new(bounds.origin, size(px(1200.), px(800.)));
        let fitted = view.transform.image_bounds(resized, wide).unwrap();
        assert_eq!(fitted.center().y, resized.center().y);
        view.release();
        assert!(!view.move_pointer(bounds.center(), true, image));
    }

    #[test]
    fn slider_and_wheel_share_scale_and_release_stops_dragging() {
        let (bounds, image) = geometry();
        let mut view = PreviewViewport::default();
        view.bounds.set(Some(bounds));
        let track = Bounds::new(point(px(200.), px(20.)), size(px(112.), px(22.)));
        view.slider_bounds.set(Some(track));
        view.zoom(bounds.center(), 200., image);
        view.press_slider(track.center(), image);
        assert_eq!(view.transform.zoom, 2.5);
        assert!(view.move_pointer(point(px(1000.), px(100.)), true, image));
        assert_eq!(view.transform.zoom, 4.);
        assert!(view.move_pointer(point(px(0.), px(100.)), true, image));
        assert_eq!(view.transform.zoom, 1.);
        view.release();
        assert!(!view.move_pointer(track.center(), true, image));
        assert_eq!(view.transform.zoom, 1.);
    }

    #[test]
    fn displayed_source_changes_freeze_the_outgoing_transform_and_reset_the_new_one() {
        let (bounds, image) = geometry();
        let mut view = PreviewViewport::default();
        view.bounds.set(Some(bounds));
        view.sync_source(Some(Path::new("first.jpg")));
        view.zoom(bounds.center(), 200., image);
        view.press(bounds.center());
        view.move_pointer(bounds.center() + point(px(50.), px(30.)), true, image);
        let visible = view.transform.image_bounds(bounds, image);
        let zoom = view.transform.zoom;
        view.sync_source(Some(Path::new("first.jpg")));
        assert_eq!(view.transform.zoom, zoom);
        view.press(bounds.center());
        view.sync_source(Some(Path::new("second.jpg")));
        assert_eq!(view.transform.zoom, 1.);
        assert_eq!(view.previous_transform.zoom, zoom);
        assert_eq!(view.previous_transform.image_bounds(bounds, image), visible);
        assert!(!view.move_pointer(bounds.center(), true, image));
        view.zoom(bounds.center(), 200., image);
        view.sync_source(None);
        assert_eq!(view.transform.zoom, 1.);
    }
}
