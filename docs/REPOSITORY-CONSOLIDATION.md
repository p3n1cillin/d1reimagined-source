# Repository consolidation — 20 October 2026

The owner requested one public repository containing the former public source and both private D1 repositories. The canonical repository is https://github.com/p3n1cillin/d1reimagined-source.

## Preserved history and layout

| Original repository/ref | Original commit | Destination |
| --- | --- | --- |
| d1reimagined-source/main | 37d49305ae70fafacae60dc57b1be7a9ded8b96b | main history; archive/d1reimagined-source-main-2026-10-20 tag |
| d1reimagined/master | d2a172a7a47d085a26d0e9bf4cfa1b5e4fa359a9 | merged main history; archive/d1reimagined-master-2026-10-20 tag |
| d1reimagined/preserve/main-pc-2026-09-29 | 7293ffc98044624c057de9b27e187316c12f226e | archive/legacy-main-pc branch |
| d1reimagined-data/main | cd388df8e62dbbea410621f6c2a43ca135db5bd6 | merged main history; data/ tree; archive/d1reimagined-data-main-2026-10-20 tag |

The two historical pull-request head commits are ancestors of their respective default branches and remain reachable. Existing public files win overlapping source conflicts, preserving the current PS4 implementation and publication adaptations. Original variants remain available in the imported history. New research notes and tools are included. The retired process-wide punch-sweep script is archived under `notes/Invoke-Final124PunchSweep.retired.ps1`; current owned-run helpers remain the entry points.

The data tree is imported under `data/`. Its README has an explanatory banner; all other data snapshot files retain their original Git blob identities. Package parts stay split to remain within GitHub's file limit. Launch helpers now prefer this checkout's `data/`, with explicit overrides and legacy sibling directories still supported.

## Data verification

From the repository root:

```powershell
python data/reassemble_packages.py --verify-only
```

`data/SHA256SUMS.txt` records the original snapshot hashes. Data assets and third-party binaries retain their original ownership; project source licensing does not relicense them. Historical statements about private or separate repositories describe the former arrangement.

## Migration validation

Before publication, all reachable blobs in both private histories were checked for common credential patterns and GitHub's 100 MiB file limit. The only source-history matches were dummy private-key headers in rejection tests, without key material. Data checksum verification and existing Python regression suites are run against the combined checkout. Original repositories are retained until the public refs and data tree are verified.

Validation results: all 87 manifest entries match size and SHA-256; all five split packages verify. Python suites pass: 225 server tests (one explicit skip), 176 script tests and 82 capture tests. The data-history credential scan has no matches. Neither private history contains a blob above 100 MiB. The bundled data path resolves correctly in PowerShell.
