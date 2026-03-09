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

## PR 규칙

- PR에는 변경 내용, 테스트 방법, 추가된 기능을 모두 기술할 것
- 모든 새 기능에는 테스트가 반드시 포함되어야 함
- UI 변경 시 스냅샷 테스트 업데이트 필수
- CI (fmt + clippy + test) 통과 필수
