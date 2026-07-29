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

use crate::folder_flatten::{FolderFlatten, FolderFlattenState};

use crate::gui::runtime;

pub fn setup_flatten_ui<F, G>(
    window: &ApplicationWindow,
    on_delete: F,
    on_updated: G,
    saved_state: Option<&FolderFlattenState>,
) -> (Grid, Arc<Mutex<FolderFlatten>>)
where
    F: Fn() + 'static,
    G: Fn() + 'static + Clone,
{
    let (sender, receiver) = mpsc::channel();
    let folder_flatten = if let Some(ss) = saved_state {
        FolderFlatten::from_state(ss, sender)
    } else {
        Arc::new(Mutex::new(FolderFlatten::new(sender)))
    };

    let input_file_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Export Folder")
        .build();
    let input_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let input_button = Button::builder()
        .width_request(100)
        .hexpand(false)
        .label("Choose...")
        .build();

    let output_file_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Flat Folder")
        .build();
    let output_entry = Entry::builder().hexpand(true).sensitive(false).build();
    let output_button = Button::builder()
        .width_request(100)
        .hexpand(false)
        .label("Choose...")
        .build();

    let extensions_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Extensions")
        .build();
    let extensions_entry = Entry::builder()
        .hexpand(true)
        .placeholder_text("jpg,dcm (empty = all files)")
        .build();

    let cutoff_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Only since")
        .build();
    let cutoff_entry = Entry::builder()
        .hexpand(true)
        .placeholder_text("YYYYMMDD, e.g. 20260723 (empty = all)")
        .build();

    let exclude_label = Label::builder()
        .halign(gtk::Align::End)
        .label("Exclude")
        .build();
    let exclude_entry = Entry::builder()
        .hexpand(true)
        .placeholder_text("Comma separated, e.g. EM-,Muster")
        .build();

    if let Some(ss) = saved_state {
        if let Some(s) = &ss.input_dir_path {
            input_entry.buffer().set_text(s.to_str().unwrap_or(""));
        }
        if let Some(s) = &ss.output_dir_path {
            output_entry.buffer().set_text(s.to_str().unwrap_or(""));
        }
        if let Some(s) = &ss.extensions {
            extensions_entry.buffer().set_text(s);
        }
        if let Some(s) = &ss.cutoff_date {
            cutoff_entry.buffer().set_text(s);
        }
        if let Some(s) = &ss.exclude_patterns {
            exclude_entry.buffer().set_text(s);
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
        .label("Remove flatten folder")
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

    grid_layout.attach(&extensions_label, 0, 2, 1, 1);
    grid_layout.attach(&extensions_entry, 1, 2, 3, 1);
    grid_layout.attach(&cutoff_label, 0, 3, 1, 1);
    grid_layout.attach(&cutoff_entry, 1, 3, 3, 1);
    grid_layout.attach(&exclude_label, 0, 4, 1, 1);
    grid_layout.attach(&exclude_entry, 1, 4, 3, 1);

    grid_layout.attach(&log_expander, 0, 5, 4, 1);
    grid_layout.attach(&remove_button, 3, 6, 1, 1);

    let on_updated2 = on_updated.clone();
    input_button.connect_clicked(clone!(
        #[weak]
        window,
        #[weak]
        input_entry,
        #[weak]
        folder_flatten,
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
                    folder_flatten,
                    move |result| match result {
                        Err(err) => {
                            println!("err {:?}", err);
                        }
                        Ok(file) => {
                            if let Some(input_path) = file.path() {
                                if let Some(p) = input_path.to_str() {
                                    input_entry.buffer().set_text(p);
                                    if let std::sync::LockResult::Ok(mut ff) = folder_flatten.lock()
                                    {
                                        let result = ff.set_input_dir_path(
                                            Some(PathBuf::from(p)),
                                            folder_flatten.clone(),
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
    output_button.connect_clicked(clone!(
        #[weak]
        window,
        #[weak]
        output_entry,
        #[weak]
        folder_flatten,
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
                    output_entry,
                    #[weak]
                    folder_flatten,
                    move |result| match result {
                        Err(err) => {
                            println!("err {:?}", err);
                        }
                        Ok(file) => {
                            if let Some(output_path) = file.path() {
                                if let Some(p) = output_path.to_str() {
                                    output_entry.buffer().set_text(p);
                                    if let std::sync::LockResult::Ok(mut ff) = folder_flatten.lock()
                                    {
                                        let result = ff.set_output_dir_path(Some(PathBuf::from(p)));
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
    extensions_entry.connect_changed(clone!(
        #[weak]
        folder_flatten,
        #[weak]
        extensions_entry,
        move |_| {
            if let std::sync::LockResult::Ok(mut ff) = folder_flatten.lock() {
                let text = extensions_entry.buffer().text().as_str().to_string();
                ff.set_extensions_string(text);
                on_updated2();
            };
        }
    ));

    let on_updated2 = on_updated.clone();
    cutoff_entry.connect_changed(clone!(
        #[weak]
        folder_flatten,
        #[weak]
        cutoff_entry,
        move |_| {
            if let std::sync::LockResult::Ok(mut ff) = folder_flatten.lock() {
                let text = cutoff_entry.buffer().text().as_str().to_string();
                ff.set_cutoff_date_string(text);
                on_updated2();
            };
        }
    ));

    let on_updated2 = on_updated.clone();
    exclude_entry.connect_changed(clone!(
        #[weak]
        folder_flatten,
        #[weak]
        exclude_entry,
        move |_| {
            if let std::sync::LockResult::Ok(mut ff) = folder_flatten.lock() {
                let text = exclude_entry.buffer().text().as_str().to_string();
                ff.set_exclude_patterns_string(text);
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
        folder_flatten,
        move |_| {
            on_delete();
            if let std::sync::LockResult::Ok(mut ff) = folder_flatten.lock() {
                ff.unwatch_input_dir();
            };
        }
    ));

    return (grid_layout, folder_flatten.clone());
}
