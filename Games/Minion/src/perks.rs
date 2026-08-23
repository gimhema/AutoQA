use crate::minion::Minion;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PERKS_TYPE
{
    DEFAULT = 0,
    INC_ATTACK = 1,
    INC_HEALTH = 2,
    INC_SPEED = 3,
    INC_DEFENSE = 4,
    INC_BULLET_NUM = 5
}

#[derive(Clone, Copy, Debug)]
pub struct PerkInfo
{
    pub perk_type : PERKS_TYPE,
    pub amount : i32
}

pub fn AllPerks() -> Vec<PerkInfo> {
    vec![
        PerkInfo { perk_type : PERKS_TYPE::INC_ATTACK, amount : 5 },
        PerkInfo { perk_type : PERKS_TYPE::INC_HEALTH, amount : 30 },
        PerkInfo { perk_type : PERKS_TYPE::INC_SPEED, amount : 1 },
        PerkInfo { perk_type : PERKS_TYPE::INC_DEFENSE, amount : 3 }
    ]
}

pub fn ApplyPerk(minion : &mut Minion, perk : PerkInfo) {
    match perk.perk_type {
        PERKS_TYPE::INC_ATTACK => minion.actorInfo.status.power += perk.amount,
        PERKS_TYPE::INC_HEALTH => minion.actorInfo.status.health += perk.amount,
        PERKS_TYPE::INC_SPEED => minion.actorInfo.status.speed += perk.amount,
        PERKS_TYPE::INC_DEFENSE => minion.actorInfo.status.defense += perk.amount,
        PERKS_TYPE::INC_BULLET_NUM => { /* TODO: 다중 발사는 RangedAttack 쪽 확장이 필요 */ },
        PERKS_TYPE::DEFAULT => {}
    }
}
