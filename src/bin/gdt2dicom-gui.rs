#![windows_subsystem = "windows"]

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex, OnceLock};

use gdt2dicom::command::check_if_binary_exists;
use gdt2dicom::dcm_worklist::dcm_xml_to_worklist;
use gdt2dicom::dcm_xml::{default_dcm_xml, file_to_xml, DcmTransferType};
use gdt2dicom::gdt::{parse_file, GdtError};
use gdt2dicom::worklist_conversion::{WorklistConversion, WorklistConversionState};
use gtk::gio::prelude::FileExt;
use gtk::gio::{ActionEntry, ListStore, Menu};
use gtk::glib::{clone, spawn_future_local};
use gtk::prelude::*;
use gtk::{
    glib, AboutDialog, AlertDialog, Application, ApplicationWindow, Button, Entry, Expander,
    FileDialog, FileFilter, Frame, Grid, Label, ScrolledWindow, Separator, TextView, Window,
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

    let worklist_dir_label = Label::new(Some("DICOM worklist dir"));
    let worklist_dir_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let worklist_dir_button = Button::builder().label("Choose...").build();

    let port_label = Label::new(Some("Port"));
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
    grid_layout.attach(&worklist_dir_label, 0, 0, 1, 1);
    grid_layout.attach(&worklist_dir_entry, 1, 0, 1, 1);
    grid_layout.attach(&worklist_dir_button, 2, 0, 1, 1);
    grid_layout.attach(&port_label, 0, 1, 1, 1);
    grid_layout.attach(&port_entry, 1, 1, 1, 1);
    grid_layout.attach(&run_button, 0, 2, 1, 1);
    grid_layout.attach(&status_label, 1, 2, 1, 1);
    grid_layout.attach(&log_expander, 0, 3, 3, 1);

    grid.attach(&frame, 0, grid_y_index, 4, 1);
    return grid_y_index + 1;
}

fn setup_auto_convert_list_ui(window: &ApplicationWindow, grid: &Grid, grid_y_index: i32) -> i32 {
    let worklist_conversions: Arc<Mutex<Vec<Arc<Mutex<WorklistConversion>>>>> =
        Arc::new(Mutex::new(vec![]));
    let conversion_scroll_window = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .height_request(400)
        .build();
    grid.attach(&conversion_scroll_window, 0, grid_y_index, 4, 1);

    let box1 = gtk::Box::new(gtk::Orientation::Vertical, 12);
    conversion_scroll_window.set_child(Some(&box1));

    let wcs1 = worklist_conversions.clone();
    let on_updated = move || {
        let wcs1 = wcs1.clone();
        spawn_future_local(async move {
            let wcs = wcs1.lock().unwrap();
            let all_states: Vec<WorklistConversionState> = wcs
                .iter()
                .map(|arc| arc.lock().unwrap().to_state())
                .collect();
            _ = write_state_to_file(all_states);
        });
    };

    let new_convertion_button = Button::builder().label("Add new worklist folder").build();
    let wcs2 = worklist_conversions.clone();
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
            let mut cs = wcs2.lock().unwrap();
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

    let input_file_label = Label::new(Some("Input Folder"));
    let input_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let input_button = Button::builder()
        .width_request(100)
        .hexpand(false)
        .label("Choose...")
        .build();

    let output_file_label = Label::new(Some("Output Folder"));
    let output_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let output_button = Button::builder()
        .width_request(100)
        .hexpand(false)
        .label("Choose...")
        .build();

    let aetitle_label = Label::new(Some("AETitle"));
    let aetitle_entry = Entry::builder().hexpand(true).build();
    let modality_label = Label::new(Some("Modality"));
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

    let w2 = window.clone();
    let input_entry2 = input_entry.clone();
    let wc1 = worklist_conversion.clone();
    let on_updated2 = on_updated.clone();
    input_button.connect_clicked(move |_| {
        let input_entry3 = input_entry2.clone();
        let dialog = FileDialog::builder().build();
        let wc2 = wc1.clone();
        let w3 = w2.clone();
        let on_updated2 = on_updated2.clone();
        dialog.select_folder(
            Some(&w2),
            None::<gtk::gio::Cancellable>.as_ref(),
            move |result| match result {
                Err(err) => {
                    println!("err {:?}", err);
                }
                Ok(file) => {
                    if let Some(input_path) = file.path() {
                        if let Some(p) = input_path.to_str() {
                            input_entry3.buffer().set_text(p);
                            if let std::sync::LockResult::Ok(mut wc) = wc2.lock() {
                                let wc3 = wc2.clone();
                                let result = wc.set_input_dir_path(Some(PathBuf::from(p)), wc3);
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
                                            .show(Some(&w3));
                                    }
                                }
                            }
                        }
                    }
                }
            },
        );
    });

    let w3 = window.clone();
    let output_entry2 = output_entry.clone();
    let wc3 = worklist_conversion.clone();
    let on_updated2 = on_updated.clone();
    output_button.connect_clicked(move |_| {
        let output_entry3 = output_entry2.clone();
        let dialog = FileDialog::builder().build();
        let wc4 = wc3.clone();
        let on_updated2 = on_updated2.clone();
        dialog.select_folder(
            Some(&w3),
            None::<gtk::gio::Cancellable>.as_ref(),
            move |result| match result {
                Err(err) => {
                    println!("err {:?}", err);
                }
                Ok(file) => {
                    if let Some(input_path) = file.path() {
                        if let Some(p) = input_path.to_str() {
                            output_entry3.buffer().set_text(p);
                            if let std::sync::LockResult::Ok(mut wc) = wc4.lock() {
                                wc.output_dir_path = Some(PathBuf::from(p));
                                on_updated2();
                            }
                        }
                    }
                }
            },
        );
    });

    let wc4 = worklist_conversion.clone();
    let ae = aetitle_entry.clone();
    let on_updated2 = on_updated.clone();
    aetitle_entry.connect_changed(move |_| {
        if let std::sync::LockResult::Ok(mut wc) = wc4.lock() {
            let text = ae.buffer().text().as_str().to_string();
            wc.set_aetitle_string(text);
            on_updated2();
        }
    });

    let wc5 = worklist_conversion.clone();
    let on_updated2 = on_updated.clone();
    modality_entry.connect_changed(clone!(
        #[weak]
        modality_entry,
        move |_| {
            if let std::sync::LockResult::Ok(mut wc) = wc5.lock() {
                let text = modality_entry.buffer().text().as_str().to_string();
                wc.set_modality_string(text);
                on_updated2();
            }
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

    let wc6 = worklist_conversion.clone();
    remove_button.connect_clicked(move |_| {
        on_delete();
        if let std::sync::LockResult::Ok(mut wc) = wc6.lock() {
            wc.unwatch_input_dir();
        }
    });

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
