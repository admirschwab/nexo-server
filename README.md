# Nexo Server

A minimal directory and relay server for [Nexo](https://github.com/admirschwab/nexo-cli).
It knows public keys, nicknames and who is online. Messages are end-to-end encrypted
by the clients and only passed through, never stored.

## Running

```sh
cargo run --release
```

The database (`nexo.db`) is created in the current directory.

## Configuration

All settings are environment variables:

| Variable               | Default          | Meaning                                                        |
|------------------------|------------------|----------------------------------------------------------------|
| `NEXO_BIND`            | `127.0.0.1:3000` | Address and port the server listens on                         |
| `NEXO_TRUST_PROXY`     | `false`          | Read the client IP from `X-Forwarded-For` (see below)          |
| `NEXO_TRUSTED_PROXIES` | `127.0.0.1,::1`  | Comma-separated proxy IPs whose `X-Forwarded-For` is trusted   |

## Running behind a reverse proxy

The server does not handle TLS itself. To make it reachable as `https://` / `wss://`,
put a reverse proxy such as nginx or Caddy in front of it.

The server limits registrations and connection attempts per IP address. Behind a proxy,
every connection comes from the proxy's address, so all users would share one limit.
Set `NEXO_TRUST_PROXY=true` so the server takes the real client IP from the
`X-Forwarded-For` header instead:

```sh
NEXO_TRUST_PROXY=true cargo run --release
```

The header is only trusted for connections coming from `NEXO_TRUSTED_PROXIES`
(by default: the same machine). Otherwise any client could fake its address.
The server uses the **last** entry of the header, which is the one your proxy appended.

If the proxy runs somewhere else (for example in its own Docker container),
list its address:

```sh
NEXO_TRUST_PROXY=true NEXO_TRUSTED_PROXIES=172.18.0.2 cargo run --release
```

Only enable `NEXO_TRUST_PROXY` when the server is actually behind a proxy and not
directly reachable from the internet.

### Apache

See [apache.md](apache.md) for a complete step-by-step setup: systemd service, Apache modules,
virtual host (also under a sub-path like `https://api.example.com/nexo`), logging and testing.

### Caddy

Caddy sets `X-Forwarded-For` and handles WebSockets automatically:

```
chat.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

### nginx

WebSockets need the `Upgrade` and `Connection` headers, and the read timeout must be
longer than the server's ping interval (20 seconds):

```nginx
server {
    listen 443 ssl;
    server_name chat.example.com;

    # ssl_certificate ...;
    # ssl_certificate_key ...;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_read_timeout 120s;
    }
}
```

Clients then use `server = "https://chat.example.com"` in their `config.toml`.
