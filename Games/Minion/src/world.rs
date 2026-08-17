use crate::minion::{Minion, EMINION};
use crate::config::CONSTANTS;

pub struct World
{
    pub minions : Vec<Minion>
}

impl World
{
    pub fn New() -> Self {
        World { minions : Vec::new() }
    }

    pub fn SpawnMinion(&mut self, kind : EMINION::KIND) -> Option<usize> {
        if matches!(kind, EMINION::KIND::ENEMY_MINI_BALL | EMINION::KIND::ENEMY_BOSS_RECT)
            && self.CountEnemies() >= CONSTANTS::MAX_ENEMY_COUNT as usize {
            return None;
        }

        let id = self.minions.len();
        let mut minion = Minion::New(id, kind);
        minion.Init();
        self.minions.push(minion);
        Some(id)
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
}
