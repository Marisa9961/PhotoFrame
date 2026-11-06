#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
use photo_frame::tr;
mod ui;
use gpui::{prelude::*, *};
fn main() {
    Application::new().run(|cx: &mut App| {
        ui::init(cx);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1360.), px(900.)), cx);
        cx.open_window(
            ui::window_options(bounds, tr!("app.name").into()),
            |window, cx| {
                let studio = cx.new(ui::Studio::new);
                window.set_window_title(&tr!("app.name"));
                let paths = std::env::args_os()
                    .skip(1)
                    .map(std::path::PathBuf::from)
                    .collect::<Vec<_>>();
                if !paths.is_empty() {
                    studio.update(cx, |studio, cx| studio.add(paths, cx));
                }
                studio
            },
        )
        .expect(&tr!("errors.window"));
        cx.activate(true);
    });
}
