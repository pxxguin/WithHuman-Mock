# WithHuman — PoC

Minimal connectivity PoC. It returns **DENY** for every tool call, on both
entry points:

```text
External tool:  AgentGateway --gRPC extAuthz--> WithHuman :9000 --> DENY on tools/call
Internal tool:  Agent hook   --HTTP JSON------> WithHuman :8080 --> DENY
```

There is no policy engine yet: a single shared `evaluate()` denies every tool
call and allows everything else (so an MCP client can still connect and list
tools), and each adapter only translates that decision into its own wire format.

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

Envoy External Authorization v3 compatible. WithHuman reads the JSON-RPC
`method` from the forwarded request body:

- `tools/call` (alone or inside a batch) answers `PERMISSION_DENIED` plus a
  denied HTTP response of `403 Forbidden` and the body `Blocked by WithHuman`,
  so AgentGateway blocks the call before it reaches the MCP server.
- Anything else (`initialize`, `tools/list`, bodyless `GET`/`DELETE`) answers
  `OK`, so AgentGateway forwards it.
- A body that is not JSON is denied (fail closed).

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
  -H 'Authorization: Bearer <GITHUB_TOKEN>' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_me","arguments":{}}}'
```

```text
HTTP/1.1 403 Forbidden

Blocked by WithHuman
```

An `initialize` sent the same way is allowed and answered by GitHub.

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

The MCP route proxies to GitHub's remote MCP server
(`https://api.githubcopilot.com/mcp/`). It is reached over HTTPS, so the
official distroless AgentGateway image works as is; a `stdio` target such as
`npx` could not be spawned there.

The gateway holds no GitHub token. Each client sends its own, and AgentGateway
forwards the `Authorization` header upstream:

```sh
claude mcp add --transport http github http://<ELASTIC_IP>:3000/mcp \
  --header "Authorization: Bearer <GITHUB_TOKEN>"
```

## Local development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
