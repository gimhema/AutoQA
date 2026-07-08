# Invader

좌우 이동과 탄환 발사만으로 상단 블록을 모두 파괴하는 CLI 슈팅 게임.

- 이동은 좌/우 1칸씩만 가능
- 탄환은 발사 후 일정 시간(0.4초) 동안 재발사 불가 (연사 방지)
- 제한시간 내에 상단 블록을 모두 파괴하면 승리, 시간이 끝나면 패배

---

## 빌드

```bash
cd Games/Invader
cargo build --release
```

---

## 실행 (사람이 직접 플레이)

```bash
cargo run --release
# 또는
cargo run --release -- play
```

실행하면 게임 시작 전에 아래 두 값을 입력받는다. 그냥 Enter를 누르면 기본값이 적용된다.

```
=== Invader 설정 ===
제한시간(초) (기본 60):
블록 갯수 (기본 20, 최대 150):
```

- **제한시간(초)**: 게임이 유지되는 시간. 이 시간 안에 블록을 모두 파괴하지 못하면 패배.
- **블록 갯수**: 상단에 배치할 블록 수. 블록의 위치(가로 30칸 × 세로 5줄 영역 내)는 매 게임마다 랜덤하게 배치되며, 최대치(150)를 넘게 입력하면 최대치로 조정된다.

---

## 조작법

| 키 | 동작 |
|----|------|
| `A` | 왼쪽으로 1칸 이동 |
| `D` | 오른쪽으로 1칸 이동 |
| `S` | 탄환 발사 (발사 후 0.4초 동안 재발사 불가) |
| `Q` / `Esc` | 게임 종료 |

화면 구성:

```
[ Invader ]  남은 시간:  45s   남은 블록:  12/20    (A/D 이동, S 발사, Q 종료)
------------------------------
..............................
.......#....#........#........   <- 블록(#) 배치 영역
..............................
...............|...............  <- 발사된 탄환(|)
..............................
..............................
..............................
...............A...............  <- 플레이어(A)
```

---

## 승리 / 패배 조건

- **승리**: 제한시간이 끝나기 전에 화면에 남은 블록(`#`)이 0개가 되면 승리.
- **패배**: 블록이 남아있는 상태로 제한시간이 0초가 되면 패배.
- `Q` 또는 `Esc`를 누르면 승패 판정 없이 즉시 종료된다.

---

## Ouroboros 에이전트 자동 플레이

Invader는 턴이 없는 실시간 1인 게임이라, MiniChess의 `ai` 모드와 달리 사람과 번갈아
플레이하지 않는다. `ai` 모드에서는 **Ouroboros가 게임 전체를 담당**하고, 터미널은 관전용으로만
쓰인다(`Q`/`Esc`로 조기 종료만 가능).

**터미널 2개** 필요. **항상 Invader를 먼저 실행**해야 한다. 순서가 반대면 Ouroboros가 접속할
대상을 찾지 못해 실패한다.

**Step 1** — 터미널 A: 게임 실행 (에이전트 접속 대기)
```bash
cd Games/Invader
cargo run --release -- ai --time-limit 60 --blocks 20 --ouroboros-port 9000
# "포트 9000에서 Ouroboros 접속 대기 중…" 메시지 후 대기
```

**Step 2** — 터미널 B: 에이전트 접속
```bash
cd Ouroboros
cargo run --release -- 127.0.0.1:9000 "제한시간 안에 블록을 모두 파괴해라" \
  --action-space dynamic \
  --rulebook ../Games/Invader/Rule/RULEBOOK.md \
  --llm-model phi4-mini
# Ouroboros가 접속하면 터미널 A에서 게임이 시작되고, Ouroboros가 자동으로 플레이한다
```

### ai 모드 옵션

| 옵션 | 기본값 | 설명 |
|------|--------|------|
| `--time-limit N` | 60 | 제한시간(초) |
| `--blocks N` | 20 | 블록 갯수 (최대 150, 초과 입력 시 자동 조정) |
| `--ouroboros-port P` | 9000 | Ouroboros 대기 포트 |

### 관측/액션 포맷

게임은 매 틱 아래와 같은 관측을 Ouroboros에 보낸다. `valid_actions`는 그 순간 실제로
가능한 행동만 담는다 (쿨다운 중이면 `shoot`이 빠짐).

```json
{
  "width": 30, "height": 20,
  "player_x": 15, "player_y": 19,
  "remaining_blocks": 12, "total_blocks": 20,
  "remaining_time_ms": 45230,
  "can_shoot": true,
  "blocks":  [{"x": 3, "y": 1}],
  "bullets": [{"x": 15, "y": 10}],
  "valid_actions": [
    {"action": "move_left",  "resulting_x": 14, "blocks_in_column": 1},
    {"action": "move_right", "resulting_x": 16, "blocks_in_column": 0},
    {"action": "shoot", "aligned_blocks": 1},
    {"action": "stay", "blocks_in_column": 0}
  ]
}
```

Ouroboros는 `valid_actions` 항목 중 하나를 그대로 되돌려 보낸다. 게임은 `action` 필드만
읽고 나머지 피처는 무시한다.
