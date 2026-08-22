use crate::enemy;
use crate::world::World;
use crate::common::Geometry;
use macroquad::prelude::*;



#[derive(Clone)]
pub struct GameStage
{
    pub level : i32,
    pub enemy_group : enemy::EnemyGroup
}

impl GameStage
{
    pub fn New() -> Self {
        return GameStage { level: 0, enemy_group: enemy::EnemyGroup::New() }
    }

    pub fn Start() {

    }

    pub fn Run(&mut self, world : &mut World) {
        for kind in self.enemy_group.Tick() {
            let Some(id) = world.SpawnMinion(kind) else { continue; };
            let Some(minion) = world.GetMinionMut(id) else { continue; };

            minion.actorInfo.geometry = Geometry {
                x : rand::gen_range(0, screen_width() as i32),
                y : rand::gen_range(0, screen_height() as i32)
            };
        }
    }

    pub fn End() {

    }
}

pub struct GameStageManager
{
    pub stages : Vec<GameStage>
}

impl GameStageManager
{
    pub fn New() -> Self {
        return GameStageManager { stages: Vec::new() }
    }
}