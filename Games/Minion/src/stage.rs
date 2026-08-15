use crate::enemy;



#[derive(Clone, Copy)]
pub struct GameStage
{
    pub level : i32,
    pub enemy_group : enemy::EnemyGroup
}

impl GameStage
{
    pub fn new() -> Self {
        return GameStage { level: 0, enemy_group: enemy::EnemyGroup::new() }
    }
}

pub struct GameStageManager
{
    pub stages : Vec<GameStage>
}

impl GameStageManager
{
    pub fn new() -> Self {
        return GameStageManager { stages: Vec::new() }
    }
}