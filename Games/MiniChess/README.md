# MiniChess

King과 Pawn만 존재하는 TCP 2인 대전 미니 체스.

- King이 잡히면 승부 결정
- 모든 기물은 상하좌우 1칸 이동
- White가 선공

---

## 빌드

```bash
cargo build --release
```

---

## 인간 VS 인간

**터미널 2개** 필요. host가 TCP 서버, join이 접속한다.

**Step 1** — 터미널 A: 방 만들기 (White·선공)
```bash
cargo run --release -- host --port 9500
# "포트 9500에서 상대 접속 대기 중…" 메시지 후 대기
```

**Step 2** — 터미널 B: 방 참가 (Black·후공)
```bash
cargo run --release -- join 127.0.0.1:9500
# 연결되면 양쪽 터미널에서 게임 시작
```

---

## 인간 VS Ouroboros 에이전트

**터미널 2개** 필요. MiniChess가 TCP 서버(방 개설), Ouroboros가 접속(join)한다.  
**항상 MiniChess를 먼저 실행**해야 한다. 순서가 반대면 Ouroboros가 접속할 대상을 찾지 못해 실패한다.

---

### 인간이 White (선공) — 기본값

**Step 1** — 터미널 A: 게임 실행 (에이전트 접속 대기)
```bash
cd Games/MiniChess
cargo run --release -- ai --ouroboros-port 9000
# "포트 9000에서 Ouroboros 접속 대기 중…" 메시지 후 대기
```

**Step 2** — 터미널 B: 에이전트 접속
```bash
cd Ouroboros
cargo run --release -- 127.0.0.1:9000 "체스에서 이겨라" \
  --action-space dynamic \
  --rulebook ../Games/MiniChess/Rule/RULEBOOK.md \
  --llm-model phi4-mini
# Ouroboros가 접속하면 양쪽 터미널에서 게임 시작
```

**Step 3** — 터미널 A에서 인간(White) 플레이  
인간이 먼저 이동하고, Ouroboros(Black)가 응답한다.

---

### Ouroboros가 White (선공)

**Step 1** — 터미널 A: 게임 실행 (에이전트 접속 대기)
```bash
cd Games/MiniChess
cargo run --release -- ai --ouroboros-port 9000 --ai-side white
# "포트 9000에서 Ouroboros 접속 대기 중…" 메시지 후 대기
```

**Step 2** — 터미널 B: 에이전트 접속
```bash
cd Ouroboros
cargo run --release -- 127.0.0.1:9000 "체스에서 이겨라" \
  --action-space dynamic \
  --rulebook ../Games/MiniChess/Rule/RULEBOOK.md \
  --llm-model phi4-mini
# Ouroboros가 접속하면 양쪽 터미널에서 게임 시작
```

**Step 3** — 터미널 A에서 인간(Black) 플레이  
Ouroboros(White)가 먼저 이동하고, 인간(Black)이 응답한다.

---

## 조작법

```
이동 입력: <col> <row> <방향>
방향: w=위  a=좌  s=아래  d=우
예:  2 4 w   →  (2,4) 기물을 위로 한 칸
종료: q
```

좌표는 좌상단이 (0, 0), x는 오른쪽으로, y는 아래쪽으로 증가한다.

---

## 옵션

### host
| 옵션 | 기본값 | 설명 |
|------|--------|------|
| `--width W` | 6 | 보드 가로 |
| `--height H` | 6 | 보드 세로 |
| `--pawns N` | 4 | 진영당 Pawn 수 |
| `--port P` | 9500 | 대기 포트 |

### ai
| 옵션 | 기본값 | 설명 |
|------|--------|------|
| `--width W` | 6 | 보드 가로 |
| `--height H` | 6 | 보드 세로 |
| `--pawns N` | 4 | 진영당 Pawn 수 |
| `--ouroboros-port P` | 9000 | Ouroboros 대기 포트 |
| `--ai-side black\|white` | black | Ouroboros가 담당할 진영 |
