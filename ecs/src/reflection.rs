use crate::World;
use glam::{Quat, Vec2, Vec3, Vec4};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct FieldMeta {
    pub range: Option<(f32, f32)>,
    pub speed: Option<f32>,
    pub is_color: bool,
}

pub trait FieldVisitor {
    fn visit_f32(&mut self, name: &'static str, val: &mut f32, meta: &FieldMeta);
    fn visit_f64(&mut self, name: &'static str, val: &mut f64, meta: &FieldMeta);
    fn visit_i32(&mut self, name: &'static str, val: &mut i32, meta: &FieldMeta);
    fn visit_u32(&mut self, name: &'static str, val: &mut u32, meta: &FieldMeta);
    fn visit_usize(&mut self, name: &'static str, val: &mut usize, meta: &FieldMeta);
    fn visit_bool(&mut self, name: &'static str, val: &mut bool, meta: &FieldMeta);
    fn visit_string(&mut self, name: &'static str, val: &mut String, meta: &FieldMeta);
    fn visit_vec2(&mut self, name: &'static str, val: &mut Vec2, meta: &FieldMeta);
    fn visit_vec3(&mut self, name: &'static str, val: &mut Vec3, meta: &FieldMeta);
    fn visit_vec4(&mut self, name: &'static str, val: &mut Vec4, meta: &FieldMeta);
    fn visit_quat(&mut self, name: &'static str, val: &mut Quat, meta: &FieldMeta);
    fn visit_color3(&mut self, name: &'static str, val: &mut [f32; 3], meta: &FieldMeta);
    fn visit_f32_slice(&mut self, name: &'static str, val: &mut [f32; 3], meta: &FieldMeta);
}

pub trait Inspectable: Send + Sync + 'static {
    const NAME: &'static str;
    const CATEGORY: &'static str;
    fn inspect_fields(&mut self, visitor: &mut dyn FieldVisitor);
}

pub trait ReflectField {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor);
}

impl ReflectField for f32 {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        visitor.visit_f32(name, self, meta);
    }
}

impl ReflectField for f64 {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        visitor.visit_f64(name, self, meta);
    }
}

impl ReflectField for i32 {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        visitor.visit_i32(name, self, meta);
    }
}

impl ReflectField for u32 {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        visitor.visit_u32(name, self, meta);
    }
}

impl ReflectField for usize {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        visitor.visit_usize(name, self, meta);
    }
}

impl ReflectField for bool {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        visitor.visit_bool(name, self, meta);
    }
}

impl ReflectField for String {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        visitor.visit_string(name, self, meta);
    }
}

impl ReflectField for Vec2 {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        visitor.visit_vec2(name, self, meta);
    }
}

impl ReflectField for Vec3 {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        visitor.visit_vec3(name, self, meta);
    }
}

impl ReflectField for Vec4 {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        visitor.visit_vec4(name, self, meta);
    }
}

impl ReflectField for Quat {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        visitor.visit_quat(name, self, meta);
    }
}

impl ReflectField for [f32; 3] {
    fn reflect_field(&mut self, name: &'static str, meta: &FieldMeta, visitor: &mut dyn FieldVisitor) {
        if meta.is_color {
            visitor.visit_color3(name, self, meta);
        } else {
            visitor.visit_f32_slice(name, self, meta);
        }
    }
}

#[derive(Clone)]
pub struct DynamicComponentEntry {
    pub name: &'static str,
    pub category: &'static str,
    pub add_default: Arc<dyn Fn(&World, u64) + Send + Sync>,
    pub has: Arc<dyn Fn(&World, u64) -> bool + Send + Sync>,
    pub remove: Arc<dyn Fn(&World, u64) + Send + Sync>,
    pub inspect: Arc<dyn Fn(&World, u64, &mut dyn FieldVisitor) + Send + Sync>,
}

pub struct ComponentRegistryEntry {
    pub name: &'static str,
    pub category: &'static str,
    pub register_fn: fn(&World),
}

inventory::collect!(ComponentRegistryEntry);

// --- RON Serializer ---

pub struct RonSerializer<'a> {
    pub map: &'a mut BTreeMap<String, ron::Value>,
}

