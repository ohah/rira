# rira — Rust Native Code Editor

## 프로젝트 비전

AI가 수정한 코드를 git diff로 확인하고, 수락/거절하고, commit까지 한 화면에서 끝내는 개발자용 코드 에디터.
winit 네이티브 윈도우 위에 ratatui TUI 렌더링으로 VSCode-like UX를 구현한다.
macOS Cmd 키를 포함한 모든 단축키를 완벽히 지원하며, Rust 100% 크로스플랫폼(macOS, Linux, Windows).

### 핵심 유즈케이스

```
┌─ 파일 트리 ─┬─ 에디터 ──────────────────────────────┐
│ 📁 src/     │  fn main() {                           │
│  ├ main.rs  │-     println!("hello");    ← AI 삭제   │
│  └ lib.rs   │+     println!("world");    ← AI 추가   │
│ 📁 tests/   │      let x = 42;                       │
│             │  }                                      │
│             ├─ Git ───────────────────────────────────┤
│  [Skills]   │  ✓ 3줄 수정됨                          │
│  - refactor │  [Stage] [Commit] [Diff] [Reject]      │
│  - test-gen │                                         │
│  - explain  ├─ 터미널 ───────────────────────────────┤
│             │  $ cargo build                          │
│             │  Compiling rira v0.1.0                  │
└─────────────┴─────────────────────────────────────────┘
```

---

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
| 언어 | Rust 100% | 크로스플랫폼 단일 코드베이스 |

### 렌더링 파이프라인

```
winit (Cmd 키 캡처) → ratatui (TUI 위젯으로 UI 구성) → wgpu (텍스트 그리드 GPU 렌더링)
```

- **winit**: macOS에서 Cmd 키를 잡으려면 네이티브 윈도우가 필요
- **ratatui**: UI 구성 비용 최소화 — 위젯, 레이아웃, 스타일 생태계 활용
- **wgpu**: ratatui 출력을 모노스페이스 셀 그리드로 GPU 렌더링
- 나중에 GPU 커스텀 렌더링이 필요하면 ratatui를 점진적으로 대체 가능

---

## 아키텍처

### 이벤트 루프: 싱글 스레드 메인 + 워커 스레드

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

### 크레이트 구조 (모노레포 workspace)

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

### 의존 방향 (단방향, 순환 없음)

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

### 크레이트별 테스트 전략

| 크레이트 | GPU 필요 | 테스트 방법 | 독립 실행 |
|----------|---------|-----------|----------|
| editor | 아니오 | `cargo test -p editor` | `cargo run -p editor --example demo` |
| keymap | 아니오 | `cargo test -p keymap` | `cargo run -p keymap --example parse` |
| highlight | 아니오 | `cargo test -p highlight` | `cargo run -p highlight --example demo -- file.rs` |
| git | 아니오 | `cargo test -p git` (임시 repo) | `cargo run -p git --example diff` |
| terminal-core | 아니오 | `cargo test -p terminal-core` | `cargo run -p terminal-core --example vte` |
| terminal-pty | 아니오 | `cargo test -p terminal-pty` | `cargo run -p terminal-pty --example pty` |
| theme | 아니오 | `cargo test -p theme` | `cargo run -p theme --example preview` |
| ui | 아니오 | `cargo test -p ui` (ratatui TestBackend) | `cargo run -p ui --example demo` |
| renderer | **예** | `cargo test -p renderer` | `cargo run -p renderer --example grid` |
| app | **예** | 통합 테스트 | `cargo run` |

**10개 중 8개가 GPU 없이 독립 테스트 + 독립 실행 가능.**

---

## CI/CD 파이프라인

### GitHub Actions 워크플로우

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"          # 경고를 에러로 취급

