# Running Nexo behind nginx

This guide sets up the Nexo server behind nginx with HTTPS, so clients connect to an address like
`https://api.example.com/nexo`.

nginx takes care of TLS and forwards requests to the Nexo server, which only listens on the local machine:

```
Nexo client ──https / wss──▶ nginx (api.example.com:443)
                                 │  /nexo/register    → http://127.0.0.1:3000/register
                                 │  /nexo/unregister  → http://127.0.0.1:3000/unregister
                                 │  /nexo/ws          → ws://127.0.0.1:3000/ws
                                 ▼
                             nexo-server (127.0.0.1:3000)
```

Replace `api.example.com` with your domain in all examples below. The paths are those of Debian/Ubuntu;
on other distributions the site configuration usually lives in `/etc/nginx/conf.d/`.

## 1. Build and install the server

```sh
cargo build --release
sudo install -m 755 target/release/nexo-server /usr/local/bin/nexo-server
```

## 2. Run the server as a service

The server stores its database (`nexo.db`) in its working directory, so give it a directory of its own
and a dedicated user:

```sh
sudo useradd --system --home /var/lib/nexo --shell /usr/sbin/nologin nexo
sudo mkdir -p /var/lib/nexo
sudo chown nexo:nexo /var/lib/nexo
sudo chmod 700 /var/lib/nexo
```

Create `/etc/systemd/system/nexo-server.service`:

```ini
[Unit]
Description=Nexo server
After=network.target

[Service]
User=nexo
Group=nexo
WorkingDirectory=/var/lib/nexo
ExecStart=/usr/local/bin/nexo-server

# Listen on the local machine only. nginx is the only way in from outside.
Environment=NEXO_BIND=127.0.0.1:3000

# Take the real client IP from nginx's X-Forwarded-For header (for the rate limits).
# Only trusted for connections from 127.0.0.1, i.e. from nginx on this machine.
Environment=NEXO_TRUST_PROXY=true

# Only the nexo user may read the database
UMask=0077

Restart=on-failure

[Install]
WantedBy=multi-user.target
```

Start it:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now nexo-server
sudo systemctl status nexo-server
```

On `systemctl stop`, the server closes all connections cleanly before it exits.

To require a minimum client version (see [README.md](README.md#minimum-client-version)):

```sh
echo "0.2.0" | sudo -u nexo tee /var/lib/nexo/min_client_version
```

## 3. Install nginx

```sh
sudo apt install nginx
```

No extra modules are needed: proxying and WebSockets are built in.

## 4. Configure the site

Nexo must be reachable via **https**. Over plain http, the messages stay end-to-end encrypted, but anyone on
the network can see who talks to whom and when, and the client shows a warning.

If you don't have a certificate yet, [Certbot](https://certbot.eff.org/) can get one from Let's Encrypt:

```sh
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d api.example.com
```

### WebSocket helper

WebSockets need the `Upgrade` and `Connection` headers to be passed on, which nginx does not do by default.
Create `/etc/nginx/conf.d/websocket.conf` (files in `conf.d/` are loaded in the `http` block):

```nginx
# Connection: upgrade for WebSocket requests, close for everything else
map $http_upgrade $connection_upgrade {
    default upgrade;
    ''      close;
}
```

If this `map` already exists somewhere in your configuration, skip this step: it may only be defined once.

### Server block

Add the `location` block to the `server` block of `api.example.com` that listens on port 443, for example in
`/etc/nginx/sites-available/api.example.com`:

```nginx
server {
    listen 443 ssl;
    listen [::]:443 ssl;
    server_name api.example.com;

    # (ssl_certificate / ssl_certificate_key, e.g. set up by certbot)

    # Nexo: registration, account deletion and the WebSocket for the chat.
    # The trailing slash in proxy_pass removes the /nexo prefix:
    # /nexo/register becomes /register on the Nexo server.
    location /nexo/ {
        proxy_pass http://127.0.0.1:3000/;

        # WebSocket (/nexo/ws)
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;

        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;

        # Idle WebSockets are pinged every 20 seconds. The timeout must be longer,
        # otherwise nginx closes quiet connections.
        proxy_read_timeout 120s;
        proxy_send_timeout 120s;

        # Don't log Nexo traffic (see below)
        access_log off;
    }
}
```

A single `location` covers both the normal requests and the WebSocket: for `/nexo/register` the `Upgrade`
header is empty and the connection is handled as plain HTTP.

`$proxy_add_x_forwarded_for` appends the client's IP address to the header. The Nexo server uses the
**last** entry, so a client can't fake its address by sending its own `X-Forwarded-For`.

Enable the site (if it isn't already), check the configuration and reload nginx:

```sh
sudo ln -s /etc/nginx/sites-available/api.example.com /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

