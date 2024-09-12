#![windows_subsystem = "windows"]

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex, OnceLock};

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
    FileDialog, FileFilter, Frame, Grid, Label, ScrolledWindow, Separator, TextView,
};
use serde_json::json;
use tokio::runtime::Runtime;

fn main() -> glib::ExitCode {
    let application = Application::builder()
        .application_id("ch.ywesee.gdt2dicom")
        .build();

    application.connect_activate(|app| {
        let menubar = Menu::new();

        let file_menu = Menu::new();
        file_menu.append(Some("About"), Some("win.open-about"));
        menubar.append_submenu(Some("Help"), &file_menu);
        app.set_menubar(Some(&menubar));

        let window = ApplicationWindow::builder()
            .application(app)
            .title("gdt2dicom")
            .default_width(350)
            .default_height(70)
            .show_menubar(true)
            .build();

        let action_close = ActionEntry::builder("open-about")
            .activate(|_window: &ApplicationWindow, _, _| {
                open_about_dialog();
            })
            .build();
        window.add_action_entries([action_close]);

        let grid_layout = Grid::builder()
            .column_spacing(12)
            .row_spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        // let y = setup_simple_convert(&window, &grid_layout.clone(), 0);
        setup_auto_convert_list_ui(&window.clone(), &grid_layout.clone(), 0);

        window.set_child(Some(&grid_layout));
        window.present();
    });

    return application.run();
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

fn setup_simple_convert(window: &ApplicationWindow, grid: &Grid, grid_y_index: i32) -> i32 {
    let input_file_label = Label::new(Some("Input File"));
    let input_entry = Entry::builder().hexpand(true).build();
    let output_file_label = Label::new(Some("Output File"));
    let output_entry = Entry::builder().hexpand(true).build();

    grid.attach(&input_file_label, 0, 0, 1, 1);
    grid.attach(&input_entry, 1, 0, 2, 1);
    grid.attach(&output_file_label, 0, 1, 1, 1);
    grid.attach(&output_entry, 1, 1, 2, 1);

    let input_button = Button::builder().label("Choose GDT file...").build();

    let w2 = window.clone();
    let input_entry2 = input_entry.clone();
    input_button.connect_clicked(move |_| {
        let input_entry3 = input_entry2.clone();
        let ff = FileFilter::new();
        ff.add_suffix("gdt");

        let filters = ListStore::new::<FileFilter>();
        filters.append(&ff);
        let dialog = FileDialog::builder().filters(&filters).build();
        dialog.open(
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
                        }
                    }
                }
            },
        );
    });
    grid.attach(&input_button, 3, grid_y_index, 1, 1);

    let output_button = Button::builder().label("Choose WL path...").build();

    let w3 = window.clone();
    let output_entry2 = output_entry.clone();
    output_button.connect_clicked(move |_| {
        let output_entry3 = output_entry2.clone();
        let ff = FileFilter::new();
        ff.add_suffix("wl");

        let filters = ListStore::new::<FileFilter>();
        filters.append(&ff);
        let dialog = FileDialog::builder().filters(&filters).build();
        dialog.save(
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
                        }
                    }
                }
            },
        );
    });
    grid.attach(&output_button, 3, grid_y_index + 1, 1, 1);

    let run_button = Button::builder().label("Run").build();
    let input_entry4 = input_entry.clone();
    let output_entry4 = output_entry.clone();
    let w4 = window.clone();
    run_button.connect_clicked(move |_| {
        let input_text = input_entry4.buffer().text();
        let output_text = output_entry4.buffer().text();
        let input_path = Path::new(input_text.as_str());
        let output_path = PathBuf::from(output_text.as_str());
        let result = convert_gdt_file(&input_path, &output_path);
        if let Err(err) = result {
            AlertDialog::builder()
                .message("Error")
                .detail(err.to_string())
                .modal(true)
                .build()
                .show(Some(&w4));
        } else {
            AlertDialog::builder()
                .message("Success!")
                .detail("File written")
                .modal(true)
                .build()
                .show(Some(&w4));
        }
    });
    grid.attach(&run_button, 3, grid_y_index + 2, 1, 1);

    let separator = Separator::new(gtk::Orientation::Horizontal);
    grid.attach(&separator, 0, grid_y_index + 3, 4, 1);

    return grid_y_index + 4;
}

fn convert_gdt_file(input_path: &Path, output_path: &PathBuf) -> Result<(), GdtError> {
    let gdt_file = parse_file(input_path)?;
    let xml_events = default_dcm_xml(DcmTransferType::LittleEndianExplicit);
    let temp_file = file_to_xml(gdt_file, &xml_events).unwrap();
    return Ok(dcm_xml_to_worklist(None, &temp_file.path(), output_path)?);
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
    new_convertion_button.connect_clicked(clone!(
        #[weak]
        window,
        move |_| {
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
                setup_auto_convert_ui(&window.clone(), on_delete, on_updated.clone(), None);
            let mut cs = wcs2.lock().unwrap();
            cs.push(wc);
            frame.set_child(Some(&this_ui));
            box1.append(&frame);
        }
    ));
    grid.attach(&new_convertion_button, 3, grid_y_index + 1, 1, 1);
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
    let worklist_conversion = Arc::new(Mutex::new(WorklistConversion::new(sender)));
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
    let modality = modality_entry.clone();
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
                // println!("async {msg}");
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

fn write_state_to_file(
    worklist_conversions: Vec<WorklistConversionState>,
) -> Result<(), std::io::Error> {
    let state_string = json!({
        "conversions": worklist_conversions
    })
    .to_string();
    let mut current_path = std::env::current_exe()?;
    current_path.set_file_name("state.json");
    let mut f = File::create(current_path)?;
    f.write_all(state_string.as_bytes())?;
    Ok(())
}

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("Setting up tokio runtime needs to succeed."))
}
