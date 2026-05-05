#!/usr/bin/env bash
# V1.17 Welle B — One-time maintainer setup for SSH-based tag signing.
#
# Purpose: configure the local git checkout so that `git tag -s vX.Y.Z`
# produces an SSH-signed tag using a key that's listed in
# `.github/allowed_signers`. Idempotent — re-running on an already-
# configured checkout is a no-op except for emitting the current state.
#
# Subcommands:
#
#   bash tools/setup-tag-signing.sh init [--key <path-to-pubkey>]
#     Configures local git: gpg.format=ssh, user.signingkey=<path>,
#     tag.gpgSign=true, gpg.ssh.allowedSignersFile=.github/allowed_signers.
#     If `--key` is OMITTED entirely (auto-pick mode), walks ~/.ssh/*.pub
#     and picks the first key whose public counterpart is present in
#     .github/allowed_signers. Errors out cleanly if no match — better
#     than silently configuring a key whose public counterpart isn't
#     trusted. Note: there is no literal `--key auto` value; pass `--key
#     <path>` for explicit selection or omit `--key` for auto-pick.
#
#   bash tools/setup-tag-signing.sh add <principal> <pubkey-path>
#     Appends a new (principal, key) entry to .github/allowed_signers
#     in the canonical format. Strips comments / trailing whitespace.
#     Refuses to add a duplicate (same key bytes already present).
#
#   bash tools/setup-tag-signing.sh status
#     Reports the local git config for tag signing + the contents of
#     .github/allowed_signers, without modifying anything.
#
# Why a script (rather than a docs-only flow):
#   * Five `git config` calls + a path lookup + a sanity check is
#     enough machinery that a copy-paste from the runbook drifts. The
#     script makes the setup atomic + idempotent.
#   * The `--key auto` path requires walking ~/.ssh/, comparing public
#     bytes against `.github/allowed_signers`, and picking a match. That
#     logic shouldn't be inlined in OPERATOR-RUNBOOK.md prose.
#   * Locally-runnable means a contributor can `bash tools/setup-tag-
#     signing.sh status` to confirm their checkout is correctly wired
#     before pushing a tag, catching misconfiguration before a CI
#     red-fail.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
ALLOWED_SIGNERS="${REPO_ROOT}/.github/allowed_signers"

usage() {
  cat <<EOF
Usage:
  bash tools/setup-tag-signing.sh init [--key <path-to-pubkey>]
  bash tools/setup-tag-signing.sh add <principal> <pubkey-path>
  bash tools/setup-tag-signing.sh status

See docs/OPERATOR-RUNBOOK.md §13 for the full one-time setup flow.
EOF
}

