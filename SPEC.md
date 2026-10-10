# SPEC.md — Bashop

Source unique de la conception. Toute décision structurante (cadre, keymap,
thèmes, `~/.opshellrc`, index de distribution) est consignée ici avant
implémentation. En cas de contradiction entre le code et ce document, ce
document fait foi : on corrige le code ou on amende ce document, jamais les
deux à moitié.

## 1. Objet

Bashop est un **cadre terminal permanent à 3 lignes** enveloppant un shell
interactif : une ligne d'état, une zone centrale émulée, une ligne de contexte.
Il fonctionne de façon homogène sur les shells usuels (`bash`, `zsh`, `sh`,
PowerShell, `cmd`) et sur plusieurs OS (Linux, FreeBSD, Windows, DSM/SRM).

Produit en Rust (edition 2024, `rust-version = 1.88`), licence
GPL-3.0-or-later, crate `bashop` (lib + bin). Version cible de première
release : `0.1.0` (le squelette porte `0.0.1`).

## 2. Positionnement et modèle d'exécution

Bashop **prend possession du terminal** (TTY) et se comporte comme un
**mini-multiplexeur** doté d'un **émulateur VT** pour la zone centrale. Le
shell tourne dans un pseudo-terminal enfant.

- **Modèle B** : le shell conserve l'édition de ligne native ; Bashop **observe**
  les frappes sans les accaparer. Il n'y a pas de réimplémentation de
  l'édition.
- **Détection d'état du shell** : `OSC 133` (marqueurs prompt / commande /
  sortie) en priorité, **repli « idle »** (silence détecté) pour les shells
  sans OSC 133 (`cmd.exe`).
- **Coexistence** : Bashop doit fonctionner sous `tmux`/`screen` et en mode
  **dégradé** si un vrai multiplexeur occupe déjà le terminal.
- **Cadre toujours visible** : la ligne d'état et la ligne de contexte restent
  affichées même pendant `vi`, `less`, `man`.

## 3. Cadre (3 lignes)

Pour un terminal de `N` lignes :

| Ligne     | Nom            | Contenu |
|-----------|----------------|---------|
| 1         | `term_stat`    | état général (voir §3.1) |
| 2..N-1    | zone centrale  | sortie du shell, émulée sur `N-2` lignes |
| N-1       | `term_prompt`  | prompt du shell (voir §3.3) |
| N         | `term_context` | contexte dynamique (voir §3.2) |

`term_prompt` (N-1) et `term_context` (N) forment le bas du cadre ; la zone
centrale démarre à la ligne 2. La zone centrale a donc `N-2` lignes visibles.

### 3.1 `term_stat` (ligne 1)

- Gauche : `user@host` avec `[FQDN IP]` (FQDN et IP réelle entre crochets).
- Centre : `OS + flavor`.
- Droite : drapeau pays + `[code pays]`, date et heure locales.
- Extra compact : heure UTC au format **jour/mois heure:minute**, charge système
  ultra-compacte.
- Le FQDN privilégie `hostname -f`, puis `hostname -d`, `dnsdomainname` et
  `/etc/resolv.conf` (domaine/localisé), sans jamais revenir à un hôte nu si un
  domaine est résoluble.
- Style par défaut : fond B1 `#2E3440`, texte F1 `#E6EDF3`, gras.

### 3.2 `term_context` (dernière ligne)

Barre pleine largeur à trois zones :

1. **Gauche — commandes/raccourcis** : `F1 - Aide contextuelle`, `F10 - Aide`,
   `F2 - Capture`, `Ctrl+F1 - Suggestion`, puis les options découvertes et la
   suggestion fantôme.
2. **Centre — code retour** : `✓ ok` en **vert** si `0`, `✗ N` en **rouge**
   sinon ; le préfixe `ROOT!` est ajouté sous bascule root.
3. **Droite — compteurs système** compacts (cpu / mémoire / io), rafraîchis
   toutes les **0,5 s**.

Style par défaut : fond B2 `#2E3440` (identique au reste du cadre), texte F1
`#E6EDF3`, gras.

### 3.3 `term_prompt` (avant-dernière ligne)

Composants, dans l'ordre : **heure de début de commande** (`HH:MM:SS`, capturée à
l'affichage du prompt), `user@host`, chemin **absolu**, numéro d'historique entre
parenthèses (**gris, italique, cadré à droite**), invite (flèche grasse), charset.

- **Fond commun au cadre** : la couleur de fond par défaut du terminal est fixée
  à celle des trois lignes (`#2E3440`, via `OSC 11`, rétablie à la sortie par
  `OSC 110`) ; le prompt partage donc le fond de `term_stat` et `term_context`.

- **Épinglé sur l'avant-dernière ligne** : le curseur du shell est repositionné
  sur la dernière ligne de sa zone (juste au-dessus de `term_context`) à chaque
  prompt.

- **Une seule ligne physique**, toujours.
- Pendant la saisie, **défilement horizontal ponctuel** de la zone de saisie.
- Après validation, **se rétracte en une seule entrée d'historique**.
- L'**édition native** du shell reste complète au rappel (flèches, `Ctrl+A`,
  etc.).
