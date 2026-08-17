use crate::enemy;
use crate::world::World;



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
            world.SpawnMinion(kind);
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