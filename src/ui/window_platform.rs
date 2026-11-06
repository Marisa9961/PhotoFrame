use gpui::{
    AnyElement, App, Bounds, CursorStyle, Decorations, Div, MouseButton, Pixels, Point, ResizeEdge,
    SharedString, Size, Stateful, Tiling, TitlebarOptions, Window, WindowBounds, WindowControlArea,
    WindowDecorations, WindowOptions, div, point, prelude::*, px, size,
};

const TITLEBAR_HEIGHT: f32 = 36.;
const RESIZE_BORDER: f32 = 5.;
const DRAG_DISTANCE: f64 = 4.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Platform {
    Windows,
    Linux,
    Mac,
    Other,
}

impl Platform {
    fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            Self::Linux
        } else if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Other
        }
    }
}

pub(crate) fn window_options(bounds: Bounds<Pixels>, title: SharedString) -> WindowOptions {
    let platform = Platform::current();
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(1060.), px(680.))),
        titlebar: Some(TitlebarOptions {
            title: Some(title),
            appears_transparent: platform == Platform::Windows,
            ..Default::default()
        }),
        window_decorations: (platform == Platform::Linux).then_some(WindowDecorations::Client),
        ..Default::default()
    }
}

pub(super) fn titlebar_height(window: &Window) -> Pixels {
    let custom = match Platform::current() {
        Platform::Windows => true,
        Platform::Linux => matches!(window.window_decorations(), Decorations::Client { .. }),
        // Keep AppKit's native title bar and traffic lights.
        Platform::Mac | Platform::Other => false,
    };
    px(if custom && !window.is_fullscreen() {
        TITLEBAR_HEIGHT
    } else {
        0.
    })
}

fn native_resize_inset(window: &Window) -> Pixels {
    px(if window.is_maximized() || window.is_fullscreen() {
        0.
    } else {
        RESIZE_BORDER
    })
}

#[derive(Clone, Copy)]
pub(super) enum Control {
    Minimize,
    Maximize,
    Close,
}

pub(super) fn bind_button(
    button: Stateful<Div>,
    control: Control,
    window: &Window,
) -> Stateful<Div> {
    match Platform::current() {
        // GPUI 0.2.2 handles these through HTMINBUTTON/HTMAXBUTTON/HTCLOSE.
        // Registering a click callback here would bypass its maximize/restore logic.
        Platform::Windows => {
            let inset = native_resize_inset(window);
            // The outer button paints to the window edge. Only its native control
            // hitbox is inset, leaving the frame edges available for OS resizing.
            button.child(
                div()
                    .id("native-window-control")
                    .absolute()
                    .inset_0()
                    .top(inset)
                    .right(match control {
                        Control::Close => inset,
                        _ => px(0.),
                    })
                    .window_control_area(match control {
                        Control::Minimize => WindowControlArea::Min,
                        Control::Maximize => WindowControlArea::Max,
                        Control::Close => WindowControlArea::Close,
                    }),
            )
        }
        Platform::Linux => button
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                match control {
                    Control::Minimize => window.minimize_window(),
                    Control::Maximize => window.zoom_window(),
                    Control::Close => window.remove_window(),
                }
                window.refresh();
            }),
        Platform::Mac | Platform::Other => button,
    }
}

pub(super) fn bind_drag_area(
    area: Stateful<Div>,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    match Platform::current() {
        Platform::Windows => {
            let inset = native_resize_inset(window);
            area.child(
                div()
                    .id("native-window-drag")
                    .absolute()
                    .inset_0()
                    .top(inset)
                    .left(inset)
                    .window_control_area(WindowControlArea::Drag),
            )
        }
        Platform::Linux => {
            let drag = window.use_keyed_state("titlebar-drag", cx, |_, _| DragState::default());
            let down = drag.clone();
            let up = drag.clone();
            let down_out = drag.clone();
            let up_out = drag.clone();
            let click = drag.clone();
            area.on_mouse_down(MouseButton::Left, move |event, _, cx| {
                down.update(cx, |state, _| state.press(event.position));
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                up.update(cx, |state, _| state.clear());
            })
            .on_mouse_down_out(move |_, _, cx| {
                down_out.update(cx, |state, _| state.clear());
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                up_out.update(cx, |state, _| state.clear());
            })
            .on_mouse_move(move |event, window, cx| {
                if drag.update(cx, |state, _| state.moved(event.position, event.dragging())) {
                    window.start_window_move();
                }
            })
            .on_click(move |event, window, cx| {
                if event.click_count() == 2
                    && !click.read(cx).dragged
                    && window.window_controls().maximize
                {
                    window.zoom_window();
                }
            })
            .on_mouse_down(MouseButton::Right, |event, window, _| {
                if window.window_controls().window_menu {
                    window.show_window_menu(event.position);
                }
            })
        }
        // If custom content is added to the native macOS title bar later, preserve
        // the system's double-click preference instead of treating macOS as Linux.
        Platform::Mac => area.on_click(|event, window, _| {
            if event.click_count() == 2 {
                window.titlebar_double_click();
            }
        }),
        Platform::Other => area,
    }
}

#[derive(Default)]
struct DragState {
    pressed_at: Option<Point<Pixels>>,
    dragged: bool,
}

