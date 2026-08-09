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

use common::Geometry;
use controller::PlayerController;
use minion::EMINION;
use world::World;
use macroquad::prelude::*;

#[macroquad::main("Minion")]
async fn main() {
    let mut world = World::New();
    let player_id = world.SpawnMinion(EMINION::KIND::RED);

    if let Some(minion) = world.GetMinionMut(player_id) {
        minion.actorInfo.geometry = Geometry { x: 400, y: 300 };
    }

    let mut player = PlayerController::New(0);
    player.Possess(&mut world, player_id);

    loop {
        clear_background(BLACK);

        player.Update(&mut world);

        if let Some(minion) = world.GetMinion(player_id) {
            let pos = minion.actorInfo.geometry;
            draw_circle(pos.x as f32, pos.y as f32, 15.0, YELLOW);

            let aim_len = 40.0;
            let aim_end_x = pos.x as f32 + player.aim_angle.cos() * aim_len;
            let aim_end_y = pos.y as f32 + player.aim_angle.sin() * aim_len;
            draw_line(pos.x as f32, pos.y as f32, aim_end_x, aim_end_y, 3.0, RED);
        }

        next_frame().await;
    }
}
