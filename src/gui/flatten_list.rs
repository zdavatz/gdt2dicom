use std::default::Default;
use std::sync::{mpsc, Arc, Mutex};

use gtk::glib::{clone, spawn_future_local};
use gtk::prelude::*;
use gtk::{glib, ApplicationWindow, Button, Frame, Grid};

use crate::folder_flatten::{FolderFlatten, FolderFlattenState};
use crate::gui::flatten::setup_flatten_ui;
use crate::gui::state::FolderFlattensState;

pub fn setup_flatten_list_ui(
    initial_state: &FolderFlattensState,
    window: &ApplicationWindow,
    grid: &Grid,
    grid_y_index: i32,
) -> (i32, mpsc::Receiver<FolderFlattensState>) {
    let (state_sender, state_receiver) = mpsc::channel();
    let folder_flattens: Arc<Mutex<Vec<Arc<Mutex<FolderFlatten>>>>> = Arc::new(Mutex::new(vec![]));

    let box1 = gtk::Box::new(gtk::Orientation::Vertical, 12);
    grid.attach(&box1, 0, grid_y_index, 4, 1);

    let on_updated = clone!(
        #[weak]
        folder_flattens,
        move || {
            let state_sender1 = state_sender.clone();
            spawn_future_local(clone!(
                #[weak]
                folder_flattens,
                async move {
                    let ffs = folder_flattens.lock().unwrap();
                    let all_states: FolderFlattensState = ffs
                        .iter()
                        .map(|arc| arc.lock().unwrap().to_state())
                        .collect();
                    _ = state_sender1.send(all_states);
                }
            ));
        }
    );

    let new_flatten_button = Button::builder().label("Add new flatten folder").build();
    let folder_flattens1 = folder_flattens.clone();
    let add_new_flatten = clone!(
        #[weak]
        window,
        move |state: Option<&FolderFlattenState>| {
            let frame = Frame::new(Some("Flatten folder"));
            let on_delete = clone!(
                #[weak]
                box1,
                #[weak]
                frame,
                move || {
                    box1.remove(&frame);
                }
            );

            let (this_ui, ff) =
                setup_flatten_ui(&window.clone(), on_delete, on_updated.clone(), state);
            let mut fs = folder_flattens1.lock().unwrap();
            fs.push(ff);
            frame.set_child(Some(&this_ui));
            box1.append(&frame);
        }
    );
    let add_new_flatten2 = add_new_flatten.clone();
    new_flatten_button.connect_clicked(move |_| {
        add_new_flatten2(None);
    });
    grid.attach(&new_flatten_button, 3, grid_y_index + 1, 1, 1);

    for state in initial_state {
        add_new_flatten(Some(state));
    }

    return (grid_y_index + 2, state_receiver);
}
