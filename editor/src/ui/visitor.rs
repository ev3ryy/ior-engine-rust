use ecs::{FieldMeta, FieldVisitor};
use egui::{DragValue, Slider, Ui};

pub struct EguiFieldVisitor<'a> {
    pub ui: &'a mut Ui,
    pub changed: bool,
}

impl<'a> FieldVisitor for EguiFieldVisitor<'a> {
    fn visit_f32(&mut self, name: &'static str, val: &mut f32, meta: &FieldMeta) {
        let speed = meta.speed.unwrap_or(0.1);
        if let Some((min, max)) = meta.range {
            self.changed |= self.ui.add(Slider::new(val, min..=max).text(name)).changed();
        } else {
            self.changed |= self
                .ui
                .add(DragValue::new(val).speed(speed).prefix(format!("{}: ", name)))
                .changed();
        }
    }

    fn visit_f64(&mut self, name: &'static str, val: &mut f64, meta: &FieldMeta) {
        let speed = meta.speed.unwrap_or(0.1) as f64;
        self.changed |= self
            .ui
            .add(DragValue::new(val).speed(speed).prefix(format!("{}: ", name)))
            .changed();
    }

    fn visit_i32(&mut self, name: &'static str, val: &mut i32, meta: &FieldMeta) {
        let speed = meta.speed.unwrap_or(1.0);
        if let Some((min, max)) = meta.range {
            self.changed |= self
                .ui
                .add(Slider::new(val, (min as i32)..=(max as i32)).text(name))
                .changed();
        } else {
            self.changed |= self
                .ui
                .add(DragValue::new(val).speed(speed).prefix(format!("{}: ", name)))
                .changed();
        }
    }

    fn visit_u32(&mut self, name: &'static str, val: &mut u32, meta: &FieldMeta) {
        let speed = meta.speed.unwrap_or(1.0);
        self.changed |= self
            .ui
            .add(DragValue::new(val).speed(speed).prefix(format!("{}: ", name)))
            .changed();
    }

    fn visit_usize(&mut self, name: &'static str, val: &mut usize, meta: &FieldMeta) {
        let speed = meta.speed.unwrap_or(1.0);
        self.changed |= self
            .ui
            .add(DragValue::new(val).speed(speed).prefix(format!("{}: ", name)))
            .changed();
    }

    fn visit_bool(&mut self, name: &'static str, val: &mut bool, _meta: &FieldMeta) {
        self.changed |= self.ui.checkbox(val, name).changed();
    }

    fn visit_string(&mut self, name: &'static str, val: &mut String, _meta: &FieldMeta) {
        self.ui.horizontal(|ui| {
            ui.label(format!("{}:", name));
            self.changed |= ui.text_edit_singleline(val).changed();
        });
    }

    fn visit_vec2(&mut self, name: &'static str, val: &mut glam::Vec2, meta: &FieldMeta) {
        let speed = meta.speed.unwrap_or(0.1);
        self.ui.horizontal(|ui| {
            ui.label(format!("{}:", name));
            self.changed |= ui.add(DragValue::new(&mut val.x).speed(speed).prefix("X: ")).changed();
            self.changed |= ui.add(DragValue::new(&mut val.y).speed(speed).prefix("Y: ")).changed();
        });
    }

    fn visit_vec3(&mut self, name: &'static str, val: &mut glam::Vec3, meta: &FieldMeta) {
        let speed = meta.speed.unwrap_or(0.1);
        self.ui.horizontal(|ui| {
            ui.label(format!("{}:", name));
            self.changed |= ui.add(DragValue::new(&mut val.x).speed(speed).prefix("X: ")).changed();
            self.changed |= ui.add(DragValue::new(&mut val.y).speed(speed).prefix("Y: ")).changed();
            self.changed |= ui.add(DragValue::new(&mut val.z).speed(speed).prefix("Z: ")).changed();
        });
    }

    fn visit_vec4(&mut self, name: &'static str, val: &mut glam::Vec4, meta: &FieldMeta) {
        let speed = meta.speed.unwrap_or(0.1);
        self.ui.horizontal(|ui| {
            ui.label(format!("{}:", name));
            self.changed |= ui.add(DragValue::new(&mut val.x).speed(speed).prefix("X: ")).changed();
            self.changed |= ui.add(DragValue::new(&mut val.y).speed(speed).prefix("Y: ")).changed();
            self.changed |= ui.add(DragValue::new(&mut val.z).speed(speed).prefix("Z: ")).changed();
            self.changed |= ui.add(DragValue::new(&mut val.w).speed(speed).prefix("W: ")).changed();
        });
    }

    fn visit_quat(&mut self, name: &'static str, val: &mut glam::Quat, _meta: &FieldMeta) {
        let (rx, ry, rz) = val.to_euler(glam::EulerRot::XYZ);
        let mut deg_x = rx.to_degrees();
        let mut deg_y = ry.to_degrees();
        let mut deg_z = rz.to_degrees();

        let mut rot_changed = false;
        self.ui.horizontal(|ui| {
            ui.label(format!("{}:", name));
            rot_changed |= ui
                .add(DragValue::new(&mut deg_x).speed(0.5).prefix("R_X: ").suffix("°"))
                .changed();
            rot_changed |= ui
                .add(DragValue::new(&mut deg_y).speed(0.5).prefix("R_Y: ").suffix("°"))
                .changed();
            rot_changed |= ui
                .add(DragValue::new(&mut deg_z).speed(0.5).prefix("R_Z: ").suffix("°"))
                .changed();
        });

        if rot_changed {
            *val = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                deg_x.to_radians(),
                deg_y.to_radians(),
                deg_z.to_radians(),
            );
            self.changed = true;
        }
    }

    fn visit_color3(&mut self, name: &'static str, val: &mut [f32; 3], _meta: &FieldMeta) {
        self.ui.horizontal(|ui| {
            ui.label(format!("{}:", name));
            self.changed |= ui.color_edit_button_rgb(val).changed();
        });
    }

    fn visit_f32_slice(&mut self, name: &'static str, val: &mut [f32; 3], meta: &FieldMeta) {
        let speed = meta.speed.unwrap_or(0.1);
        self.ui.horizontal(|ui| {
            ui.label(format!("{}:", name));
            self.changed |= ui.add(DragValue::new(&mut val[0]).speed(speed).prefix("0: ")).changed();
            self.changed |= ui.add(DragValue::new(&mut val[1]).speed(speed).prefix("1: ")).changed();
            self.changed |= ui.add(DragValue::new(&mut val[2]).speed(speed).prefix("2: ")).changed();
        });
    }
}