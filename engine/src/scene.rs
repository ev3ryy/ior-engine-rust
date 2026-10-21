use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneTransition {
    None,
    Load(PathBuf),
    NewScene,
}

#[derive(Debug, Clone)]
pub struct SceneManager {
    pub current_scene_path: Option<PathBuf>,
    pub pending_transition: SceneTransition,
    pub is_dirty: bool,
}

impl Default for SceneManager {
    fn default() -> Self {
        Self {
            current_scene_path: None,
            pending_transition: SceneTransition::None,
            is_dirty: false,
        }
    }
}

impl SceneManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue_load(&mut self, path: impl AsRef<Path>) {
        self.pending_transition = SceneTransition::Load(path.as_ref().to_path_buf());
    }

    pub fn queue_new(&mut self) {
        self.pending_transition = SceneTransition::NewScene;
    }

    pub fn current_scene_name(&self) -> String {
        self.current_scene_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled Scene".to_string())
    }
}