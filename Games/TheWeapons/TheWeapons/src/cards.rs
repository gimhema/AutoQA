


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

pub struct CardActionResult
{
    Value : i32
}

pub trait CardAction {
    fn Action(&self) -> CardActionResult;
}


/*
Card Implementation Guid

pub struct SwordCard
{
    cardInfo : CardInfo
}

impl CardAction for SwordCard
{
    fn Action(&self) -> CardActionResult {
    
    }
}

*/