jobs:
  # ── 코드 품질 체크 (빠름, 모든 PR에서 실행) ──
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2

      # 포맷 체크
      - name: rustfmt
        run: cargo fmt --all -- --check

      # clippy (가장 엄격한 린트)
      - name: clippy
        run: cargo clippy --workspace --all-targets --all-features -- -D warnings

  # ── GPU 없이 돌아가는 유닛 테스트 (핵심) ──
  test-core:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      # GPU 불필요 크레이트만 테스트
      - name: test editor
        run: cargo test -p editor
      - name: test keymap
        run: cargo test -p keymap
      - name: test highlight
        run: cargo test -p highlight
      - name: test git
        run: cargo test -p git
      - name: test terminal-core
        run: cargo test -p terminal-core
      - name: test terminal-pty
        run: cargo test -p terminal-pty
      - name: test theme
        run: cargo test -p theme
      - name: test ui
        run: cargo test -p ui

  # ── 빌드 검증 (전체 컴파일 되는지) ──
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: build
        run: cargo build --workspace --all-targets

  # ── 의존성 보안 감사 ──
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: rustsec/audit-check@v2
        with:
          token: ${{ secrets.GITHUB_TOKEN }}

  # ── 커버리지 리포트 ──
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Install cargo-llvm-cov
        run: cargo install cargo-llvm-cov
      - name: Generate coverage
        run: cargo llvm-cov --workspace --exclude app --exclude renderer --lcov --output-path lcov.info
      - name: Upload coverage
        uses: codecov/codecov-action@v4
        with:
          files: lcov.info

  # ── MSRV (최소 지원 Rust 버전) 체크 ──
  msrv:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.75.0  # MSRV
      - name: check
        run: cargo check --workspace
```

### Clippy 룰 (엄격 모드)

```toml
# Cargo.toml (workspace root)
[workspace.lints.clippy]
# 정확성
correctness = { level = "deny" }
# 의심스러운 코드
suspicious = { level = "warn" }
# 복잡도
complexity = { level = "warn" }
# 성능
perf = { level = "warn" }
# 스타일
style = { level = "warn" }
# 추가 엄격 룰
needless_pass_by_value = "warn"
cloned_instead_of_copied = "warn"
redundant_closure_for_method_calls = "warn"
enum_glob_use = "warn"
unwrap_used = "warn"           # .unwrap() 금지 → expect() 또는 ? 사용
```

### rustfmt 설정

```toml
# rustfmt.toml
edition = "2021"
max_width = 100
tab_spaces = 4
use_field_init_shorthand = true
use_try_shorthand = true
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```

---

## 테스트 전략

### 계층별 테스트

```
┌─────────────────────────────────────────┐
│  통합 테스트 (E2E)                      │  ← CI에서 GPU 있는 환경만
│  "파일 열고 편집하고 저장" 시나리오      │
├─────────────────────────────────────────┤
│  UI 스냅샷 테스트                       │  ← ratatui TestBackend
│  "파일 트리가 올바르게 렌더링되는가"     │
├─────────────────────────────────────────┤
│  크레이트 유닛 테스트                   │  ← 모든 CI에서 실행
│  "ropey 삽입 후 줄 번호 정확한가"       │
├─────────────────────────────────────────┤
│  속성 기반 테스트 (proptest)            │  ← 랜덤 입력으로 불변조건 검증
│  "임의의 편집 시퀀스 후 버퍼 일관성"    │
└─────────────────────────────────────────┘
```

### 유닛 테스트 — 각 크레이트 내부

```rust
// editor/src/buffer.rs
#[cfg(test)]
mod tests {
    #[test]
    fn insert_at_position() {
        let mut buf = Buffer::from("hello");
        buf.insert(5, " world");
        assert_eq!(buf.to_string(), "hello world");
    }

