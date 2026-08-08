use crate::common;
use crate::common_logic;

pub mod CommonLogicUtil
{
    use crate::{common::Geometry, minion::Minion};

    pub fn IsExistMinion(target_pos : Geometry, minions : &mut Vec<Minion>) -> Option<&mut Minion> {

        return None
    }
}

pub mod DamageInterface
{
    use crate::{common::Geometry, minion::Minion};

    pub fn ApplyDamage(attack: &Minion, target: &mut Minion) {
//        target.actorInfo.status.health -= attack.actorInfo.status.power;
    }

    pub fn ApplyRangeDamage(attack : &Minion, target_pos : Geometry, range : i32) {

    }
}