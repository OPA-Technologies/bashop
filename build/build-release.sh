#!/usr/bin/env bash
#
# Construit le binaire Bashop en release et assemble l'archive pré-compilée.
#
# Aucun binaire n'est « noarch » : chaque OS/architecture/variante reçoit son
# archive. C'est l'installateur (`install.sh`) qui est noarch et choisit la
# bonne entrée de l'index selon l'OS et l'architecture détectés.
#
# Usage : build/build-release.sh [cible]
#   cible : "native" (défaut) ou un triplet rust, ex. x86_64-unknown-linux-musl
#
# Compilation croisée : les cibles non natives sont bâties via `cargo zigbuild`
# (zig) quand l'outil est disponible, sinon via `cargo build --target`. Les
# cibles musl donnent un binaire statique (ciblé DSM/SRM) et portent la
# variante « musl » dans le nom d'archive.
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

# Variante d'archive associée à une cible (suffixe du nom), vide si aucune.
variante_cible()
  {
    local cible="$1"
    case "$cible" in
      *musl*) printf 'musl' ;;
      *) printf '' ;;
    esac
  }

read -r OS ARCH <<<"$(os_arch "$CIBLE")"
[ "$OS" != "inconnu" ] && [ "$ARCH" != "inconnu" ] || { printf 'Cible non reconnue : %s\n' "$CIBLE" >&2; exit 1; }
VARIANTE="$(variante_cible "$CIBLE")"

if [ "$OS" = "windows" ]
  then
    BIN_NAME="$BIN.exe"
  else
    BIN_NAME="$BIN"
  fi

if [ "$CIBLE" = "native" ]
  then
    SPEC=()
    BIN_PATH="$ROOT/target/release/$BIN_NAME"
  else
    SPEC=(--target "$CIBLE")
    rustup target add "$CIBLE" >/dev/null 2>&1 || true
    BIN_PATH="$ROOT/target/$CIBLE/release/$BIN_NAME"
  fi

if [ "$CIBLE" = "native" ] || ! command -v cargo-zigbuild >/dev/null 2>&1
  then
    cargo build --release "${SPEC[@]}" --manifest-path "$ROOT/Cargo.toml"
  else
    rustup target add "$CIBLE" >/dev/null 2>&1 || true
    cargo zigbuild --release "${SPEC[@]}" --manifest-path "$ROOT/Cargo.toml"
  fi

DIST="$ROOT/dist"
NOM="bashop-$VERSION-$OS-$ARCH${VARIANTE:+-$VARIANTE}"
STAGE="$DIST/$NOM"
rm -rf "$STAGE"
mkdir -p "$STAGE/licenses"
cp "$BIN_PATH" "$STAGE/$BIN_NAME"
cp "$ROOT/LICENSE" "$STAGE/licenses/"
strip "$STAGE/$BIN_NAME" 2>/dev/null || true

if [ "$OS" = "windows" ]
  then
    ARCHIVE="$DIST/$NOM.zip"
    ( cd "$STAGE" && zip -q -r "$ARCHIVE" "$BIN_NAME" licenses )
  else
    ARCHIVE="$DIST/$NOM.tar.gz"
    tar -C "$STAGE" -czf "$ARCHIVE" "$BIN_NAME" licenses
  fi
rm -rf "$STAGE"

printf 'Archive  : %s\n' "$(basename "$ARCHIVE")"
printf 'SHA256   : %s\n' "$(cd "$DIST" && sha256sum "$(basename "$ARCHIVE")" | awk '{print $1}')"