    #[test]
    fn undo_restores_previous_state() {
        let mut buf = Buffer::from("hello");
        buf.insert(5, " world");
        buf.undo();
        assert_eq!(buf.to_string(), "hello");
    }
}
```

### UI 스냅샷 테스트 — ratatui TestBackend

```rust
// ui/src/file_tree.rs
#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn file_tree_renders_correctly() {
        let backend = TestBackend::new(30, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        let tree = FileTree::new(vec!["src/main.rs", "src/lib.rs"]);

        terminal.draw(|f| tree.render(f, f.area())).unwrap();

        // 스냅샷 비교 — 기능 변경 시 렌더링 변화 감지
        let expected = Buffer::with_lines(vec![
            "▼ src/                        ",
            "  ├ main.rs                   ",
            "  └ lib.rs                    ",
        ]);
        terminal.backend().assert_buffer(&expected);
    }
}
```

### 속성 기반 테스트 (proptest) — 버그 재현 보장

```rust
// editor/src/buffer.rs
#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    proptest! {
        // 임의의 편집 시퀀스 후에도 버퍼가 일관된 상태인지 검증
        #[test]
        fn random_edits_maintain_consistency(
            ops in prop::collection::vec(edit_operation_strategy(), 0..100)
        ) {
            let mut buf = Buffer::new();
            for op in ops {
                buf.apply(op);
            }
            // 불변조건: 줄 수 == 줄바꿈 수 + 1
            assert_eq!(buf.line_count(), buf.newline_count() + 1);
            // 불변조건: 바이트 길이 == 실제 내용 길이
            assert_eq!(buf.byte_len(), buf.to_string().len());
        }

        // undo 후 redo하면 원래 상태로 돌아오는지
        #[test]
        fn undo_redo_roundtrip(
            ops in prop::collection::vec(edit_operation_strategy(), 1..50)
        ) {
            let mut buf = Buffer::new();
            for op in &ops {
                buf.apply(op.clone());
            }
            let after_edits = buf.to_string();

            for _ in &ops { buf.undo(); }
            for _ in &ops { buf.redo(); }

            assert_eq!(buf.to_string(), after_edits);
        }
    }
}
```

### 영향도 분석 — 기능 추가 시 깨지는 것 감지

```
1. UI 스냅샷 테스트  → 렌더링 변화 즉시 감지 (의도된 변경이면 스냅샷 업데이트)
2. proptest          → 엣지 케이스 자동 탐색, 실패 시 최소 재현 케이스 자동 축소
3. 크레이트 분리     → 한 크레이트 변경 시 의존하는 크레이트만 재테스트
4. clippy 엄격 모드  → .unwrap() 금지, 의심스러운 패턴 차단
5. cargo-deny        → 의존성 라이선스/보안 문제 차단
```

---

## CLI 인터페이스

```bash
# 현재 디렉토리 열기
rira .

# 특정 프로젝트 열기
rira ~/projects/my-app

# 특정 파일 열기
rira main.rs

# 특정 파일의 특정 줄로 열기
rira main.rs:42
```

---

## 키맵 시스템

- 기본값: VSCode 키바인딩 (Cmd+S, Cmd+D, Cmd+Shift+P 등)
- 사용자 커스터마이징: TOML 설정 파일로 키맵 오버라이드
- 플랫폼별 자동 매핑: macOS는 Cmd, Linux/Windows는 Ctrl
- 키 바인딩 충돌 감지 및 경고

```toml
# ~/.config/rira/keymap.toml 예시
[editor]
"cmd+s" = "file.save"
"cmd+d" = "editor.add_selection_to_next_find_match"
"cmd+shift+p" = "command_palette.open"
"cmd+b" = "sidebar.toggle"

[terminal]
"ctrl+`" = "terminal.toggle"
"ctrl+shift+`" = "terminal.new"

[git]
"cmd+shift+g" = "git.panel.toggle"
"cmd+enter" = "git.commit"
```

---

## 테마 시스템

```toml
# ~/.config/rira/themes/dracula.toml
[colors]
background = "#282A36"
foreground = "#F8F8F2"
cursor = "#F8F8F2"
selection = "#44475A"
line_highlight = "#44475A"

[syntax]
keyword = "#FF79C6"
string = "#F1FA8C"
comment = "#6272A4"
function = "#50FA7B"
variable = "#F8F8F2"
number = "#BD93F9"
type = "#8BE9FD"

