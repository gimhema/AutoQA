use crate::common::Geometry;
use crate::world::World;
use crate::object;

#[derive(Clone, Copy)]
pub struct AttackContext
{
    pub origin : Geometry,
    pub aim_angle : f32,
    pub power : i32
}

pub trait AttackBehavior
{
    fn Attack(&self, ctx : AttackContext, world : &mut World);
}

#[derive(Clone, Copy)]
pub struct MeleeAttack;

impl AttackBehavior for MeleeAttack
{
    fn Attack(&self, ctx : AttackContext, world : &mut World) {
        // TODO: 근접 공격 로직
    }
}

#[derive(Clone, Copy)]
pub struct RangedAttack
{
    pub projectile_speed : i32
}

impl AttackBehavior for RangedAttack
{
    fn Attack(&self, ctx : AttackContext, world : &mut World) {
        // TODO: 투사체 발사 로직
    }
}

#[derive(Clone, Copy)]
pub enum AttackKind
{
    Melee(MeleeAttack),
    Ranged(RangedAttack)
}

impl AttackKind
{
    pub fn attack(&self, ctx : AttackContext, world : &mut World) {
        match self {
            AttackKind::Melee(behavior) => behavior.Attack(ctx, world),
            AttackKind::Ranged(behavior) => behavior.Attack(ctx, world)
        }
    }
}
