# Security policy

## Project status and supported versions

InstantDelay is experimental and does not yet provide a production support or response-time guarantee. Use the latest source on the default branch when assessing a security issue.

## Reporting a vulnerability

Please do not publish exploit details, stream keys, access tokens, or other secrets in a public issue. Use GitHub's **Report a vulnerability** flow from the repository's Security tab to send a private report. If that option is unavailable, contact the repository owner privately through GitHub before disclosing details publicly.

Include, where possible:

- A concise impact summary and affected version/commit.
- Reproduction steps or a proof of concept that does not expose real credentials or target third parties.
- Any suggested mitigation.

Please allow time for triage and a coordinated fix before public disclosure.

## Security notes for users

- Never share or commit RTMP stream keys or local settings containing credentials.
- The HTTP API and overlay services are intended for local use. Do not expose their ports to untrusted networks.
- Treat releases as experimental until the full ingest, buffer, and relay path has been tested with a private stream.