impl<'a> FieldVisitor for RonSerializer<'a> {
    fn visit_f32(&mut self, name: &'static str, val: &mut f32, _meta: &FieldMeta) {
        self.map.insert(name.to_string(), ron::Value::from(*val));
    }
    fn visit_f64(&mut self, name: &'static str, val: &mut f64, _meta: &FieldMeta) {
        self.map.insert(name.to_string(), ron::Value::from(*val));
    }
    fn visit_i32(&mut self, name: &'static str, val: &mut i32, _meta: &FieldMeta) {
        self.map.insert(name.to_string(), ron::Value::from(*val));
    }
    fn visit_u32(&mut self, name: &'static str, val: &mut u32, _meta: &FieldMeta) {
        self.map.insert(name.to_string(), ron::Value::from(*val));
    }
    fn visit_usize(&mut self, name: &'static str, val: &mut usize, _meta: &FieldMeta) {
        self.map.insert(name.to_string(), ron::Value::from(*val as u64));
    }
    fn visit_bool(&mut self, name: &'static str, val: &mut bool, _meta: &FieldMeta) {
        self.map.insert(name.to_string(), ron::Value::from(*val));
    }
    fn visit_string(&mut self, name: &'static str, val: &mut String, _meta: &FieldMeta) {
        self.map.insert(name.to_string(), ron::Value::from(val.clone()));
    }
    fn visit_vec2(&mut self, name: &'static str, val: &mut Vec2, _meta: &FieldMeta) {
        self.map.insert(
            name.to_string(),
            ron::Value::Seq(vec![ron::Value::from(val.x), ron::Value::from(val.y)]),
        );
    }
    fn visit_vec3(&mut self, name: &'static str, val: &mut Vec3, _meta: &FieldMeta) {
        self.map.insert(
            name.to_string(),
            ron::Value::Seq(vec![
                ron::Value::from(val.x),
                ron::Value::from(val.y),
                ron::Value::from(val.z),
            ]),
        );
    }
    fn visit_vec4(&mut self, name: &'static str, val: &mut Vec4, _meta: &FieldMeta) {
        self.map.insert(
            name.to_string(),
            ron::Value::Seq(vec![
                ron::Value::from(val.x),
                ron::Value::from(val.y),
                ron::Value::from(val.z),
                ron::Value::from(val.w),
            ]),
        );
    }
    fn visit_quat(&mut self, name: &'static str, val: &mut Quat, _meta: &FieldMeta) {
        self.map.insert(
            name.to_string(),
            ron::Value::Seq(vec![
                ron::Value::from(val.x),
                ron::Value::from(val.y),
                ron::Value::from(val.z),
                ron::Value::from(val.w),
            ]),
        );
    }
    fn visit_color3(&mut self, name: &'static str, val: &mut [f32; 3], _meta: &FieldMeta) {
        self.map.insert(
            name.to_string(),
            ron::Value::Seq(vec![
                ron::Value::from(val[0]),
                ron::Value::from(val[1]),
                ron::Value::from(val[2]),
            ]),
        );
    }
    fn visit_f32_slice(&mut self, name: &'static str, val: &mut [f32; 3], _meta: &FieldMeta) {
        self.map.insert(
            name.to_string(),
            ron::Value::Seq(vec![
                ron::Value::from(val[0]),
                ron::Value::from(val[1]),
                ron::Value::from(val[2]),
            ]),
        );
    }
}

// --- RON Deserializer ---

pub struct RonDeserializer<'a> {
    pub entity_name: &'a str,
    pub comp_name: &'a str,
    pub fields: &'a BTreeMap<String, ron::Value>,
}