cmd="${1:-}"
case "${cmd}" in
  init)
    shift
    KEY_PATH=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --key)
          KEY_PATH="${2:-}"
          shift 2
          ;;
        *)
          echo "ERROR: unknown init flag: $1" >&2
          usage >&2
          exit 2
          ;;
      esac
    done

    # Auto-pick: walk ~/.ssh/*.pub, find the first one whose public
    # bytes match a line in .github/allowed_signers (substring match
    # on the key body). This is the maintainer's canonical signing key.
    if [ -z "${KEY_PATH}" ]; then
      if [ ! -f "${ALLOWED_SIGNERS}" ]; then
        echo "ERROR: ${ALLOWED_SIGNERS} missing — populate the trust root first." >&2
        exit 2
      fi
      for candidate in "${HOME}"/.ssh/*.pub; do
        [ -f "${candidate}" ] || continue
        # Extract the base64-encoded key body (the middle field of an
        # OpenSSH public-key line: `<type> <base64> [comment]`).
        CANDIDATE_BODY="$(awk '{print $2}' < "${candidate}")"
        [ -n "${CANDIDATE_BODY}" ] || continue
        if grep -qF "${CANDIDATE_BODY}" "${ALLOWED_SIGNERS}"; then
          KEY_PATH="${candidate}"
          echo "INFO: auto-picked signing key: ${KEY_PATH}"
          break
        fi
      done
      if [ -z "${KEY_PATH}" ]; then
        echo "ERROR: no key in ~/.ssh/*.pub matches an entry in" >&2
        echo "       ${ALLOWED_SIGNERS}" >&2
        echo "       Either pass --key <path> explicitly, or add your key" >&2
        echo "       via:  bash tools/setup-tag-signing.sh add <principal> <pubkey-path>" >&2
        exit 2
      fi
    fi

    if [ ! -f "${KEY_PATH}" ]; then
      echo "ERROR: pubkey file does not exist: ${KEY_PATH}" >&2
      exit 2
    fi

    # Confirm the chosen key's body is actually in allowed_signers
    # (the auto-pick path ensured this; the explicit-flag path needs
    # the same guard so `--key /path/to/random.pub` doesn't silently
    # configure an untrusted key).
    KEY_BODY="$(awk '{print $2}' < "${KEY_PATH}")"
    if [ -z "${KEY_BODY}" ]; then
      echo "ERROR: ${KEY_PATH} doesn't look like an OpenSSH public key" >&2
      exit 2
    fi
    if ! grep -qF "${KEY_BODY}" "${ALLOWED_SIGNERS}"; then
      echo "ERROR: ${KEY_PATH} is not in ${ALLOWED_SIGNERS}" >&2
      echo "       Tags signed by this key would fail CI verification." >&2
      echo "       Add it first via:" >&2
      echo "         bash tools/setup-tag-signing.sh add <principal> ${KEY_PATH}" >&2
      exit 2
    fi

    git config --local gpg.format ssh
    git config --local user.signingkey "${KEY_PATH}"
    git config --local tag.gpgSign true
    # Note: `gpg.ssh.allowedSignersFile` only matters for `git verify-tag`
    # (verification side). Setting it locally lets the maintainer
    # `git verify-tag <local-tag>` after signing, without needing to
    # re-pass `-c` flags.
    git config --local gpg.ssh.allowedSignersFile "${ALLOWED_SIGNERS}"

    echo "OK: tag-signing configured."
    echo "  gpg.format                    = $(git config --local --get gpg.format)"
    echo "  user.signingkey               = $(git config --local --get user.signingkey)"
    echo "  tag.gpgSign                   = $(git config --local --get tag.gpgSign)"
    echo "  gpg.ssh.allowedSignersFile    = $(git config --local --get gpg.ssh.allowedSignersFile)"
    echo ""
    echo "Cut a signed tag with:  git tag -s v1.17.0 -m 'release v1.17.0'"
    echo "Verify locally with:    bash tools/verify-tag-signatures.sh v1.17.0"
    ;;

  add)
    shift
    if [ "$#" -ne 2 ]; then
      echo "ERROR: 'add' requires <principal> <pubkey-path>" >&2
      usage >&2
      exit 2
    fi
    PRINCIPAL="$1"
    PUBKEY_PATH="$2"

    if [ ! -f "${PUBKEY_PATH}" ]; then
      echo "ERROR: pubkey file does not exist: ${PUBKEY_PATH}" >&2
      exit 2
    fi
    # Public-key file format: `<type> <base64> [comment]`. Strip the
    # comment field so the resulting line in allowed_signers is
    # canonical. Reject if the type isn't on the allow-list (DSA is
    # deprecated; non-NIST ECDSA curves are footguns; FIDO2-backed
    # `sk-` variants are explicitly OK).
    KEY_TYPE="$(awk '{print $1}' < "${PUBKEY_PATH}")"
    KEY_BODY="$(awk '{print $2}' < "${PUBKEY_PATH}")"
    if [ -z "${KEY_TYPE}" ] || [ -z "${KEY_BODY}" ]; then
      echo "ERROR: ${PUBKEY_PATH} does not look like an OpenSSH public key" >&2
      exit 2
    fi
    case "${KEY_TYPE}" in
      ssh-ed25519|sk-ssh-ed25519@openssh.com|ecdsa-sha2-nistp256|ecdsa-sha2-nistp384|ecdsa-sha2-nistp521)
        ;;
      ssh-rsa)
        # RSA-1024 has been factorable by well-resourced adversaries
        # since the early 2010s; RSA-2048 is end-of-life per NIST
        # SP800-131A (deprecated post-2030 for new signatures, but
        # already too short for an auditor-grade trust root in 2026).
        # `ssh-keygen -l -f <pubkey>` prints `<bits> <fp> ...`. Reject
        # anything below 3072.
        BITS="$(ssh-keygen -l -f "${PUBKEY_PATH}" 2>/dev/null | awk '{print $1}')"
        if [ -z "${BITS}" ] || ! [ "${BITS}" -eq "${BITS}" ] 2>/dev/null; then
          echo "ERROR: could not determine RSA modulus length of ${PUBKEY_PATH}" >&2
          echo "       (ssh-keygen output: $(ssh-keygen -l -f "${PUBKEY_PATH}" 2>&1))" >&2
          exit 2
        fi
        if [ "${BITS}" -lt 3072 ]; then
          echo "ERROR: ssh-rsa key is ${BITS} bits; minimum 3072 required." >&2
          echo "       Generate a stronger key: ssh-keygen -t rsa -b 4096 ..." >&2
          echo "       Or prefer ed25519:        ssh-keygen -t ed25519 ..." >&2
          exit 2
        fi
        ;;
      *)
        echo "ERROR: key type '${KEY_TYPE}' is not on the allow-list." >&2
        echo "       Permitted: ssh-ed25519, ssh-rsa (>=3072 bits)," >&2
        echo "                  sk-ssh-ed25519@openssh.com (FIDO2)," >&2
        echo "                  ecdsa-sha2-nistp{256,384,521}." >&2
        exit 2
        ;;
    esac

    if [ -f "${ALLOWED_SIGNERS}" ] && grep -qF "${KEY_BODY}" "${ALLOWED_SIGNERS}"; then
      echo "INFO: key body already present in ${ALLOWED_SIGNERS}; nothing to do."
      exit 0
    fi

    # Append on its own line — ensure the file ends with a newline
    # before appending so we don't accidentally concatenate two entries.
    if [ -f "${ALLOWED_SIGNERS}" ] && [ -s "${ALLOWED_SIGNERS}" ]; then
      LAST_BYTE="$(tail -c 1 "${ALLOWED_SIGNERS}" | od -An -c | tr -d ' ')"
      if [ "${LAST_BYTE}" != "\\n" ]; then
        printf '\n' >> "${ALLOWED_SIGNERS}"
      fi
    fi
    printf '%s %s %s\n' "${PRINCIPAL}" "${KEY_TYPE}" "${KEY_BODY}" >> "${ALLOWED_SIGNERS}"
    echo "OK: appended ${PRINCIPAL} (${KEY_TYPE}) to ${ALLOWED_SIGNERS}"
    echo ""
    echo "Now commit and push the trust-root update:"
    echo "  git add ${ALLOWED_SIGNERS}"
    echo "  git commit -m 'chore(v1.17/welle-b): add maintainer key to allowed_signers'"
    ;;

  status)
    echo "=== Local git config (tag signing) ==="
    for k in gpg.format user.signingkey tag.gpgSign gpg.ssh.allowedSignersFile; do
      v="$(git config --local --get "${k}" 2>/dev/null || echo '<unset>')"
      printf '  %-30s = %s\n' "${k}" "${v}"
    done
    echo ""
    echo "=== Trust root (${ALLOWED_SIGNERS}) ==="
    if [ -f "${ALLOWED_SIGNERS}" ]; then
      # Pure-bash while-read loop — earlier `awk system()` pattern
      # interpolated the key body field directly into a shell command
      # string passed to system(), which would execute attacker-
      # controlled bytes if `.github/allowed_signers` was mutated by a
      # compromised maintainer. Read fields safely into shell vars,
      # then pipe each via printf (no eval, no system()).
      while IFS=' ' read -r principal keytype keybody _rest; do
        # Skip comment / blank lines.
        case "${principal}" in
          ''|\#*) continue ;;
        esac
        if [ -z "${keytype}" ] || [ -z "${keybody}" ]; then
          continue
        fi
        # SHA-256 of the key body, truncated to 16 hex chars for compact
        # display. printf '%s' avoids any locale-dependent escape
        # interpretation; sha256sum reads stdin which is not parsed as
        # shell.
        keybody_hash="$(printf '%s' "${keybody}" | sha256sum | cut -c1-16)"
        printf '  %s   %s   (key body sha256: %s...)\n' \
          "${principal}" "${keytype}" "${keybody_hash}"
      done < "${ALLOWED_SIGNERS}"
    else
      echo "  <missing>"
    fi
    ;;

  ""|-h|--help)
    usage
    ;;

  *)
    echo "ERROR: unknown subcommand: ${cmd}" >&2
    usage >&2
    exit 2
    ;;
esac
