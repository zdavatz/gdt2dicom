use std::sync::{Arc, Mutex};

use gtk::gio::prelude::FileExt;
use gtk::glib::clone;
use gtk::prelude::*;
use gtk::{
    glib, ApplicationWindow, Button, Entry, Expander, FileDialog, Frame, Grid, Label,
    ScrolledWindow, TextView,
};

use crate::gui::state::CStoreServerState;
use std::sync::mpsc;

pub fn setup_cstore_server(
    initial_state: &CStoreServerState,
    window: &ApplicationWindow,
    parent_grid: &Grid,
    grid_y_index: i32,
) -> (i32, mpsc::Receiver<CStoreServerState>) {
    let (state_sender, state_receiver) = mpsc::channel();
    let frame = Frame::builder()
        .label("CStore server")
        .vexpand(false)
        .build();

    let dir_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Directory")
        .build();
    let dir_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let dir_button = Button::builder()
        .width_request(100)
        .hexpand(false)
        .label("Choose...")
        .build();

    let port_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Port")
        .build();
    let port_entry = Entry::builder().hexpand(true).build();

    let jpeg_dir_label = Label::builder()
        .halign(gtk::Align::End)
        .label("JPEG output dir")
        .build();
    let jpeg_dir_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let jpeg_dir_button = Button::builder()
        .width_request(100)
        .hexpand(false)
        .label("Choose...")
        .build();

    let run_button = Button::builder().label("Run").build();
    // let status_label = Label::new(Some("Stopped"));
    let status_label = Label::builder()
        .label("Stopped")
        .halign(gtk::Align::Start)
        .build();
    let log_text_view = TextView::builder().build();
    let log_scroll_window = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .height_request(100)
        .child(&log_text_view)
        .build();
    let log_expander = Expander::builder()
        .label("Logs")
        .resize_toplevel(true)
        .child(&log_scroll_window)
        .build();

    let grid_layout = Grid::builder()
        .column_spacing(12)
        .row_spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    frame.set_child(Some(&grid_layout));
    grid_layout.attach(&dir_label, 0, 0, 1, 1);
    grid_layout.attach(&dir_entry, 1, 0, 2, 1);
    grid_layout.attach(&dir_button, 3, 0, 1, 1);

    grid_layout.attach(&port_label, 0, 1, 1, 1);
    grid_layout.attach(&port_entry, 1, 1, 2, 1);

    grid_layout.attach(&jpeg_dir_label, 0, 2, 1, 1);
    grid_layout.attach(&jpeg_dir_entry, 1, 2, 2, 1);
    grid_layout.attach(&jpeg_dir_button, 3, 2, 1, 1);

    grid_layout.attach(&run_button, 0, 3, 1, 1);
    grid_layout.attach(&status_label, 1, 3, 1, 1);

    grid_layout.attach(&log_expander, 0, 4, 4, 1);

    parent_grid.attach(&frame, 0, grid_y_index, 4, 1);
    return (grid_y_index + 1, state_receiver);
}
