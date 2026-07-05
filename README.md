# AutoQA

LLM 기반 게임 자동 플레이 / QA 에이전트 프레임워크.

**Ouroboros**가 게임과 TCP로 연결해 관측을 받고, 로컬 LLM이 생성한 policy로 액션을 보낸다.

---

## 구조

```
AutoQA/
├── Ouroboros/          # AI 에이전트 (Rust)
├── External/rust/      # 게임 연동 SDK (ouroboros-link 크레이트)
└── Games/
    └── MiniChess/      # 예제 게임 (Rust)
```

### 동작 방식

```
게임 (TCP 서버)  ──관측──►  Ouroboros
                ◄──액션──
```

- **빠른 루프 (~60Hz)**: 현재 policy로 즉시 액션 결정·전송
- **느린 루프 (~5s)**: 로컬 LLM이 의도 부합을 평가하고 policy를 재생성

---

## 사전 준비

### 1. Rust 설치
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. 로컬 LLM 서버 (Ollama 권장)
```bash
# Ollama 설치: https://ollama.com
ollama pull phi4-mini    # 기본값 (~2.5GB, 경량)
# 또는
ollama pull qwen2.5:7b   # 더 높은 JSON 품질 (~5GB)

ollama serve             # 서버 실행 (기본 포트 11434)
```

> **최소 사양**: RAM 16GB, GPU 없어도 동작 (CPU 추론)
> `llama3.2:1b`는 너무 작아 policy JSON이 자주 깨질 수 있음

---

## 빌드

```bash
# Ouroboros 빌드
cd Ouroboros && cargo build --release

# MiniChess 빌드
cd Games/MiniChess && cargo build --release
```

---

## 실행: MiniChess 2인 대전 (인간 vs 인간)

**터미널 1 — 방 만들기 (host, White·선공)**
```bash
cd Games/MiniChess
cargo run --release -- host --port 9500
```

**터미널 2 — 방 참가 (join, Black·후공)**
```bash
cd Games/MiniChess
cargo run --release -- join 127.0.0.1:9500
```

---

## 실행: MiniChess + Ouroboros

MiniChess AI 모드에서 인간이 White(선공), Ouroboros가 Black(후공)을 담당한다.

**터미널 1 — MiniChess 실행** (Ouroboros 접속 대기)
```bash
cd Games/MiniChess
cargo run --release -- ai --ouroboros-port 9000
```

**터미널 2 — Ouroboros 실행**
```bash
cd Ouroboros
cargo run --release -- 127.0.0.1:9000 "체스에서 이겨라" \
  --action-space dynamic \
  --rulebook ../Games/MiniChess/Rule/RULEBOOK.md \
  --llm-model phi4-mini
```

Ouroboros가 MiniChess에 접속하면 게임이 시작된다.

### MiniChess 조작법
```
이동 입력: <col> <row> <방향>
방향: w=위  a=좌  s=아래  d=우
예: 2 4 w   →  (2,4) 기물을 위로 한 칸
종료: q
```

### 옵션

**MiniChess**
```
ai [--width W]           보드 가로 (기본 6)
   [--height H]          보드 세로 (기본 6)
   [--pawns N]           진영당 Pawn 수 (기본 4)
   [--ouroboros-port P]  Ouroboros 대기 포트 (기본 9000)
   [--ai-side black|white]  Ouroboros 진영 (기본 black)
```

**Ouroboros**
```
<host:port>              MiniChess 주소
<intent>                 에이전트 목표 (자연어)
--llm-endpoint URL       LLM 서버 주소 (기본 http://localhost:11434/v1/chat/completions)
--llm-model NAME         모델명 (기본 phi4-mini)
--action-space VALUE     'dynamic' 또는 JSON 배열
--rulebook PATH          게임 룰북 파일 경로
```

---

## 테스트

```bash
# Ouroboros 단위 테스트 (61개)
cd Ouroboros && cargo test

# MiniChess 단위 테스트 (14개)
cd Games/MiniChess && cargo test
```

---

## External SDK (ouroboros-link)

게임에 Ouroboros 연동을 추가하려면 `External/rust` 크레이트를 사용한다.

```toml
# Cargo.toml
[dependencies]
ouroboros-link = { path = "../../External/rust" }
serde_json = "1"
```

```rust
use ouroboros_link::OuroborosLink;
use serde_json::json;

// 게임이 TCP 서버 역할 — Ouroboros 접속 대기
let mut link = OuroborosLink::accept("0.0.0.0:9000")?;

// 게임 루프
loop {
    let state = json!({ /* 현재 게임 상태 */ });
    link.send_observation(state)?;

    if let Some(action) = link.poll_action() {
        // action.command 를 게임에 적용
    }
}
```

### 액션 자제 (self-throttle)

관측의 `valid_actions` 배열이 비어 있으면 `DynamicDiscretePolicy`가 `None`을 반환해 Ouroboros가 액션을 보내지 않는다. 인간 턴처럼 AI가 개입하지 않아야 하는 구간에 활용한다.

```rust
// 인간 턴: AI 액션 억제
link.send_observation(json!({ ..., "valid_actions": [] }))?;

// AI 턴: 정상 액션
link.send_observation(json!({ ..., "valid_actions": [...] }))?;
```