[ui]
sidebar_bg = "#21222C"
tab_active_bg = "#282A36"
tab_inactive_bg = "#21222C"
terminal_bg = "#1E1F29"
gutter = "#6272A4"
diff_added = "#50FA7B"
diff_removed = "#FF5555"
```

- VSCode .json 테마 임포트 지원
- 런타임 테마 전환 (Command Palette에서)
- 누락 필드는 기본값 폴백

---

## 개발 로드맵

### Phase 1 — 빈 창에 글자 찍기 (기초 인프라)

목표: winit 윈도우에서 키 입력 → ratatui → wgpu 텍스트 렌더링 파이프라인 완성

- [x] 프로젝트 workspace 초기화 (Cargo.toml, 크레이트 구조, CI)
- [x] rustfmt, clippy, cargo-deny 설정
- [x] winit으로 macOS/Linux/Windows 네이티브 윈도우 생성
- [x] wgpu 초기화 및 렌더 파이프라인 구축
- [x] cosmic-text로 모노스페이스 폰트 로딩 및 글리프 렌더링
- [x] ratatui 백엔드 구현 (ratatui → wgpu 텍스트 그리드)
- [x] 키 입력 → 화면에 글자 출력
- [x] 커서 렌더링 및 깜빡임
- [x] 기본 색상/배경 처리

### Phase 2 — 에디터 코어

목표: 실제로 파일을 열어서 편집하고 저장할 수 있는 상태

- [x] ropey 텍스트 버퍼 통합 + 유닛 테스트 + proptest
- [ ] 파일 열기 (Cmd+O) / 저장 (Cmd+S)
- [ ] CLI: `rira .`, `rira file.rs`, `rira file.rs:42`
- [ ] 기본 편집: 삽입, 삭제, 백스페이스, 줄바꿈
- [ ] 방향키 커서 이동
- [ ] 마우스 클릭으로 커서 위치 이동
- [ ] 텍스트 선택 (Shift+방향키, 마우스 드래그, 더블/트리플 클릭)
- [ ] 수직/수평 스크롤 (마우스 휠, 트랙패드)
- [ ] undo/redo (Cmd+Z, Cmd+Shift+Z) + proptest 라운드트립 검증
- [ ] 클립보드 복사/붙여넣기 (Cmd+C, Cmd+V, Cmd+X)
- [ ] 줄 번호 거터

### Phase 3 — VSCode 느낌 입히기

목표: "이거 진짜 에디터네" 수준의 UX

- [ ] 다중 커서 (Cmd+D, Cmd+클릭, Alt+클릭)
- [ ] tree-sitter 구문 강조 (Rust, JS/TS, Python, Go 등)
- [ ] 파일 트리 사이드바 (Cmd+B) + UI 스냅샷 테스트
- [ ] Command Palette (Cmd+Shift+P)
- [ ] 탭 (다중 파일 열기)
- [x] 테마 시스템 (다크/라이트, 사용자 정의 테마, VSCode 임포트) — crate 구현 완료, UI 연동 미완
- [x] 커스텀 키맵 시스템 (keymap.toml 로딩) — crate 구현 완료, UI 연동 미완
- [ ] 미니맵
- [ ] 들여쓰기 가이드 라인
- [ ] 괄호 매칭 하이라이트
- [ ] 검색/치환 (Cmd+F, Cmd+H)

### Phase 4 — 내장 터미널

목표: 에디터 아래에 터미널 패널, 다중 탭/분할

- [ ] portable-pty로 PTY 생성 (zsh/bash/powershell)
- [ ] vte로 VT 이스케이프 시퀀스 파싱 + terminal-core 유닛 테스트
- [ ] 터미널 텍스트 그리드 렌더링 (에디터와 렌더러 공유)
- [ ] 터미널 토글 (Ctrl+`)
- [ ] 다중 터미널 탭
- [ ] 터미널 분할 (가로 나란히)
- [ ] 에디터 ↔ 터미널 포커스 전환
- [ ] 터미널 텍스트 선택/복사
- [ ] 스크롤백 버퍼

---

## 장기 로드맵 (Post-MVP)

### Phase 5 — Git 통합

목표: 에디터 안에서 git 워크플로우 완결

- [ ] Git 상태 표시 (거터에 수정/추가/삭제 마커)
- [ ] Git diff 인라인 뷰 (줄 단위 배경색)
- [ ] Git blame
- [ ] 소스 컨트롤 사이드바 (stage, unstage, discard)
- [ ] 커밋 패널 (메시지 작성 + commit)
- [ ] 브랜치 전환
- [ ] 커밋 히스토리 뷰

### Phase 6 — LSP 통합

