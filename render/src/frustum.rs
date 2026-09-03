use glam::{Mat4, Vec3, Vec4};

pub struct Frustum {
    pub planes: [Vec4; 6],
}

impl Frustum {
    pub fn from_view_proj(vp: Mat4) -> Self {
        let r1 = vp.row(0);
        let r2 = vp.row(1);
        let r3 = vp.row(2);
        let r4 = vp.row(3);

        let mut planes = [
            r4 + r1, // Left
            r4 - r1, // Right
            r4 + r2, // Bottom
            r4 - r2, // Top
            r3,      // Near
            r4 - r3, // Far
        ];

        for p in &mut planes {
            let len = p.truncate().length();
            *p /= len;
        }

        Self { planes }
    }

    #[inline]
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        for p in &self.planes {
            if p.truncate().dot(center) + p.w < -radius {
                return false;
            }
        }
        true
    }
}