use crate::ui::EditorContext;
use egui::{DragValue, ScrollArea, Slider, Ui};

pub fn draw_metrics(ui: &mut Ui, ctx: &mut EditorContext) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.heading("Performance");
        ui.separator();

        ui.label(format!("FPS: {:.1}", ctx.metrics.fps));
        ui.label(format!("Frame Time: {:.2} ms", ctx.metrics.frame_time_ms));
        ui.label(format!("Draw Calls: {}", ctx.metrics.draw_calls));
        ui.label(format!("Total Instances: {}", ctx.metrics.instance_count));

        ui.add_space(15.0);

        ui.heading("Camera");
        ui.separator();

        ui.label("Position");
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut ctx.camera.eye.x).speed(0.1).prefix("X: "));
            ui.add(DragValue::new(&mut ctx.camera.eye.y).speed(0.1).prefix("Y: "));
            ui.add(DragValue::new(&mut ctx.camera.eye.z).speed(0.1).prefix("Z: "));
        });

        ui.add_space(10.0);
        ui.add(Slider::new(&mut ctx.camera.fovy, 10.0..=120.0).text("FOV"));
    });
}