use gtk::prelude::*;
use gtk::{Application, ScrolledWindow, TextView, Window};

pub fn open_copyright_dialog(app: &Application) {
    let copyright_str = include_str!("../../COPYRIGHT");
    let text_view = TextView::builder().build();
    let buffer = text_view.buffer();
    buffer.set_text(&copyright_str);

    let scrolled_window = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&text_view)
        .build();

    let window = Window::builder()
        .application(app)
        .child(&scrolled_window)
        .title("Copyright")
        .default_width(400)
        .default_height(400)
        .build();
    window.present();
}
