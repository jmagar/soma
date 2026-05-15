# Docker

Docker support lives in `config/Dockerfile` and `docker-compose.yml`.

## Common commands

```bash
just docker-build      # build image
just docker-up         # start compose stack
just docker-down       # stop stack
just docker-rebuild    # rebuild image and recreate container
just docker-logs       # follow logs
just runtime-current   # compare running image with local compose image
```

## Compose expectations

The compose service is named `example-mcp` and reads `.env`. Recreate the container after editing `.env`; `docker restart` does not reload environment variables.

```bash
docker compose up -d --force-recreate
```

The template compose setup expects an external Docker network. Create it first if needed:

```bash
docker network create jakenet
```

## Health and auth

- Healthcheck probes `/health`.
- `/mcp` should require auth outside loopback unless you intentionally run behind a trusted gateway.
- Use `scripts/test-mcp-auth.sh` for bearer auth smoke tests.

## Build artifacts

`just build-plugin` copies the release binary to both `bin/example` and `plugins/example/bin/example`. The plugin binary path is allowlisted in `scripts/blob-size-allowlist.txt` because it is an intentional checked-in plugin artifact.
