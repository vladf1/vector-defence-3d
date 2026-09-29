//! Campaign progress in `localStorage`. Any failure (blocked storage, private mode, quota)
//! drops to memory for the rest of the session, like the TypeScript store did.
use vd_core::progress::ProgressStorage;

pub struct BrowserStorage {
    storage: Option<web_sys::Storage>,
    failed: bool,
}

impl BrowserStorage {
    pub fn new() -> BrowserStorage {
        let storage = web_sys::window().and_then(|window| window.local_storage().ok().flatten());
        BrowserStorage { failed: storage.is_none(), storage }
    }
}

impl BrowserStorage {
    fn fail(&mut self) {
        self.storage = None;
        self.failed = true;
    }
}

impl ProgressStorage for BrowserStorage {
    fn failed(&self) -> bool {
        self.failed
    }

    fn read(&mut self, key: &str) -> Option<String> {
        let storage = self.storage.as_ref()?;
        match storage.get_item(key) {
            Ok(value) => value,
            Err(_) => {
                self.fail();
                None
            }
        }
    }

    fn write(&mut self, key: &str, value: &str) {
        if let Some(storage) = &self.storage
            && storage.set_item(key, value).is_err()
        {
            self.fail();
        }
    }

    fn remove(&mut self, key: &str) {
        if let Some(storage) = &self.storage
            && storage.remove_item(key).is_err()
        {
            self.fail();
        }
    }
}
