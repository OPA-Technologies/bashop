# Règles de travail — Bashop

Ce fichier reprend les règles globales d'assistance et les conventions du
projet. Toute modification de fond doit être portée ici, puis dans
`~/.config/opencode/instructions.md` via le projet `opencode-start-menu`.

## Langue

Toujours répondre en français. Toutes les conversations et interactions
doivent être en français.

## Norme d'accolade (appliquée dans tout le code du projet)

Pour toute fonction et tout groupe d'instructions, l'accolade ouvrante `{` est
placée sur une ligne séparée, indentée de 2 caractères par rapport au groupe.
Le contenu du bloc est indenté de 2 caractères supplémentaires par rapport au
`{` (soit +4 par rapport à la ligne du groupe). L'accolade fermante `}` est
alignée sur le `{`. Chaque niveau d'imbrication ajoute 2 caractères.

Exemple de référence :

```rust
fn activer(nom: &str) -> bool
  {
    if nom.is_empty()
      {
        false
      }
    else
      {
        true
      }
  }
```

## Convention de nommage

- Code (identifiants, modules, types) : anglais ou français cohérent, jamais de
  mélange au sein d'un même module.
- Messages utilisateur et documentation : français.

## Tests

- Tests unitaires inline `#[cfg(test)] mod tests`, style de test explicite
  (`fn les_defauts_sont_charges_quand_le_fichier_est_vide()`).
- `cargo test` doit passer avant tout commit.
- `cargo build` doit rester sans avertissement.

## Référence de conception

`SPEC.md` est la source unique de la conception (cadre 3 lignes, keymap,
thèmes, `~/.opshellrc`, index de distribution). Toute décision structurante y
est consignée.
