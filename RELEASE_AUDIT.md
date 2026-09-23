# Release audit — 2026-09-23

This is a snapshot of local `master` at `6e56537` before this report was added.
It covers release readiness on Ubuntu 24.04 / GNOME Shell 46, Git history, and
public exposure. SEO is outside this audit.

## Verified

- `bash scripts/check.sh` passed: formatting, Clippy, 166 Rust tests, shell
  syntax, 43 Python tests, and both shell regression scripts.
- Rust 1.75.0 built the locked release binary and passed all Rust tests in a
  Docker container. This matches Ubuntu 24.04's apt toolchain.
- The pinned Resource Monitor v27 archive was downloadable, matched its
  recorded SHA-256, and all eight installer patch steps succeeded on a fresh
  extraction in a temporary home.
- The installed extension was active on GNOME Shell 46 with user extensions
  enabled; the last 24 hours of the user journal showed no matching Resource
  Monitor errors. This checks the current desktop, not a fresh GUI installation.
- OSV returned no advisories for the 51 locked crates.io package versions on
  the audit date.
- All 208 commits reachable from local refs, and all 294 local commit objects
  including unreachable ones, used `mikaeltorni25@gmail.com` as both author and
  committer. The remote advertised only `master` and no tags.
- All 660 local blob objects, including 110 unreachable blobs, were scanned for
  private-key blocks, common provider tokens, credential assignments and URLs,
  and SSH private-key material. No match was found. Historical file names and
  commit messages showed no credential-bearing names or values.

## Publication notes

- One historical `AGENTS.md` comment contains a local home-directory path. It
  is reachable from the remote branch and reveals a local username, but no
  password, token, key, or private file content. Removing it from existing
  history would require an explicitly authorized history rewrite.
- The GitHub repository was **private** at audit time. Local `master` had the
  release fix and its merge commit, two commits ahead of `origin/master`.
  Those fixes must reach the remote before changing its visibility.
- A pattern scan cannot prove that an unknown secret format was never present.
  A fresh Ubuntu desktop installation and visual check were not performed.
