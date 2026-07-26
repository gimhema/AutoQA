


pub enum CardType
{
    SWORD,
    SHIELD,
    SPEAR,
    ARMOR,
    POISON_ARROW,
    POTION,
    FIREBOMB,
    AXE,
    HOOK
}

pub struct CardInfo
{

}

pub trait CardAction {
    fn Action(&self);
}


/*
Card Implementation Guid

pub struct SwordCard
{
    cardInfo : CardInfo
}

impl CardAction for SwordCard
{
    fn Action(&self);
}

*/