

pub mod EOBJECT
{
    enum OTYPE {
        DEFAULT = -1,
        BLOCK = 0,
        BULLET = 1        
    }
}

pub trait ObjectBehavior 
{
    fn Spawn(&mut self, objInfo : ObjectInfo);
    fn Destroy(&mut self);
}

struct ObjectInfo
{

}