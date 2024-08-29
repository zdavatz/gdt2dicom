use notify::poll::PollWatcher;
use notify::{recommended_watcher, Config, Event, EventHandler, RecursiveMode, Result, Watcher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct WorklistConversion {
    pub uuid: Uuid,
    input_watcher: Option<(PathBuf, Box<dyn Watcher + Send>)>,
    pub output_dir_path: Option<PathBuf>,
    pub aetitle: String,
    pub modality: String,
}

impl WorklistConversion {
    pub fn new() -> WorklistConversion {
        let uuid = Uuid::new_v4();
        return WorklistConversion {
            uuid: uuid,
            input_watcher: None,
            output_dir_path: None,
            aetitle: "".to_string(),
            modality: "".to_string(),
        };
    }
    pub fn input_dir_path(&self) -> Option<PathBuf> {
        if let Some((p, _)) = &self.input_watcher {
            return Some(p.clone());
        }
        return None;
    }
    pub fn set_input_dir_path(
        &mut self,
        path: Option<PathBuf>,
        self_arc: Arc<Mutex<WorklistConversion>>,
    ) {
        if self.input_dir_path() == path {
            return;
        }
        if let Some(new_path) = path {
            if let Some((current_path, w)) = &mut self.input_watcher {
                w.unwatch(&current_path.as_path());
            }
            let handler = FSEventHandler {
                conversion: self_arc,
            };
            let mut w = recommended_watcher(handler).unwrap(); // TODO
            w.watch(&new_path.as_path(), RecursiveMode::Recursive);
            self.input_watcher = Some((new_path, Box::new(w)));
        } else {
            self.input_watcher = None;
        }
    }

    pub fn scan_folder(&self) {
        println!("Scan folder");
    }
}

struct FSEventHandler {
    pub conversion: Arc<Mutex<WorklistConversion>>,
}

impl EventHandler for FSEventHandler {
    fn handle_event(&mut self, event: Result<Event>) {
        if let Ok(event) = event {
            match event.kind {
                notify::event::EventKind::Create(notify::event::CreateKind::File) => {
                    if let std::sync::LockResult::Ok(c) = self.conversion.lock() {
                        c.scan_folder();
                    }
                }
                _ => {
                    // Skip
                }
            }
            println!("Event: {:?}", event);
        }
    }
}
