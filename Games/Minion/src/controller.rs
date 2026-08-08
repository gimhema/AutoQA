use crate::world::World;
use macroquad::prelude::*;

pub struct PlayerController
{
    pub possessed_id : Option<usize>,
    pub aim_angle : f32,
    pub speed : i32
}

impl PlayerController
{
    pub fn New() -> Self {
        PlayerController {
            possessed_id : None,
            aim_angle : 0.0,
            speed : 5
        }
    }

    pub fn Possess(&mut self, id : usize) {
        self.possessed_id = Some(id);
    }

    pub fn Update(&mut self, world : &mut World) {
        let Some(id) = self.possessed_id else { return; };
        let Some(minion) = world.GetMinionMut(id) else { return; };

        if is_key_down(KeyCode::W) { minion.actorInfo.geometry.y -= self.speed; }
        if is_key_down(KeyCode::S) { minion.actorInfo.geometry.y += self.speed; }
        if is_key_down(KeyCode::A) { minion.actorInfo.geometry.x -= self.speed; }
        if is_key_down(KeyCode::D) { minion.actorInfo.geometry.x += self.speed; }

        let (mouse_x, mouse_y) = mouse_position();
        let dx = mouse_x - minion.actorInfo.geometry.x as f32;
        let dy = mouse_y - minion.actorInfo.geometry.y as f32;
        self.aim_angle = dy.atan2(dx);

        if is_key_down(KeyCode::Space) { self.Shoot(world); }
    }

    pub fn Shoot(&mut self, world : &mut World) {

    }
}