impl<'a> FieldVisitor for RonDeserializer<'a> {
    fn visit_f32(&mut self, name: &'static str, val: &mut f32, _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok(v) = ron_val.clone().into_rust::<f32>() {
                *val = v;
            } else if let Ok(v) = ron_val.clone().into_rust::<i64>() {
                *val = v as f32;
            } else if let Ok(s) = ron_val.clone().into_rust::<String>() {
                if let Ok(parsed) = s.parse::<f32>() {
                    *val = parsed;
                } else {
                    log::warn!("[ECS] '{}.{}' on entity '{}': cannot parse string \"{}\" to f32. Defaulting to: {}", self.comp_name, name, self.entity_name, s, *val);
                }
            } else if let Ok(b) = ron_val.clone().into_rust::<bool>() {
                *val = if b { 1.0 } else { 0.0 };
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': incompatible type ({:?}). Defaulting to: {}", self.comp_name, name, self.entity_name, ron_val, *val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing in save file. Using default: {}", self.comp_name, name, self.entity_name, *val);
        }
    }

    fn visit_f64(&mut self, name: &'static str, val: &mut f64, _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok(v) = ron_val.clone().into_rust::<f64>() {
                *val = v;
            } else if let Ok(v) = ron_val.clone().into_rust::<i64>() {
                *val = v as f64;
            } else if let Ok(s) = ron_val.clone().into_rust::<String>() {
                if let Ok(parsed) = s.parse::<f64>() {
                    *val = parsed;
                } else {
                    log::warn!("[ECS] '{}.{}' on entity '{}': cannot parse string to f64. Defaulting to: {}", self.comp_name, name, self.entity_name, *val);
                }
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': incompatible type. Defaulting to: {}", self.comp_name, name, self.entity_name, *val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: {}", self.comp_name, name, self.entity_name, *val);
        }
    }

    fn visit_i32(&mut self, name: &'static str, val: &mut i32, _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok(v) = ron_val.clone().into_rust::<i32>() {
                *val = v;
            } else if let Ok(v) = ron_val.clone().into_rust::<f64>() {
                *val = v.round() as i32;
            } else if let Ok(s) = ron_val.clone().into_rust::<String>() {
                if let Ok(parsed) = s.parse::<i32>() {
                    *val = parsed;
                } else {
                    log::warn!("[ECS] '{}.{}' on entity '{}': cannot parse string to i32. Defaulting to: {}", self.comp_name, name, self.entity_name, *val);
                }
            } else if let Ok(b) = ron_val.clone().into_rust::<bool>() {
                *val = if b { 1 } else { 0 };
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': incompatible type. Defaulting to: {}", self.comp_name, name, self.entity_name, *val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: {}", self.comp_name, name, self.entity_name, *val);
        }
    }

    fn visit_u32(&mut self, name: &'static str, val: &mut u32, _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok(v) = ron_val.clone().into_rust::<u32>() {
                *val = v;
            } else if let Ok(v) = ron_val.clone().into_rust::<i64>() {
                *val = v.max(0) as u32;
            } else if let Ok(v) = ron_val.clone().into_rust::<f64>() {
                *val = v.round().max(0.0) as u32;
            } else if let Ok(s) = ron_val.clone().into_rust::<String>() {
                if let Ok(parsed) = s.parse::<u32>() {
                    *val = parsed;
                } else {
                    log::warn!("[ECS] '{}.{}' on entity '{}': cannot parse string to u32. Defaulting to: {}", self.comp_name, name, self.entity_name, *val);
                }
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': incompatible type. Defaulting to: {}", self.comp_name, name, self.entity_name, *val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: {}", self.comp_name, name, self.entity_name, *val);
        }
    }

    fn visit_usize(&mut self, name: &'static str, val: &mut usize, _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok(v) = ron_val.clone().into_rust::<usize>() {
                *val = v;
            } else if let Ok(v) = ron_val.clone().into_rust::<u64>() {
                *val = v as usize;
            } else if let Ok(v) = ron_val.clone().into_rust::<i64>() {
                *val = v.max(0) as usize;
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': incompatible type for usize. Defaulting to: {}", self.comp_name, name, self.entity_name, *val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: {}", self.comp_name, name, self.entity_name, *val);
        }
    }

    fn visit_bool(&mut self, name: &'static str, val: &mut bool, _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok(v) = ron_val.clone().into_rust::<bool>() {
                *val = v;
            } else if let Ok(v) = ron_val.clone().into_rust::<i64>() {
                *val = v != 0;
            } else if let Ok(s) = ron_val.clone().into_rust::<String>() {
                *val = s.eq_ignore_ascii_case("true") || s == "1";
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': incompatible type for bool. Defaulting to: {}", self.comp_name, name, self.entity_name, *val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: {}", self.comp_name, name, self.entity_name, *val);
        }
    }

    fn visit_string(&mut self, name: &'static str, val: &mut String, _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok(s) = ron_val.clone().into_rust::<String>() {
                *val = s;
            } else if let Ok(v) = ron_val.clone().into_rust::<i64>() {
                *val = v.to_string();
            } else if let Ok(v) = ron_val.clone().into_rust::<f64>() {
                *val = v.to_string();
            } else if let Ok(v) = ron_val.clone().into_rust::<bool>() {
                *val = v.to_string();
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': cannot convert type to String. Defaulting to: \"{}\"", self.comp_name, name, self.entity_name, val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: \"{}\"", self.comp_name, name, self.entity_name, val);
        }
    }

    fn visit_vec2(&mut self, name: &'static str, val: &mut Vec2, _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok([x, y]) = ron_val.clone().into_rust::<[f32; 2]>() {
                *val = Vec2::new(x, y);
            } else if let Ok([x, y]) = ron_val.clone().into_rust::<[f64; 2]>() {
                *val = Vec2::new(x as f32, y as f32);
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': invalid format for Vec2. Defaulting to: {:?}", self.comp_name, name, self.entity_name, val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: {:?}", self.comp_name, name, self.entity_name, val);
        }
    }

    fn visit_vec3(&mut self, name: &'static str, val: &mut Vec3, _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok([x, y, z]) = ron_val.clone().into_rust::<[f32; 3]>() {
                *val = Vec3::new(x, y, z);
            } else if let Ok([x, y, z]) = ron_val.clone().into_rust::<[f64; 3]>() {
                *val = Vec3::new(x as f32, y as f32, z as f32);
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': invalid format for Vec3. Defaulting to: {:?}", self.comp_name, name, self.entity_name, val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: {:?}", self.comp_name, name, self.entity_name, val);
        }
    }

    fn visit_vec4(&mut self, name: &'static str, val: &mut Vec4, _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok([x, y, z, w]) = ron_val.clone().into_rust::<[f32; 4]>() {
                *val = Vec4::new(x, y, z, w);
            } else if let Ok([x, y, z, w]) = ron_val.clone().into_rust::<[f64; 4]>() {
                *val = Vec4::new(x as f32, y as f32, z as f32, w as f32);
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': invalid format for Vec4. Defaulting to: {:?}", self.comp_name, name, self.entity_name, val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: {:?}", self.comp_name, name, self.entity_name, val);
        }
    }

    fn visit_quat(&mut self, name: &'static str, val: &mut Quat, _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok([x, y, z, w]) = ron_val.clone().into_rust::<[f32; 4]>() {
                *val = Quat::from_xyzw(x, y, z, w);
            } else if let Ok([x, y, z, w]) = ron_val.clone().into_rust::<[f64; 4]>() {
                *val = Quat::from_xyzw(x as f32, y as f32, z as f32, w as f32);
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': invalid format for Quat. Defaulting to: {:?}", self.comp_name, name, self.entity_name, val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: {:?}", self.comp_name, name, self.entity_name, val);
        }
    }

    fn visit_color3(&mut self, name: &'static str, val: &mut [f32; 3], _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok(c) = ron_val.clone().into_rust::<[f32; 3]>() {
                *val = c;
            } else if let Ok([x, y, z]) = ron_val.clone().into_rust::<[f64; 3]>() {
                *val = [x as f32, y as f32, z as f32];
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': invalid format for Color3. Defaulting to: {:?}", self.comp_name, name, self.entity_name, val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: {:?}", self.comp_name, name, self.entity_name, val);
        }
    }

    fn visit_f32_slice(&mut self, name: &'static str, val: &mut [f32; 3], _meta: &FieldMeta) {
        if let Some(ron_val) = self.fields.get(name) {
            if let Ok(c) = ron_val.clone().into_rust::<[f32; 3]>() {
                *val = c;
            } else if let Ok([x, y, z]) = ron_val.clone().into_rust::<[f64; 3]>() {
                *val = [x as f32, y as f32, z as f32];
            } else {
                log::warn!("[ECS] '{}.{}' on entity '{}': invalid format for [f32; 3]. Defaulting to: {:?}", self.comp_name, name, self.entity_name, val);
            }
        } else {
            log::warn!("[ECS] '{}.{}' on entity '{}': field missing. Using default: {:?}", self.comp_name, name, self.entity_name, val);
        }
    }
}