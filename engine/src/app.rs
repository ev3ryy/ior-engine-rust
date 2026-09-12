use crate::camera::{Camera, CameraController};
use crate::game::{PlayControlAction, PlayState};
use crate::EngineMetrics;
use ecs::World;
use std::sync::Arc;
use winit::event::WindowEvent;

pub trait Application {
    fn on_init(
        &mut self,
        world: &mut World,
        camera: &mut Camera,
        asset_server: &Arc<asset::AssetServer>,
    );

    fn on_update(
        &mut self,
        world: &mut World,
        camera: &mut Camera,
        camera_controller: &mut CameraController,
    );

    fn on_window_event(
        &mut self,
        world: &mut World,
        camera: &mut Camera,
        camera_controller: &mut CameraController,
        event: &WindowEvent,
    ) -> bool;

    fn on_ui(
        &mut self,
        ctx: &egui::Context,
        world: &mut World,
        camera: &mut Camera,
        postprocess: &mut render::postprocess::PostProcessSettings,
        metrics: &EngineMetrics,
        load_requests: &mut Vec<String>,
        play_state: PlayState,
        asset_server: &Arc<asset::AssetServer>,
        viewport_texture_id: Option<egui::TextureId>,
        desired_viewport_size: &mut Option<(u32, u32)>,
    ) -> PlayControlAction {
        let _ = (
            ctx,
            world,
            camera,
            postprocess,
            metrics,
            load_requests,
            play_state,
            asset_server,
            viewport_texture_id,
            desired_viewport_size,
        );
        PlayControlAction::None
    }

    fn on_resize(&mut self, _width: u32, _height: u32) {}

    fn selected_entity(&self) -> Option<u64> {
        None
    }
}