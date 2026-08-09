
use crate::perks;
use crate::common::{ActorInfo, CommonStatus, Geometry};

pub mod EMINION
{
    enum MODE {
        DEFAULT = -1,
        NONEPLAYER = 0,
        PLAYERBLE = 1,
        ENEMY = 2
    }

    enum KIND {
        DEFAULT = -1,
        RED = 0,
        ENEMY_MINI_BALL = 1,
        ENEMY_BOSS_RECT = 2
    }
}


pub struct Minion
{
    pub id : usize,
    pub controller_id : Option<usize>,
    pub actorInfo : ActorInfo
}

impl Minion
{
    pub fn New(id : usize) -> Self {
        Minion {
            id,
            controller_id : None,
            actorInfo : ActorInfo {
                status : CommonStatus {
                    health : 0,
                    name : String::new(),
                    speed : 0,
                    power : 0,
                    defense : 0
                },
                geometry : Geometry { x : 0, y : 0 }
            }
        }
    }

    pub fn Init (&mut self) {
        self.actorInfo.Init();
    }

    pub fn GetOwner(&self) -> Option<usize> {
        self.controller_id
    }

    pub fn Attack(&mut self) {
        
    }
}