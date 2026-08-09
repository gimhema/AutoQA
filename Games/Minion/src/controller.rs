use crate::world::World;
use crate::minion::Minion;
use macroquad::prelude::*;

pub struct PlayerController
{
    pub id : usize,
    pub possessed_id : Option<usize>,
    pub aim_angle : f32,
    pub speed : i32
}

impl PlayerController
{
    pub fn New(id : usize) -> Self {
        PlayerController {
            id,
            possessed_id : None,
            aim_angle : 0.0,
            speed : 5
        }
    }

    pub fn Possess(&mut self, world : &mut World, id : usize) {
        self.possessed_id = Some(id);
        if let Some(minion) = world.GetMinionMut(id) {
            minion.controller_id = Some(self.id);
        }
    }

    pub fn GetPawn<'a>(&self, world : &'a mut World) -> Option<&'a mut Minion> {
        world.GetMinionMut(self.possessed_id?)
    }

    pub fn Update(&mut self, world : &mut World) {
        let Some(minion) = self.GetPawn(world) else { return; };

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
        let Some(minion) = self.GetPawn(world) else { return; };

        minion.Attack();
    }
}
