mod common;
mod common_logic;
mod controller;
mod enemy;
mod minion;
mod config;
mod perks;

use common::Geometry;
use macroquad::prelude::*;

#[macroquad::main("Minion")]
async fn main() {
    let mut player_pos = Geometry { x: 400, y: 300 };
    let speed = 5;

    loop {
        clear_background(BLACK);

        if is_key_down(KeyCode::W) { player_pos.y -= speed; }
        if is_key_down(KeyCode::S) { player_pos.y += speed; }
        if is_key_down(KeyCode::A) { player_pos.x -= speed; }
        if is_key_down(KeyCode::D) { player_pos.x += speed; }

        let (mouse_x, mouse_y) = mouse_position();
        let dx = mouse_x - player_pos.x as f32;
        let dy = mouse_y - player_pos.y as f32;
        let aim_angle = dy.atan2(dx);

        draw_circle(player_pos.x as f32, player_pos.y as f32, 15.0, YELLOW);

        let aim_len = 40.0;
        let aim_end_x = player_pos.x as f32 + aim_angle.cos() * aim_len;
        let aim_end_y = player_pos.y as f32 + aim_angle.sin() * aim_len;
        draw_line(player_pos.x as f32, player_pos.y as f32, aim_end_x, aim_end_y, 3.0, RED);

        next_frame().await;
    }
}
