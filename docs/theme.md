# Theme

## 테마 설정 파일

`~/.config/rira/themes/<name>.toml`:

```toml
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

## 기능

- **기본 테마**: 다크 모드
- **런타임 전환**: Command Palette에서 테마 변경
- **VSCode 테마 임포트**: `.json` 형식의 VSCode 테마를 rira `.toml`로 변환
- **폴백**: 누락된 필드는 기본값으로 대체
- **3개 섹션**: `colors` (에디터 기본), `syntax` (구문 강조), `ui` (UI 컴포넌트)

## 활성 테마 설정

`~/.config/rira/config.toml`:

```toml
[appearance]
theme = "dracula"   # ~/.config/rira/themes/dracula.toml
font_family = "JetBrains Mono"
font_size = 14
```

## theme 크레이트 역할

- TOML 테마 파일 파싱 → `Theme` 구조체
- VSCode JSON 테마 → rira TOML 변환
- `highlight` 크레이트에 `syntax` 색상 제공
- `ui` 크레이트에 `ui` 색상 제공
- `renderer` 크레이트에 `colors` 기본 색상 제공
