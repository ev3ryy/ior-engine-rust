use editor::{Editor, EditorConfig};
use engine::{run, EngineConfig};
use game::MyGamePlugin;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let editor_config = EditorConfig::default();

    let engine_config = EngineConfig {
        window_title: "My Game Engine Editor".to_string(),
        window_width: editor_config.window_width,
        window_height: editor_config.window_height,
        msaa_samples: 1,
    };

    let game_plugin = Box::new(MyGamePlugin::default());
    let editor_app = Editor::new(editor_config, game_plugin);

    run(engine_config, editor_app)
}