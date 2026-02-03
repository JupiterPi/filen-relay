# Filen Relay

> [!IMPORTANT]
> **THIS IS NOT AN OFFICIAL FILEN PRODUCT**

> [!IMPORTANT]
> This project is in active development. **Do not use it.**

Filen Relay provides a convenient way to serve your Filen Drive via WebDAV/HTTP/FTP/SFTP. Access the web interface to start multiple servers mapping to multiple users, manage basic permissions and view server logs. 

## Usage

```bash
docker run -e FILEN_RELAY_ADMIN_EMAIL='your-filen-account@email.com' -p 80:80 ghcr.io/jupiterpi/filen-relay:main
```

Options:
- optionally set `FILEN_RELAY_DB_DIR` env variable to configure where the database file is stored inside the container (so it can be mounted more easily)

**Important:** By default, any Filen user is allowed to log into your Filen Relay and create servers. Open "Manage Allowed Users" with your admin account to change this setting.

## Development

This project uses [Dioxus](https://dioxuslabs.com/). Install it, then:

```bash
dx serve
```
