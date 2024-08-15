// use gtk4 as gtk;
use gdt2dicom::dcm_worklist::dcm_xml_to_worklist;
use gdt2dicom::dcm_xml::{default_dcm_xml, file_to_xml, parse_dcm_xml, DcmTransferType};
use gtk::gio::prelude::FileExt;
use gtk::gio::ListStore;
use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow, Button, FileDialog, FileFilter};
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

        let button = Button::builder()
            .label("Press me!")
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let w2 = window.clone();
        button.connect_clicked(move |_| {
            eprintln!("Clicked!");

            let ff = FileFilter::new();
            ff.add_suffix("gdt");

            let filters = ListStore::new::<FileFilter>();
            filters.append(&ff);

            let dialog = FileDialog::builder().filters(&filters).build();

            let w3 = w2.clone();
            dialog.open(
                Some(&w2),
                None::<gtk::gio::Cancellable>.as_ref(),
                move |result| {
                    match result {
                        Err(err) => {
                            println!("err {:?}", err);
                        }
                        Ok(file) => {
                            if let Some(inputPath) = file.path() {
                                let saveFF = FileFilter::new();
                                saveFF.add_suffix("wl");
                                let saveFilters = ListStore::new::<FileFilter>();
                                saveFilters.append(&saveFF);
                                let saveDialog =
                                    FileDialog::builder().filters(&saveFilters).build();
                                saveDialog.save(
                                    Some(&w3),
                                    None::<gtk::gio::Cancellable>.as_ref(),
                                    move |result| match result {
                                        Err(err) => {
                                            println!("err {:?}", err);
                                        }
                                        Ok(file) => {
                                            if let Some(outputPath) = file.path() {
                                                convertGDTFile(&inputPath, &outputPath);
                                            }
                                        }
                                    },
                                )
                            }
                        }
                    }
                    eprintln!("Back from open");
                },
            );
        });
        window.set_child(Some(&button));

        window.present();
    });

    application.run()
}

fn convertGDTFile(inputPath: &Path, outputPath: &PathBuf) {
    let gdt_file = gdt2dicom::gdt::parse_file(inputPath).unwrap();
    let dicom_xml_path: Option<PathBuf> = None;
    let xml_events = match dicom_xml_path {
        Some(p) => parse_dcm_xml(&p).expect("Expecting a good xml file."),
        _ => default_dcm_xml(DcmTransferType::LittleEndianExplicit),
    };
    let temp_file = file_to_xml(gdt_file, &xml_events).unwrap();
    dcm_xml_to_worklist(&temp_file.path(), outputPath);
}
