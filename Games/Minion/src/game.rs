use crate::common::Geometry;
use crate::controller::PlayerController;
use crate::enemy::EnemyUnitInfo;
use crate::minion::EMINION;
use crate::perks::{self, PerkInfo};
use crate::stage::{GameStage, GameStageManager};
use crate::world::World;
use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum GameFlowState
{
    Playing,
    ChoosingPerk,
    Victory
}

fn BuildStage(boss_threshold : i32, spawn_num : i32, spawn_tick : i32) -> GameStage {
    let mut stage = GameStage::New(boss_threshold);
    stage.enemy_group.AddUnitInfo(EnemyUnitInfo {
        minion_type : EMINION::KIND::ENEMY_MINI_BALL,
        spawn_tick,
        spawn_num
    });
    stage
}

// 사람 조작(main.rs)과 Ouroboros 조작(ouroboros.rs)이 공유하는 게임 상태.
// 입력을 어떻게 받는지는 각 프론트엔드가 결정하고, 여기서는 시뮬레이션/렌더링만 담당한다.
pub struct GameSession
{
    pub world : World,
    pub player : PlayerController,
    pub player_id : usize,
    pub manager : GameStageManager,
    pub flow : GameFlowState,
    pub perk_choices : Vec<PerkInfo>
}

impl GameSession
{
    pub fn New() -> Self {
        let mut world = World::New();
        let player_id = world.SpawnMinion(EMINION::KIND::RED).expect("failed to spawn player");

        if let Some(minion) = world.GetMinionMut(player_id) {
            minion.actorInfo.geometry = Geometry { x: 400, y: 300 };
        }

        let mut player = PlayerController::New(0);
        player.Possess(&mut world, player_id);

        let mut manager = GameStageManager::New();
        manager.AddStage(BuildStage(3, 5, 120));
        manager.AddStage(BuildStage(5, 8, 100));
        manager.AddStage(BuildStage(8, 12, 90));

        GameSession {
            world,
            player,
            player_id,
            manager,
            flow : GameFlowState::Playing,
            perk_choices : perks::AllPerks()
        }
    }

    // 입력 처리 이후 매 프레임 호출: 월드 시뮬레이션 + 스테이지 진행.
    // Playing 상태가 아닐 때(특전 선택/승리)는 판을 멈춘다.
    pub fn Tick(&mut self) {
        if self.flow != GameFlowState::Playing {
            return;
        }

        self.world.UpdateObjects();
        self.world.ProcessBulletCollisions();

        if let Some(stage) = self.manager.Current() {
            stage.Run(&mut self.world);

            if stage.cleared {
                self.flow = if self.manager.IsFinalStage() {
                    GameFlowState::Victory
                } else {
                    GameFlowState::ChoosingPerk
                };
            }
        }
    }

    // 1-based index (화면에 표시되는 번호와 동일)
    pub fn ChoosePerk(&mut self, index : usize) {
        if self.flow != GameFlowState::ChoosingPerk || index == 0 {
            return;
        }
        let Some(perk) = self.perk_choices.get(index - 1).copied() else { return; };

        if let Some(minion) = self.world.GetMinionMut(self.player_id) {
            perks::ApplyPerk(minion, perk);
        }
        self.world.ClearEnemiesAndObjects();
        self.manager.AdvanceStage();
        self.flow = GameFlowState::Playing;
    }

    pub fn Render(&self) {
        clear_background(BLACK);

        let boss_id = self.manager.Peek().and_then(|s| s.boss_id);

        for minion in self.world.minions.iter() {
            let pos = minion.actorInfo.geometry;

            if Some(minion.id) == boss_id {
                let size = 40.0;
                draw_rectangle(pos.x as f32 - size / 2.0, pos.y as f32 - size / 2.0, size, size, PURPLE);
            } else {
                let color = if minion.id == self.player_id { YELLOW } else { RED };
                draw_circle(pos.x as f32, pos.y as f32, 15.0, color);
            }
        }

        for obj in self.world.objects.iter() {
            let pos = obj.GetInfo().pos;
            draw_circle(pos.x as f32, pos.y as f32, 4.0, ORANGE);
        }

        if let Some(minion) = self.world.GetMinion(self.player_id) {
            let pos = minion.actorInfo.geometry;
            let aim_len = 40.0;
            let aim_end_x = pos.x as f32 + self.player.aim_angle.cos() * aim_len;
            let aim_end_y = pos.y as f32 + self.player.aim_angle.sin() * aim_len;
            draw_line(pos.x as f32, pos.y as f32, aim_end_x, aim_end_y, 3.0, WHITE);
        }

        if let Some(stage) = self.manager.Peek() {
            let stage_number = self.manager.current_index + 1;
            draw_text(&format!("Stage {} — Kills: {}/{}", stage_number, stage.kill_count, stage.boss_threshold), 10.0, 20.0, 24.0, WHITE);
            if stage.boss_spawned && !stage.cleared {
                draw_text("BOSS!", 10.0, 44.0, 24.0, RED);
            }
        }

        match self.flow {
            GameFlowState::ChoosingPerk => {
                draw_text("STAGE CLEAR! Choose a perk:", 10.0, 100.0, 24.0, GREEN);
                for (i, perk) in self.perk_choices.iter().enumerate() {
                    draw_text(&format!("{}. {:?} (+{})", i + 1, perk.perk_type, perk.amount), 10.0, 130.0 + i as f32 * 24.0, 20.0, WHITE);
                }
            }
            GameFlowState::Victory => {
                draw_text("VICTORY!", 10.0, 100.0, 32.0, GREEN);
            }
            GameFlowState::Playing => {}
        }
    }
}
