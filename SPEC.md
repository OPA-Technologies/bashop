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
- **Environnement du shell enfant** : le shell lancé par le cadre émule un
  démarrage **login + interactif** — il source `/etc/profile`, le premier de
  `~/.bash_profile` / `~/.bash_login` / `~/.profile`, puis `/etc/bash.bashrc`
  (`/etc/bashrc`) et `~/.bashrc` — afin que variables d'environnement, fonctions
  **et alias** de l'utilisateur soient disponibles. `PS1`/`PROMPT_COMMAND` sont
  ensuite reposés par Bashop (les réglages de prompt des rc sont neutralisés).
  Réserve connue : si un profil de connexion source déjà `~/.bashrc` (défaut
  Ubuntu), celui-ci peut être lu deux fois (alias idempotents ; un `PATH` mal
  gardé peut être dupliqué).

### 2.1 Anti-nidification et reprise de session

Bashop est branché par **injection dans les rc** des shells (`bashop install`
pose un bloc **idempotent** délimité `# >>> bashop >>>` … `# <<< bashop <<<`).
Avant de lancer `bashop run`, la bootstrap vérifie trois choses :

1. **Voie rapide (env)** : `BASHOP_ACTIVE=1` est transmis au shell enfant ; sa
   simple présence suffit à ne pas relancer le cadre (couvre aussi Windows).
2. **Remontée du parent** : `sudo -i` / `su -` réinitialisent l'environnement
   (donc effacent `BASHOP_ACTIVE`) ; on remonte la chaîne des processus parents
   (`/proc/<pid>/status` sous Linux, `ps -o ppid=`/`ps -o comm=` ailleurs) et on
   s'arrête dès qu'un ancêtre est un binaire `bashop`. **C'est ce test qui
   empêche le double cadre sous `sudo -i`.**
3. **Multiplexeur** : sous `tmux`/`screen` (variables `TMUX`/`STY`), le cadre est
   **désactivé** (mode dégradé) : le shell garde son prompt natif.

`bashop run` **refuse** également de démarrer s'il détecte une nidification
(cas d'un lancement manuel depuis un shell déjà encadré).

Sous `sudo -i`/`su -`, le nouveau shell **reprend la session** (il n'ouvre pas
un cadre imbriqué). L'environnement est réinitialisé (donc `BASHOP_ACTIVE`
disparaît), mais un ancêtre `bashop` subsiste : la commande interne
`bashop _nested` le détecte. Le shell de connexion reçoit alors le prompt
épinglé et les marqueurs `OSC 133` par `eval "$(bashop prompt --shell <s>)"`,
ce qui évite d'ouvrir un second cadre tout en gardant le cadre parent
réactualisé. Son `precmd` publie un OSC privé
`ESC ] 1337 ; Bashop ; user=<u> ; priv=<root|wheel|classique> BEL` : le cadre
adopte aussitôt l'identité et le thème correspondants (`term_stat` affiche le
`user@host` réel, `term_context` le badge `ROOT!`, et les **deux lignes
`term_*`** passent au fond rouge du thème `root` ; la zone centrale, dont le
fond est fixé par l'`OSC 11` initial, reste inchangée). Le privilège est
recalculé à chaque démarrage de shell, jamais par fork à chaque prompt.

