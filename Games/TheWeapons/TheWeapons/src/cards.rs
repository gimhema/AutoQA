//! 카드 정의와 소켓 대결 판정 로직.
//!
//! 이 게임의 카드는 검(Sword)/방패(Shield)/창(Spear) 세 종류뿐이다(README 룰북 기준).
//! 모든 카드는 소모형이라, 소켓에 낸 카드는 판정 결과와 무관하게 손에서 사라진다.

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Card {
    Sword,
    Shield,
    Spear,
}

impl Card {
    /// 입력 토큰 한 글자를 카드로 해석한다. s=검 d=방패 p=창.
    pub fn from_short(c: char) -> Option<Card> {
        match c.to_ascii_lowercase() {
            's' => Some(Card::Sword),
            'd' => Some(Card::Shield),
            'p' => Some(Card::Spear),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Card::Sword => "검",
            Card::Shield => "방패",
            Card::Spear => "창",
        }
    }

    /// 와이어 프로토콜(net.rs, ouroboros.rs)에서 공용으로 쓰는 영문 키.
    pub fn as_str(self) -> &'static str {
        match self {
            Card::Sword => "sword",
            Card::Shield => "shield",
            Card::Spear => "spear",
        }
    }

    /// `as_str`의 역변환.
    pub fn parse_str(s: &str) -> Option<Card> {
        match s {
            "sword" => Some(Card::Sword),
            "shield" => Some(Card::Shield),
            "spear" => Some(Card::Spear),
            _ => None,
        }
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// 한 플레이어가 보유한 카드 수량(검/방패/창). 전체 구성은 게임 시작 전 세팅으로 정해지며
/// 양측이 동일한 구성을 받는다.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Hand {
    pub sword: u32,
    pub shield: u32,
    pub spear: u32,
}

impl Hand {
    pub fn new(sword: u32, shield: u32, spear: u32) -> Self {
        Self { sword, shield, spear }
    }

    fn count_mut(&mut self, card: Card) -> &mut u32 {
        match card {
            Card::Sword => &mut self.sword,
            Card::Shield => &mut self.shield,
            Card::Spear => &mut self.spear,
        }
    }

    pub fn total(&self) -> u32 {
        self.sword + self.shield + self.spear
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// `assignment`(소켓별 카드, `None`=빈 소켓)이 현재 손으로 감당 가능한지 검사한다.
    pub fn can_afford(&self, assignment: &[Option<Card>]) -> bool {
        let mut used = Hand::default();
        for card in assignment.iter().flatten() {
            *used.count_mut(*card) += 1;
        }
        used.sword <= self.sword && used.shield <= self.shield && used.spear <= self.spear
    }

    /// 이번 턴 낸 카드를 손에서 제거한다. 호출 전 `can_afford`로 검증되어 있어야 한다.
    pub fn play(&mut self, assignment: &[Option<Card>]) {
        for card in assignment.iter().flatten() {
            *self.count_mut(*card) -= 1;
        }
    }
}

/// 공격자의 카드가 방어자의 카드를 뚫고 1 데미지를 주는지 여부.
///
/// - 검: 상대가 방패면 막힘, 그 외(빈 소켓/검/창)엔 관통.
/// - 방패: 공격력이 없다(항상 막지 못함이 아니라 애초에 데미지가 없음).
/// - 창: 방패를 무시하고 항상 관통.
fn pierces(attacker: Option<Card>, defender: Option<Card>) -> bool {
    match attacker {
        None => false,
        Some(Card::Shield) => false,
        Some(Card::Sword) => defender != Some(Card::Shield),
        Some(Card::Spear) => true,
    }
}

/// 한 소켓의 대결 결과를 `(내 HP 손실, 상대 HP 손실)`로 반환한다. 각 값은 0 또는 1이다.
///
/// 각 카드의 데미지는 상대에게만 적용되므로, 소켓 전체 결과는 "내 카드가 상대를 뚫는가"와
/// "상대 카드가 나를 뚫는가"를 독립적으로 계산해 합치면 된다. 이 분해 덕분에
/// `resolve_socket(a, b)`와 `resolve_socket(b, a)`가 항상 서로 뒤집힌 관계로 일관된다.
pub fn resolve_socket(my_card: Option<Card>, their_card: Option<Card>) -> (i32, i32) {
    let my_loss = pierces(their_card, my_card) as i32;
    let opp_loss = pierces(my_card, their_card) as i32;
    (my_loss, opp_loss)
}

#[cfg(test)]
mod tests {
    use super::*;

    // README 상황별 결과표를 그대로 검증한다.

    #[test]
    fn sword_vs_empty_deals_damage() {
        assert_eq!(resolve_socket(Some(Card::Sword), None), (0, 1));
    }

    #[test]
    fn sword_vs_sword_mutual_damage() {
        assert_eq!(resolve_socket(Some(Card::Sword), Some(Card::Sword)), (1, 1));
    }

    #[test]
    fn shield_blocks_sword_both_consumed_no_damage() {
        assert_eq!(resolve_socket(Some(Card::Sword), Some(Card::Shield)), (0, 0));
        assert_eq!(resolve_socket(Some(Card::Shield), Some(Card::Sword)), (0, 0));
    }

    #[test]
    fn sword_vs_spear_mutual_damage() {
        assert_eq!(resolve_socket(Some(Card::Sword), Some(Card::Spear)), (1, 1));
        assert_eq!(resolve_socket(Some(Card::Spear), Some(Card::Sword)), (1, 1));
    }

    #[test]
    fn shield_vs_shield_no_effect() {
        assert_eq!(resolve_socket(Some(Card::Shield), Some(Card::Shield)), (0, 0));
    }

    #[test]
    fn spear_ignores_shield() {
        assert_eq!(resolve_socket(Some(Card::Shield), Some(Card::Spear)), (1, 0));
        assert_eq!(resolve_socket(Some(Card::Spear), Some(Card::Shield)), (0, 1));
    }

    #[test]
    fn spear_vs_empty_deals_damage() {
        assert_eq!(resolve_socket(Some(Card::Spear), None), (0, 1));
        assert_eq!(resolve_socket(None, Some(Card::Spear)), (1, 0));
    }

    #[test]
    fn spear_vs_spear_mutual_damage() {
        assert_eq!(resolve_socket(Some(Card::Spear), Some(Card::Spear)), (1, 1));
    }

    #[test]
    fn empty_vs_empty_no_damage() {
        assert_eq!(resolve_socket(None, None), (0, 0));
    }

    #[test]
    fn hand_afford_and_play() {
        let mut hand = Hand::new(1, 1, 0);
        let assignment = vec![Some(Card::Sword), Some(Card::Shield)];
        assert!(hand.can_afford(&assignment));
        hand.play(&assignment);
        assert_eq!(hand, Hand::new(0, 0, 0));
    }

    #[test]
    fn hand_rejects_overspend() {
        let hand = Hand::new(1, 0, 0);
        let assignment = vec![Some(Card::Sword), Some(Card::Sword)];
        assert!(!hand.can_afford(&assignment));
    }

    #[test]
    fn empty_slots_are_free() {
        let hand = Hand::new(0, 0, 0);
        assert!(hand.can_afford(&[None, None, None]));
    }
}
