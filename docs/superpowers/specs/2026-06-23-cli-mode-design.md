# Mode CLI pour deez-notes

**Date:** 2026-06-23
**Statut:** Validé, prêt pour planification

## Objectif

Permettre d'utiliser les notes sans passer par le TUI : découvrir, lire, rechercher
et modifier les notes de manière programmatique (scriptable, pipeable) en ligne de
commande. `deez-notes` invoqué seul continue de lancer le TUI exactement comme avant.

## Principe directeur

Le `NoteManager` (`src/core/note_manager.rs`) est déjà un module profond qui gère
tout le métier : scan disque, front-matter, CRUD, dossiers, recherche. Le mode CLI
est un **second front-end** par-dessus ce même core, à côté du TUI. Aucune logique
métier nouvelle, aucune duplication de connaissance.

## Architecture

- Nouveau module unique : `src/cli.rs` (exposé via `lib.rs` : `pub mod cli;`).
- `main.rs` devient un aiguillage :
  - Parse les args, charge la config et applique les overrides (`--config`, `--editor`,
    `directory`) — **avant** toute initialisation de terminal.
  - Si une sous-commande est présente → `cli::run(command, config)` puis `return`.
    Le chemin CLI n'entre **jamais** en raw mode ni en alternate screen.
  - Sinon → le flux TUI actuel, strictement inchangé.
- `cli::run` instancie le core comme le fait `App::new` :
  `NoteManager::new(config.resolve_notes_dir())?` puis `.scan()?`.

## Surface CLI (clap derive)

`Cli` reçoit un champ `command: Option<Command>`. `None` ⇒ TUI. Les options globales
existantes (`directory`, `config`, `editor`) restent au niveau racine et s'appliquent
aussi au mode CLI.

| Commande | Effet | Entrée / Sortie |
|---|---|---|
| `list [--json] [--folder F] [--tag T]` | Découvrir les notes | Défaut : lignes `chemin_relatif\ttitre`. `--json` : tableau de `NoteInfo`. Filtres `--folder`/`--tag` optionnels. |
| `get <note>` | Lire le corps markdown | Corps brut (sans front-matter) sur stdout, pipeable. |
| `search <query> [--json]` | Fuzzy search (core existant) | Même format que `list`. |
| `new <title> [--folder F]` | Créer une note | Corps depuis stdin si piped, sinon ouvre `$EDITOR`. Imprime le chemin relatif créé. |
| `set <note>` | Écraser le corps d'une note | Corps depuis stdin si piped, sinon ouvre `$EDITOR` sur le fichier. |
| `rm <note>` | Supprimer une note | Aucune sortie en cas de succès. |

`--folder` défaut = racine (`""`). Pas de `mv`/`rename`/`tags` dans cette itération
(YAGNI ; ajoutables plus tard sans toucher au reste).

## Désignation d'une note (`<note>`)

Helper `resolve_note(manager, &str) -> anyhow::Result<usize>` :

1. **Match exact** : si l'argument égale le chemin relatif (à `notes_dir`) d'une note,
   retourne cet index.
2. **Fallback fuzzy** : sinon, fuzzy search sur les titres via `core::search`. Prend le
   meilleur match.
3. Erreur explicite (message sur stderr, code de sortie non-zéro) si aucun match.

Les index sont ceux internes au `NoteManager` (résolus juste avant l'opération, donc
jamais stockés ni exposés à l'utilisateur).

## Entrée stdin vs éditeur (`new`, `set`)

Détection via `std::io::IsTerminal` :

- `std::io::stdin().is_terminal() == false` (stdin est un pipe) ⇒ lire tout stdin comme
  corps. `set` : `Note::save_content(body)`. `new` : créer puis écrire le corps.
- `true` (interactif, pas de pipe) ⇒ ouvrir le fichier dans l'éditeur via
  `editor::external::open_in_editor(&path, editor_override)` (réutilisé tel quel, comme
  le TUI). `new` crée d'abord le fichier vide via `NoteManager::create_note`, puis l'ouvre.

L'override d'éditeur vient de `config.general.editor` (déjà appliqué par les args racine),
même résolution que le TUI.

## Format de sortie

- **Humain (défaut)** : lignes `TAB`-séparées, directement exploitables (`cut`, `awk`).
- **`--json`** : petit DTO `NoteInfo` sérialisable (serde_json déjà en dépendance),
  champs `{ path, title, tags, created, modified }`. Construit par mapping depuis `Note`
  — `Note` reste inchangé (donnée plate qui traverse la frontière CLI, conforme aux
  règles de modularité). Le contenu n'est pas inclus dans `list`/`search` (lazy).

## Gestion d'erreurs

- Tout remonte en `anyhow::Result` jusqu'à `main`, qui retourne déjà `Result<()>`.
- Code de sortie non-zéro + message sur stderr en cas d'échec.
- Aucun terminal à restaurer : le chemin CLI n'initialise jamais le terminal.

## Tests

- Tests d'intégration sous `tests/` (le crate en a déjà), utilisant `tempfile` pour un
  répertoire de notes jetable :
  - `list` / `search` retournent les notes attendues (humain + `--json`).
  - `resolve_note` : match exact de chemin et fallback fuzzy.
  - `new` (stdin) crée le fichier avec le bon corps ; `set` (stdin) l'écrase ; `rm` supprime.
  - Le mode éditeur n'est pas testé automatiquement (interactif) — vérification manuelle.

## Hors périmètre (YAGNI)

- `mv` / `rename` / `tags` en CLI.
- Sortie JSON pour `get` (le corps brut est déjà pipeable).
- Option `--raw` pour inclure le front-matter dans `get`.
