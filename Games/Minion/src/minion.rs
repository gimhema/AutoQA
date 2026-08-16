
use crate::perks;
use crate::common::{ActorInfo, CommonStatus, Geometry};
use crate::attack::{AttackKind, MeleeAttack, RangedAttack};


pub mod EMINION
{
    pub enum MODE {
        DEFAULT = -1,
        NONEPLAYER = 0,
        PLAYERBLE = 1,
        ENEMY = 2
    }

    #[derive(Clone, Copy)]
    pub enum KIND {
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
    pub actorInfo : ActorInfo,
    pub attack_kind : AttackKind
}

impl Minion
{
    pub fn New(id : usize, kind : EMINION::KIND) -> Self {
        let attack_kind = match kind {
            EMINION::KIND::ENEMY_MINI_BALL => AttackKind::Ranged(RangedAttack { projectile_speed : 6 }),
            EMINION::KIND::ENEMY_BOSS_RECT => AttackKind::Melee(MeleeAttack),
            _ => AttackKind::Melee(MeleeAttack)
        };

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
            },
            attack_kind
        }
    }

    pub fn Init (&mut self) {
        self.actorInfo.Init();
    }

    pub fn GetOwner(&self) -> Option<usize> {
        self.controller_id
    }
}