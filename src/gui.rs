use std::sync::OnceLock;
use tokio::runtime::Runtime;

pub mod about_dialog;
pub mod auto_convert;
pub mod auto_convert_list;
pub mod copyright_dialog;
pub mod dicom_server;
pub mod state;
pub mod worklist_folder;

pub fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(50)
            .thread_name("gdt2dicom")
            .build()
            .expect("Setting up tokio runtime needs to succeed.")
    })
}
