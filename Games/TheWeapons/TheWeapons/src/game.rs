//! TheWeapons 세팅과 진행 상태(한 플레이어 시점에서 본 매치 상태).
//!
//! 이 게임은 정보가 전부 공개된다: 카드 구성(세팅)과 매 턴 공개되는 카드로부터
//! 상대 손패도 결정론적으로 추적할 수 있어, `Match`는 `my_hand`와 `opp_hand`를
//! 둘 다 들고 있는다.

use crate::cards::{resolve_socket, Card, Hand};

/// 게임 시작 전 합의하는 세팅. 양측에 동일하게 적용된다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    pub socket_count: usize,
    pub initial_hp: i32,
    pub sword_count: u32,
    pub shield_count: u32,
    pub spear_count: u32,
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.socket_count == 0 {
            return Err("소켓 수는 1 이상이어야 합니다".into());
        }
        if self.initial_hp <= 0 {
            return Err("초기 HP는 1 이상이어야 합니다".into());
        }
        Ok(())
    }

    pub fn starting_hand(&self) -> Hand {
        Hand::new(self.sword_count, self.shield_count, self.spear_count)
    }
}

/// 게임 종료 시 내 시점에서 본 결과.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Win,
    Lose,
    Draw,
}

/// 직전 턴의 소켓별 공개 결과(렌더링용 기록).
#[derive(Clone, Debug)]
pub struct TurnLog {
    pub my_play: Vec<Option<Card>>,
    pub their_play: Vec<Option<Card>>,
    pub my_hp_loss: i32,
    pub their_hp_loss: i32,
}

pub struct Match {
    pub config: Config,
    pub my_hp: i32,
    pub opp_hp: i32,
    pub my_hand: Hand,
    pub opp_hand: Hand,
    pub turn_number: u32,
    pub outcome: Option<Outcome>,
    pub last_turn: Option<TurnLog>,
}

impl Match {
    pub fn new(config: Config) -> Self {
        let hand = config.starting_hand();
        Match {
            config,
            my_hp: config.initial_hp,
            opp_hp: config.initial_hp,
            my_hand: hand,
            opp_hand: hand,
            turn_number: 0,
            outcome: None,
            last_turn: None,
        }
    }

    pub fn is_over(&self) -> bool {
        self.outcome.is_some()
    }

    /// 이번 턴 배치가 소켓 수와 내 손패 범위에 맞는지 검사한다.
    pub fn can_afford(&self, assignment: &[Option<Card>]) -> bool {
        assignment.len() == self.config.socket_count && self.my_hand.can_afford(assignment)
    }

    /// 양측 소켓 배치를 동시에 적용한다: 카드 소모 → 소켓별 판정 → HP 반영 → 종료 조건 확인.
    ///
    /// 호출 전 두 배치 모두 길이가 `socket_count`와 같고, `my_play`는 `can_afford`를
    /// 통과했어야 한다(호출부 책임).
    pub fn apply_turn(&mut self, my_play: Vec<Option<Card>>, their_play: Vec<Option<Card>>) {
        self.my_hand.play(&my_play);
        self.opp_hand.play(&their_play);

        let mut my_loss_total = 0;
        let mut their_loss_total = 0;
        for (mine, theirs) in my_play.iter().zip(their_play.iter()) {
            let (my_loss, their_loss) = resolve_socket(*mine, *theirs);
            my_loss_total += my_loss;
            their_loss_total += their_loss;
        }

        self.my_hp -= my_loss_total;
        self.opp_hp -= their_loss_total;
        self.turn_number += 1;
        self.last_turn = Some(TurnLog {
            my_play,
            their_play,
            my_hp_loss: my_loss_total,
            their_hp_loss: their_loss_total,
        });

        self.outcome = self.check_outcome();
    }

