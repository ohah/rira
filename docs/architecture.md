# Architecture

## 렌더링 파이프라인

```
winit (Cmd 키 캡처) → ratatui (TUI 위젯으로 UI 구성) → wgpu (텍스트 그리드 GPU 렌더링)
```

- **winit**: macOS에서 Cmd 키를 잡으려면 네이티브 윈도우가 필요
- **ratatui**: UI 구성 비용 최소화 — 위젯, 레이아웃, 스타일 생태계 활용
- **wgpu**: ratatui 출력을 모노스페이스 셀 그리드로 GPU 렌더링
- 나중에 GPU 커스텀 렌더링이 필요하면 ratatui를 점진적으로 대체 가능

## 이벤트 루프: 싱글 스레드 메인 + 워커 스레드

```
┌─ 메인 스레드 ──────────────────────────────────┐
│  winit EventLoop                               │
│  ├─ 키/마우스 입력 → EditorState 갱신          │
│  ├─ mpsc 채널에서 워커 결과 수신               │
│  ├─ ratatui로 UI 구성                          │
│  └─ wgpu로 렌더링 (매 프레임, ~3ms / 16.6ms)  │
└────────────────────────────────────────────────┘
        ↕ mpsc 채널
┌─ 워커 스레드들 ────────────────────────────────┐
│  [1] tree-sitter 증분 파싱                     │
│  [2] 파일 읽기/쓰기                            │
│  [3] PTY 읽기 (터미널 패널당 1개)              │
│  [4] Git 상태 감시                             │
│  [5] AI/LLM 요청 처리                          │
└────────────────────────────────────────────────┘
```

- 메인 스레드가 EditorState를 독점 소유 (락 없음)
- 워커 결과는 채널 메시지로 수신하여 메인 스레드에서 반영
- 60fps 기준 프레임 예산 16.6ms, 메인 루프 소요 ~3ms

## 크레이트 구조

```
crates/
├── app/              # winit 이벤트 루프, 진입점, CLI 파서
│
│   ── 순수 로직 (GPU 없이 100% 테스트 가능) ──
├── editor/           # 텍스트 버퍼(ropey), 커서, 선택, undo/redo
├── keymap/           # 키 바인딩 파싱, 매핑, 충돌 감지
├── highlight/        # tree-sitter 파싱, 하이라이트 쿼리 + theme 색상 매핑
├── git/              # Git 상태, diff, stage, commit (git2-rs)
├── terminal-core/    # vte 파싱 + 가상 터미널 그리드 버퍼 (PTY 없이 순수 로직)
├── theme/            # 테마 파싱, 색상 체계, VSCode 테마 임포트
│
│   ── I/O 있지만 독립 테스트 가능 ──
├── terminal-pty/     # portable-pty 래퍼 (PTY 생성/관리만)
│
│   ── UI (ratatui TestBackend로 GPU 없이 테스트) ──
├── ui/               # 파일 트리, 커맨드 팔레트, 탭, diff 뷰 등 UI 위젯
│
│   ── 렌더링 (GPU 필요) ──
├── renderer/         # wgpu + cosmic-text, ratatui Backend 구현
```

## 의존 방향 (단방향, 순환 없음)

```
app
 ├─→ renderer (GPU) ─→ theme
 ├─→ ui (ratatui 위젯)
 │    ├─→ editor (순수 로직)
 │    ├─→ highlight (순수 로직) ─→ theme
 │    ├─→ git (I/O)
 │    └─→ terminal-core (순수 로직)
 ├─→ terminal-pty (I/O) ─→ terminal-core
 └─→ keymap (순수 로직)
```

**renderer는 app만 의존한다.** 나머지 크레이트는 GPU를 모른다.

## 기술 스택

| 레이어 | 기술 | 역할 |
|--------|------|------|
| 윈도우 | winit | 네이티브 창 생성, Cmd/Ctrl 키 캡처, 마우스 이벤트 |
| UI 렌더링 | ratatui | TUI 위젯, 레이아웃, 텍스트 스타일링 |
| GPU 출력 | wgpu | ratatui 출력을 텍스트 그리드로 GPU 렌더링 |
| 폰트 | cosmic-text | 폰트 셰이핑, 리가처, CJK 지원 |
| 텍스트 버퍼 | ropey | Rope 자료구조 기반 텍스트 버퍼 |
| 구문 강조 | tree-sitter | AST 기반 파서 + 하이라이트 쿼리 |
| 내장 터미널 | portable-pty + vte | PTY 관리 + VT 이스케이프 시퀀스 파싱 |
| Git | git2-rs | Git 상태, diff, stage, commit |