impl DragState {
    fn press(&mut self, position: Point<Pixels>) {
        self.pressed_at = Some(position);
        self.dragged = false;
    }
    fn clear(&mut self) {
        self.pressed_at = None;
    }
    fn moved(&mut self, position: Point<Pixels>, left_pressed: bool) -> bool {
        if !left_pressed {
            self.clear();
            return false;
        }
        if self
            .pressed_at
            .is_some_and(|start| (position - start).magnitude() >= DRAG_DISTANCE)
        {
            self.clear();
            self.dragged = true;
            return true;
        }
        false
    }
}

pub(super) fn resize_handles(window: &Window) -> Vec<AnyElement> {
    if Platform::current() != Platform::Linux || window.is_fullscreen() || window.is_maximized() {
        return Vec::new();
    }
    let Decorations::Client { tiling } = window.window_decorations() else {
        return Vec::new();
    };
    resize_regions(window.viewport_size(), tiling)
        .into_iter()
        .enumerate()
        .map(|(index, (edge, bounds))| {
            div()
                .id(("window-resize", index))
                .absolute()
                .left(bounds.origin.x)
                .top(bounds.origin.y)
                .w(bounds.size.width)
                .h(bounds.size.height)
                .cursor(match edge {
                    ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
                    ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
                    ResizeEdge::TopLeft | ResizeEdge::BottomRight => {
                        CursorStyle::ResizeUpLeftDownRight
                    }
                    ResizeEdge::TopRight | ResizeEdge::BottomLeft => {
                        CursorStyle::ResizeUpRightDownLeft
                    }
                })
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    window.start_window_resize(edge);
                })
                .into_any_element()
        })
        .collect()
}

fn resize_regions(size: Size<Pixels>, tiling: Tiling) -> Vec<(ResizeEdge, Bounds<Pixels>)> {
    let (w, h, b) = (size.width, size.height, px(RESIZE_BORDER));
    [
        (ResizeEdge::Top, !tiling.top, px(0.), px(0.), w, b),
        (ResizeEdge::Bottom, !tiling.bottom, px(0.), h - b, w, b),
        (ResizeEdge::Left, !tiling.left, px(0.), px(0.), b, h),
        (ResizeEdge::Right, !tiling.right, w - b, px(0.), b, h),
        (
            ResizeEdge::TopLeft,
            !tiling.top && !tiling.left,
            px(0.),
            px(0.),
            b,
            b,
        ),
        (
            ResizeEdge::TopRight,
            !tiling.top && !tiling.right,
            w - b,
            px(0.),
            b,
            b,
        ),
        (
            ResizeEdge::BottomLeft,
            !tiling.bottom && !tiling.left,
            px(0.),
            h - b,
            b,
            b,
        ),
        (
            ResizeEdge::BottomRight,
            !tiling.bottom && !tiling.right,
            w - b,
            h - b,
            b,
            b,
        ),
    ]
    .into_iter()
    .filter(|(_, enabled, ..)| *enabled)
    .map(|(edge, _, x, y, w, h)| (edge, Bounds::new(point(x, y), gpui::size(w, h))))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn dragging_requires_a_titlebar_press_and_starts_once_after_pointer_movement() {
        let mut drag = DragState::default();
        let origin = point(px(50.), px(20.));
        let moved = origin + point(px(10.), px(0.));
        assert!(
            !drag.moved(moved, true),
            "Entering with a held button must not move the window"
        );
        drag.press(origin);
        assert!(
            !drag.moved(origin + point(px(1.), px(1.)), true),
            "Click jitter must not start a drag"
        );
        assert!(drag.moved(moved, true));
        assert!(
            !drag.moved(moved, true),
            "Send only one compositor move request per press"
        );
        drag.clear();
        assert!(
            drag.dragged,
            "Releasing a completed drag must not turn it into a double-click"
        );
        drag.press(origin);
        assert!(!drag.dragged);
        drag.clear();
        assert!(
            !drag.moved(moved, true),
            "A release or outside press must disarm dragging"
        );
        drag.press(origin);
        assert!(!drag.moved(moved, false));
        assert!(
            !drag.moved(moved, true),
            "A missed release is cleared by the next move"
        );
    }

    #[test]
    fn resize_corners_take_precedence_and_tiled_edges_are_disabled() {
        let size = size(px(800.), px(600.));
        let regions = resize_regions(size, Tiling::default());
        for (x, y, expected) in [
            (2., 2., ResizeEdge::TopLeft),
            (798., 2., ResizeEdge::TopRight),
            (2., 598., ResizeEdge::BottomLeft),
            (798., 598., ResizeEdge::BottomRight),
            (400., 2., ResizeEdge::Top),
            (400., 598., ResizeEdge::Bottom),
            (2., 300., ResizeEdge::Left),
            (798., 300., ResizeEdge::Right),
        ] {
            let hit = regions
                .iter()
                .rev()
                .find(|(_, bounds)| bounds.contains(&point(px(x), px(y))));
            assert_eq!(hit.map(|(edge, _)| *edge), Some(expected));
        }
        assert!(
            regions
                .iter()
                .all(|(_, bounds)| !bounds.contains(&point(px(400.), px(300.))))
        );
        assert!(resize_regions(size, Tiling::tiled()).is_empty());
        let top_tiled = resize_regions(
            size,
            Tiling {
                top: true,
                ..Tiling::default()
            },
        );
        assert!(!top_tiled.iter().any(|(edge, _)| matches!(
            edge,
            ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight
        )));
    }
}
