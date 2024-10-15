use std::default::Default;
use std::io::BufRead;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{mpsc, Arc, Mutex};

use crate::command::{binary_to_path, new_command, ChildOutput};
use crate::gui::runtime;
use crate::gui::state::DicomServerState;
use gtk::glib::{clone, spawn_future_local};
use gtk::prelude::*;
use gtk::{
    glib, AlertDialog, ApplicationWindow, Button, Entry, Expander, Frame, Grid, Label,
    ScrolledWindow, TextView,
};

pub fn setup_dicom_server(
    initial_state: &DicomServerState,
    window: &ApplicationWindow,
    grid: &Grid,
    grid_y_index: i32,
    worklist_dir_arc: Arc<Mutex<Option<PathBuf>>>,
) -> (i32, mpsc::Receiver<DicomServerState>) {
    let (state_sender, state_receiver) = mpsc::channel();
    let frame = Frame::builder()
        .label("DICOM worklist server")
        .vexpand(false)
        .build();

    let port_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Port")
        .build();
    let port_entry = Entry::builder().hexpand(true).build();
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
    grid_layout.attach(&port_label, 0, 0, 1, 1);
    grid_layout.attach(&port_entry, 1, 0, 1, 1);
    grid_layout.attach(&status_label, 1, 1, 1, 1);
    grid_layout.attach(&run_button, 0, 1, 1, 1);
    grid_layout.attach(&log_expander, 0, 2, 3, 1);

    grid.attach(&frame, 0, grid_y_index, 4, 1);

    if let Some(p) = &initial_state.port {
        port_entry.buffer().set_text(p.to_string());
    }

    let notify_state_update = clone!(
        #[weak]
        port_entry,
        move || {
            let port_str = port_entry.buffer().text();
            let port_int = u16::from_str(port_str.as_str());
            let state = DicomServerState {
                port: port_int.ok(),
            };
            _ = state_sender.send(state);
        }
    );

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

    let running_child: Arc<Mutex<Option<Arc<shared_child::SharedChild>>>> =
        Arc::new(Mutex::new(None));

    let update_run_status = clone!(
        #[weak]
        run_button,
        #[weak]
        status_label,
        #[weak]
        port_entry,
        #[weak]
        running_child,
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
                async move {
                    let rc = running_child.lock().unwrap();
                    if rc.is_some() {
                        run_button.set_label("Stop");
                        status_label.set_label("Running");
                        port_entry.set_sensitive(false);
                    } else {
                        run_button.set_label("Run");
                        status_label.set_label("Stopped");
                        port_entry.set_sensitive(true);
                    }
                }
            ));
        }
    );

    run_button.connect_clicked(clone!(
        #[weak]
        window,
        #[weak]
        worklist_dir_arc,
        #[weak]
        port_entry,
        #[weak]
        log_text_view,
        move |_| {
            let o_worklist_dir = worklist_dir_arc.lock().unwrap();
            let worklist_dir = match o_worklist_dir.deref() {
                Some(a) => a,
                None => {
                    AlertDialog::builder()
                        .message("Please select a Worklist folder first")
                        .modal(true)
                        .build()
                        .show(Some(&window));
                    return;
                }
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

            let mut rc = running_child.lock().unwrap();
            if let Some(ref mut child) = rc.deref_mut() {
                _ = child.kill();
                *rc = None;
                let buffer = log_text_view.buffer();
                buffer.insert(&mut buffer.end_iter(), "Killed process");
                buffer.insert(&mut buffer.end_iter(), "\n");
            } else {
                let (sender, receiver) = mpsc::channel::<ChildOutput>();

                let full_path = binary_to_path("wlmscpfs".to_string());
                let mut command = new_command(full_path);
                command
                    .args(vec![
                        "-v",
                        "-d",
                        "-dfr",
                        "-dfp",
                        worklist_dir.to_str().unwrap(),
                        &format!("{}", port_int),
                    ])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());

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
            }
            update_run_status();
        }
    ));

    return (grid_y_index + 1, state_receiver);
}
