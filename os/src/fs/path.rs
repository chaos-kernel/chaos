use alloc::{borrow::ToOwned, string::String};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Path {
    path: String,
}

impl Path {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_owned(),
        }
    }
    pub fn is_absolute(&self) -> bool {
        self.path.starts_with('/')
    }
    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }
}

impl From<&str> for Path {
    fn from(path: &str) -> Self {
        Self::new(path)
    }
}

impl From<String> for Path {
    fn from(path: String) -> Self {
        Self::new(&path)
    }
}
