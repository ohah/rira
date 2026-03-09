# Code Rules

## 레거시 / 미사용 코드

- **레거시 코드는 즉시 삭제**: 새로운 구현으로 대체된 코드는 즉시 제거. 죽은 코드, 주석 처리된 블록, 하위호환 shim 금지.
- **미사용 코드 삭제**: 호출자가 없는 함수, 타입, import는 삭제.

## 테스트 커버리지

- **모든 새 기능에 테스트 필수**: PR에 새 기능이 포함되면 해당 기능의 테스트도 반드시 포함.
- 유닛 테스트: `cargo test -p <crate>` 로 크레이트 단위 격리 테스트.
- UI 스냅샷 테스트: ratatui TestBackend으로 렌더링 결과 검증.
- 속성 기반 테스트: proptest로 에디터 코어 불변조건 검증.

## PR 규칙

PR 본문에 반드시 포함할 내용:

1. **변경 내용** — 무엇을 변경했는지 요약 (1~3줄)
2. **테스트 방법** — 어떻게 테스트했는지 (실행한 명령, 검증한 시나리오)
3. **추가된 기능** — 새로 추가된 기능이 있다면 설명
4. **영향 범위** — 어떤 크레이트가 영향을 받는지
5. **스크린샷** — UI 변경이 있다면 전/후 비교

```markdown
## Summary
- 에디터 버퍼에 다중 커서 삽입 기능 추가

## Test plan
- `cargo test -p editor` — 다중 커서 삽입 유닛 테스트
- `cargo test -p ui` — 다중 커서 렌더링 스냅샷 테스트

## Affected crates
- editor (기능 추가)
- ui (렌더링 변경)

## Screenshots
(UI 변경 전/후 스크린샷)
```

## Clippy / Lint

- `RUSTFLAGS="-D warnings"` — 경고를 에러로 취급
- `clippy::unwrap_used = "warn"` — `.unwrap()` 금지, `expect()` 또는 `?` 사용
- CI에서 `cargo fmt --check` + `cargo clippy -D warnings` 통과 필수

## 의존성

- 새 의존성 추가 시 라이선스 확인 (MIT, Apache-2.0, BSD 허용)
- `cargo-deny`로 보안 취약점 및 라이선스 자동 검사
