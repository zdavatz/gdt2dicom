use std::default::Default;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

use gtk::glib::{clone, spawn_future_local};
use gtk::prelude::*;
use gtk::{glib, ApplicationWindow, Button, Frame, Grid};

use crate::gui::auto_convert::setup_auto_convert_ui;
use crate::gui::state::WorklistConversionsState;
use crate::worklist_conversion::{WorklistConversion, WorklistConversionState};

pub fn setup_auto_convert_list_ui(
    initial_state: &WorklistConversionsState,
    window: &ApplicationWindow,
    grid: &Grid,
    grid_y_index: i32,
    worklist_dir_arc: Arc<Mutex<Option<PathBuf>>>,
) -> (i32, mpsc::Receiver<WorklistConversionsState>) {
    let (state_sender, state_receiver) = mpsc::channel();
    let worklist_conversions: Arc<Mutex<Vec<Arc<Mutex<WorklistConversion>>>>> =
        Arc::new(Mutex::new(vec![]));

    let box1 = gtk::Box::new(gtk::Orientation::Vertical, 12);
    grid.attach(&box1, 0, grid_y_index, 4, 1);

    let on_updated = clone!(
        #[weak]
        worklist_conversions,
        move || {
            let state_sender1 = state_sender.clone();
            spawn_future_local(clone!(
                #[weak]
                worklist_conversions,
                async move {
                    let wcs = worklist_conversions.lock().unwrap();
                    let all_states: WorklistConversionsState = wcs
                        .iter()
                        .map(|arc| arc.lock().unwrap().to_state())
                        .collect();
                    _ = state_sender1.send(all_states);
                }
            ));
        }
    );

    let new_convertion_button = Button::builder().label("Add new worklist folder").build();
    let worklist_conversions1 = worklist_conversions.clone();
    let add_new_worklist = clone!(
        #[weak]
        window,
        #[weak]
        worklist_dir_arc,
        move |state: Option<&WorklistConversionState>| {
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

            let (this_ui, wc) = setup_auto_convert_ui(
                &window.clone(),
                on_delete,
                on_updated.clone(),
                worklist_dir_arc,
                state,
            );
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

    for state in initial_state {
        add_new_worklist(Some(state));
    }

    return (grid_y_index + 2, state_receiver);
}
