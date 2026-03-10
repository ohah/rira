# CLAUDE.md — rira

Rust 100% 네이티브 코드 에디터. winit + ratatui + wgpu 기반 VSCode-like UX.
AI 코드 편집 → git diff 리뷰 → commit을 한 화면에서 완결.

```
winit (Cmd 키 캡처) → ratatui (TUI 위젯) → wgpu (GPU 텍스트 그리드)
```

## Docs

- [Architecture](docs/architecture.md) — 이벤트 루프, 크레이트 구조, 의존 방향, 렌더링 파이프라인
- [Testing](docs/testing.md) — 테스트 전략, CI/CD, clippy/fmt 룰, 회귀 테스트
- [Keymap](docs/keymap.md) — VSCode-like 키바인딩, 커스텀 키맵 시스템
- [Theme](docs/theme.md) — 테마 시스템, VSCode 테마 임포트, 설정 파일 형식
- [Code Rules](docs/code-rules.md) — 코드 품질 정책, PR 규칙, 레거시 코드 처리
- [Roadmap](docs/roadmap.md) — 전체 개발 로드맵 (Phase 1~11)

## Quick Reference

```bash
# 빌드
cargo build --workspace

# 전체 테스트 (GPU 불필요 크레이트만)
cargo test --workspace --exclude app --exclude renderer

# 개별 크레이트 테스트
cargo test -p editor
cargo test -p ui

# 린트
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# 실행
cargo run
```

## 개발/설계 원칙

### 성능 최우선 (Performance First)
- **제로 불필요 할당**: 매 프레임, 매 이벤트마다 힙 할당을 최소화. `Vec` collect 대신 참조, 재사용 가능한 버퍼는 재할당 없이 `.fill(0)` 또는 `.clear()`
- **비용이 큰 연산은 캐싱**: `FontSystem::new()` 같은 시스템 리소스 스캔은 최초 1회만. 이후 변경 시 metrics만 재계산
- **GPU 리소스 재생성 최소화**: 텍스처, 파이프라인, 바인드그룹은 실제 크기가 변할 때만 재생성. 동일 크기면 기존 리소스 재사용
- **Hot path에서 clone 금지**: `draw()` 같은 매 프레임 호출 경로에서 데이터 복사를 피하고 참조를 사용

### 크레이트 격리 (Crate Isolation)
- **editor**: 순수 텍스트 로직. UI/렌더러 의존성 없음. `ropey` + `unicode-width`만 사용
- **renderer**: GPU 렌더링. editor를 직접 의존하지 않음. ratatui Backend trait 구현
- **app**: 유일한 통합 지점. winit 이벤트 → editor 조작 → ratatui 위젯 → renderer
- 크레이트 간 의존 방향은 단방향. 순환 의존 금지

### 좌표계 일관성 (Coordinate Consistency)
- winit 이벤트(`CursorMoved`)는 **물리 픽셀(physical pixels)** 기준
- WgpuBackend의 `cell_width()`, `cell_height()`도 물리 픽셀 기준
- hit test 시 `/ scale_factor` 하지 않음. 물리 픽셀끼리 직접 비교
- 와이드 문자(한글/CJK)는 `unicode-width`로 셀 폭 계산. 단순 `col / cell_width`가 아닌 문자 폭 누적 방식

### Rust 철학
- 안정성은 Rust 타입 시스템과 borrow checker에 위임
- `unsafe` 사용 금지 (외부 FFI 바인딩 제외)
- 에러 처리는 `Result`/`Option` — `unwrap()` 대신 적절한 에러 전파

## PR 규칙

- PR에는 변경 내용, 테스트 방법, 추가된 기능을 모두 기술할 것
- 모든 새 기능에는 테스트가 반드시 포함되어야 함
- UI 변경 시 스냅샷 테스트 업데이트 필수
- CI (fmt + clippy + test) 통과 필수
