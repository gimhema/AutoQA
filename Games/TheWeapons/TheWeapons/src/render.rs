//! 매치 상태의 CLI 렌더링. 매 턴 화면을 지우고 제자리에서 다시 그린다.

use crate::cards::Card;
use crate::game::{Match, Outcome};

/// 화면 전체 지우기 + 커서를 좌상단으로.
pub const CLEAR: &str = "\x1b[2J\x1b[H";

fn card_glyph(card: Option<Card>) -> String {
    match card {
        None => "[ 빈 ]".to_string(),
        Some(c) => format!("[{c}]"),
    }
}

/// 현재 매치 상태(HP, 손패, 직전 턴 결과, 승패)를 한 화면으로 렌더링한다.
pub fn render(m: &Match) -> String {
    let mut out = String::new();
    out.push_str(&format!("=== TheWeapons — {}턴 ===\n", m.turn_number + 1));
    out.push_str(&format!("내 HP: {:>3}   상대 HP: {:>3}\n", m.my_hp, m.opp_hp));
    out.push_str(&format!(
        "내 손패        — 검:{} 방패:{} 창:{}\n",
        m.my_hand.sword, m.my_hand.shield, m.my_hand.spear
    ));
    out.push_str(&format!(
        "상대 손패(공개 정보로 추적) — 검:{} 방패:{} 창:{}\n",
        m.opp_hand.sword, m.opp_hand.shield, m.opp_hand.spear
    ));

    if let Some(log) = &m.last_turn {
        out.push_str("\n직전 턴 결과:\n");
        for (i, (mine, theirs)) in log.my_play.iter().zip(log.their_play.iter()).enumerate() {
            out.push_str(&format!(
                "  소켓{}: 나 {} vs 상대 {}\n",
                i + 1,
                card_glyph(*mine),
                card_glyph(*theirs)
            ));
        }
        out.push_str(&format!(
            "  → 내 HP -{}, 상대 HP -{}\n",
            log.my_hp_loss, log.their_hp_loss
        ));
    }

    match m.outcome {
        Some(Outcome::Win) => out.push_str("\n게임 종료 — 당신의 승리! 🎉\n"),
        Some(Outcome::Lose) => out.push_str("\n게임 종료 — 패배…\n"),
        Some(Outcome::Draw) => out.push_str("\n게임 종료 — 무승부\n"),
        None => {}
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Config;

    fn config() -> Config {
        Config {
            socket_count: 2,
            initial_hp: 10,
            sword_count: 5,
            shield_count: 5,
            spear_count: 5,
        }
    }

    #[test]
    fn render_shows_hp_and_hand() {
        let m = Match::new(config());
        let s = render(&m);
        assert!(s.contains("내 HP:  10"));
        assert!(s.contains("검:5"));
    }

    #[test]
    fn render_shows_last_turn_and_outcome() {
        let mut m = Match::new(config());
        m.apply_turn(vec![Some(Card::Sword), None], vec![None, Some(Card::Spear)]);
        let s = render(&m);
        assert!(s.contains("소켓1: 나 [검] vs 상대 [ 빈 ]"));
        assert!(s.contains("소켓2: 나 [ 빈 ] vs 상대 [창]"));
    }

    #[test]
    fn render_shows_win_message() {
        let mut m = Match::new(Config { initial_hp: 1, ..config() });
        m.apply_turn(vec![Some(Card::Sword), None], vec![None, None]);
        let s = render(&m);
        assert!(s.contains("당신의 승리"));
    }
}
