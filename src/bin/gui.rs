// use gtk4 as gtk;
use gtk::gio::ListStore;
use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow, Button, FileDialog, FileFilter};

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

            dialog.open(
                Some(&w2),
                None::<gtk::gio::Cancellable>.as_ref(),
                |result| {
                    eprintln!("Back from open");
                },
            );
        });
        window.set_child(Some(&button));

        window.present();
    });

    application.run()
}
