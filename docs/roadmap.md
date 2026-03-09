# Roadmap

## MVP (Phase 1~4)

### Phase 1 — 빈 창에 글자 찍기 (기초 인프라)

목표: winit 윈도우에서 키 입력 → ratatui → wgpu 텍스트 렌더링 파이프라인 완성

- [ ] 프로젝트 workspace 초기화 (Cargo.toml, 크레이트 구조, CI)
- [ ] rustfmt, clippy, cargo-deny 설정
- [ ] winit으로 macOS/Linux/Windows 네이티브 윈도우 생성
- [ ] wgpu 초기화 및 렌더 파이프라인 구축
- [ ] cosmic-text로 모노스페이스 폰트 로딩 및 글리프 렌더링
- [ ] ratatui 백엔드 구현 (ratatui → wgpu 텍스트 그리드)
- [ ] 키 입력 → 화면에 글자 출력
- [ ] 커서 렌더링 및 깜빡임
- [ ] 기본 색상/배경 처리

### Phase 2 — 에디터 코어

목표: 실제로 파일을 열어서 편집하고 저장할 수 있는 상태

- [ ] ropey 텍스트 버퍼 통합 + 유닛 테스트 + proptest
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
- [ ] 테마 시스템 (다크/라이트, 사용자 정의 테마, VSCode 임포트)
- [ ] 커스텀 키맵 시스템 (keymap.toml 로딩)
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