    fn check_outcome(&self) -> Option<Outcome> {
        let i_dead = self.my_hp <= 0;
        let opp_dead = self.opp_hp <= 0;

        // 같은 턴에 양측 모두 HP가 0 이하가 되면 무승부로 처리한다(룰북에 명시되지 않은
        // 동시 결착 상황에 대한 보수적 기본값).
        if i_dead && opp_dead {
            return Some(Outcome::Draw);
        }
        if opp_dead {
            return Some(Outcome::Win);
        }
        if i_dead {
            return Some(Outcome::Lose);
        }

        if self.my_hand.is_empty() && self.opp_hand.is_empty() {
            return Some(match self.my_hp.cmp(&self.opp_hp) {
                std::cmp::Ordering::Greater => Outcome::Win,
                std::cmp::Ordering::Less => Outcome::Lose,
                std::cmp::Ordering::Equal => Outcome::Draw,
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(socket_count: usize, hp: i32, sword: u32, shield: u32, spear: u32) -> Config {
        Config {
            socket_count,
            initial_hp: hp,
            sword_count: sword,
            shield_count: shield,
            spear_count: spear,
        }
    }

    #[test]
    fn config_validation() {
        assert!(config(3, 10, 5, 5, 5).validate().is_ok());
        assert!(config(0, 10, 5, 5, 5).validate().is_err());
        assert!(config(3, 0, 5, 5, 5).validate().is_err());
    }

    #[test]
    fn opponent_dying_wins_the_match() {
        let mut m = Match::new(config(1, 1, 5, 5, 5));
        m.apply_turn(vec![Some(Card::Sword)], vec![None]);
        assert_eq!(m.outcome, Some(Outcome::Win));
        assert_eq!(m.opp_hp, 0);
    }

    #[test]
    fn my_death_loses_the_match() {
        let mut m = Match::new(config(1, 1, 5, 5, 5));
        m.apply_turn(vec![None], vec![Some(Card::Sword)]);
        assert_eq!(m.outcome, Some(Outcome::Lose));
    }

    #[test]
    fn simultaneous_death_is_a_draw() {
        let mut m = Match::new(config(1, 1, 5, 5, 5));
        m.apply_turn(vec![Some(Card::Sword)], vec![Some(Card::Sword)]);
        assert_eq!(m.outcome, Some(Outcome::Draw));
    }

    #[test]
    fn hand_exhaustion_favors_higher_hp() {
        // 소켓 1개, 카드 1장씩(방패만). 서로 방패만 내면 데미지 없이 카드가 고갈된다.
        let mut m = Match::new(config(1, 10, 0, 1, 0));
        m.my_hp = 8;
        m.opp_hp = 5;
        m.apply_turn(vec![Some(Card::Shield)], vec![Some(Card::Shield)]);
        assert!(m.my_hand.is_empty());
        assert!(m.opp_hand.is_empty());
        assert_eq!(m.outcome, Some(Outcome::Win));
    }

    #[test]
    fn hand_exhaustion_with_equal_hp_is_draw() {
        let mut m = Match::new(config(1, 10, 0, 1, 0));
        m.apply_turn(vec![Some(Card::Shield)], vec![Some(Card::Shield)]);
        assert_eq!(m.outcome, Some(Outcome::Draw));
    }

    #[test]
    fn game_continues_while_hands_remain() {
        let mut m = Match::new(config(2, 10, 5, 5, 5));
        m.apply_turn(vec![Some(Card::Shield), None], vec![Some(Card::Shield), None]);
        assert_eq!(m.outcome, None);
        assert_eq!(m.turn_number, 1);
    }

    #[test]
    fn multi_socket_damage_sums_across_sockets() {
        let mut m = Match::new(config(3, 10, 5, 5, 5));
        // 나: 검 방패 검 / 상대: 창 검 없음
        m.apply_turn(
            vec![Some(Card::Sword), Some(Card::Shield), Some(Card::Sword)],
            vec![Some(Card::Spear), Some(Card::Sword), None],
        );
        // 소켓1: 검 vs 창 -> 둘 다 -1 / 소켓2: 방패 vs 검 -> 0,0 / 소켓3: 검 vs 없음 -> 상대 -1
        assert_eq!(m.my_hp, 9);
        assert_eq!(m.opp_hp, 8);
    }
}
