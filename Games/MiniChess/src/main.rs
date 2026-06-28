use ouroboros_link::OuroborosLink;
use serde_json::json;

use std::time::Duration;
use std::thread;

const DEFAULT_PORT: u16 = 9000;

fn main() {
    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    eprintln!("[MiniChess] 포트 {port}에서 Ouroboros 접속 대기 중…");
    let mut link = OuroborosLink::accept(("0.0.0.0", port))
        .expect("failed to bind/accept");
    eprintln!("[MiniChess] Ouroboros 연결됨, 게임 시작");

    // TODO: 실제 체스 게임 상태로 교체
    let mut tick = 0u64;

    while link.is_connected() {
        let state = json!({
            "tick": tick,
            "board": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR",
            "turn": if tick % 2 == 0 { "white" } else { "black" },
        });

        if let Err(e) = link.send_observation(state) {
            eprintln!("[MiniChess] 전송 실패: {e}");
            break;
        }

        if let Some(action) = link.poll_action() {
            eprintln!("[MiniChess] 액션 수신: {}", action.command);
            // TODO: action.command를 파싱해 게임 상태에 적용
        }

        tick += 1;
        thread::sleep(Duration::from_millis(16));
    }

    eprintln!("[MiniChess] 종료 (tick={tick})");
}
