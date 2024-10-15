use std::default::Default;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gdt2dicom::command::check_if_binary_exists;
use gtk::gio::{ActionEntry, Menu};
use gtk::glib::clone;
use gtk::prelude::*;
use gtk::{glib, AlertDialog, Application, ApplicationWindow, Grid, Separator};

use gdt2dicom::gui::about_dialog::open_about_dialog;
use gdt2dicom::gui::auto_convert_list::setup_auto_convert_list_ui;
use gdt2dicom::gui::copyright_dialog::open_copyright_dialog;
use gdt2dicom::gui::cstore_server::setup_cstore_server;
use gdt2dicom::gui::dicom_server::setup_dicom_server;
use gdt2dicom::gui::runtime;
use gdt2dicom::gui::state::{
    read_saved_states, write_state_to_file, CStoreServerState, DicomServerState, StateFile,
};
use gdt2dicom::gui::worklist_folder::setup_worklist_folder_ui;

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

        // TODO: make dicom_server and cstore_server non-optional when user upgraded from old save data
        let dicom_server_state = &saved_state
            .dicom_server
            .clone()
            .unwrap_or(DicomServerState::default());

        let cstore_server_state = &saved_state
            .cstore_server
            .clone()
            .unwrap_or(CStoreServerState::default());

        let state_arc = Arc::new(Mutex::new(saved_state.clone()));

        let state_arc1 = state_arc.clone();
        let on_worklist_path_updated = move |new_path| {
            let mut state = state_arc1.lock().unwrap();
            let new_state = StateFile {
                worklist_path: new_path,
                ..state.deref().clone()
            };
            _ = write_state_to_file(&new_state);
            *state = new_state;
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
        let (y, convert_list_state_receiver) = setup_auto_convert_list_ui(
            &saved_state.conversions,
            &window.clone(),
            &grid_layout.clone(),
            y,
            worklist_dir_arc.clone(),
        );
        let (_y, cstore_server_state_receiver) = setup_cstore_server(
            &cstore_server_state,
            &window.clone(),
            &grid_layout.clone(),
            y,
        );

        let state_arc1 = state_arc.clone();
        runtime().spawn(async move {
            while let Ok(dicom_server_state) = dicom_server_state_receiver.recv() {
                let mut state = state_arc1.lock().unwrap();
                let new_state = StateFile {
                    dicom_server: Some(dicom_server_state),
                    ..state.deref().clone()
                };
                _ = write_state_to_file(&new_state);
                *state = new_state;
            }
        });

        let state_arc1 = state_arc.clone();
        runtime().spawn(async move {
            while let Ok(convert_list_state) = convert_list_state_receiver.recv() {
                let mut state = state_arc1.lock().unwrap();
                let new_state = StateFile {
                    conversions: convert_list_state,
                    ..state.deref().clone()
                };
                _ = write_state_to_file(&new_state);
                *state = new_state;
            }
        });

        let state_arc1 = state_arc.clone();
        runtime().spawn(async move {
            while let Ok(cstore_server_state) = cstore_server_state_receiver.recv() {
                let mut state = state_arc1.lock().unwrap();
                let new_state = StateFile {
                    cstore_server: Some(cstore_server_state),
                    ..state.deref().clone()
                };
                _ = write_state_to_file(&new_state);
                *state = new_state;
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
