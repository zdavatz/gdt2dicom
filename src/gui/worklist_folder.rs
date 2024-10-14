use std::default::Default;
use std::fs::{read_dir, File};
use std::io::Error as IoError;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gtk::gio::prelude::FileExt;
use gtk::glib::clone;
use gtk::prelude::*;
use gtk::{glib, ApplicationWindow, Button, Entry, FileDialog, Grid, Label};

pub fn setup_worklist_folder_ui<F>(
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
