// use gtk4 as gtk;
use gdt2dicom::dcm_worklist::dcm_xml_to_worklist;
use gdt2dicom::dcm_xml::{default_dcm_xml, file_to_xml, parse_dcm_xml, DcmTransferType};
use gdt2dicom::gdt::{parse_file, GdtError};
use gdt2dicom::worklist_conversion::WorklistConversion;
use gtk::gio::prelude::FileExt;
use gtk::gio::ListStore;
use gtk::prelude::*;
use gtk::{
    glib, AlertDialog, Application, ApplicationWindow, Button, Entry, FileDialog, FileFilter,
    Frame, Grid, Label, ScrolledWindow, Separator,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

fn main() -> glib::ExitCode {
    let application = Application::builder()
        .application_id("com.example.FirstGtkApp")
        .build();

    application.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("First GTK Program")
            .default_width(350)
            .default_height(70)
            .build();

        let input_file_label = Label::new(Some("Input File"));
        let input_entry = Entry::new();
        let output_file_label = Label::new(Some("Output File"));
        let output_entry = Entry::new();

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
        grid_layout.attach(&output_file_label, 0, 1, 1, 1);
        grid_layout.attach(&output_entry, 1, 1, 2, 1);

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
                move |result| {
                    match result {
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
                    }
                    eprintln!("Back from open");
                },
            );
        });
        grid_layout.attach(&input_button, 3, 0, 1, 1);

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
                move |result| {
                    match result {
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
                    }
                    eprintln!("Back from save");
                },
            );
        });
        grid_layout.attach(&output_button, 3, 1, 1, 1);

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
        grid_layout.attach(&run_button, 3, 2, 1, 1);

        let separator = Separator::new(gtk::Orientation::Horizontal);
        grid_layout.attach(&separator, 0, 3, 4, 1);

        setup_auto_convert_list_ui(&window.clone(), &grid_layout.clone(), 4);

        window.set_child(Some(&grid_layout));
        window.present();
    });

    return application.run();
}

fn convert_gdt_file(input_path: &Path, output_path: &PathBuf) -> Result<(), GdtError> {
    let gdt_file = parse_file(input_path)?;
    let dicom_xml_path: Option<PathBuf> = None;
    let xml_events = match dicom_xml_path {
        Some(p) => parse_dcm_xml(&p).expect("Expecting a good xml file."),
        _ => default_dcm_xml(DcmTransferType::LittleEndianExplicit),
    };
    let temp_file = file_to_xml(gdt_file, &xml_events).unwrap();
    return dcm_xml_to_worklist(&temp_file.path(), output_path).map_err(GdtError::IoError);
}

fn setup_auto_convert_list_ui(window: &ApplicationWindow, grid: &Grid, grid_y_index: i32) -> i32 {
    let conversions: Arc<Mutex<Vec<WorklistConversion>>> = Arc::new(Mutex::new(vec![]));

    let conversation_scroll_window = ScrolledWindow::builder().height_request(400).build();
    grid.attach(&conversation_scroll_window, 0, grid_y_index, 4, 1);

    let box1 = gtk::Box::new(gtk::Orientation::Vertical, 12);
    conversation_scroll_window.set_child(Some(&box1));

    let new_convertion_button = Button::builder().label("Add new worklist folder").build();
    let w2 = window.clone();
    new_convertion_button.connect_clicked(move |_| {
        // let mut cs = conversions.lock().unwrap();
        let frame = Frame::new(Some("Worklist folder"));
        let uuid = Uuid::new_v4();
        let on_delete = || {};
        frame.set_child(Some(&setup_auto_convert_ui(&w2.clone(), uuid, on_delete)));
        box1.append(&frame);
        // cs.push();
    });
    grid.attach(&new_convertion_button, 3, grid_y_index + 1, 1, 1);
    return grid_y_index + 2;
}

fn setup_auto_convert_ui<F>(window: &ApplicationWindow, uuid: Uuid, on_delete: F) -> Grid
where
    F: Fn() + 'static,
{
    let worklist_conversion = Arc::new(Mutex::new(WorklistConversion {
        uuid: uuid,
        input_dir_path: None,
        output_dir_path: None,
        aetitle: "".to_string(),
        modality: "".to_string(),
    }));
    let input_file_label = Label::new(Some("Input File"));
    let input_entry = Entry::builder().sensitive(false).build();
    let input_button = Button::builder().label("Choose GDT file...").build();

    let output_file_label = Label::new(Some("Output File"));
    let output_entry = Entry::builder().sensitive(false).build();
    let output_button = Button::builder().label("Choose WL path...").build();

    let aetitle_label = Label::new(Some("AETitle"));
    let aetitle_entry = Entry::new();
    let modality_label = Label::new(Some("Modality"));
    let modality_entry = Entry::new();

    let remove_button = Button::builder().label("Remove worklist folder").build();

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

    grid_layout.attach(&remove_button, 3, 4, 1, 1);

    // TODO: update filter, select dir only
    let w2 = window.clone();
    let input_entry2 = input_entry.clone();
    let wc1 = worklist_conversion.clone();
    input_button.connect_clicked(move |_| {
        let input_entry3 = input_entry2.clone();
        let dialog = FileDialog::builder().build();
        let wc2 = wc1.clone();
        dialog.select_folder(
            Some(&w2),
            None::<gtk::gio::Cancellable>.as_ref(),
            move |result| {
                match result {
                    Err(err) => {
                        println!("err {:?}", err);
                    }
                    Ok(file) => {
                        if let Some(input_path) = file.path() {
                            if let Some(p) = input_path.to_str() {
                                input_entry3.buffer().set_text(p);
                                if let std::sync::LockResult::Ok(mut wc) = wc2.lock() {
                                    wc.input_dir_path = Some(PathBuf::from(p));
                                }
                            }
                        }
                    }
                }
                eprintln!("Back from open");
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
            move |result| {
                match result {
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
                }
                eprintln!("Back from save");
            },
        );
    });

    remove_button.connect_clicked(move |_| {
        on_delete();
    });

    return grid_layout;
}
