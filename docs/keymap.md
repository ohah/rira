# Keymap

## 기본 키바인딩 (VSCode-like)

### 에디터

| 단축키 | 액션 |
|--------|------|
| Cmd+S | 파일 저장 |
| Cmd+O | 파일 열기 |
| Cmd+Z | undo |
| Cmd+Shift+Z | redo |
| Cmd+C | 복사 |
| Cmd+V | 붙여넣기 |
| Cmd+X | 잘라내기 |
| Cmd+D | 다음 일치 항목 선택 추가 (다중 커서) |
| Cmd+F | 파일 내 검색 |
| Cmd+H | 파일 내 치환 |
| Cmd+Shift+P | Command Palette |
| Cmd+P | 파일 빠른 열기 (fuzzy finder) |
| Cmd+B | 사이드바 토글 |
| Cmd+클릭 | 다중 커서 추가 |
| Alt+클릭 | 다중 커서 추가 (대체) |
| Cmd+Shift+F | 전역 텍스트 검색 |
| Cmd+T | 심볼 검색 |

### 터미널

| 단축키 | 액션 |
|--------|------|
| Ctrl+` | 터미널 패널 토글 |
| Ctrl+Shift+` | 새 터미널 생성 |

### Git

| 단축키 | 액션 |
|--------|------|
| Cmd+Shift+G | Git 패널 토글 |
| Cmd+Enter | Git commit (Git 패널 포커스 시) |

## 커스텀 키맵

`~/.config/rira/keymap.toml`로 오버라이드:

```toml
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

## 플랫폼별 매핑

- macOS: `Cmd` 키 사용
- Linux/Windows: `Ctrl` 키로 자동 매핑

설정 파일에서는 `cmd`로 통일하되, Linux/Windows에서는 자동으로 `ctrl`로 변환.

## 키 바인딩 충돌 감지

동일한 키 조합이 여러 액션에 바인딩되면 경고를 표시하고, 나중에 정의된 바인딩이 우선.
