mod common;
mod common_logic;
mod controller;
mod enemy;
mod minion;
mod config;
mod perks;

use controller::PlayerController;
use macroquad::prelude::*;

#[macroquad::main("Minion")]
async fn main() {
    let mut player = PlayerController::New();
    player.Init();

    loop {
        clear_background(BLACK);

        player.Update();

        draw_circle(player.pos.x as f32, player.pos.y as f32, 15.0, YELLOW);

        let aim_len = 40.0;
        let aim_end_x = player.pos.x as f32 + player.aim_angle.cos() * aim_len;
        let aim_end_y = player.pos.y as f32 + player.aim_angle.sin() * aim_len;
        draw_line(player.pos.x as f32, player.pos.y as f32, aim_end_x, aim_end_y, 3.0, RED);

        next_frame().await;
    }
}
