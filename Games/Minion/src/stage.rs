use crate::enemy;
use crate::world::World;
use crate::common::Geometry;
use crate::minion::EMINION;
use macroquad::prelude::*;



#[derive(Clone)]
pub struct GameStage
{
    pub level : i32,
    pub enemy_group : enemy::EnemyGroup,
    pub boss_threshold : i32,
    pub kill_count : i32,
    pub boss_id : Option<usize>,
    pub boss_spawned : bool,
    pub cleared : bool
}

impl GameStage
{
    pub fn New(boss_threshold : i32) -> Self {
        return GameStage {
            level : 0,
            enemy_group : enemy::EnemyGroup::New(),
            boss_threshold,
            kill_count : 0,
            boss_id : None,
            boss_spawned : false,
            cleared : false
        }
    }

    pub fn Start() {

    }

    pub fn Run(&mut self, world : &mut World) {
        for dead in world.RemoveDeadMinions() {
            if !matches!(dead.mode, EMINION::MODE::ENEMY) {
                continue;
            }

            if Some(dead.id) == self.boss_id {
                self.cleared = true;
            } else {
                self.kill_count += 1;
            }
        }

        if !self.boss_spawned && self.kill_count >= self.boss_threshold {
            if let Some(id) = Self::SpawnEnemyAt(world, EMINION::KIND::ENEMY_BOSS_RECT) {
                self.boss_id = Some(id);
                self.boss_spawned = true;
            }
        }

        if !self.boss_spawned {
            for kind in self.enemy_group.Tick() {
                Self::SpawnEnemyAt(world, kind);
            }
        }
    }

    fn SpawnEnemyAt(world : &mut World, kind : EMINION::KIND) -> Option<usize> {
        let id = world.SpawnMinion(kind)?;
        let minion = world.GetMinionMut(id)?;

        minion.actorInfo.geometry = Geometry {
            x : rand::gen_range(0, screen_width() as i32),
            y : rand::gen_range(0, screen_height() as i32)
        };

        Some(id)
    }

    pub fn End() {

    }
}

pub struct GameStageManager
{
    pub stages : Vec<GameStage>,
    pub current_index : usize
}

impl GameStageManager
{
    pub fn New() -> Self {
        return GameStageManager { stages: Vec::new(), current_index : 0 }
    }

    pub fn AddStage(&mut self, stage : GameStage) {
        self.stages.push(stage);
    }

    pub fn Current(&mut self) -> Option<&mut GameStage> {
        self.stages.get_mut(self.current_index)
    }

    pub fn Peek(&self) -> Option<&GameStage> {
        self.stages.get(self.current_index)
    }

    pub fn IsFinalStage(&self) -> bool {
        self.current_index + 1 >= self.stages.len()
    }

    // 다음 스테이지가 있으면 넘어가고 true, 이미 마지막 스테이지면 false
    pub fn AdvanceStage(&mut self) -> bool {
        if self.current_index + 1 < self.stages.len() {
            self.current_index += 1;
            true
        } else {
            false
        }
    }
}