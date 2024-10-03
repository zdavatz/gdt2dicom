#![windows_subsystem = "windows"]

use std::io::BufRead;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use std::sync::{mpsc, Arc, Mutex, OnceLock};

use gdt2dicom::command::{binary_to_path, check_if_binary_exists};
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

        let y = 0;
        let y = setup_dicom_server(&window, &grid_layout.clone(), y);
        setup_auto_convert_list_ui(&window.clone(), &grid_layout.clone(), y);

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

fn setup_dicom_server(window: &ApplicationWindow, grid: &Grid, grid_y_index: i32) -> i32 {
    let frame = Frame::builder()
        .label("DICOM worklist server")
        .vexpand(false)
        .build();

    let worklist_dir_label = Label::builder()
        .halign(gtk::Align::End)
        .label("DICOM worklist dir")
        .build();
    let worklist_dir_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let worklist_dir_button = Button::builder().label("Choose...").build();

    let port_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Port")
        .build();
    let port_entry = Entry::builder().hexpand(true).build();
    let run_button = Button::builder().label("Run").sensitive(false).build();
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
    grid_layout.attach(&worklist_dir_label, 0, 0, 1, 1);
    grid_layout.attach(&worklist_dir_entry, 1, 0, 1, 1);
    grid_layout.attach(&worklist_dir_button, 2, 0, 1, 1);
    grid_layout.attach(&port_label, 0, 1, 1, 1);
    grid_layout.attach(&port_entry, 1, 1, 1, 1);
    grid_layout.attach(&status_label, 2, 1, 1, 1);
    grid_layout.attach(&run_button, 0, 2, 1, 1);
    grid_layout.attach(&log_expander, 0, 3, 3, 1);

    grid.attach(&frame, 0, grid_y_index, 4, 1);

    let update_run_button = clone!(
        #[weak]
        worklist_dir_entry,
        #[weak]
        port_entry,
        #[weak]
        run_button,
        move || {
            let worklist_dir = worklist_dir_entry.buffer().text();
            let port_str = port_entry.buffer().text();
            let port_int = u16::from_str(port_str.as_str());
            if worklist_dir.len() > 0 && port_int.is_ok() {
                run_button.set_sensitive(true);
            } else {
                run_button.set_sensitive(false);
            }
        }
    );

    let update_run_button1 = update_run_button.clone();
    worklist_dir_button.connect_clicked(clone!(
        #[weak]
        window,
        #[weak]
        worklist_dir_entry,
        move |_| {
            let update_run_button2 = update_run_button1.clone();
            let dialog = FileDialog::builder().build();
            dialog.select_folder(
                Some(&window),
                None::<gtk::gio::Cancellable>.as_ref(),
                move |result| match result {
                    Err(err) => {
                        println!("err {:?}", err);
                    }
                    Ok(file) => {
                        if let Some(path) = file.path() {
                            if let Some(p) = path.to_str() {
                                worklist_dir_entry.buffer().set_text(p);
                                update_run_button2();
                            }
                        }
                    }
                },
            );
        }
    ));

    port_entry
        .delegate()
        .unwrap()
        .connect_insert_text(move |entry, text, position| {
            let pattern = |c: char| -> bool { !c.is_ascii_digit() };
            if text.contains(pattern) {
                glib::signal::signal_stop_emission_by_name(entry, "insert-text");
                entry.insert_text(&text.replace(pattern, ""), position);
            }
        });

    let update_run_button1 = update_run_button.clone();
    port_entry.connect_changed(move |_| {
        update_run_button1();
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
        worklist_dir_entry,
        #[weak]
        port_entry,
        #[weak]
        log_text_view,
        move |_| {
            let worklist_dir = worklist_dir_entry.buffer().text();
            if worklist_dir.len() == 0 {
                return;
            }
            let port_str = port_entry.buffer().text();
            let port_int = match u16::from_str(port_str.as_str()) {
                Err(_) => return,
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
                let mut command = Command::new(full_path);
                command
                    .args(vec![
                        "-v",
                        "-d",
                        "-dfr",
                        "-dfp",
                        worklist_dir.as_str(),
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

    return grid_y_index + 1;
}

fn setup_auto_convert_list_ui(window: &ApplicationWindow, grid: &Grid, grid_y_index: i32) -> i32 {
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
            spawn_future_local(clone!(
                #[weak]
                worklist_conversions,
                async move {
                    let wcs = worklist_conversions.lock().unwrap();
                    let all_states: Vec<WorklistConversionState> = wcs
                        .iter()
                        .map(|arc| arc.lock().unwrap().to_state())
                        .collect();
                    _ = write_state_to_file(all_states);
                }
            ));
        }
    );

    let new_convertion_button = Button::builder().label("Add new worklist folder").build();
    let worklist_conversions1 = worklist_conversions.clone();
    let add_new_worklist = clone!(
        #[weak]
        window,
        move |state: Option<WorklistConversionState>| {
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

            let (this_ui, wc) =
                setup_auto_convert_ui(&window.clone(), on_delete, on_updated.clone(), state);
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

    match read_saved_states() {
        Err(err) => {
            println!("Error while restoring state: {:?}", err);
        }
        Ok(saved_states) => {
            for state in saved_states {
                add_new_worklist(Some(state));
            }
        }
    }

    return grid_y_index + 2;
}

fn setup_auto_convert_ui<F, G>(
    window: &ApplicationWindow,
    on_delete: F,
    on_updated: G,
    saved_state: Option<WorklistConversionState>,
) -> (Grid, Arc<Mutex<WorklistConversion>>)
where
    F: Fn() + 'static,
    G: Fn() + 'static + Clone,
{
    let (sender, receiver) = mpsc::channel();
    let worklist_conversion = if let Some(ref ss) = saved_state {
        WorklistConversion::from_state(&ss, sender)
    } else {
        Arc::new(Mutex::new(WorklistConversion::new(sender)))
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

    let output_file_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Output Folder")
        .build();
    let output_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let output_button = Button::builder()
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
        if let Some(s) = &ss.output_dir_path {
            output_entry.buffer().set_text(s.to_str().unwrap_or(""));
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

    grid_layout.attach(&output_file_label, 0, 1, 1, 1);
    grid_layout.attach(&output_entry, 1, 1, 2, 1);
    grid_layout.attach(&output_button, 3, 1, 1, 1);

    grid_layout.attach(&aetitle_label, 0, 2, 1, 1);
    grid_layout.attach(&aetitle_entry, 1, 2, 3, 1);
    grid_layout.attach(&modality_label, 0, 3, 1, 1);
    grid_layout.attach(&modality_entry, 1, 3, 3, 1);

    grid_layout.attach(&log_expander, 0, 4, 4, 1);
    grid_layout.attach(&remove_button, 3, 5, 1, 1);

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
    output_button.connect_clicked(clone!(
        #[weak]
        window,
        #[weak]
        output_entry,
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
                    output_entry,
                    move |result| match result {
                        Err(err) => {
                            println!("err {:?}", err);
                        }
                        Ok(file) => {
                            if let Some(input_path) = file.path() {
                                if let Some(p) = input_path.to_str() {
                                    output_entry.buffer().set_text(p);
                                    if let std::sync::LockResult::Ok(mut wc) =
                                        worklist_conversion.lock()
                                    {
                                        wc.output_dir_path = Some(PathBuf::from(p));
                                        on_updated2();
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

#[derive(Serialize, Deserialize)]
struct StateFile {
    conversions: Vec<WorklistConversionState>,
}

fn write_state_to_file(
    worklist_conversions: Vec<WorklistConversionState>,
) -> Result<(), std::io::Error> {
    let state_string = json!(StateFile {
        conversions: worklist_conversions
    })
    .to_string();
    let mut current_path = std::env::current_exe()?;
    current_path.set_file_name("state.json");
    std::fs::write(current_path, state_string)?;
    Ok(())
}

fn read_saved_states() -> Result<Vec<WorklistConversionState>, std::io::Error> {
    let mut current_path = std::env::current_exe()?;
    current_path.set_file_name("state.json");
    if !current_path.is_file() {
        return Ok(Vec::new());
    }
    let data = std::fs::read(&current_path)?;

    match serde_json::from_slice::<StateFile>(&data) {
        Ok(v) => Ok(v.conversions),
        Err(err) => {
            AlertDialog::builder()
                .message("Cannot read saved data")
                .detail(err.to_string())
                .modal(true)
                .build()
                .show(None::<&ApplicationWindow>);
            Ok(Vec::new())
        }
    }
}

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("Setting up tokio runtime needs to succeed."))
}