Deux points d'injection coopèrent : le bloc utilisateur (`~/.bashrc`,
`~/.zshrc`) pour l'utilisateur lui-même, et — sous Linux — le **bloc système**
`bashop install --system` (`/etc/profile.d/bashop.sh`) pour le shell de
connexion **root** de `sudo -i` (qui ne lit pas le rc de l'utilisateur). Le
bloc système est **inerte** hors `sudo -i` : il exige `bashop _nested` et un
`bashop` présent dans le `PATH` de root (installation système recommandée).

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
- Extra compact : heure UTC au format **jour/mois heure:minute**.
- Le FQDN privilégie `hostname -f`, puis `hostname -d`, `dnsdomainname` et
  `/etc/resolv.conf` (domaine/localisé), sans jamais revenir à un hôte nu si un
  domaine est résoluble.
- Style par défaut : fond B1 `#2E3440`, texte F1 `#E6EDF3`, gras.

### 3.2 `term_context` (dernière ligne)

Barre pleine largeur à trois zones :

1. **Gauche — commandes/raccourcis** : `F1 - Aide rapide`, `F2 - Secret`,
   `F10 - Aide`, puis les options découvertes. La suggestion fantôme n'y figure
   pas : elle est déjà dessinée en gris après le curseur, sur la ligne de saisie.
2. **Centre — code retour** : `✓ ok` en **vert** si `0`, `✗ N` en **rouge**
   sinon ; le préfixe `ROOT!` est ajouté sous bascule root. En cas d'échec
   (code non nul), la **première ligne** du message d'erreur est recopiée après
   le code (`✗ N — message`). Copie **purement observationnelle** : la sortie
   réelle du shell, son code retour et ses redirections ne sont jamais altérés.
3. **Droite — compteurs système** compacts (io / réseau / mémoire / cpu),
   rafraîchis toutes les **0,5 s**.

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
sauf **PagePréc/PageSuiv** qui pilotent le défilement de la zone centrale.

| Touche     | Action |
|------------|--------|
| `F1`       | Aide rapide de la commande en cours de saisie : `--help` d'abord (fork borné), sinon page `man` (bornée), sinon historique. |
| `Ctrl+F1`  | Ouvre la page `man` de la commande en cours dans un pop-up (bornée). |
| `PagePréc` / `PageSuiv` | Défilement de l'historique (respectivement vers les entrées plus anciennes / le bas). |
| `F10`      | Aide de Bashop (touches, thèmes, options). Sert aussi de **menu** : `F2` y bascule le mode secret, `F10` rafraîchit. |
| `F2`       | Bascule le **mode secret / capture** on/off (session courante, persistable via `bashop capture`). |
| `Alt+Entrée` | Récupère **toute** la suggestion fantôme (le texte gris après le curseur) et laisse le curseur en fin de ligne, **sans valider**. |
| `Ctrl+Entrée` | Récupère le **mot suivant** de la suggestion fantôme, curseur après ce mot, **sans valider** et sans effacer la fin du ghost. |

- **Ghost accepté au clavier** : la suggestion affichée après le curseur peut
  être reprise progressivement, mot à mot (`Ctrl+Entrée`) ou d'un coup
  (`Alt+Entrée`). Ces touches étant ambiguës sur les terminaux classiques,
  le **protocole clavier Kitty** (`DISAMBIGUATE_ESCAPE_CODES`) est activé au
  démarrage en *best-effort* (ignoré par les terminaux qui ne le gèrent pas) ;
  sans ce protocole, `Ctrl+Entrée` retombe sur `Entrée` et valide la commande.

- **Historique défilant** : les lignes évincées de la zone centrale sont
  conservées (10 000 lignes max) et redeviennent consultables par le défilement.
- Au retour au bas de l'affichage (PageSuiv), le curseur réapparaît sur le
  prompt.

Les **pop-ups** (F1, F10) recouvrent la **zone centrale** uniquement : les 3
lignes du cadre (stat, prompt, context) restent visibles.

- **Souris déléguée à la demande** : la souris n'est captée au niveau du
  terminal que lorsque le programme lancé active le suivi souris
  (DECSET `1000`/`1002`/`1003` ; encodage `1006`/`1015`), auquel cas les
  événements souris lui sont **transmis**. Sinon la capture est relâchée : le
  terminal conserve sa **sélection** et son **copier-coller natifs** (molette
  comprise), et l'historique se parcourt avec `PagePréc`/`PageSuiv`.
- **Écran alterné** : `DECSET 1049` sauvegarde grille et curseur et les
  restaure à la sortie (le scrollback survit à `vim`/`less`).

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
- **Paste safety** : un collage **multi-lignes** ouvre un **overlay de
  confirmation** (aperçu des premières lignes) avant transmission ; le
  *bracketed paste* est respecté (DECSET `2004`) si le programme enfant l'a
  activé. Avertir avant exécution d'un collage suspect.
- Détection de **secrets** dans la ligne.
- Proposition de **`--dry-run` / `-n`** quand c'est applicable : si la commande
  jugée risquée appartient à un programme dont la simulation est stable
  (`rsync`, `make`, `apt`/`apt-get`, `pip`/`pip3`, `npm`, `git clean`),
  l'overlay de confirmation affiche la variante simulée et la touche `d`
  l'exécute (`rsync --dry-run …`, `make -n …`, `apt-get -s …`, …) au lieu de la
  commande brute.
- Signalement de l'**historique dangereux** : toute ligne risquée présente au
  prompt — saisie ou **rappelée** par `↑` — est signalée en continu au centre de
  `term_context` (`⚠ motif…`), avant même de valider ; le gating de l'Entrée
  reste la barrière bloquante.
- ~~Correction « vouliez-vous dire ? » (distance d'édition)~~ : **non retenue**.
  Seule la complétion par préfixe déjà offerte par le ghost-text continu de
  saisie est proposée.

## 8. Mode capture

- **Implémenté** et **actif par défaut**.
- Masque à l'affichage de la zone centrale : **IP, MAC, hostname/FQDN, secrets**
  (remplacements à longueur constante, la mise en page n'est pas décalée).
- Bascule par `F2` (session courante), par le **menu `F10`**, et par la
  sous-commande CLI `bashop capture on|off|toggle` qui **persiste** la valeur
  dans `~/.opshellrc` (`CAPTURE=`).

## 9. Productivité et robustesse

- **Notification de fin de commande longue** : quand une commande dure plus de
  `NOTIFICATION_SEUIL` secondes (défaut 5), son retour au prompt (marqueur
  `OSC 133;A`, garanti bash/zsh) affiche pendant quelques secondes
  `⏱ terminée en N s` au centre de `term_context`. Désactivable par
  `NOTIFICATION=off`.
- **Horodatage optionnel** des commandes : avec `HORODATAGE=on`, l'heure de
  lancement (`🕐 HH:MM:SS`) s'affiche au centre de `term_context`.
- Mode **dégradé** si le terminal ne supporte pas les capacités requises.
- **Rappel de touches** toujours présent dans `term_context`.
- **Restauration terminal** : `SIGTERM`/`SIGINT`/`SIGHUP`/`SIGQUIT` (et le hook
  de panique) rétablissent termios, écran alterné, souris, fond et curseur, puis
  tuent l'enfant ; le redimensionnement (`SIGWINCH`/crossterm) répercute la
  taille sur le PTY.
- **Fournisseurs en cache** : FQDN, IP locale, identité, flavor OS, pays et
  privilège sont résolus **une fois** (`OnceLock`) ; la charge système est lue
  au tick de 0,5 s, jamais à chaque rendu.

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
| `CAPTURE` | Mode capture `on`/`off` (défaut `on`) ; réécrit par `bashop capture`. |
| `NOTIFICATION` | Notification de fin de commande longue `on`/`off` (défaut `on`). |
| `NOTIFICATION_SEUIL` | Durée minimale en secondes (défaut `5`). |
| `HORODATAGE` | Horodatage des commandes `on`/`off` (défaut `off`). |

`bashop config` ouvre l'interface de configuration (formulaire op-tui sur
ratatui : navigation `↑/↓`, édition `Entrée`, annulation `Échap` d'un champ,
`F2` ou `Échap` en liste = **enregistrer et quitter**). `--reload-rc` relit
`~/.opshellrc` et affiche le résumé de la configuration active **sans** ouvrir
l'interface (pratique après édition manuelle ou en non-tty). La règle « ne
jamais éditer un fichier déployé à la main » s'applique aussi aux rc.

## 11. Architecture du code

```
src/
  lib.rs         module racine
  main.rs        point d'entrée, dispatch clap
  cli.rs         sous-commandes run|install|uninstall|config|detect|template|prompt|capture (+ _guard/_nested)
  config.rs      lecture ~/.opshellrc (KEY=VALUE) + tests
  config_ui.rs   interface `bashop config` (formulaire op-tui + --reload-rc)
  theme.rs       themes, suffixe, couleurs + tests
  lexer.rs       decoupage et detection de declencheur + tests
  detect.rs      OS, arch, shell, privilege, anti-nidification
  install.rs     injection/retrait idempotent du bloc dans les rc
  bootstrap.rs   contenu du bloc (rc bash/zsh, OSC 1337, PROMPT_COMMAND)
  observe.rs     observation des frappes (OSC 133 + repli idle)
  restauration.rs restauration terminal (signaux + RAII + hook panique)
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
