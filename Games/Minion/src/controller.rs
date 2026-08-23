use crate::world::World;
use crate::minion::Minion;
use crate::attack::AttackContext;
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

    // 사람 입력(키보드/마우스)과 네트워크 액션(Ouroboros)이 공유하는 원시 동작.
    // dx/dy는 각 축 -1..1 (WASD와 동일한 의미: 눌린 방향으로 speed만큼 이동).
    pub fn Move(&mut self, world : &mut World, dx : i32, dy : i32) {
        let Some(minion) = self.GetPawn(world) else { return; };
        minion.actorInfo.geometry.x += dx * self.speed;
        minion.actorInfo.geometry.y += dy * self.speed;
    }

    pub fn AimAt(&mut self, world : &mut World, target_x : f32, target_y : f32) {
        let Some(minion) = self.GetPawn(world) else { return; };
        let dx = target_x - minion.actorInfo.geometry.x as f32;
        let dy = target_y - minion.actorInfo.geometry.y as f32;
        self.aim_angle = dy.atan2(dx);
    }

    pub fn Update(&mut self, world : &mut World) {
        let mut dx = 0;
        let mut dy = 0;
        if is_key_down(KeyCode::W) { dy -= 1; }
        if is_key_down(KeyCode::S) { dy += 1; }
        if is_key_down(KeyCode::A) { dx -= 1; }
        if is_key_down(KeyCode::D) { dx += 1; }
        self.Move(world, dx, dy);

        let (mouse_x, mouse_y) = mouse_position();
        self.AimAt(world, mouse_x, mouse_y);

        if is_key_down(KeyCode::Space) { self.Shoot(world); }
    }

    pub fn Shoot(&mut self, world : &mut World) {
        let Some(minion) = self.GetPawn(world) else { return; };

        let ctx = AttackContext {
            origin : minion.actorInfo.geometry,
            aim_angle : self.aim_angle,
            power : minion.actorInfo.status.power,
            owner_id : Some(minion.id)
        };
        let attack_kind = minion.attack_kind;

        attack_kind.attack(ctx, world);
    }
}
