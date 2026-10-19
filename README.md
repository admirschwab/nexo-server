# Nexo Server

**A minimal directory and relay server for [Nexo](https://github.com/admirschwab/nexo),
the end-to-end encrypted messenger for the terminal.**

The server connects Nexo clients. It knows who is registered and who is online, and passes
messages on. It cannot read them: they are encrypted by the clients before they are sent.

## What the server knows

| Data | Where | How long |
|---|---|---|
| Public key and nickname | `nexo.db` | Until the user runs `nexo unregister` |
| Who is online | Memory | While the user is connected |
| Encrypted messages | Memory | Only while they are passed on, never stored |
| IP addresses | Memory | Only for the rate limits, forgotten after a short time |

That's all. Nexo itself writes no logs. Deleted accounts are overwritten in `nexo.db` right away
(SQLite `secure_delete`), so they can't be recovered from the file.

## Quick start

With [Rust](https://rustup.rs) 1.85 or newer:

```sh
git clone https://github.com/admirschwab/nexo-server.git
cd nexo-server
cargo run --release
```

The server listens on `127.0.0.1:3000` and creates `nexo.db` in the current directory. Clients on
the same machine can now register with the server address `http://127.0.0.1:3000`.

On Debian/Ubuntu, building needs a C compiler for the bundled SQLite: `sudo apt install build-essential`.

## Running in production

The server does not handle TLS itself. Put a reverse proxy in front of it that provides `https://`
and forwards WebSockets.

* **Apache:** [apache.md](apache.md) is a complete step-by-step guide: systemd service, Apache
  modules, virtual host (also under a sub-path like `https://api.example.com/nexo`), logging and testing.
* **Caddy** and **nginx:** see [Reverse proxy](#reverse-proxy) below.

Clients then use the public address, e.g. `https://chat.example.com`. It is asked for during
`nexo register` and can be changed later with `nexo server <url>`.

## Configuration

All settings are environment variables:

| Variable | Default | Meaning |
|---|---|---|
| `NEXO_BIND` | `127.0.0.1:3000` | Address and port the server listens on |
| `NEXO_TRUST_PROXY` | `false` | Take the client IP from `X-Forwarded-For` (see [Reverse proxy](#reverse-proxy)) |
| `NEXO_TRUSTED_PROXIES` | `127.0.0.1,::1` | Comma-separated proxy IPs whose `X-Forwarded-For` is trusted |

### Minimum client version

To make users update, put the oldest client version you still accept into a file
`min_client_version` in the server's working directory (next to `nexo.db`):

```sh
echo "0.2.0" > min_client_version
```

* Older clients can't log in or register. They are shown the required version and the command to
  install the latest release.
* `nexo unregister` always works, so users can delete their account with any version.
* The file is read on every login, no restart needed.
* Without the file, every version is accepted.
* Clients before 0.2.0 don't send their version and are rejected as soon as the file exists.

Only raise the version **after** the new client release is published, otherwise nobody can update.

## Limits

| What | Limit |
|---|---|
| Registrations and account deletions | 5 at once per IP, then 1 per minute |
| Connection attempts | 10 at once per IP, then 1 every 5 seconds |
| Messages | 20 at once per connection, then 5 per second |
| Message size | 16 KiB |
| Login | The challenge must be answered within 10 seconds |
| Idle connections | Pinged every 20 seconds, closed after 60 seconds without an answer |

Messages to users who are offline are dropped, and the sender is told. Nothing is queued.

## Reverse proxy

The server limits registrations and connection attempts per IP address. Behind a proxy, every
connection comes from the proxy's address, so all users would share one limit. Set
`NEXO_TRUST_PROXY=true` so the server takes the real client IP from the `X-Forwarded-For` header:

```sh
NEXO_TRUST_PROXY=true cargo run --release
```

* The header is only trusted for connections from `NEXO_TRUSTED_PROXIES` (by default: the same
  machine). Otherwise any client could fake its address.
* The server uses the **last** entry of the header, which is the one your proxy appended.
* Only enable it when the server is behind a proxy and not directly reachable from the internet.

If the proxy runs somewhere else (for example in its own Docker container), list its address:

```sh
NEXO_TRUST_PROXY=true NEXO_TRUSTED_PROXIES=172.18.0.2 cargo run --release
```

### Caddy

Caddy sets `X-Forwarded-For` and handles WebSockets automatically:

```
chat.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

### nginx

WebSockets need the `Upgrade` and `Connection` headers, and the read timeout must be longer than
the server's ping interval (20 seconds):

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

Most proxies write every request with IP address and time to their access log. That is exactly the
metadata Nexo avoids, so consider turning it off for Nexo ([apache.md](apache.md#5-logging) shows how).

### Apache

See [apache.md](apache.md).

## Updating

```sh
cd nexo-server
git pull
cargo build --release
sudo install -m 755 target/release/nexo-server /usr/local/bin/nexo-server
sudo systemctl restart nexo-server
```

Connected users are disconnected cleanly and can log in again right away. Accounts are kept.

## Stopping

On Ctrl+C or `SIGTERM` (e.g. `systemctl stop`), the server stops accepting new connections, closes
all open ones cleanly and exits.

## API

For reference, the endpoints used by the client:

| Endpoint | Purpose |
|---|---|
| `POST /register` | Register a public key and nickname, signed with the private key |
| `POST /unregister` | Delete an account, signed with the private key |
| `GET /ws` | WebSocket: login by challenge-response, online list, message relay |

The message format is defined in `src/models/protocol.rs` and must match the client's
`src/models/protocol.rs`.
