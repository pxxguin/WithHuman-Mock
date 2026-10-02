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

These are the ports WithHuman itself binds, which is what you talk to under
`cargo run`. In the deployed topology clients go through AgentGateway instead —
see [Deployment](#deployment-single-ec2-instance).

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

## Deployment (single EC2 instance)

AgentGateway is the only entry point. WithHuman has no published ports and is
reachable only as `withhuman:8080` / `withhuman:9000` on the Compose network.

```text
Claude Code / Agent
        |
        |  http://<ELASTIC_IP>:3000
        v
   AgentGateway  (published)
        |
        |-- gRPC extAuthz ----> withhuman:9000   (internal only)
        `-- HTTP proxy -------> withhuman:8080   (internal only)
```

| Port | Where |
| ---- | ----- |
| 3000 | published to the EC2 host — AgentGateway |
| 8080 | Compose network only — WithHuman hook API |
| 9000 | Compose network only — WithHuman gRPC extAuthz |

Start it on the instance:

```sh
docker compose up -d --build
docker compose ps
```

The client endpoint is `http://<ELASTIC_IP>:3000` for now. HTTPS is out of
scope; the Elastic IP association and Security Group (inbound 3000 only) are
handled separately.

### Test the extAuthz path

```sh
curl -i -X POST http://<ELASTIC_IP>:3000/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
```

```text
HTTP/1.1 403 Forbidden

Blocked by WithHuman
```

### Test the hook proxy path

```sh
curl -X POST http://<ELASTIC_IP>:3000/v1/hooks/pre-tool \
  -H 'Content-Type: application/json' \
  -d '{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"ls"}}'
```

```json
{"decision":"DENY","reason":"Blocked by WithHuman"}
```

### Building for the EC2 architecture

The Dockerfile pins no architecture, so `docker compose up --build` on the
instance always produces a native image. If you build on an Apple Silicon
machine instead, target the instance explicitly — an arm64 image will not run
on an `amd64` instance:

```bash
docker buildx build \
  --platform linux/amd64 \
  -t withhuman-core:latest \
  --load .
```

### Exposure caveat

`expose:` documents the internal ports; it publishes nothing. Nothing outside
the instance can reach 8080 or 9000. Note that with standard Docker bridge
networking the container IP (e.g. `172.18.0.2:8080`) is still reachable from
the EC2 host itself — that is inherent to Docker, not a missing setting here.

### MCP backend

The MCP route has an empty `targets: []` list. The official AgentGateway image
is distroless, so a `stdio` target such as `npx` cannot be spawned, and this
PoC denies every request at extAuthz before a backend is ever selected. Real
MCP targets are a later change.

## Local development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
