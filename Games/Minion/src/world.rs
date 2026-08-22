use crate::minion::{Minion, EMINION};
use crate::object::{ObjectKind, ObjectInfo, Bullet, EOBJECT};
use crate::common::Geometry;
use crate::common_logic::DamageInterface;
use crate::config::CONSTANTS;

pub struct World
{
    pub minions : Vec<Minion>,
    pub objects : Vec<ObjectKind>,
    next_id : usize
}

impl World
{
    pub fn New() -> Self {
        World { minions : Vec::new(), objects : Vec::new(), next_id : 0 }
    }

    fn NextId(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn SpawnMinion(&mut self, kind : EMINION::KIND) -> Option<usize> {
        if matches!(kind, EMINION::KIND::ENEMY_MINI_BALL | EMINION::KIND::ENEMY_BOSS_RECT)
            && self.CountEnemies() >= CONSTANTS::MAX_ENEMY_COUNT as usize {
            return None;
        }

        let id = self.NextId();
        let minion = Minion::New(id, kind);
        self.minions.push(minion);
        Some(id)
    }

    pub fn SpawnBullet(&mut self, owner_id : Option<usize>, pos : Geometry, aim_angle : f32, speed : i32, power : i32) -> usize {
        let id = self.NextId();
        let bullet = Bullet {
            info : ObjectInfo { id, otype : EOBJECT::OTYPE::BULLET, owner_id, pos },
            aim_angle,
            speed,
            power
        };
        self.objects.push(ObjectKind::Bullet(bullet));
        id
    }

    pub fn CountEnemies(&self) -> usize {
        self.minions.iter().filter(|m| matches!(m.mode, EMINION::MODE::ENEMY)).count()
    }

    pub fn GetMinion(&self, id : usize) -> Option<&Minion> {
        self.minions.iter().find(|m| m.id == id)
    }

    pub fn GetMinionMut(&mut self, id : usize) -> Option<&mut Minion> {
        self.minions.iter_mut().find(|m| m.id == id)
    }

    // 오브젝트(투사체 등) 이동 + 맵 밖으로 나가면 제거
    pub fn UpdateObjects(&mut self) {
        for obj in self.objects.iter_mut() {
            obj.Update();
        }

        self.objects.retain(|obj| {
            let pos = obj.GetInfo().pos;
            pos.x >= 0 && pos.x <= CONSTANTS::TILE_LIMIT_X && pos.y >= 0 && pos.y <= CONSTANTS::TILE_LIMIT_Y
        });
    }

    // 투사체 vs 적 미니언 충돌 판정 + 데미지 적용, 명중한 투사체는 제거
    pub fn ProcessBulletCollisions(&mut self) {
        let mut hit_object_ids = Vec::new();

        for obj in self.objects.iter() {
            let ObjectKind::Bullet(bullet) = obj else { continue; };

            for minion in self.minions.iter_mut() {
                if !matches!(minion.mode, EMINION::MODE::ENEMY) {
                    continue;
                }
                if Some(minion.id) == bullet.info.owner_id {
                    continue;
                }

                let dx = (minion.actorInfo.geometry.x - bullet.info.pos.x) as f32;
                let dy = (minion.actorInfo.geometry.y - bullet.info.pos.y) as f32;
                if dx * dx + dy * dy <= (CONSTANTS::HIT_RADIUS * CONSTANTS::HIT_RADIUS) as f32 {
                    DamageInterface::ApplyDamageDirect(minion, bullet.power);
                    hit_object_ids.push(bullet.info.id);
                    break;
                }
            }
        }

        self.objects.retain(|obj| !hit_object_ids.contains(&obj.GetInfo().id));
    }

    // 체력이 0 이하인 미니언 제거
    pub fn RemoveDeadMinions(&mut self) {
        self.minions.retain(|m| m.actorInfo.status.health > 0);
    }
}
