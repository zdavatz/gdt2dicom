use gtk::prelude::*;
use gtk::AboutDialog;

pub fn open_about_dialog() {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let license_str = include_str!("../../LICENSE");
    let credits = "Credit: Windows-10 theme (https://github.com/B00merang-Project/Windows-10) is bundled with the windows build.\n\n\n\n".to_string();
    let a = AboutDialog::builder()
        .title("About gdt2dicom")
        .program_name("gdt2dicom")
        .license(credits + license_str)
        .wrap_license(true)
        .version(VERSION)
        .build();
    a.set_visible(true);
}
