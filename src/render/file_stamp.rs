use std::{io, path::Path, time::SystemTime};

#[derive(PartialEq, Eq)]
pub(super) struct FileStamp {
    length: u64,
    modified: Option<SystemTime>,
}

impl FileStamp {
    pub fn read(path: &Path) -> io::Result<Self> {
        let metadata = path.metadata()?;
        Ok(Self {
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    pub fn is_reliable(&self) -> bool {
        self.modified.is_some()
    }
}
