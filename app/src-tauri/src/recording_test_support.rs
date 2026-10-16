//! Isolated filesystem and environment fixtures shared by behavior tests; never observes or injects desktop input.
use crate::{
    input::EventKind,
    macro_format::{Display, RecordingEnvironment},
    repository::Repository,
};
use std::{path::PathBuf, sync::Arc};
pub const TEST_LAYOUT: &str = "00000409";
pub const TEST_KEY: u16 = 0x41;
pub const TEST_SCAN: u16 = 0x1e;
pub const TEST_X: i32 = -20;
pub const TEST_Y: i32 = 100;
pub const TEST_WIDTH: u32 = 1920;
pub const TEST_HEIGHT: u32 = 1080;
pub const TEST_DISPLAY_X: i32 = -1920;
pub const TEST_SCALE: f64 = 1.25;
const TEST_DIRECTORY_PREFIX: &str = "macroloom-recording-tests";
/// Owns one uniquely named temporary directory and its loaded real repository.
pub struct Folder {
    pub path: PathBuf,
    pub repository: Arc<Repository>,
}
impl Folder {
    /// Creates and loads an isolated writable library; setup errors fail the calling test.
    pub fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("{TEST_DIRECTORY_PREFIX}-{}", uuid::Uuid::new_v4()));
        let repository = Arc::new(Repository::new(path.clone()));
        repository.load(|_| {});
        assert!(repository.snapshot().expect("Fixture snapshot").writable);
        Self { path, repository }
    }
}
impl Drop for Folder {
    /// Removes only the unique temporary fixture root, after checking its resolved parent and prefix.
    fn drop(&mut self) {
        assert_eq!(self.path.parent(), Some(std::env::temp_dir().as_path()));
        assert!(self
            .path
            .file_name()
            .expect("Fixture directory name")
            .to_string_lossy()
            .starts_with(TEST_DIRECTORY_PREFIX));
        if self.path.is_dir() {
            std::fs::remove_dir_all(&self.path).expect("Remove fixture directory");
        }
    }
}
/// Returns realistic signed multi-DPI metadata without consulting the live desktop.
pub fn environment() -> RecordingEnvironment {
    super::environment(
        TEST_LAYOUT.into(),
        vec![Display {
            x: TEST_DISPLAY_X,
            y: 0,
            width: TEST_WIDTH,
            height: TEST_HEIGHT,
            scale_factor: TEST_SCALE,
            extra: Default::default(),
        }],
    )
}
/// Returns one supported native key transition for capture and persistence tests.
pub fn key(down: bool) -> EventKind {
    EventKind::Key {
        vk: TEST_KEY,
        scan: TEST_SCAN,
        extended: false,
        down,
    }
}
