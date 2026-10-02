# WithHuman — PoC

Minimal connectivity PoC. It always returns **DENY**, for both entry points:

```text
External tool:  AgentGateway --gRPC extAuthz--> WithHuman :9000 --> DENY
Internal tool:  Agent hook   --HTTP JSON------> WithHuman :8080 --> DENY
```

There is no policy engine yet: a single shared `evaluate()` always denies, and
each adapter only translates that decision into its own wire format.

## Run

```sh
cargo run
```

Both servers start in the same process:

```text
WithHuman hook API listening on 0.0.0.0:8080
WithHuman extAuthz listening on 0.0.0.0:9000
```

## Endpoints

```text
HTTP hook:
POST http://localhost:8080/v1/hooks/pre-tool

gRPC extAuthz:
localhost:9000  (envoy.service.auth.v3.Authorization/Check)
```

### HTTP hook

Accepts any JSON body and always denies it.

```sh
curl -X POST http://localhost:8080/v1/hooks/pre-tool \
  -H 'Content-Type: application/json' \
  -d '{"tool_name":"Bash","tool_input":{"command":"ls"}}'
```

```json
{"decision":"DENY","reason":"Blocked by WithHuman"}
```

### gRPC extAuthz

Envoy External Authorization v3 compatible. Every `Check` call answers with
`PERMISSION_DENIED` plus a denied HTTP response of `403 Forbidden` and the body
`Blocked by WithHuman`, so AgentGateway blocks the original request.

`proto/` holds a trimmed but wire-compatible copy of the upstream Envoy
definitions — field numbers match, so callers can keep using the real protos.

## Docker

```sh
docker build -t withhuman .
docker run --rm -p 8080:8080 -p 9000:9000 withhuman
```

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
