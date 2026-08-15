use crate::enemy;



#[derive(Clone, Copy)]
pub struct GameStage
{

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