- [ ] LSP 클라이언트 구현 (JSON-RPC over stdio)
- [ ] 자동완성 (인라인 제안 + 팝업 목록)
- [ ] Go to Definition (Cmd+클릭, F12)
- [ ] 호버 정보 (타입, 문서)
- [ ] 에러/경고 진단 (빨간/노란 밑줄, 문제 패널)
- [ ] 코드 액션 (빠른 수정, 리팩터링)
- [ ] 심볼 검색 (Cmd+T)
- [ ] 리네임 (F2)
- [ ] 시그니처 도움말

### Phase 7 — AI 통합

목표: AI 코드 편집을 한 화면에서 확인/수락/거절

- [ ] AI 채팅 사이드바 (에디터 내장)
- [ ] 인라인 AI 코드 완성 (Ghost Text)
- [ ] AI diff 뷰 (AI 수정 사항을 git diff 형태로 표시)
- [ ] AI 수정 수락/거절/부분 수정 UI
- [ ] 스킬 목록 패널 (refactor, test-gen, explain 등)
- [ ] MCP (Model Context Protocol) 지원
- [ ] 다양한 LLM 백엔드 지원 (Claude, OpenAI, 로컬 모델)
- [ ] AI 커맨드 → git stage → commit 원클릭 워크플로우

### Phase 8 — 전역 검색 & 네비게이션

- [ ] 전역 파일 검색 (Cmd+P — fuzzy finder)
- [ ] 전역 텍스트 검색 (Cmd+Shift+F, ripgrep 기반)
- [ ] 에디터 분할 (좌우, 상하)
- [ ] 드래그 앤 드롭으로 탭 이동
- [ ] 브레드크럼 네비게이션

### Phase 9 — 플러그인 시스템

- [ ] WASM 기반 플러그인 런타임 (wasmtime)
- [ ] 플러그인 API 설계 (에디터 상태 읽기/쓰기, UI 확장)
- [ ] 플러그인 매니페스트 (설치, 활성화, 비활성화)
- [ ] 내장 플러그인 마켓플레이스 또는 레지스트리

### Phase 10 — 워크스페이스 & 세션

- [ ] 워크스페이스 설정 (프로젝트별 설정)
- [ ] 세션 복원 (마지막 열린 파일/레이아웃 기억)
- [ ] 멀티 루트 워크스페이스

### Phase 11 — 협업 & 고급 기능

- [ ] CRDT 기반 실시간 공동 편집
- [ ] 원격 개발 (SSH 연결 후 원격 파일 편집)
- [ ] 디버거 통합 (DAP — Debug Adapter Protocol)
- [ ] 노트북 지원 (Jupyter-like)
- [ ] 이미지/미디어 미리보기 (GPU 커스텀 렌더링 도입 시점)
- [ ] 접근성 (스크린 리더 지원)

---

## 설계 원칙

1. **성능 우선** — 10만 줄 파일도 부드럽게 편집 가능해야 한다
2. **점진적 복잡도** — 각 Phase가 독립적으로 동작 가능한 상태여야 한다
3. **크로스플랫폼** — macOS 우선, Linux/Windows는 동일 코드베이스로 지원
4. **커스터마이징** — 키맵, 테마, 설정 모두 사용자 정의 가능
5. **단순한 내부 구조** — async 지양, 명시적 스레딩, 상태 독점 소유
6. **AI-first 워크플로우** — AI 코드 편집 → 리뷰 → 커밋을 최단 경로로
7. **테스트 가능성** — 모든 크레이트 독립 테스트, GPU 의존 최소화
8. **안정성** — proptest로 불변조건 검증, UI 스냅샷으로 회귀 방지

---

## 라이선스

MIT License

---

## 참고 프로젝트

| 프로젝트 | 참고할 점 |
|----------|-----------|
| Helix | ropey + tree-sitter 통합, 이벤트 루프 구조 |
| Zed | wgpu GPU 렌더링, CRDT, AI 통합 |
| Alacritty | winit + wgpu 터미널 렌더링, vte 파서 |
| Warp | 네이티브 윈도우 + GPU 렌더링 + TUI 미학 |
| COSMIC Edit | cosmic-text 폰트 렌더링 |
| Lapce | LSP 통합, 플러그인 시스템 (WASM) |
