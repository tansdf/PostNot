# Security Policy

## Supported Versions

PostNot is pre-1.0. Security fixes are applied on a best-effort basis to the current development line and the latest release line.

| Version | Supported |
| --- | --- |
| `master` | Yes |
| `0.18.x` | Yes |
| `< 0.18` | No |

## Reporting a Vulnerability

Please report vulnerabilities privately.

- Preferred: use GitHub's private vulnerability reporting flow from the repository's Security tab when it is available.
- Fallback: until the project publishes a dedicated security inbox, contact the repository owner privately using a non-public contact method listed on the maintainer's GitHub profile or project website.
- Do not open a public issue, discussion, or pull request for a suspected vulnerability.

When reporting, include as much of the following as you can:

- affected version, commit, or branch
- impact and likely attacker capabilities
- reproduction steps or a proof of concept
- whether secrets, local files, updater trust, or script execution are involved
- any suggested mitigation or patch direction

## Response Expectations

Maintainer response times are best-effort, but the target is:

- initial acknowledgement within 5 business days
- a follow-up status update within 10 business days
- coordinated public disclosure after a fix or mitigation is available

## High-Priority Areas

Reports in these areas are especially important for PostNot:

- secret environment storage or redaction failures
- request history leaking resolved secret values
- updater trust, release signing, or install-path vulnerabilities
- script runtime escapes or privilege boundary issues
- local file access, import/export, or credential-store misuse

## Scripting Security Notes

PostNot request scripts are local automation code. They are intended for scripts you wrote or scripts from API workspaces you trust.

As of `0.18.1`, pre-request and test scripts run in a short-lived worker-backed JavaScript sandbox with explicit bridges for `pn.http.send(...)` helper requests and active-environment variable writes. Please report any way for scripts to escape that boundary, access app/page globals unexpectedly, bypass helper-request history behavior, or read/write secrets outside the documented `pn.variables` APIs.

Imported Postman collection scripts are preserved for portability, but complex or untrusted imported scripts should be reviewed before running.

## Disclosure Expectations

Please give maintainers reasonable time to investigate and fix the issue before public disclosure. We will credit reporters who want attribution once a fix is available.

This file is informed by GitHub's security-policy guidance:

- [Adding a security policy to your repository](https://docs.github.com/articles/adding-a-security-policy-to-your-repository)
