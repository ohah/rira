# Testing

## 크레이트별 테스트

| 크레이트 | GPU 필요 | 테스트 명령 | 독립 실행 |
|----------|---------|-----------|----------|
| editor | 아니오 | `cargo test -p editor` | `cargo run -p editor --example demo` |
| keymap | 아니오 | `cargo test -p keymap` | `cargo run -p keymap --example parse` |
| highlight | 아니오 | `cargo test -p highlight` | `cargo run -p highlight --example demo -- file.rs` |
| git | 아니오 | `cargo test -p git` | `cargo run -p git --example diff` |
| terminal-core | 아니오 | `cargo test -p terminal-core` | `cargo run -p terminal-core --example vte` |
| terminal-pty | 아니오 | `cargo test -p terminal-pty` | `cargo run -p terminal-pty --example pty` |
| theme | 아니오 | `cargo test -p theme` | `cargo run -p theme --example preview` |
| ui | 아니오 | `cargo test -p ui` | `cargo run -p ui --example demo` |
| renderer | **예** | `cargo test -p renderer` | `cargo run -p renderer --example grid` |
| app | **예** | 통합 테스트 | `cargo run` |

**10개 중 8개가 GPU 없이 독립 테스트 + 독립 실행 가능.**

## 테스트 계층

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

## 회귀 테스트

1. **UI 스냅샷 테스트** — ratatui TestBackend로 렌더링 결과를 스냅샷 비교. 기능 변경 시 렌더링 변화 즉시 감지
2. **proptest** — 랜덤 입력으로 엣지 케이스 자동 탐색. 실패 시 최소 재현 케이스 자동 축소. 발견된 버그는 고정 테스트로 추가
3. **크레이트 분리** — 한 크레이트 변경 시 의존하는 크레이트만 재테스트. `cargo test --workspace`로 전체 영향도 검증

## CI/CD

GitHub Actions로 모든 PR에 대해 자동 실행:

- **lint** — `cargo fmt --check` + `cargo clippy -D warnings`
- **test-core** — GPU 불필요 크레이트 테스트 (macOS, Linux, Windows)
- **build** — 전체 workspace 빌드 (3 OS)
- **audit** — 의존성 보안 감사 (rustsec)
- **coverage** — cargo-llvm-cov로 커버리지 리포트 (codecov)
- **msrv** — 최소 지원 Rust 버전 체크

### Clippy 룰 (엄격 모드)

```toml
[workspace.lints.clippy]
correctness = { level = "deny" }
suspicious = { level = "warn" }
complexity = { level = "warn" }
perf = { level = "warn" }
style = { level = "warn" }
unwrap_used = "warn"           # .unwrap() 금지 → expect() 또는 ? 사용
```

### rustfmt 설정

```toml
edition = "2021"
max_width = 100
tab_spaces = 4
use_field_init_shorthand = true
use_try_shorthand = true
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```

### 환경 변수

```yaml
RUSTFLAGS: "-D warnings"  # 경고를 에러로 취급
```
