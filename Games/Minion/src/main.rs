mod common;
mod common_logic;
mod controller;
mod enemy;
mod minion;
mod config;
mod perks;
mod world;
mod attack;
mod object;
mod stage;

use common::Geometry;
use controller::PlayerController;
use enemy::EnemyUnitInfo;
use minion::EMINION;
use stage::{GameStage, GameStageManager};
use world::World;
use macroquad::prelude::*;

#[derive(PartialEq)]
enum GameFlowState
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

#[macroquad::main("Minion")]
async fn main() {
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

    let mut flow = GameFlowState::Playing;
    let perk_choices = perks::AllPerks();
    let perk_keys = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4];

    loop {
        clear_background(BLACK);

        match flow {
            GameFlowState::Playing => {
                player.Update(&mut world);

                world.UpdateObjects();
                world.ProcessBulletCollisions();

                if let Some(stage) = manager.Current() {
                    stage.Run(&mut world);

                    if stage.cleared {
                        flow = if manager.IsFinalStage() {
                            GameFlowState::Victory
                        } else {
                            GameFlowState::ChoosingPerk
                        };
                    }
                }
            }
            GameFlowState::ChoosingPerk => {
                for (i, perk) in perk_choices.iter().enumerate() {
                    if i < perk_keys.len() && is_key_pressed(perk_keys[i]) {
                        if let Some(minion) = world.GetMinionMut(player_id) {
                            perks::ApplyPerk(minion, *perk);
                        }
                        world.ClearEnemiesAndObjects();
                        manager.AdvanceStage();
                        flow = GameFlowState::Playing;
                    }
                }
            }
            GameFlowState::Victory => {}
        }

        for minion in world.minions.iter() {
            let pos = minion.actorInfo.geometry;
            let is_boss = manager.Current().is_some_and(|s| Some(minion.id) == s.boss_id);

            if is_boss {
                let size = 40.0;
                draw_rectangle(pos.x as f32 - size / 2.0, pos.y as f32 - size / 2.0, size, size, PURPLE);
            } else {
                let color = if minion.id == player_id { YELLOW } else { RED };
                draw_circle(pos.x as f32, pos.y as f32, 15.0, color);
            }
        }

        for obj in world.objects.iter() {
            let pos = obj.GetInfo().pos;
            draw_circle(pos.x as f32, pos.y as f32, 4.0, ORANGE);
        }

        if let Some(minion) = world.GetMinion(player_id) {
            let pos = minion.actorInfo.geometry;
            let aim_len = 40.0;
            let aim_end_x = pos.x as f32 + player.aim_angle.cos() * aim_len;
            let aim_end_y = pos.y as f32 + player.aim_angle.sin() * aim_len;
            draw_line(pos.x as f32, pos.y as f32, aim_end_x, aim_end_y, 3.0, WHITE);
        }

        let stage_number = manager.current_index + 1;
        if let Some(stage) = manager.Current() {
            draw_text(&format!("Stage {} — Kills: {}/{}", stage_number, stage.kill_count, stage.boss_threshold), 10.0, 20.0, 24.0, WHITE);
            if stage.boss_spawned && !stage.cleared {
                draw_text("BOSS!", 10.0, 44.0, 24.0, RED);
            }
        }

        match flow {
            GameFlowState::ChoosingPerk => {
                draw_text("STAGE CLEAR! Choose a perk:", 10.0, 100.0, 24.0, GREEN);
                for (i, perk) in perk_choices.iter().enumerate() {
                    draw_text(&format!("{}. {:?} (+{})", i + 1, perk.perk_type, perk.amount), 10.0, 130.0 + i as f32 * 24.0, 20.0, WHITE);
                }
            }
            GameFlowState::Victory => {
                draw_text("VICTORY!", 10.0, 100.0, 32.0, GREEN);
            }
            GameFlowState::Playing => {}
        }

        next_frame().await;
    }
}
