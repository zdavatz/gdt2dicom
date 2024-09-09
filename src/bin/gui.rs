#![windows_subsystem = "windows"]

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

use gdt2dicom::dcm_worklist::dcm_xml_to_worklist;
use gdt2dicom::dcm_xml::{default_dcm_xml, file_to_xml, DcmTransferType};
use gdt2dicom::gdt::{parse_file, GdtError};
use gdt2dicom::worklist_conversion::WorklistConversion;
use gtk::gio::prelude::FileExt;
use gtk::gio::{ActionEntry, ListStore, Menu};
use gtk::prelude::*;
use gtk::{
    glib, AboutDialog, AlertDialog, Application, ApplicationWindow, Button, Entry, FileDialog,
    FileFilter, Frame, Grid, Label, ScrolledWindow, Separator,
};

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
            .activate(|window: &ApplicationWindow, _, _| {
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
    return dcm_xml_to_worklist(None, &temp_file.path(), output_path).map_err(GdtError::IoError);
}

fn setup_auto_convert_list_ui(window: &ApplicationWindow, grid: &Grid, grid_y_index: i32) -> i32 {
    let conversation_scroll_window = ScrolledWindow::builder()
        // .width_request(300)
        .hexpand(true)
        .vexpand(true)
        .height_request(400)
        .build();
    grid.attach(&conversation_scroll_window, 0, grid_y_index, 4, 1);

    let box1 = gtk::Box::new(gtk::Orientation::Vertical, 12);
    conversation_scroll_window.set_child(Some(&box1));

    let new_convertion_button = Button::builder().label("Add new worklist folder").build();
    let w2 = window.clone();
    new_convertion_button.connect_clicked(move |_| {
        let frame = Frame::new(Some("Worklist folder"));
        let f = frame.clone();
        let box2 = box1.clone();
        let on_delete = move || {
            box2.remove(&f);
        };
        let this_ui = setup_auto_convert_ui(&w2.clone(), on_delete);
        frame.set_child(Some(&this_ui));
        box1.append(&frame);
    });
    grid.attach(&new_convertion_button, 3, grid_y_index + 1, 1, 1);
    return grid_y_index + 2;
}

fn setup_auto_convert_ui<F>(window: &ApplicationWindow, on_delete: F) -> Grid
where
    F: Fn() + 'static,
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

    grid_layout.attach(&log_text_view, 0, 4, 4, 1);
    grid_layout.attach(&remove_button, 3, 5, 1, 1);

    let w2 = window.clone();
    let input_entry2 = input_entry.clone();
    let wc1 = worklist_conversion.clone();
    input_button.connect_clicked(move |_| {
        let input_entry3 = input_entry2.clone();
        let dialog = FileDialog::builder().build();
        let wc2 = wc1.clone();
        let w3 = w2.clone();
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
                                    Ok(()) => {}
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
    output_button.connect_clicked(move |_| {
        let output_entry3 = output_entry2.clone();
        let dialog = FileDialog::builder().build();
        let wc4 = wc3.clone();
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
                            }
                        }
                    }
                }
            },
        );
    });

    let wc4 = worklist_conversion.clone();
    let ae = aetitle_entry.clone();
    aetitle_entry.connect_changed(move |_| {
        if let std::sync::LockResult::Ok(mut wc) = wc4.lock() {
            let text = ae.buffer().text().as_str().to_string();
            wc.set_aetitle_string(text);
        }
    });

    let wc5 = worklist_conversion.clone();
    let modality = modality_entry.clone();
    modality_entry.connect_changed(move |_| {
        if let std::sync::LockResult::Ok(mut wc) = wc5.lock() {
            let text = modality.buffer().text().as_str().to_string();
            wc.set_modality_string(text);
        }
    });

    let wc6 = worklist_conversion.clone();
    remove_button.connect_clicked(move |_| {
        on_delete();
        if let std::sync::LockResult::Ok(mut wc) = wc6.lock() {
            wc.unwatch_input_dir();
        }
    });

    return grid_layout;
}