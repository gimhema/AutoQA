use crate::common::Geometry;

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
    fn Update(&mut self);
    fn OnHit(&mut self);
}

#[derive(Clone, Copy)]
pub struct ObjectInfo
{
    pub id : usize,
    pub otype : EOBJECT::OTYPE,
    pub owner_id : Option<usize>,
    pub pos : Geometry
}

#[derive(Clone, Copy)]
pub struct BlockUnbreakable
{
    pub info : ObjectInfo
}

impl ObjectBehavior for BlockUnbreakable
{
    fn Update(&mut self) {}
    fn OnHit(&mut self) {}
}

#[derive(Clone, Copy)]
pub struct BlockBreakable
{
    pub info : ObjectInfo
}

impl ObjectBehavior for BlockBreakable
{
    fn Update(&mut self) {}
    fn OnHit(&mut self) {}
}

#[derive(Clone, Copy)]
pub struct Bullet
{
    pub info : ObjectInfo,
    pub aim_angle : f32,
    pub speed : i32,
    pub power : i32
}

impl ObjectBehavior for Bullet
{
    fn Update(&mut self) {
        self.info.pos.x += (self.aim_angle.cos() * self.speed as f32) as i32;
        self.info.pos.y += (self.aim_angle.sin() * self.speed as f32) as i32;
    }

    fn OnHit(&mut self) {
        // 충돌 시 데미지 적용/제거는 World가 처리 (여기선 훅만 남겨둠)
    }
}

#[derive(Clone, Copy)]
pub enum ObjectKind
{
    Bullet(Bullet),
    BlockUnbreakable(BlockUnbreakable),
    BlockBreakable(BlockBreakable)
}

impl ObjectKind
{
    pub fn GetInfo(&self) -> ObjectInfo {
        match self {
            ObjectKind::Bullet(b) => b.info,
            ObjectKind::BlockUnbreakable(b) => b.info,
            ObjectKind::BlockBreakable(b) => b.info
        }
    }

    pub fn Update(&mut self) {
        match self {
            ObjectKind::Bullet(b) => b.Update(),
            ObjectKind::BlockUnbreakable(b) => b.Update(),
            ObjectKind::BlockBreakable(b) => b.Update()
        }
    }
}
