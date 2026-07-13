# Security Policy

## Supported versions

bitrep is pre-1.0 and follows a rolling release model. Security fixes are
applied to the latest published release on the current `0.x` line; older
`0.x` releases are not maintained. Please upgrade to the newest release
before reporting.

| Version        | Supported          |
| -------------- | ------------------ |
| latest `0.x`   | :white_check_mark: |
| older `0.x`    | :x:                |

## Reporting a vulnerability

Please report suspected vulnerabilities **privately** — do not open a public
issue, pull request, or discussion for a security report.

- **Preferred:** email **kyle@simgen.dev** with the details below.
- If you prefer GitHub's coordinated flow, use **"Report a vulnerability"**
  under the repository's **Security** tab (private advisory).

Include, where possible:

- affected version / commit and platform (target triple, `--features`),
- a description of the issue and its impact,
- a minimal reproduction (input values, code snippet, or failing vector),
- any known mitigation.

## Disclosure expectations

- We aim to **acknowledge** a report within **5 business days**.
- We will work with you on a fix and a coordinated disclosure timeline,
  targeting a resolution within **90 days** of acknowledgement.
- Please give us a reasonable opportunity to release a fix before any public
  disclosure. We are happy to credit reporters who wish to be named.

Because bitrep is a deterministic numerical library, the highest-value reports
are ones that break a **correctness or reproducibility guarantee** (e.g. a
platform-dependent result, a golden/conformance-vector mismatch, or a decoder
that accepts malformed input), in addition to conventional memory-safety or
supply-chain issues.