- Le mode **mono-ligne** est forcé côté shell :
  - bash : `horizontal-scroll-mode on` (forcer la valeur `HISTFILESIZE`
    d'affichage) ;
  - zsh : `setopt singlelinezle` ;
  - PowerShell : PSReadLine en mode ligne unique ;
  - `cmd.exe` : comportement natif mono-ligne.

## 4. Keymap

Actif **uniquement au prompt** (état détecté). Dès qu'une commande est en
cours d'exécution, **toutes** les touches sont transmises au shell/au programme,
sauf **PagePréc/PageSuiv** et la **molette** qui pilotent le défilement de la
zone centrale.

| Touche     | Action |
|------------|--------|
| `F1`       | Aide contextuelle de la commande en cours de saisie : `man` si disponible, sinon `--help` (fork du binaire). |
| `Ctrl+F1`  | Complète la ligne par une **suggestion fantôme** (ghost-text) à droite du curseur : options de `--help` d'abord, puis historique. |
| `PagePréc` / `PageSuiv` | Défilement (respectivement vers les entrées plus anciennes / le bas), avec la **molette** dispo en continu. |
| `F10`      | Aide de Bashop (touches, thèmes, options). Sert aussi de **menu** (dont bascule du mode capture). |
| `F2`       | Bascule le **mode capture** on/off. |

- **Historique défilant** : les lignes évincées de la zone centrale sont
  conservées (10 000 lignes max) et redeviennent consultables par le défilement.
- Au retour au bas de l'affichage (PageSuiv / molette bas), le curseur réapparaît
  sur le prompt.

Les **pop-ups** (F1, F10) recouvrent la **zone centrale** uniquement : les 3
lignes du cadre (stat, prompt, context) restent visibles.

## 5. Thèmes et prompt

Un thème décrit le **suffixe** affiché et les couleurs. Trois thèmes fournis :

| Thème      | Suffixe | Couleur / style |
|------------|---------|-----------------|
| `classique`| `>>`    | vert foncé (base) |
| `wheel`    | `>>>>`  | violet |
| `root`     | `###>`  | rouge, **reverse** |

- Largeur de suffixe **normalisée** : champ de **6 colonnes** (les suffixes sont
  complétés/paddés pour ne pas décaler le curseur).
- **Bascule automatique** vers `root` si un token `sudo`, `doas`, `su` ou
  `sudoedit` apparaît **n'importe où** dans la ligne — y compris dans une
  **commande composée** (`sudo apt && reboot`, `echo $(sudo x)`…). La détection
  passe par `lexer.rs` (découpage en mots + test de déclencheur), et non par un
  simple `contains` (pour éviter `pseudo`, `ls /sudo`…).
- **Thème initial `auto`** : suit le privilège détecté — `root` si uid 0,
  `wheel` si l'utilisateur appartient au groupe d'administration **de l'OS
  hôte** (`wheel`/`sudo`/`admin`/`adm` sous Linux, `admin`/`wheel` sous macOS,
  `wheel`/`operator` sous FreeBSD, `Administrateurs` sous Windows), sinon
  `classique`. Un nom explicite (`classique`, `wheel`, `root`) prime.
- **Badge root** : clignotement via **SGR** (pas de clignotement de l'invite
  entière).

Couleurs par défaut (surchargeables via `~/.opshellrc`) :

| Élément | Couleur | Fond |
|---------|---------|------|
| `term_stat` | F1 `#E6EDF3` gras | B1 `#2E3440` |
| `term_context` | F1 `#E6EDF3` gras | B2 `#2E3440` |
| utilisateur | F2 `#A6E3A1` | B1 |
| chemin | F3 `#89B4FA` | transparent |
| historique | F5 `#6C7086` italique | transparent |
| heure | F6 `#8BA1B9` italique | transparent |
| invite | vert foncé `#4E9A06` | transparent |
| charset | F8 `#FAB387` | transparent |

## 6. Options et aide contextuelle

Dès le **premier espace** saisi, `term_context` propose des options pour la
commande, dans cet ordre de résolution :

1. `--help` (exécution rapide, sortie capturée) ;
2. sinon page `man` ;
3. sinon suggestions issues de l'**historique**.

Contraintes : **cache borné** indexé `(commande, mtime)`, **thread non
bloquant** (le prompt ne gèle jamais), suggestions disponibles hors-ligne.

## 7. Sécurité

Bashop est un **garde-fou** au-dessus du shell, jamais un empêchement : il
alerte et propose, l'utilisateur décide.

- **Gating de l'Entrée** au prompt : analyse de la ligne avant exécution, suivie
  d'un **overlay de confirmation** si un risque est détecté.
- Détection de **commandes destructrices** (`rm -rf /`, `dd`, `mkfs`, …).
- **Paste safety** : avertir avant exécution d'un collage multi-lignes suspect.
- Détection de **secrets** dans la ligne.
- Correction « **vouliez-vous dire ?** » pour les commandes proches d'une
  commande connue.
- Proposition systématique de **`--dry-run` / `-n`** quand c'est applicable.
- Signalement de l'usage de l'**historique dangereux** (commande risquée
  rappelée).

## 8. Mode capture

- **Implémenté** et **actif par défaut**.
- Masque à l'affichage : **IP, MAC, hostname, secrets**.
- Bascule par `F2`, par le **menu `F10`**, et par la sous-commande CLI
  `bashop capture`.

## 9. Productivité et robustesse

- **Notification de fin de commande longue** (seuil configurable).
- **Horodatage optionnel** des commandes.
- Mode **dégradé** si le terminal ne supporte pas les capacités requises.
- **Rappel de touches** toujours présent dans `term_context`.

## 10. Configuration — `~/.opshellrc`

Format **neutre `KEY=VALUE`**, lu et interprété par le binaire Bashop, **jamais
sourcé** par un shell (pas d'exécution de code arbitraire). Les clés inconnues
sont ignorées ; les valeurs par défaut s'appliquent en l'absence de fichier.

Clés reconnues (extrait) :

| Clé | Effet |
|-----|-------|
| `THEME` | Thème de prompt (`classique`, `wheel`, `root`). |
| `TRIGGERS` | Tokens déclenchant la bascule root (défaut `sudo doas su sudoedit`). |
| `SUFFIX_WIDTH` | Largeur normalisée du champ suffixe (défaut `6`). |
| `CAPTURE` | Mode capture `on`/`off` (défaut `on`). |

`bashop config` ouvre l'interface de configuration (via `op-tui`). La règle
« ne jamais éditer un fichier déployé à la main » s'applique aussi aux rc.

## 11. Architecture du code

```
src/
  lib.rs         module racine
  main.rs        point d'entrée, dispatch clap
  cli.rs         sous-commandes run|install|uninstall|config|detect|template|capture
  config.rs      lecture ~/.opshellrc (KEY=VALUE) + tests
  theme.rs       themes, suffixe, couleurs + tests
  lexer.rs       decoupage et detection de declencheur + tests
  detect.rs      OS, arch, shell, privilege
  observe.rs     observation des frappes
  safety.rs      garde-fous (commandes destructrices, secrets)
  providers.rs   fournisseurs de suggestions (~ help, man, historique)
  shells.rs      noms et resolution des shells + tests
  keymap.rs      touches du cadre
  host/
    mod.rs       hote PTY
    pty.rs       allocation (forkpty Unix, ConPTY Windows)
    pump.rs      boucle entree/sortie  (ex-« loop », renomme : mot reserve)
    resize.rs    SIGWINCH / resize ConPTY
  term/
    mod.rs       emulation zone centrale
    parser.rs    parseur VT
    grid.rs      grille N-2 lignes
    overlay.rs   ghost-text, pop-up
  frame/
    mod.rs       cadre
    stat.rs      term_stat
    context.rs   term_context
```

Dépendances : `op-tui` (**path**, dépôt de primitives TUI dans
`~/projets/Libs/op-tui`, symlink `op-tui -> ../Libs/op-tui`), `clap 4.5`
(derive), `anyhow 1.0`, `serde`, `serde_json`. Profil `release` :
`lto`, `codegen-units = 1`, `strip`, `panic = "abort"`.

## 12. Distribution

- **Binaires pré-compilés** : aucune chaîne de compilation sur la cible
  (indispensable DSM 7.2 / SRM).
- **Installateurs noarch** : `build/install.sh` (Unix) et `build/install.ps1`
  (Windows) détectent OS/arch, téléchargent l'archive, vérifient le **SHA256**,
  installent puis lancent `bashop install`.
- **Index JSON** (`build/index.sh` → `dist/index.json`) : associe `os`/`arch` à
  une `url` et un `sha256`.
- **Cibles** : `linux x86_64`, `linux aarch64`/`armv7`, `freebsd x86_64`/
  `armv7`, `windows x86_64`. Binaires Linux **statiques (musl)** quand possible
  (DSM/SRM).
- Compilation locale = option de **développement** uniquement.

## 13. Jalons

| Jalon | Contenu |
|-------|---------|
| **M0** | Squelette du crate, `SPEC.md`, licence, docs, scripts de build. |
| **M1** | Hôte PTY Unix + émulateur VT + intégration `bash`. |
| **M2** | Redimensionnement, plein écran, ghost-text. |
| **M3** | `zsh`, `sh`, PowerShell, `cmd`. |
| **M4** | Sécurité (§7) + configuration (§10). |
| **M5** | Mode capture, session, charge, notifications. |
| **M6** | Distribution (releases, index, installateurs). |

## 14. Conventions de code

- Langue : identifiants cohérents par module (français ou anglais, jamais
  mélangés) ; messages utilisateur et documentation en **français**.
- **Norme d'accolade** : `{` ouvrante sur ligne séparée indentée de 2, contenu
  +4, `}` fermante alignée ; +2 par niveau d'imbrication (voir `AGENTS.md`).
- Tests unitaires inline `#[cfg(test)] mod tests`, noms explicites en français.
- `cargo test` doit passer et `cargo build` rester sans avertissement avant
  tout commit.
