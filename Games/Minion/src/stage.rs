use crate::enemy;



#[derive(Clone, Copy)]
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

    pub fn Run() {

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