#!/usr/bin/env bash
#
# Génère dist/index.json à partir des archives pré-compilées de dist/.
#
# L'index est le point d'entrée de l'installateur noarch : il associe os/arch
# (et variante optionnelle) à une URL d'archive et à son empreinte SHA256.
# Le nom de fichier porte la nomenclature :
#   bashop-<version>-<os>-<arch>.tar.gz          (sans variante)
#   bashop-<version>-<os>-<arch>-<variante>.tar.gz
# ex. bashop-0.5.1-linux-x86_64-musl.tar.gz
#
# Variable d'environnement :
#   BASHOP_BASE_URL   préfixe des URL (défaut : dépôt GitHub releases)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
BASE_URL="${BASHOP_BASE_URL:-https://github.com/OPA-Technologies/bashop/releases/download}"
VERSION="$(awk -F' +' '/^version *=/{print $3; exit}' "$ROOT/Cargo.toml" | tr -d '"')"

shopt -s nullglob
archives=("$DIST"/bashop-*.tar.gz)
if [ "${#archives[@]}" -eq 0 ]
  then
    printf 'Aucune archive dans %s (lancer build-release.sh d abord).\n' "$DIST" >&2
    exit 1
  fi

{
  printf '{\n'
  printf '  "name": "bashop",\n'
  printf '  "version": "%s",\n' "$VERSION"
  printf '  "assets": [\n'
  premier=1
  for archive in "${archives[@]}"
    do
      base="$(basename "$archive")"
      # bashop-<version>-<os>-<arch>[-<variante>].tar.gz
      reste="${base#bashop-}"
      sans_ext="${reste%.tar.gz}"
      v="${sans_ext%%-*}"
      reste2="${sans_ext#*-}"
      os="${reste2%%-*}"
      reste3="${reste2#*-}"
      arch="${reste3%%-*}"
      case "$reste3" in
        *-*) variante="${reste3#*-}" ;;
        *) variante="" ;;
      esac
      sha="$(sha256sum "$archive" | awk '{print $1}')"
      url="$BASE_URL/v$v/$base"
      [ "$premier" -eq 0 ] && printf ',\n'
      premier=0
      printf '    { "os": "%s", "arch": "%s", "version": "%s", "variante": "%s", "url": "%s", "sha256": "%s" }' \
        "$os" "$arch" "$v" "$variante" "$url" "$sha"
    done
  printf '\n  ]\n'
  printf '}\n'
} > "$DIST/index.json"

printf 'Index écrit : %s\n' "$DIST/index.json"