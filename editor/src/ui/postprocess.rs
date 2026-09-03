use crate::ui::EditorContext;
use egui::{RichText, ScrollArea, Slider, Ui};
use render::postprocess::{PostProcessSettings, TonemappingMode};

pub fn draw_postprocess(ui: &mut Ui, ctx: &mut EditorContext) {
    let pp = &mut ctx.postprocess;

    ui.horizontal(|ui| {
        ui.checkbox(&mut pp.enabled, "Enable Post-Processing");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("↺ Reset").on_hover_text("Reset to Defaults").clicked() {
                **pp = PostProcessSettings::default();
            }
        });
    });
    ui.separator();

    if !pp.enabled {
        ui.label(RichText::new("Post-processing pipeline is bypassed.").italics());
        return;
    }

    ScrollArea::vertical().show(ui, |ui| {
        // tonemapping
        ui.group(|ui| {
            ui.label(RichText::new("📷 Tonemapping & Exposure").strong());
            egui::ComboBox::from_label("Tonemap")
                .selected_text(match pp.tonemapping_mode {
                    TonemappingMode::None => "None (Clamp)",
                    TonemappingMode::ACES => "ACES Filmic",
                    TonemappingMode::Reinhard => "Reinhard",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut pp.tonemapping_mode, TonemappingMode::ACES, "ACES Filmic");
                    ui.selectable_value(&mut pp.tonemapping_mode, TonemappingMode::Reinhard, "Reinhard");
                    ui.selectable_value(&mut pp.tonemapping_mode, TonemappingMode::None, "None (Clamp)");
                });

            ui.add(Slider::new(&mut pp.exposure, 0.01..=5.0).text("Exposure").logarithmic(true));
            ui.checkbox(&mut pp.manual_gamma_correction, "Manual Gamma (1 / 2.2)");
        });
        ui.add_space(5.0);

        // bloom
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("✨ Bloom (Dual Filtering)").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut pp.bloom_enabled, "");
                });
            });

            if pp.bloom_enabled {
                ui.add(Slider::new(&mut pp.bloom_intensity, 0.0..=0.5).text("Intensity"));
                ui.add(Slider::new(&mut pp.bloom_threshold, 0.0..=10.0).text("Threshold"));
                ui.add(Slider::new(&mut pp.bloom_knee, 0.0..=1.0).text("Knee (Softness)"));
                ui.add(Slider::new(&mut pp.bloom_scatter, 0.1..=2.0).text("Scatter / Radius"));
            }
        });
        ui.add_space(5.0);

        // color grading
        ui.group(|ui| {
            ui.label(RichText::new("🎨 Color Grading").strong());
            ui.add(Slider::new(&mut pp.contrast, 0.5..=2.0).text("Contrast"));
            ui.add(Slider::new(&mut pp.saturation, 0.0..=2.0).text("Saturation"));
        });
        ui.add_space(5.0);

        // vignette
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("⭕ Vignette").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut pp.vignette_enabled, "");
                });
            });

            if pp.vignette_enabled {
                ui.add(Slider::new(&mut pp.vignette_intensity, 0.0..=1.0).text("Intensity"));
                ui.add(Slider::new(&mut pp.vignette_radius, 0.1..=1.5).text("Radius"));
                ui.add(Slider::new(&mut pp.vignette_smoothness, 0.01..=1.0).text("Smoothness"));
            }
        });
        ui.add_space(5.0);

        // film grain
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🎞 Film Grain").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut pp.grain_enabled, "");
                });
            });

            if pp.grain_enabled {
                ui.add(Slider::new(&mut pp.grain_intensity, 0.0..=0.1).text("Intensity"));
            }
        });
    });
}