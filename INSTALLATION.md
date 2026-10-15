# Installation (préstable)

## Doctrine de distribution

Bashop est distribué sous forme de **binaires pré-compilés** : aucune chaîne de
compilation Rust n'est requise sur la machine cible. C'est indispensable sur
les cibles sans compilateur (DSM 7.2, SRM…).

L'installateur est **noarch** : un script unique détecte l'OS et l'architecture,
télécharge le binaire correspondant depuis les *releases* GitHub, vérifie son
SHA256, l'installe, puis branche le bootstrap dans les rc des shells.

## Voie recommandée — installateur noarch

Unix (Linux, FreeBSD, DSM/SRM) :

```sh
sh build/install.sh
```

Windows (PowerShell) :

```powershell
pwsh -File build/install.ps1
```

Sous Git Bash / MSYS / Cygwin, `install.sh` fonctionne aussi et installe
`bashop.exe`. `install.ps1` est réservé à PowerShell natif.

Options de `install.sh` :

- `PREFIX` : répertoire d'installation (défaut `~/.local`).
- `VERSION` : version à installer (défaut : la plus récente de l'index).
- `VARIANTE` : variante d'archive à privilégier (`musl` sur Synology, vide =
  binaire natif de l'OS). Détectée automatiquement sur DSM.
- `BASHOP_BIN` : chemin d'un binaire local à installer au lieu de télécharger.
- `BASHOP_INDEX` : URL de l'index JSON (défaut : `dist/index.json` du dépôt,
  servi via `raw.githubusercontent.com`).

`install.sh` **sélectionne** l'entrée de l'index selon l'OS et l'architecture
détectés (et la variante), puis vérifie le SHA256 de l'archive avant de
l'installer.

## Dépendances (aucune obligatoire)

Bashop ne requiert **aucune** dépendance à la compilation ou à l'exécution pour
fonctionner : les cibles pré-compilées sont autonomes (musl statique pour
DSM/SRM). Seul le **clic-milieu (coller)** cherche, en option, un outil de
presse-papier système selon l'OS :

| OS | Outils consultés (par ordre) | Paquet |
|----|------------------------------|--------|
| Linux (Wayland) | `wl-paste`, `wl-paste -p` | `wl-clipboard` |
| Linux (X11) | `xclip`, `xsel` | `xclip` / `xsel` |
| macOS, FreeBSD | `pbpaste` | fourni par l'OS |
| Windows | `pwsh`/`powershell` (`Get-Clipboard`) | fourni par l'OS |

En l'absence de tout outil, le collage retombe sur la **dernière copie interne**
(OSC 52) ; s'il n'y en a pas, une notification « presse-papier vide » s'affiche.
Aucun paquet n'est donc indispensable.

## Cibles binaires produites

| Plateforme | Archive | Liaison |
|------------|---------|---------|
| Ubuntu / Linux x86_64 | `bashop-<v>-linux-x86_64.tar.gz` | glibc |
| Synology DS220+ / DSM 7.2 (Linux x86_64) | `bashop-<v>-linux-x86_64-musl.tar.gz` | statique (musl) |
| Synology SRM : RT1900ac (SRM 1.2), MR2200ac (SRM 1.3) | `bashop-<v>-linux-armv7-musl.tar.gz` | statique (musl, soft-float) |
| FreeBSD CURRENT / STABLE (amd64) | `bashop-<v>-freebsd-x86_64.tar.gz` | `libc.so.7` |
| Windows 10 (x86_64) | `bashop-<v>-windows-x86_64.zip` | PE (`bashop.exe`) |

La cible `linux armv7-musl` utilise le triplet **`armv7-unknown-linux-musleabi`**
(ABI **soft-float**, requis par les routeurs Synology et les DSM ARMv7).
`linux aarch64` est également produit. La compilation croisée utilise
`cargo zigbuild` (zig).

## Depuis les sources (développement)

```sh
ln -s ../Libs/op-tui op-tui   # depuis la racine du dépôt (déjà posé)
cargo build --release
sudo install -Dm755 target/release/bashop ~/.local/bin/bashop
bashop install
```

`op-tui` est une dépendance `path` : dans un paquet pré-compilé elle est liée
dans le binaire et invisible à l'installation ; en archive de sources elle doit
être présente (vendorisée).

## Configuration

- `~/.opshellrc` : format neutre `KEY=VALUE`, lu par le binaire (jamais sourcé
  par un shell). Détaillé dans `SPEC.md`.
- Le bootstrap écrit dans les rc est court : il se contente d'appeler Bashop au
  bon moment (OSC 133, édition mono-ligne, prompt).

### Par shell

- **bash** : bloc dans `~/.bashrc` (`bashop run`) ; `sh` : `~/.profile`.
- **zsh** : bloc dans `~/.zshrc`.
- **PowerShell** : bloc dans `$PROFILE.CurrentUserCurrentHost`
  (`…/Documents/PowerShell/Microsoft.PowerShell_profile.ps1` sous Windows,
  `~/.config/powershell/…` ailleurs) ; il dot-source le profil existant puis
  redéfinit `prompt`.
- **cmd** : *best-effort* via `AutoRun`
  (`HKCU\Software\Microsoft\Command Processor`) pointant sur
  `~/.bashop/bashop-cmd.cmd`, qui pose `PROMPT`. cmd ne pouvant pas émettre le
  marqueur de fin `OSC 133;D`, l'état de la commande est déduit par repli.

## Essai rapide (développement)

Depuis un **vrai terminal** (pas un pipe) :

```sh
cargo build
./target/debug/bashop run
```

À l'intérieur du cadre :

- saisir une commande : l'édition reste celle de bash (flèches, `Ctrl+A`…) ;
- `F1` : aide de la commande en cours (`--help`), en surcouche ;
- `Ctrl+F1` : suggestion issue de l'historique (affichée dans la ligne de
  contexte) ;
- `F10` : aide de Bashop ;
- `F2` : bascule le mode capture (masquage IP/MAC/secrets) ;
- taper `sudo ls` : la ligne de contexte signale `ROOT!` (bascule détectée) ;
- taper une commande destructrice (`rm -rf /`) puis Entrée : une **confirmation**
  s'interpose (`o` exécute, `n` annule) ;
- `exit` ou `Ctrl+D` quitte le cadre et restaure le terminal.

Le prompt injecté (`OSC 133` + thème) est posé via un `--rcfile` temporaire pour
bash et `ZDOTDIR` pour zsh ; `bashop template bash|zsh` l'affiche.

## Règle : ne jamais éditer un fichier déployé à la main

Le bootstrap dans les rc et le fichier `~/.opshellrc` sont générés/édités via
`bashop install` et `bashop config`. Toute modification manuelle peut être
écrasée au prochain passage.
