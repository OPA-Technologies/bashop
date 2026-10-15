#!/bin/sh
#
# Installateur noarch de Bashop.
#
# Détecte l'OS et l'architecture, lit l'index JSON, choisit l'archive
# pré-compilée correspondante (os + architecture + variante éventuelle),
# télécharge, vérifie son empreinte SHA256, installe sous PREFIX puis branche
# le bootstrap dans les rc des shells.
#
# Aucune chaîne de compilation Rust n'est requise : c'est indispensable sur les
# cibles sans compilateur (DSM 7.2, SRM...).
#
# Variables d'environnement :
#   PREFIX         répertoire d'installation (défaut : ~/.local)
#   VERSION        version à installer (défaut : la plus récente de l'index)
#   VARIANTE       variante demandée (défaut : détection — musl sur Synology)
#   BASHOP_BIN     chemin d'un binaire local à installer au lieu de télécharger
#   BASHOP_INDEX   URL de l'index JSON
#
# POSIX sh uniquement (compatible BusyBox ash).
set -eu

PREFIX="${PREFIX:-$HOME/.local}"
VERSION="${VERSION:-}"
VARIANTE="${VARIANTE:-}"
BASHOP_BIN="${BASHOP_BIN:-}"
BASHOP_INDEX="${BASHOP_INDEX:-https://raw.githubusercontent.com/OPA-Technologies/bashop/master/dist/index.json}"

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

# Détecte une machine Synology (DSM) : binaire Linux lié statiquement (musl).
est_synology()
  {
    [ -f /etc/synoinfo.conf ] || [ -d /etc.defaults ]
  }

# Variante préférée pour l'os donné (détection si VARIANTE non renseignée).
variante_preferee()
  {
    [ -n "$VARIANTE" ] && { printf '%s' "$VARIANTE"; return; }
    if [ "$1" = "linux" ] && est_synology
      then
        printf 'musl'
      else
        printf ''
      fi
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

# Choisit l'archive de l'index : filtre os/arch/version, puis préfère la
# variante voulue (repli : première archive éligible). Écrit url puis sha sur
# deux lignes dans $1/index.json et un tmp.
choisir_archive()
  {
    index="$1"
    want="$(variante_preferee "$os")"
    match=""
    repli=""
    awk -F'"' -v OS="$os" -v ARCH="$arch" -v VER="$VERSION" '
      /"os"/ {
        if ($4 == OS && $8 == ARCH && (VER == "" || $12 == VER))
          printf "%s|%s|%s|%s\n", $16, $12, $20, $24
      }' "$index" > "$index.cand"
    [ -s "$index.cand" ] || erreur "aucune archive pour $os/$arch${VERSION:+ (version $VERSION)}."
    while IFS='|' read -r va _ url sha
      do
        [ -z "$repli" ] && repli="$url|$sha"
        if [ -z "$match" ] && [ "$va" = "$want" ]
          then
            match="$url|$sha"
          fi
      done < "$index.cand"
    rm -f "$index.cand"
    [ -z "$match" ] && match="$repli"
    url="${match%%|*}"
    sha="${match##*|}"
    [ -n "$url" ] || erreur "index invalide : champ url absent."
    printf '%s\n%s\n' "$url" "$sha"
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
        # « url » puis « sha256 » sur deux lignes successives.
        choisi="$(choisir_archive "$tmp/index.json")"
        url="$(printf '%s\n' "$choisi" | sed -n 1p)"
        sha="$(printf '%s\n' "$choisi" | sed -n 2p)"
        info "téléchargement : $url"
        case "$url" in
          *.zip) archive="$tmp/archive.zip" ;;
          *) archive="$tmp/archive.tar.gz" ;;
        esac
        telecharger "$url" "$archive"
        [ -n "$sha" ] && verifier_sha256 "$archive" "$sha"
        if [ "${archive##*.}" = "zip" ]
          then
            if command -v unzip >/dev/null 2>&1
              then
                unzip -q "$archive" -d "$tmp"
              elif command -v powershell >/dev/null 2>&1
                then
                  powershell -NoProfile -Command "Expand-Archive -LiteralPath '$archive' -DestinationPath '$tmp' -Force"
              else
                erreur "unzip (ou powershell) est requis pour extraire une archive Windows."
              fi
          else
            tar -xzf "$archive" -C "$tmp"
          fi
        binaire="$(find "$tmp" \( -name bashop -o -name bashop.exe \) -type f 2>/dev/null | head -n1)"
        [ -n "$binaire" ] && [ -f "$binaire" ] || erreur "binaire absent de l'archive."
      fi

    if [ "$os" = "windows" ]
      then
        cible="$PREFIX/bin/bashop.exe"
      else
        cible="$PREFIX/bin/bashop"
      fi
    mkdir -p "$PREFIX/bin"
    install -m 0755 "$binaire" "$cible" 2>/dev/null \
      || { cp "$binaire" "$cible" && chmod 0755 "$cible"; }
    info "installe : $cible"

    "$cible" install \
      || info "bootstrap non pose (a relancer : $cible install)"
  }

main "$@"