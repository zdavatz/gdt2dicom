#![windows_subsystem = "windows"]

use std::default::Default;
use std::fs::{read_dir, File};
use std::io::{BufRead, Error as IoError};
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{mpsc, Arc, Mutex, OnceLock};

use gdt2dicom::command::{binary_to_path, check_if_binary_exists, new_command};
use gdt2dicom::worklist_conversion::{WorklistConversion, WorklistConversionState};
use gtk::gio::prelude::FileExt;
use gtk::gio::{ActionEntry, Menu};
use gtk::glib::{clone, spawn_future_local};
use gtk::prelude::*;
use gtk::{
    glib, AboutDialog, AlertDialog, Application, ApplicationWindow, Button, Entry, Expander,
    FileDialog, Frame, Grid, Label, ScrolledWindow, Separator, TextView, Window,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::runtime::Runtime;

fn main() -> glib::ExitCode {
    let application = Application::builder()
        .application_id("ch.ywesee.gdt2dicom")
        .build();

    application.connect_activate(|app| {
        let menubar = Menu::new();

        let help_menu = Menu::new();
        help_menu.append(Some("About"), Some("win.open-about"));
        menubar.append_submenu(Some("Help"), &help_menu);
        help_menu.append(Some("Copyright"), Some("win.open-copyright"));
        app.set_menubar(Some(&menubar));

        let window = ApplicationWindow::builder()
            .application(app)
            .title("gdt2dicom")
            .default_width(350)
            .default_height(70)
            .show_menubar(true)
            .build();

        let action_about = ActionEntry::builder("open-about")
            .activate(|_window: &ApplicationWindow, _, _| {
                open_about_dialog();
            })
            .build();
        let action_copyright = ActionEntry::builder("open-copyright")
            .activate(clone!(
                #[weak]
                app,
                move |_window: &ApplicationWindow, _, _| {
                    open_copyright_dialog(&app);
                }
            ))
            .build();
        window.add_action_entries([action_about, action_copyright]);

        let grid_layout = Grid::builder()
            .column_spacing(12)
            .row_spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let saved_state = read_saved_states().unwrap_or_else(|err| {
            println!("Error while restoring state: {:?}", err);
            StateFile::default()
        });

        // TODO: make dicom_server non-optional when user upgraded from old save data
        let dicom_server_state = &saved_state
            .dicom_server
            .clone()
            .unwrap_or(DicomServerState::default());

        let state_arc = Arc::new(Mutex::new(saved_state.clone()));

        let state_arc1 = state_arc.clone();
        let on_worklist_path_updated = move |new_path| {
            let state = state_arc1.lock().unwrap();
            let new_state = StateFile {
                worklist_path: new_path,
                ..state.deref().clone()
            };
            _ = write_state_to_file(new_state);
        };
        let y = 0;
        let (y, worklist_dir_arc) = setup_worklist_folder_ui(
            &saved_state.worklist_path,
            &window,
            &grid_layout.clone(),
            y,
            on_worklist_path_updated,
        );
        let (y, dicom_server_state_receiver) = setup_dicom_server(
            dicom_server_state,
            &window,
            &grid_layout.clone(),
            y,
            worklist_dir_arc.clone(),
        );
        let (_y, convert_list_state_receiver) = setup_auto_convert_list_ui(
            &saved_state.conversions,
            &window.clone(),
            &grid_layout.clone(),
            y,
            worklist_dir_arc.clone(),
        );

        let state_arc1 = state_arc.clone();
        runtime().spawn(async move {
            while let Ok(dicom_server_state) = dicom_server_state_receiver.recv() {
                let state = state_arc1.lock().unwrap();
                let new_state = StateFile {
                    dicom_server: Some(dicom_server_state),
                    ..state.deref().clone()
                };
                _ = write_state_to_file(new_state);
            }
        });

        let state_arc1 = state_arc.clone();
        runtime().spawn(async move {
            while let Ok(convert_list_state) = convert_list_state_receiver.recv() {
                let state = state_arc1.lock().unwrap();
                let new_state = StateFile {
                    conversions: convert_list_state,
                    ..state.deref().clone()
                };
                _ = write_state_to_file(new_state);
            }
        });

        window.set_child(Some(&grid_layout));
        window.present();

        if cfg!(target_os = "linux") {
            check_dcmtk_binaries(&window, &app);
        }
    });

    return application.run();
}

fn check_dcmtk_binaries(window: &ApplicationWindow, app: &Application) {
    let mut missing_binaries: Vec<String> = Vec::new();
    let binaries = vec!["xml2dcm", "dcmodify", "dcmdump", "dump2dcm"];
    for b in binaries {
        let p = PathBuf::from(b);
        if !check_if_binary_exists(&p) {
            missing_binaries.push(b.to_string());
        }
    }
    if !missing_binaries.is_empty() {
        AlertDialog::builder()
            .message("Error")
            .detail(format!("Missing dependencies, please make sure you have dcmtk installed. The following binaries are not found: {}", missing_binaries.join(", ")))
            .modal(true)
            .build()
            .choose(Some(window), None::<gtk::gio::Cancellable>.as_ref(), clone!(
                #[weak]
                app,move |_| app.quit())
            );
    }
}

fn open_about_dialog() {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let license_str = include_str!("../../LICENSE");
    let credits = "Credit: Windows-10 theme (https://github.com/B00merang-Project/Windows-10) is bundled with the windows build.\n\n\n\n".to_string();
    let a = AboutDialog::builder()
        .title("About gdt2dicom")
        .program_name("gdt2dicom")
        .license(credits + license_str)
        .wrap_license(true)
        .version(VERSION)
        .build();
    a.set_visible(true);
}

fn open_copyright_dialog(app: &Application) {
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

fn setup_worklist_folder_ui<F>(
    initial_state: &Option<PathBuf>,
    window: &ApplicationWindow,
    grid: &Grid,
    grid_y_index: i32,
    on_updated: F,
) -> (i32, Arc<Mutex<Option<PathBuf>>>)
where
    F: Fn(Option<PathBuf>) + 'static + Clone,
{
    let worklist_file_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Worklist Folder")
        .build();
    let worklist_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let worklist_button = Button::builder()
        .width_request(100)
        .hexpand(false)
        .label("Choose...")
        .build();

    grid.attach(&worklist_file_label, 0, grid_y_index, 1, 1);
    grid.attach(&worklist_entry, 1, grid_y_index, 2, 1);
    grid.attach(&worklist_button, 3, grid_y_index, 1, 1);

    if let Some(p) = initial_state {
        worklist_entry.buffer().set_text(p.display().to_string());
        _ = ensure_output_folder_lockfiles(p);
    }

    let worklist_dir_arc = Arc::new(Mutex::new(initial_state.clone()));
    let worklist_dir_arc2 = worklist_dir_arc.clone();
    worklist_button.connect_clicked(clone!(
        #[weak]
        window,
        #[weak]
        worklist_entry,
        move |_| {
            let dialog = FileDialog::builder().build();
            let on_updated2 = on_updated.clone();
            let worklist_dir_arc2 = worklist_dir_arc2.clone();
            dialog.select_folder(
                Some(&window),
                None::<gtk::gio::Cancellable>.as_ref(),
                clone!(
                    #[weak]
                    worklist_entry,
                    move |result| match result {
                        Err(err) => {
                            println!("err {:?}", err);
                        }
                        Ok(file) => {
                            if let Some(input_path) = file.path() {
                                if let Some(p) = input_path.to_str() {
                                    worklist_entry.buffer().set_text(p);
                                    let mut worklist_dir = worklist_dir_arc2.lock().unwrap();
                                    let new_path = PathBuf::from(p);
                                    _ = ensure_output_folder_lockfiles(&new_path);
                                    *worklist_dir = Some(new_path.clone());
                                    on_updated2(Some(new_path));
                                }
                            }
                        }
                    },
                ),
            );
        }
    ));

    return (grid_y_index + 1, worklist_dir_arc);
}

fn ensure_output_folder_lockfiles(worklist_dir: &PathBuf) -> Result<(), IoError> {
    // All subdirectory must have an empty .lockfile
    for entry in read_dir(worklist_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let mut lock_file = path.clone();
            lock_file.push(".lockfile");
            _ = File::create(lock_file)?;
        }
    }
    Ok(())
}

fn setup_dicom_server(
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
    let status_label = Label::new(Some("Stopped"));

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
                #[derive(Debug)]
                enum ChildOutput {
                    Log(String),
                    Exit(std::process::ExitStatus),
                }
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

fn setup_auto_convert_list_ui(
    initial_state: &WorklistConversionsState,
    window: &ApplicationWindow,
    grid: &Grid,
    grid_y_index: i32,
    worklist_dir_arc: Arc<Mutex<Option<PathBuf>>>,
) -> (i32, mpsc::Receiver<WorklistConversionsState>) {
    let (state_sender, state_receiver) = mpsc::channel();
    let worklist_conversions: Arc<Mutex<Vec<Arc<Mutex<WorklistConversion>>>>> =
        Arc::new(Mutex::new(vec![]));
    let conversion_scroll_window = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .height_request(300)
        .build();
    grid.attach(&conversion_scroll_window, 0, grid_y_index, 4, 1);

    let box1 = gtk::Box::new(gtk::Orientation::Vertical, 12);
    conversion_scroll_window.set_child(Some(&box1));

    let on_updated = clone!(
        #[weak]
        worklist_conversions,
        move || {
            let state_sender1 = state_sender.clone();
            spawn_future_local(clone!(
                #[weak]
                worklist_conversions,
                async move {
                    let wcs = worklist_conversions.lock().unwrap();
                    let all_states: WorklistConversionsState = wcs
                        .iter()
                        .map(|arc| arc.lock().unwrap().to_state())
                        .collect();
                    _ = state_sender1.send(all_states);
                }
            ));
        }
    );

    let new_convertion_button = Button::builder().label("Add new worklist folder").build();
    let worklist_conversions1 = worklist_conversions.clone();
    let add_new_worklist = clone!(
        #[weak]
        window,
        #[weak]
        worklist_dir_arc,
        move |state: Option<&WorklistConversionState>| {
            let frame = Frame::new(Some("Worklist folder"));
            let on_delete = clone!(
                #[weak]
                box1,
                #[weak]
                frame,
                move || {
                    box1.remove(&frame);
                }
            );

            let (this_ui, wc) = setup_auto_convert_ui(
                &window.clone(),
                on_delete,
                on_updated.clone(),
                worklist_dir_arc,
                state,
            );
            let mut cs = worklist_conversions1.lock().unwrap();
            cs.push(wc);
            frame.set_child(Some(&this_ui));
            box1.append(&frame);
        }
    );
    let add_new_worklist2 = add_new_worklist.clone();
    new_convertion_button.connect_clicked(move |_| {
        add_new_worklist2(None);
    });
    grid.attach(&new_convertion_button, 3, grid_y_index + 1, 1, 1);

    for state in initial_state {
        add_new_worklist(Some(state));
    }

    return (grid_y_index + 2, state_receiver);
}

fn setup_auto_convert_ui<F, G>(
    window: &ApplicationWindow,
    on_delete: F,
    on_updated: G,
    worklist_dir_arc: Arc<Mutex<Option<PathBuf>>>,
    saved_state: Option<&WorklistConversionState>,
) -> (Grid, Arc<Mutex<WorklistConversion>>)
where
    F: Fn() + 'static,
    G: Fn() + 'static + Clone,
{
    let (sender, receiver) = mpsc::channel();
    let worklist_conversion = if let Some(ss) = saved_state {
        WorklistConversion::from_state(ss, sender, worklist_dir_arc)
    } else {
        Arc::new(Mutex::new(WorklistConversion::new(
            sender,
            worklist_dir_arc,
        )))
    };

    let input_file_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Input Folder")
        .build();
    let input_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let input_button = Button::builder()
        .width_request(100)
        .hexpand(false)
        .label("Choose...")
        .build();

    let aetitle_label = Label::builder()
        .halign(gtk::Align::End)
        .label("AETitle")
        .build();
    let aetitle_entry = Entry::builder().hexpand(true).build();
    let modality_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Modality")
        .build();
    let modality_entry = Entry::builder().hexpand(true).build();

    if let Some(ss) = saved_state {
        if let Some(s) = &ss.input_dir_path {
            input_entry.buffer().set_text(s.to_str().unwrap_or(""));
        }
        if let Some(s) = &ss.aetitle {
            aetitle_entry.buffer().set_text(s);
        }
        if let Some(s) = &ss.modality {
            modality_entry.buffer().set_text(s);
        }
    }

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

    let remove_button = Button::builder()
        .width_request(100)
        .hexpand(false)
        .label("Remove worklist folder")
        .build();

    let grid_layout = Grid::builder()
        .column_spacing(12)
        .row_spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    grid_layout.attach(&input_file_label, 0, 0, 1, 1);
    grid_layout.attach(&input_entry, 1, 0, 2, 1);
    grid_layout.attach(&input_button, 3, 0, 1, 1);

    grid_layout.attach(&aetitle_label, 0, 1, 1, 1);
    grid_layout.attach(&aetitle_entry, 1, 1, 3, 1);
    grid_layout.attach(&modality_label, 0, 2, 1, 1);
    grid_layout.attach(&modality_entry, 1, 2, 3, 1);

    grid_layout.attach(&log_expander, 0, 3, 4, 1);
    grid_layout.attach(&remove_button, 3, 4, 1, 1);

    let on_updated2 = on_updated.clone();
    input_button.connect_clicked(clone!(
        #[weak]
        window,
        #[weak]
        input_entry,
        #[weak]
        worklist_conversion,
        move |_| {
            let dialog = FileDialog::builder().build();
            let on_updated2 = on_updated2.clone();
            dialog.select_folder(
                Some(&window),
                None::<gtk::gio::Cancellable>.as_ref(),
                clone!(
                    #[weak]
                    window,
                    #[weak]
                    input_entry,
                    #[weak]
                    worklist_conversion,
                    move |result| match result {
                        Err(err) => {
                            println!("err {:?}", err);
                        }
                        Ok(file) => {
                            if let Some(input_path) = file.path() {
                                if let Some(p) = input_path.to_str() {
                                    input_entry.buffer().set_text(p);
                                    if let std::sync::LockResult::Ok(mut wc) =
                                        worklist_conversion.lock()
                                    {
                                        let result = wc.set_input_dir_path(
                                            Some(PathBuf::from(p)),
                                            worklist_conversion.clone(),
                                        );
                                        match result {
                                            Ok(()) => {
                                                on_updated2();
                                            }
                                            Err(err) => {
                                                AlertDialog::builder()
                                                    .message("Error")
                                                    .detail(err.to_string())
                                                    .modal(true)
                                                    .build()
                                                    .show(Some(&window));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                ),
            );
        }
    ));

    let on_updated2 = on_updated.clone();
    aetitle_entry.connect_changed(clone!(
        #[weak]
        worklist_conversion,
        #[weak]
        aetitle_entry,
        move |_| {
            if let std::sync::LockResult::Ok(mut wc) = worklist_conversion.lock() {
                let text = aetitle_entry.buffer().text().as_str().to_string();
                wc.set_aetitle_string(text);
                on_updated2();
            };
        }
    ));

    let on_updated2 = on_updated.clone();
    modality_entry.connect_changed(clone!(
        #[weak]
        modality_entry,
        #[weak]
        worklist_conversion,
        move |_| {
            if let std::sync::LockResult::Ok(mut wc) = worklist_conversion.lock() {
                let text = modality_entry.buffer().text().as_str().to_string();
                wc.set_modality_string(text);
                on_updated2();
            };
        }
    ));

    let (asender, arecv) = async_channel::unbounded();
    runtime().spawn(async move {
        while let Ok(msg) = receiver.recv() {
            _ = asender.send(msg).await;
        }
    });

    spawn_future_local(clone!(
        #[weak]
        log_text_view,
        async move {
            while let Ok(msg) = arecv.recv().await {
                let buffer = log_text_view.buffer();
                buffer.insert(&mut buffer.end_iter(), &msg);
                buffer.insert(&mut buffer.end_iter(), "\n");
            }
        }
    ));

    remove_button.connect_clicked(clone!(
        #[weak]
        worklist_conversion,
        move |_| {
            on_delete();
            if let std::sync::LockResult::Ok(mut wc) = worklist_conversion.lock() {
                wc.unwatch_input_dir();
            };
        }
    ));

    return (grid_layout, worklist_conversion.clone());
}

type WorklistConversionsState = Vec<WorklistConversionState>;

#[derive(Clone, Serialize, Deserialize)]
struct StateFile {
    worklist_path: Option<PathBuf>,
    conversions: WorklistConversionsState,
    // Option for backward compatibility
    dicom_server: Option<DicomServerState>,
}

impl Default for StateFile {
    fn default() -> StateFile {
        StateFile {
            worklist_path: None,
            conversions: Vec::new(),
            dicom_server: Some(DicomServerState::default()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct DicomServerState {
    port: Option<u16>,
}

impl Default for DicomServerState {
    fn default() -> DicomServerState {
        DicomServerState { port: None }
    }
}

fn write_state_to_file(state: StateFile) -> Result<(), std::io::Error> {
    let state_string = json!(state).to_string();
    let mut current_path = std::env::current_exe()?;
    current_path.set_file_name("state.json");
    std::fs::write(current_path, state_string)?;
    Ok(())
}

fn read_saved_states() -> Result<StateFile, std::io::Error> {
    let mut current_path = std::env::current_exe()?;
    current_path.set_file_name("state.json");
    if !current_path.is_file() {
        return Ok(StateFile::default());
    }
    let data = std::fs::read(&current_path)?;

    Ok(
        serde_json::from_slice::<StateFile>(&data).unwrap_or_else(|err| {
            println!("Restore error {:?}", err);
            StateFile::default()
        }),
    )
}

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(50)
            .thread_name("gdt2dicom")
            .build()
            .expect("Setting up tokio runtime needs to succeed.")
    })
}
