use std::default::Default;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

use gtk::gio::prelude::FileExt;
use gtk::glib::{clone, spawn_future_local};
use gtk::prelude::*;
use gtk::{
    glib, AlertDialog, ApplicationWindow, Button, Entry, Expander, FileDialog, Grid, Label,
    ScrolledWindow, TextView,
};

use crate::worklist_conversion::{WorklistConversion, WorklistConversionState};

use crate::gui::runtime;

pub fn setup_auto_convert_ui<F, G>(
    window: &ApplicationWindow,
    on_delete: F,
    on_updated: G,
    worklist_dir_arc: Arc<Mutex<Option<PathBuf>>>,
    saved_state: Option<&WorklistConversionState>,
) -> (Grid, Arc<Mutex<WorklistConversion>>)
where
    F: Fn() + 'static,
    G: Fn() + 'static + Clone,
{
    let (sender, receiver) = mpsc::channel();
    let worklist_conversion = if let Some(ss) = saved_state {
        WorklistConversion::from_state(ss, sender, worklist_dir_arc)
    } else {
        Arc::new(Mutex::new(WorklistConversion::new(
            sender,
            worklist_dir_arc,
        )))
    };

    let input_file_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Input Folder")
        .build();
    let input_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let input_button = Button::builder()
        .width_request(100)
        .hexpand(false)
        .label("Choose...")
        .build();

    let aetitle_label = Label::builder()
        .halign(gtk::Align::End)
        .label("AETitle")
        .build();
    let aetitle_entry = Entry::builder().hexpand(true).build();
    let modality_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Modality")
        .build();
    let modality_entry = Entry::builder().hexpand(true).build();

    if let Some(ss) = saved_state {
        if let Some(s) = &ss.input_dir_path {
            input_entry.buffer().set_text(s.to_str().unwrap_or(""));
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

    grid_layout.attach(&aetitle_label, 0, 1, 1, 1);
    grid_layout.attach(&aetitle_entry, 1, 1, 3, 1);
    grid_layout.attach(&modality_label, 0, 2, 1, 1);
    grid_layout.attach(&modality_entry, 1, 2, 3, 1);

    grid_layout.attach(&log_expander, 0, 3, 4, 1);
    grid_layout.attach(&remove_button, 3, 4, 1, 1);

    let on_updated2 = on_updated.clone();
    input_button.connect_clicked(clone!(
        #[weak]
        window,
        #[weak]
        input_entry,
        #[weak]
        worklist_conversion,
        move |_| {
            let dialog = FileDialog::builder().build();
            let on_updated2 = on_updated2.clone();
            dialog.select_folder(
                Some(&window),
                None::<gtk::gio::Cancellable>.as_ref(),
                clone!(
                    #[weak]
                    window,
                    #[weak]
                    input_entry,
                    #[weak]
                    worklist_conversion,
                    move |result| match result {
                        Err(err) => {
                            println!("err {:?}", err);
                        }
                        Ok(file) => {
                            if let Some(input_path) = file.path() {
                                if let Some(p) = input_path.to_str() {
                                    input_entry.buffer().set_text(p);
                                    if let std::sync::LockResult::Ok(mut wc) =
                                        worklist_conversion.lock()
                                    {
                                        let result = wc.set_input_dir_path(
                                            Some(PathBuf::from(p)),
                                            worklist_conversion.clone(),
                                        );
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
                                                    .show(Some(&window));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                ),
            );
        }
    ));

    let on_updated2 = on_updated.clone();
    aetitle_entry.connect_changed(clone!(
        #[weak]
        worklist_conversion,
        #[weak]
        aetitle_entry,
        move |_| {
            if let std::sync::LockResult::Ok(mut wc) = worklist_conversion.lock() {
                let text = aetitle_entry.buffer().text().as_str().to_string();
                wc.set_aetitle_string(text);
                on_updated2();
            };
        }
    ));

    let on_updated2 = on_updated.clone();
    modality_entry.connect_changed(clone!(
        #[weak]
        modality_entry,
        #[weak]
        worklist_conversion,
        move |_| {
            if let std::sync::LockResult::Ok(mut wc) = worklist_conversion.lock() {
                let text = modality_entry.buffer().text().as_str().to_string();
                wc.set_modality_string(text);
                on_updated2();
            };
        }
    ));

    let (asender, arecv) = async_channel::unbounded::<String>();

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

    remove_button.connect_clicked(clone!(
        #[weak]
        worklist_conversion,
        move |_| {
            on_delete();
            if let std::sync::LockResult::Ok(mut wc) = worklist_conversion.lock() {
                wc.unwatch_input_dir();
            };
        }
    ));

    return (grid_layout, worklist_conversion.clone());
}
