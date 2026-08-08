
use crate::perks;
use crate::common::ActorInfo;

pub mod MINION_MODE
{
    enum EMODE {
        NONE_PLAYER = 0,
        PLAYERBLE = 1,
        ENEMY = 2
    }
}

pub struct Minion
{
    pub actorInfo : ActorInfo
}

impl Minion
{
    pub fn Init (&mut self) {
        self.actorInfo.Init();
    }
}