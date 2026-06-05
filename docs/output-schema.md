# 세션 목록 출력 schema 결정

## 배경

`agent-sessions list`는 Claude Code, Codex CLI, Pi Coding Agent의 서로 다른 세션 파일을 하나의 CLI에서 조회한다. 세 에이전트 모두 JSONL 기반 transcript를 쓰지만 원본 필드명, 저장 위치, metadata 구조가 다르므로 출력용 공통 schema가 필요하다.

지원할 출력 옵션:

```sh
agent-sessions list --agent claude --output text
agent-sessions list --agent codex --output csv
agent-sessions list --agent pi --output json
```

`--output` 기본값은 `text`다.

## 공통 schema

내부적으로는 `AgentSession` 도메인 모델을 기준으로 하고, CLI 출력은 다음 normalized schema를 사용한다.

```json
{
  "agent": "claude | codex | pi",
  "session_id": "string",
  "title": "string | null",
  "cwd": "string | null",
  "file_path": "string",
  "message_count": 0,
  "created_at": "string | null",
  "updated_at": "string | null",
  "model": "string | null",
  "branch": "string | null",
  "source": "string | null",
  "is_subsession": false,
  "parent_session_id": "string | null"
}
```

## 필드 의미

| 필드 | 의미 | 비고 |
| --- | --- | --- |
| `agent` | 세션 제공자 | `claude`, `codex`, `pi` |
| `session_id` | 원본 세션 ID | 없으면 파일명 기반 fallback |
| `title` | 세션 이름 또는 첫 user prompt 요약 | 없으면 `null` |
| `cwd` | 세션 작업 디렉터리 | 없으면 `null` |
| `file_path` | transcript 파일 경로 | 현재는 로컬 파일 경로 |
| `message_count` | user/assistant 중심 메시지 수 | 에이전트별 message 구조를 normalized count로 환산 |
| `created_at` | 세션 생성 시각 | JSON 출력은 RFC3339 string 또는 `null` |
| `updated_at` | 마지막 메시지 시각 또는 파일 수정 시각 | JSON 출력은 RFC3339 string 또는 `null` |
| `model` | 대표 모델명 | 여러 모델이 섞이면 마지막으로 관측된 모델 |
| `branch` | git branch | 원본 metadata가 제공하는 경우만 |
| `source` | 실행 출처 | 예: `cli`, `exec`, `sdk`, `rpc`, `acp` 등 추정 가능한 값 |
| `is_subsession` | 부모 세션에 딸린 하위 transcript 여부 | 현재 CLI 기본 목록에서는 Claude subagent를 제외 |
| `parent_session_id` | 부모 세션 ID | 하위 세션일 때 사용 |

## 출력별 규칙

### text

사람이 터미널에서 읽기 쉬운 TSV 스타일 테이블로 출력한다.

```text
AGENT   SESSION_ID   MESSAGES   UPDATED_AT              CWD     FILE     TITLE
claude  abc123       24         2026-06-05T04:10:00Z    /repo   ...      refactor auth
```

원칙:

- `--output text`가 기본값이다.
- 없는 값은 `-`로 표시한다.
- 필드 순서는 사람이 훑기 쉬운 순서로 둔다.

### csv

스크립트와 스프레드시트 처리를 위해 header 포함 CSV로 출력한다.

```csv
agent,session_id,title,cwd,file_path,message_count,created_at,updated_at,model,branch,source,is_subsession,parent_session_id
claude,abc123,refactor auth,/repo,/home/me/.claude/...,24,2026-06-05T04:00:00Z,2026-06-05T04:10:00Z,claude-sonnet-4-6,main,cli,false,
```

원칙:

- CSV escaping은 직접 구현하지 않고 `csv` crate에 맡긴다.
- 없는 값은 빈 칸으로 출력한다.
- header는 항상 출력한다.

### json

자동화와 API 연계를 위해 확장 가능한 object 형태로 출력한다.

```json
{
  "sessions": [
    {
      "agent": "claude",
      "session_id": "abc123",
      "title": "refactor auth",
      "cwd": "/repo",
      "file_path": "/home/me/.claude/projects/.../abc123.jsonl",
      "message_count": 24,
      "created_at": "2026-06-05T04:00:00Z",
      "updated_at": "2026-06-05T04:10:00Z",
      "model": "claude-sonnet-4-6",
      "branch": "main",
      "source": "cli",
      "is_subsession": false,
      "parent_session_id": null
    }
  ]
}
```

원칙:

- 최상위 배열 대신 `{ "sessions": [...] }` object를 사용한다.
- 없는 값은 `null`로 출력한다.
- 시간은 RFC3339 문자열로 출력한다.

## 구현 결정

- 도메인 모델은 원본 에이전트별 구조를 노출하지 않고 공통 필드만 가진다.
- outbound adapter는 가능한 metadata를 채우되, 알 수 없는 값은 `None`으로 둔다.
- inbound CLI formatter는 `text`, `csv`, `json` 포맷만 책임진다.
- timestamp는 내부에서 `SystemTime`으로 보관하고 출력 시 RFC3339 문자열로 변환한다.
- 현재 기본 목록에서는 Claude subagent transcript를 제외한다. 이후 필요하면 `--include-subagents`를 추가한다.
