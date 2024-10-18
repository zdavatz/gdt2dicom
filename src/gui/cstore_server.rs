use std::io::BufRead;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{mpsc, Arc, Mutex};

use gtk::gio::prelude::FileExt;
use gtk::glib::{clone, spawn_future_local};
use gtk::prelude::*;
use gtk::{
    glib, AlertDialog, ApplicationWindow, Button, Entry, Expander, FileDialog, Frame, Grid, Label,
    ScrolledWindow, TextView,
};

use crate::command::{binary_to_path, new_command, ChildOutput};
use crate::dicom_jpeg_extraction::DicomJpegExtraction;
use crate::gui::runtime;
use crate::gui::state::CStoreServerState;

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

    if let Some(p) = &initial_state.path {
        dir_entry.buffer().set_text(p.display().to_string());
    }
    if let Some(p) = &initial_state.port {
        port_entry.buffer().set_text(p.to_string());
    }
    if let Some(p) = &initial_state.jpeg_output_path {
        jpeg_dir_entry.buffer().set_text(p.display().to_string());
    }

    let notify_state_update = clone!(
        #[weak]
        dir_entry,
        #[weak]
        port_entry,
        #[weak]
        jpeg_dir_entry,
        move || {
            let dir = dir_entry.buffer().text().as_str().to_string();
            let dir_path = if dir.is_empty() {
                None
            } else {
                Some(PathBuf::from(dir))
            };
            let port_str = port_entry.buffer().text();
            let port_int = u16::from_str(port_str.as_str());
            let jpeg_dir = jpeg_dir_entry.buffer().text().as_str().to_string();
            let jpeg_dir_path = if jpeg_dir.is_empty() {
                None
            } else {
                Some(PathBuf::from(jpeg_dir))
            };
            let state = CStoreServerState {
                path: dir_path,
                port: port_int.ok(),
                jpeg_output_path: jpeg_dir_path,
            };
            _ = state_sender.send(state);
        }
    );

    let notify_state_update1 = notify_state_update.clone();
    dir_button.connect_clicked(clone!(
        #[weak]
        window,
        #[weak]
        dir_entry,
        move |_| {
            let dialog = FileDialog::builder().build();
            let notify_state_update1 = notify_state_update1.clone();
            dialog.select_folder(
                Some(&window),
                None::<gtk::gio::Cancellable>.as_ref(),
                clone!(
                    #[weak]
                    dir_entry,
                    move |result| match result {
                        Err(err) => {
                            println!("err {:?}", err);
                        }
                        Ok(file) => {
                            if let Some(input_path) = file.path() {
                                if let Some(p) = input_path.to_str() {
                                    dir_entry.buffer().set_text(p);
                                    notify_state_update1();
                                }
                            }
                        }
                    },
                ),
            );
        }
    ));

    port_entry
        .delegate()
        .unwrap()
        .connect_insert_text(move |entry, text, position| {
            let pattern = |c: char| -> bool { !c.is_ascii_digit() };
            if text.contains(pattern) || text.len() > 5 {
                glib::signal::signal_stop_emission_by_name(entry, "insert-text");
                entry.insert_text(&text.replace(pattern, ""), position);
            }
        });

    let notify_state_update1 = notify_state_update.clone();
    port_entry.connect_changed(move |_| {
        notify_state_update1();
    });

    let notify_state_update1 = notify_state_update.clone();
    jpeg_dir_button.connect_clicked(clone!(
        #[weak]
        window,
        #[weak]
        jpeg_dir_entry,
        move |_| {
            let dialog = FileDialog::builder().build();
            let notify_state_update1 = notify_state_update1.clone();
            dialog.select_folder(
                Some(&window),
                None::<gtk::gio::Cancellable>.as_ref(),
                clone!(
                    #[weak]
                    jpeg_dir_entry,
                    move |result| match result {
                        Err(err) => {
                            println!("err {:?}", err);
                        }
                        Ok(file) => {
                            if let Some(input_path) = file.path() {
                                if let Some(p) = input_path.to_str() {
                                    jpeg_dir_entry.buffer().set_text(p);
                                    notify_state_update1();
                                }
                            }
                        }
                    },
                ),
            );
        }
    ));

    let running_child: Arc<Mutex<Option<Arc<shared_child::SharedChild>>>> =
        Arc::new(Mutex::new(None));
    let jpeg_extraction: Arc<Mutex<Option<DicomJpegExtraction>>> = Arc::new(Mutex::new(None));

    let update_run_status = clone!(
        #[weak]
        run_button,
        #[weak]
        status_label,
        #[weak]
        port_entry,
        #[weak]
        running_child,
        #[weak]
        jpeg_dir_button,
        #[weak]
        dir_button,
        move || {
            spawn_future_local(clone!(
                #[weak]
                run_button,
                #[weak]
                status_label,
                #[weak]
                port_entry,
                #[weak]
                running_child,
                #[weak]
                jpeg_dir_button,
                #[weak]
                dir_button,
                async move {
                    let rc = running_child.lock().unwrap();
                    if rc.is_some() {
                        run_button.set_label("Stop");
                        status_label.set_label("Running");
                        port_entry.set_sensitive(false);
                        dir_button.set_sensitive(false);
                        jpeg_dir_button.set_sensitive(false);
                    } else {
                        run_button.set_label("Run");
                        status_label.set_label("Stopped");
                        port_entry.set_sensitive(true);
                        dir_button.set_sensitive(true);
                        jpeg_dir_button.set_sensitive(true);
                    }
                }
            ));
        }
    );

    run_button.connect_clicked(clone!(
        #[weak]
        window,
        #[weak]
        dir_entry,
        #[weak]
        jpeg_dir_entry,
        #[weak]
        port_entry,
        move |_| {
            let mut rc = running_child.lock().unwrap();
            if let Some(ref mut child) = rc.deref_mut() {
                _ = child.kill();
                *rc = None;
                let buffer = log_text_view.buffer();
                buffer.insert(&mut buffer.end_iter(), "Killed process");
                buffer.insert(&mut buffer.end_iter(), "\n");
                let mut ex = jpeg_extraction.lock().unwrap();
                if let Some(ref mut jpeg_child) = ex.deref_mut() {
                    jpeg_child.stop();
                }
                *ex = None;
            } else {
                let dir = dir_entry.buffer().text().as_str().to_string();
                if dir.is_empty() {
                    AlertDialog::builder()
                        .message("Please select a directory first")
                        .modal(true)
                        .build()
                        .show(Some(&window));
                    return;
                }
                let dir_path = PathBuf::from(dir);
                let jpeg_dir = jpeg_dir_entry.buffer().text().as_str().to_string();
                let jpeg_dir_path = if jpeg_dir.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(jpeg_dir))
                };
                let port_str = port_entry.buffer().text();
                let port_int = match u16::from_str(port_str.as_str()) {
                    Err(_) => {
                        AlertDialog::builder()
                            .message("Please enter a valid port")
                            .modal(true)
                            .build()
                            .show(Some(&window));
                        return;
                    }
                    Ok(a) => a,
                };

                let full_path = binary_to_path("storescp".to_string());
                let mut command = new_command(full_path);
                command
                    .args(vec![
                        "-v",
                        "-pm",
                        "+xy",
                        "-od",
                        dir_path.to_str().unwrap(),
                        &format!("{}", port_int),
                    ])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());

                let (sender, receiver) = mpsc::channel::<ChildOutput>();
                _ = sender.send(ChildOutput::Log(format!("Running command: {:?}", command)));

                let child = match shared_child::SharedChild::spawn(&mut command) {
                    Ok(c) => c,
                    Err(err) => {
                        AlertDialog::builder()
                            .message("Error")
                            .detail(err.to_string())
                            .modal(true)
                            .build()
                            .show(Some(&window));
                        return;
                    }
                };
                let stdout = child.take_stdout().expect("stdout");
                let stderr = child.take_stderr().expect("stderr");

                let err_reader = std::io::BufReader::new(stderr);
                let err_sender = sender.clone();
                runtime().spawn(async move {
                    for line in err_reader.lines() {
                        if let Ok(msg) = line {
                            _ = err_sender.send(ChildOutput::Log(msg));
                        }
                    }
                });

                let out_reader = std::io::BufReader::new(stdout);
                let out_sender = sender.clone();
                runtime().spawn(async move {
                    for line in out_reader.lines() {
                        if let Ok(msg) = line {
                            _ = out_sender.send(ChildOutput::Log(msg));
                        }
                    }
                });

                let (asender, arecv) = async_channel::unbounded::<ChildOutput>();
                runtime().spawn(async move {
                    while let Ok(msg) = receiver.recv() {
                        _ = asender.send(msg).await;
                    }
                });

                let update_run_status1 = update_run_status.clone();
                spawn_future_local(clone!(
                    #[weak]
                    running_child,
                    #[weak]
                    log_text_view,
                    async move {
                        while let Ok(msg) = arecv.recv().await {
                            let buffer = log_text_view.buffer();
                            match msg {
                                ChildOutput::Log(msg) => {
                                    buffer.insert(&mut buffer.end_iter(), &msg);
                                }
                                ChildOutput::Exit(_exit_status) => {
                                    let mut rc = running_child.lock().unwrap();
                                    *rc = None;
                                    print!("ChildOutput::Exit");
                                    update_run_status1();
                                }
                            }
                            buffer.insert(&mut buffer.end_iter(), "\n");
                        }
                    }
                ));

                let arc_child = Arc::new(child);

                let child1 = arc_child.clone();
                let exit_sender = sender.clone();
                runtime().spawn(async move {
                    let exit_result = child1.wait().expect("wait");
                    _ = exit_sender.send(ChildOutput::Log(format!("Exited {:?}", exit_result)));
                    _ = exit_sender.send(ChildOutput::Exit(exit_result));
                });
                *rc = Some(arc_child);
                update_run_status();

                if let Some(j) = jpeg_dir_path {
                    let mut ex = jpeg_extraction.lock().unwrap();
                    let mut x = DicomJpegExtraction::new(dir_path, j, sender.clone()).unwrap();
                    x.start();
                    *ex = Some(x);
                }
            }
        }
    ));

    return (grid_y_index + 1, state_receiver);
}
