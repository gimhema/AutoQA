use crate::minion;
use crate::common;
use crate::common_logic;
use crate::minion::EMINION;

#[derive(Clone, Copy)]
pub struct  EnemyUnitInfo
{
    pub minion_type : EMINION::KIND,
    pub spawn_tick : i32,
    pub spawn_num : i32
}

#[derive(Clone)]
pub struct EnemyGroup
{
    pub info_vec : Vec<EnemyUnitInfo>
}

impl EnemyGroup
{
    pub fn New() -> Self {
        return EnemyGroup { info_vec : Vec::new() }
    }
}