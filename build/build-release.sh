#!/usr/bin/env bash
#
# Construit le binaire Bashop en release et assemble l'archive pré-compilée.
#
# Aucun binaire n'est « noarch » : chaque OS/architecture reçoit son archive.
# C'est l'installateur (`install.sh`) qui est noarch et choisit la bonne.
#
# Usage : build/build-release.sh [cible]
#   cible : "native" (défaut) ou un triplet rust, ex. x86_64-unknown-linux-musl
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(awk -F' +' '/^version *=/{print $3; exit}' "$ROOT/Cargo.toml" | tr -d '"')"
BIN="bashop"
CIBLE="${1:-native}"

# Traduit un triplet rust (ou "native") en couple os/arch de notre nomenclature.
os_arch()
  {
    local cible="$1"
    if [ "$cible" = "native" ]
      then
        local os arch
        case "$(uname -s)" in
          Linux) os="linux" ;;
          FreeBSD) os="freebsd" ;;
          Darwin) os="darwin" ;;
          *) os="inconnu" ;;
        esac
        case "$(uname -m)" in
          x86_64|amd64) arch="x86_64" ;;
          aarch64|arm64) arch="aarch64" ;;
          armv7l|armv7|arm) arch="armv7" ;;
          *) arch="inconnu" ;;
        esac
        printf '%s %s' "$os" "$arch"
        return
      fi
    local os="linux" arch=""
    case "$cible" in
      *freebsd*) os="freebsd" ;;
      *windows*) os="windows" ;;
      *darwin*|*apple*) os="darwin" ;;
    esac
    case "$cible" in
      x86_64-*) arch="x86_64" ;;
      aarch64-*) arch="aarch64" ;;
      armv7-*|arm-*) arch="armv7" ;;
      *) arch="inconnu" ;;
    esac
    printf '%s %s' "$os" "$arch"
  }

if [ "$CIBLE" = "native" ]
  then
    SPEC=()
    BIN_PATH="$ROOT/target/release/$BIN"
  else
    SPEC=(--target "$CIBLE")
    rustup target add "$CIBLE" >/dev/null 2>&1 || true
    BIN_PATH="$ROOT/target/$CIBLE/release/$BIN"
  fi

cargo build --release "${SPEC[@]}" --manifest-path "$ROOT/Cargo.toml"

read -r OS ARCH <<<"$(os_arch "$CIBLE")"
[ "$OS" != "inconnu" ] && [ "$ARCH" != "inconnu" ] || { printf 'Cible non reconnue : %s\n' "$CIBLE" >&2; exit 1; }

DIST="$ROOT/dist"
NOM="bashop-$VERSION-$OS-$ARCH"
STAGE="$DIST/$NOM"
rm -rf "$STAGE"
mkdir -p "$STAGE/licenses"
cp "$BIN_PATH" "$STAGE/$BIN"
cp "$ROOT/LICENSE" "$STAGE/licenses/"
strip "$STAGE/$BIN" 2>/dev/null || true

TARBALL="$DIST/$NOM.tar.gz"
tar -C "$STAGE" -czf "$TARBALL" "$BIN" licenses
rm -rf "$STAGE"

printf 'Archive  : %s\n' "$(basename "$TARBALL")"
printf 'SHA256   : %s\n' "$(cd "$DIST" && sha256sum "$(basename "$TARBALL")" | awk '{print $1}')"
