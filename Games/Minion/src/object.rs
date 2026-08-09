

pub mod EOBJECT
{
    #[derive(Clone, Copy)]
    pub enum OTYPE {
        DEFAULT = -1,
        BLOCK = 0,
        BULLET = 1        
    }
}

pub trait ObjectBehavior 
{
    fn Spawn(&mut self, objInfo : ObjectInfo);
    fn Destroy(&mut self);
    fn OnHit(&mut self);
}

#[derive(Clone, Copy)]
struct ObjectInfo
{
    pub oID : i32,
    pub oType : EOBJECT::OTYPE
}

#[derive(Clone, Copy)]
pub struct BlockUnbreakable
{
    pub info : ObjectInfo
}

#[derive(Clone, Copy)]
pub struct BlockBreakable
{
    pub info : ObjectInfo
}

#[derive(Clone, Copy)]
pub struct Bullet
{
     pub info : ObjectInfo   
}