# Running Nexo behind Apache

This guide sets up the Nexo server behind Apache 2.4 with HTTPS, so clients connect to an address like
`https://api.example.com/nexo`.

Apache takes care of TLS and forwards requests to the Nexo server, which only listens on the local machine:

```
Nexo client ──https / wss──▶ Apache (api.example.com:443)
                                 │  /nexo/register    → http://127.0.0.1:3000/register
                                 │  /nexo/unregister  → http://127.0.0.1:3000/unregister
                                 │  /nexo/ws          → ws://127.0.0.1:3000/ws
                                 ▼
                             nexo-server (127.0.0.1:3000)
```

Replace `api.example.com` with your domain in all examples below.

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

# Listen on the local machine only. Apache is the only way in from outside.
Environment=NEXO_BIND=127.0.0.1:3000

# Take the real client IP from Apache's X-Forwarded-For header (for the rate limits).
# Only trusted for connections from 127.0.0.1, i.e. from Apache on this machine.
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

## 3. Enable the Apache modules

```sh
sudo a2enmod ssl proxy proxy_http proxy_wstunnel headers
sudo systemctl restart apache2
```

`proxy_wstunnel` is needed for the WebSocket connection (`/nexo/ws`), which carries the chat messages.

## 4. Configure the virtual host

Nexo must be reachable via **https**. Over plain http, the messages stay end-to-end encrypted, but anyone on
the network can see who talks to whom and when, and the client shows a warning.

If you don't have a certificate yet, [Certbot](https://certbot.eff.org/) can get one from Let's Encrypt:

```sh
sudo certbot --apache -d api.example.com
```

Add the following to the `<VirtualHost *:443>` block of `api.example.com`, for example in
`/etc/apache2/sites-available/api.example.com-le-ssl.conf`:

```apache
<VirtualHost *:443>
    ServerName api.example.com

    # (SSLEngine / SSLCertificateFile / SSLCertificateKeyFile, e.g. set up by certbot)

    # Apache only forwards requests, it must never act as an open proxy
    ProxyRequests Off

    # WebSocket for the chat. Must come before the general /nexo/ rule.
    ProxyPass        "/nexo/ws" "ws://127.0.0.1:3000/ws"
    ProxyPassReverse "/nexo/ws" "ws://127.0.0.1:3000/ws"

    # Registration and account deletion.
    # The /nexo prefix is removed: /nexo/register becomes /register on the Nexo server.
    ProxyPass        "/nexo/" "http://127.0.0.1:3000/"
    ProxyPassReverse "/nexo/" "http://127.0.0.1:3000/"

    # Don't log Nexo traffic (see below)
    SetEnvIf Request_URI "^/nexo/" nexo_nolog
    CustomLog ${APACHE_LOG_DIR}/access.log combined env=!nexo_nolog

    ErrorLog ${APACHE_LOG_DIR}/error.log
</VirtualHost>
```

On Apache 2.4.47 or newer, the two `ProxyPass` blocks can be replaced by a single line:

```apache
ProxyPass "/nexo/" "http://127.0.0.1:3000/" upgrade=websocket
```

Apache adds the `X-Forwarded-For` header automatically, nothing else is needed for it.

Check the configuration and reload Apache:

```sh
sudo apachectl configtest
sudo systemctl reload apache2
```

### Using a whole domain instead of a path

If Nexo gets its own domain (e.g. `https://nexo.example.com`), drop the prefix:

```apache
ProxyPass        "/ws" "ws://127.0.0.1:3000/ws"
ProxyPassReverse "/ws" "ws://127.0.0.1:3000/ws"
ProxyPass        "/"   "http://127.0.0.1:3000/"
ProxyPassReverse "/"   "http://127.0.0.1:3000/"

# No access log for this domain at all
CustomLog /dev/null combined
```

Clients then use `https://nexo.example.com` as server address.

## 5. Logging

Nexo itself does not log anything. Apache, however, writes **every request with IP address and time** to
its access log by default: who connects when, who registers, who deletes their account. This is exactly the
metadata Nexo tries to avoid, so the configuration above excludes `/nexo/` from the access log.

The `CustomLog ... env=!nexo_nolog` line **replaces** the existing `CustomLog` line of the virtual host.
If there are several `CustomLog` lines, add `env=!nexo_nolog` to each of them. Other paths on the domain
are still logged as before.

The error log is not affected by this. It usually contains nothing about Nexo, but it can include client IPs
when a request fails (e.g. while the Nexo server is not running). Keep the default `LogLevel warn` or lower.

## 6. Test the setup

**The Nexo server is running:**

```sh
sudo systemctl status nexo-server
```

**Apache forwards requests.** A GET request to the registration endpoint must be answered by the Nexo server
with `405 Method Not Allowed` (it only accepts POST):

```sh
curl -i https://api.example.com/nexo/register
```

| Response                          | Meaning                                                                 |
|-----------------------------------|-------------------------------------------------------------------------|
| `405 Method Not Allowed`          | Everything works                                                        |
| `404 Not Found`                   | The `ProxyPass` rules are missing, or the path is wrong                 |
| `503 Service Unavailable`         | Apache is configured, but the Nexo server is not running on port 3000   |

**The WebSocket works.** Register from a client with the server address `https://api.example.com/nexo`:

```sh
nexo register
nexo login
```

If `nexo register` works but `nexo login` fails with "Could not reach the Nexo server", the WebSocket is not
forwarded: check that `proxy_wstunnel` is enabled and the `/nexo/ws` rule comes before the `/nexo/` rule.

## Client configuration

Use the full address including the path, without a trailing slash:

```
https://api.example.com/nexo
```

It is asked for during `nexo register`, and can be changed later with `nexo server <url>`.

## Checklist

- [ ] `nexo-server` runs as its own user, listening on `127.0.0.1:3000`
- [ ] `NEXO_TRUST_PROXY=true` is set
- [ ] Apache modules `ssl`, `proxy`, `proxy_http`, `proxy_wstunnel` are enabled
- [ ] HTTPS with a valid certificate
- [ ] `/nexo/ws` rule before the `/nexo/` rule
- [ ] Access log disabled for `/nexo/`
- [ ] `curl -i https://api.example.com/nexo/register` returns `405`
