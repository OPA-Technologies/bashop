#!/bin/sh
#
# Installateur noarch de Bashop.
#
# Détecte l'OS et l'architecture, télécharge le binaire pré-compilé correspondant
# depuis l'index JSON, vérifie son empreinte SHA256, l'installe sous PREFIX puis
# branche le bootstrap dans les rc des shells.
#
# Aucune chaîne de compilation Rust n'est requise : c'est indispensable sur les
# cibles sans compilateur (DSM 7.2, SRM...).
#
# Variables d'environnement :
#   PREFIX         répertoire d'installation (défaut : ~/.local)
#   VERSION        version à installer (défaut : la plus récente de l'index)
#   BASHOP_BIN     chemin d'un binaire local à installer au lieu de télécharger
#   BASHOP_INDEX   URL de l'index JSON
#
# POSIX sh uniquement (compatible BusyBox ash).
set -eu

PREFIX="${PREFIX:-$HOME/.local}"
VERSION="${VERSION:-}"
BASHOP_BIN="${BASHOP_BIN:-}"
BASHOP_INDEX="${BASHOP_INDEX:-https://github.com/opa/bashop/releases/latest/download/index.json}"

erreur()
  {
    printf 'bashop-install : %s\n' "$*" >&2
    exit 1
  }

info()
  {
    printf 'bashop-install : %s\n' "$*"
  }

# Télécharge une URL vers un fichier, avec curl ou wget selon la disponibilité.
telecharger()
  {
    url="$1"
    dest="$2"
    if command -v curl >/dev/null 2>&1
      then
        curl -fsSL "$url" -o "$dest"
      else
        command -v wget >/dev/null 2>&1 || erreur "curl ou wget est requis pour télécharger."
        wget -qO "$dest" "$url"
      fi
  }

# Normalise le nom d'OS en notre nomenclature (linux, freebsd, darwin, windows).
os_normalise()
  {
    cas="$(uname -s)"
    case "$cas" in
      Linux*) printf 'linux' ;;
      FreeBSD*) printf 'freebsd' ;;
      Darwin*) printf 'darwin' ;;
      MINGW*|MSYS*|CYGWIN*) printf 'windows' ;;
      *) erreur "OS non pris en charge : $cas" ;;
    esac
  }

# Normalise l'architecture en notre nomenclature (x86_64, aarch64, armv7).
arch_normalise()
  {
    cas="$(uname -m)"
    case "$cas" in
      x86_64|amd64) printf 'x86_64' ;;
      aarch64|arm64) printf 'aarch64' ;;
      armv7l|armv7|arm) printf 'armv7' ;;
      *) erreur "architecture non prise en charge : $cas" ;;
    esac
  }

# Extrait la valeur d'une clé JSON scalaire, sans dépendre d'un parseur externe.
valeur_json()
  {
    fichier="$1"
    cle="$2"
    sed -n "s/.*\"$cle\"[[:space:]]*:[[:space:]]*\"\([^\"]*\)\".*/\1/p" "$fichier" | head -n1
  }

# Vérifie l'empreinte SHA256 d'un fichier (sha256sum, shasum ou openssl).
verifier_sha256()
  {
    fichier="$1"
    attendu="$2"
    if command -v sha256sum >/dev/null 2>&1
      then
        obtenu="$(sha256sum "$fichier" | awk '{print $1}')"
      elif command -v shasum >/dev/null 2>&1
        then
          obtenu="$(shasum -a 256 "$fichier" | awk '{print $1}')"
      elif command -v openssl >/dev/null 2>&1
        then
          obtenu="$(openssl dgst -sha256 "$fichier" | awk '{print $NF}')"
      else
        erreur "aucun outil SHA256 disponible."
      fi
    [ "$obtenu" = "$attendu" ] || erreur "SHA256 invalide (attendu $attendu, obtenu $obtenu)."
  }

main()
  {
    os="$(os_normalise)"
    arch="$(arch_normalise)"
    info "cible détectée : $os/$arch, PREFIX=$PREFIX"

    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT INT TERM

    if [ -n "$BASHOP_BIN" ]
      then
        binaire="$BASHOP_BIN"
        [ -f "$binaire" ] || erreur "binaire local introuvable : $binaire"
      else
        info "récupération de l'index : $BASHOP_INDEX"
        telecharger "$BASHOP_INDEX" "$tmp/index.json"
        url="$(valeur_json "$tmp/index.json" "url")"
        sha="$(valeur_json "$tmp/index.json" "sha256")"
        [ -n "$url" ] || erreur "index invalide : champ url absent."
        info "téléchargement : $url"
        telecharger "$url" "$tmp/archive.tar.gz"
        [ -n "$sha" ] && verifier_sha256 "$tmp/archive.tar.gz" "$sha"
        tar -xzf "$tmp/archive.tar.gz" -C "$tmp"
        binaire="$tmp/bashop"
        [ -f "$binaire" ] || binaire="$(find "$tmp" -name bashop -type f | head -n1)"
        [ -n "$binaire" ] && [ -f "$binaire" ] || erreur "binaire absent de l'archive."
      fi

    mkdir -p "$PREFIX/bin"
    install -m 0755 "$binaire" "$PREFIX/bin/bashop" 2>/dev/null \
      || { cp "$binaire" "$PREFIX/bin/bashop" && chmod 0755 "$PREFIX/bin/bashop"; }
    info "installé : $PREFIX/bin/bashop"

    "$PREFIX/bin/bashop" install \
      || info "bootstrap non posé (à relancer : $PREFIX/bin/bashop install)"
  }

main "$@"
