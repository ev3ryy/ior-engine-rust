use engine::{EngineConfig, run};
use editor::{Editor, EditorConfig};

fn main() -> anyhow::Result<()> {
    env_logger::init();
    
    let editor_config = EditorConfig::default();
    
    let engine_config = EngineConfig {
        window_title: editor_config.window_title.clone(),
        window_width: editor_config.window_width,
        window_height: editor_config.window_height,
    };
    
    // editor instance
    let editor_app = Editor::new(editor_config);
    
    run(engine_config, editor_app)
}