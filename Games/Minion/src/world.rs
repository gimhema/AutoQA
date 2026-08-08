use crate::minion::Minion;

pub struct World
{
    pub minions : Vec<Minion>
}

impl World
{
    pub fn New() -> Self {
        World { minions : Vec::new() }
    }

    pub fn SpawnMinion(&mut self) -> usize {
        let id = self.minions.len();
        let mut minion = Minion::New(id);
        minion.Init();
        self.minions.push(minion);
        id
    }

    pub fn GetMinion(&self, id : usize) -> Option<&Minion> {
        self.minions.iter().find(|m| m.id == id)
    }

    pub fn GetMinionMut(&mut self, id : usize) -> Option<&mut Minion> {
        self.minions.iter_mut().find(|m| m.id == id)
    }
}
