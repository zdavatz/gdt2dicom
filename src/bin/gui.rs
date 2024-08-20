// use gtk4 as gtk;
use gdt2dicom::dcm_worklist::dcm_xml_to_worklist;
use gdt2dicom::dcm_xml::{default_dcm_xml, file_to_xml, parse_dcm_xml, DcmTransferType};
use gdt2dicom::gdt::{parse_file, GdtError};
use gtk::gio::prelude::FileExt;
use gtk::gio::ListStore;
use gtk::prelude::*;
use gtk::{
    glib, AlertDialog, Application, ApplicationWindow, Button, Entry, FileDialog, FileFilter, Grid,
    Label,
};
use std::path::{Path, PathBuf};

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
