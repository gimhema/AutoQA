use crate::minion;
use crate::common::Geometry;
use macroquad::prelude::*;

pub struct PlayerController
{
    pub pos : Geometry,
    pub aim_angle : f32,
    pub speed : i32
}

impl PlayerController
{
    pub fn New() -> Self {
        PlayerController {
            pos : Geometry { x: 0, y: 0 },
            aim_angle : 0.0,
            speed : 5
        }
    }

    pub fn Init(&mut self) {
        self.pos = Geometry { x: 400, y: 300 };
        self.aim_angle = 0.0;
    }

    pub fn Update(&mut self) {
        if is_key_down(KeyCode::W) { self.pos.y -= self.speed; }
        if is_key_down(KeyCode::S) { self.pos.y += self.speed; }
        if is_key_down(KeyCode::A) { self.pos.x -= self.speed; }
        if is_key_down(KeyCode::D) { self.pos.x += self.speed; }
        if is_key_down(KeyCode::Space) {self.Shoot();}

        let (mouse_x, mouse_y) = mouse_position();
        let dx = mouse_x - self.pos.x as f32;
        let dy = mouse_y - self.pos.y as f32;
        self.aim_angle = dy.atan2(dx);
    }

    pub fn Shoot(&mut self) {

    }
}