### Using a whole domain instead of a path

If Nexo gets its own domain (e.g. `https://nexo.example.com`), proxy everything without a prefix:

```nginx
server {
    listen 443 ssl;
    listen [::]:443 ssl;
    server_name nexo.example.com;

    # (ssl_certificate / ssl_certificate_key)

    # No access log for this domain at all
    access_log off;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_read_timeout 120s;
        proxy_send_timeout 120s;
    }
}
```

Clients then use `https://nexo.example.com` as server address.

### Redirect http to https

Certbot usually adds this itself. If not:

```nginx
server {
    listen 80;
    listen [::]:80;
    server_name api.example.com;
    return 301 https://$host$request_uri;
}
```

## 5. Logging

Nexo itself does not log anything. nginx, however, writes **every request with IP address and time** to
its access log by default: who connects when, who registers, who deletes their account. This is exactly the
metadata Nexo tries to avoid, so the configuration above turns the access log off with `access_log off;`
inside the `/nexo/` location. Other paths on the domain are still logged as before.

The error log is not affected by this. It usually contains nothing about Nexo, but it includes client IPs
when a request fails (e.g. `connect() failed` while the Nexo server is not running). Keep the default level
`error` (`error_log /var/log/nginx/error.log;`) and don't raise it to `info` or `debug`.

## 6. Test the setup

**The Nexo server is running:**

```sh
sudo systemctl status nexo-server
```

**nginx forwards requests.** A GET request to the registration endpoint must be answered by the Nexo server
with `405 Method Not Allowed` (it only accepts POST):

```sh
curl -i https://api.example.com/nexo/register
```

| Response                   | Meaning                                                                          |
|----------------------------|----------------------------------------------------------------------------------|
| `405 Method Not Allowed`   | Everything works                                                                 |
| `404 Not Found`            | The `location /nexo/` block is missing or in the wrong `server` block, or `proxy_pass` lacks the trailing slash, so the Nexo server gets `/nexo/register` |
| `502 Bad Gateway`          | nginx is configured, but the Nexo server is not running on port 3000             |

**The WebSocket works.** Register from a client with the server address `https://api.example.com/nexo`:

```sh
nexo register
nexo login
```

If `nexo register` works but `nexo login` fails with "Could not reach the Nexo server", the WebSocket is not
forwarded: check the `proxy_http_version 1.1`, `Upgrade` and `Connection` lines and that the `map` from
`websocket.conf` is loaded (`sudo nginx -T | grep connection_upgrade`).

If connections drop after about a minute of silence, `proxy_read_timeout` is missing or too short.

## Client configuration

Use the full address including the path, without a trailing slash:

```
https://api.example.com/nexo
```

It is asked for during `nexo register`, and can be changed later with `nexo server <url>`.

## Checklist

- [ ] `nexo-server` runs as its own user, listening on `127.0.0.1:3000`
- [ ] `NEXO_TRUST_PROXY=true` is set
- [ ] `map $http_upgrade $connection_upgrade` is defined once in the `http` block
- [ ] HTTPS with a valid certificate
- [ ] `location /nexo/` with `proxy_pass http://127.0.0.1:3000/;` (trailing slash)
- [ ] `proxy_http_version 1.1`, `Upgrade` and `Connection` headers set
- [ ] `proxy_read_timeout` longer than 20 seconds
- [ ] `access_log off;` for `/nexo/`
- [ ] `curl -i https://api.example.com/nexo/register` returns `405`
