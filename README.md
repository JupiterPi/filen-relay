# Filen Relay

> [!IMPORTANT]
> **THIS IS NOT AN OFFICIAL FILEN PRODUCT**

> [!IMPORTANT]
> This project is in active development. **Do not use it.**

Filen Relay provides a convenient way to serve your Filen Drive via WebDAV/HTTP/FTP/SFTP. Access the web interface to start multiple servers mapping to multiple users, manage basic permissions and view server logs. 

## Usage

```bash
docker run -p 80:80 ghcr.io/jupiterpi/filen-relay:main
```

## Development

This project uses [Dioxus](https://dioxuslabs.com/). Install it, then:

```bash
dx serve
```
