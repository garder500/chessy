# La forge : des compétences inventées

Complète `docs/skills.md` et `docs/spec-v4.md`. Les 27 compétences écrites à la main restent telles quelles ;
la forge en **invente de nouvelles**, une par choix « compétence aléatoire » d'une récompense de victoire classée.

## Principe

Une compétence forgée est une **donnée** (`SkillDef`, un petit arbre en JSON), pas du code. Un interpréteur unique
(`Composite`) la joue. Tout le reste en est **déduit** : le nom, la description, l'icône, le son et la rareté.

- Identifiant : `forged_<n>`. `n` est la moitié basse de l'empreinte de la définition : une même définition a le même
  identifiant partout, et deux bases de données dans un même processus ne se contredisent jamais.
- Les 27 compétences gardent leurs noms (`"freeze"`, `"remover"`…). `SkillId::ALL` n'énumère que ces 27.
- Une définition est **immuable** une fois stockée. Un défaut est traité en la rendant injouable (voir « Garde-fous »).

## La définition

```json
{ "version": 1,
  "effect": { "op": "freeze", "plies": 4 },
  "constraints": ["forbid_mate"],
  "max_uses": 1, "free_action": false, "unique": false }
```

| Effet (`op`) | Ce qu'il fait |
|---|---|
| `freeze`, `shield`, `cloak` | geler une pièce ennemie / protéger / cacher une des vôtres, pendant `plies` |
| `morph` | changer une pièce en `into` pendant `plies` (`side` : `own` ou `enemy`) |
| `promote` | une de vos pièces (ni roi ni dame) devient une dame, pour de bon |
| `remove` | retirer une pièce ennemie d'un des `kinds` |
| `convert` | une pièce ennemie qui n'attaque aucune des vôtres passe de votre côté |
| `teleport`, `duplicate`, `swap` | déplacer, copier, échanger (`scope` : `own` ou `any`) |
| `spawn` | une pièce temporaire d'un des `kinds` apparaît sur les rangées 3 à 6 |
| **`revive`** | **ramener une de vos pièces capturées** (cimetière de la `Position`) |
| **`truce`** | **Armistice** : plus rien n'attaque rien pendant `plies` (ni capture ni échec) |
| **`mirror`** | **Miroir** : les deux camps échangent leurs armées, à la place symétrique |
| **`fog`** | **Brouillard** : chaque joueur ne voit que les pièces ennemies à deux cases ou moins des siennes |
| **`silence`** | **Silence** : l'adversaire ne peut utiliser aucune compétence pendant `plies` |

Les cinq effets en gras sont les **atomes de bascule** (`Effect::is_tone`) : ils changent la nature d'une partie.
Un effet fixe la forme de sa cible, donc une définition ne peut pas demander une combinaison impossible.

Contraintes : `only_in_check`, `forbid_mate`, `forbid_check`. `plies` va de 2 à 8 (un coup = un demi-coup),
`max_uses` de 1 à 3. `free_action` : l'utiliser ne passe pas la main. `unique` : Légendaire (voir ci-dessous).

`SkillDef::validate()` refuse tout ce que l'interpréteur ne promet pas de gérer ; `canonical()` trie et dédoublonne les
listes ; `fingerprint()` (FNV-1a du JSON canonique, sans `unique`) sert à ne pas stocker deux fois la même compétence.

## Briques : sélecteur et condition

Au-delà de l'effet, une définition peut porter deux briques facultatives (absentes du JSON quand elles sont neutres, donc
les définitions déjà stockées gardent la même empreinte et la même signature) :

```json
{ "version": 1, "effect": { "op": "freeze", "plies": 4 },
  "selector": { "kinds": ["rook", "queen"], "zone": "wings" },
  "condition": "behind", "max_uses": 1, "free_action": false, "unique": false }
```

- `selector.kinds` : seules ces pièces peuvent être visées (refusé pour les effets qui nomment déjà leurs pièces :
  `remove`, `spawn`, `revive`, et pour les effets sans cible). Jamais le roi.
- `selector.zone` : `own_half`, `enemy_half`, `center`, `wings`, `rim`, `light`, `dark` ; relatif au lanceur. La zone
  s'applique à la pièce visée, à sa case d'arrivée pour un déplacement, aux deux cases pour un échange.
- `condition` : `behind`, `ahead` (matériel), `early` (avant le 10e coup de chaque joueur), `late` (à partir du 20e),
  `no_queen`, `wounded` (trois pièces perdues).

La description, le coût (une restriction rembourse), la signature (suffixes `|of:`, `|in:`, `|when:`) et le générateur les lisent.
Un pouvoir muni d'une de ces briques et utilisable sur moins de 15 % des positions mesurées n'est forgé qu'en dernier recours.
`SkillDef::bricks()` donne la définition à plat, pour les icônes et les outils. Feuille de route : `docs/plan-forge-generative.md`.

## La rareté : mesurée, et relative

Toute compétence forgée est techniquement unique (elle a son identifiant), mais beaucoup de combinaisons reviennent au
même. La rareté a donc deux parties.

1. **Un score** de 0 à 100 : `0,6 × tone + 0,4 × cost`.
   - `cost` : ce que la compétence coûte sur le papier (portée, durée, usages, action gratuite ; les restrictions remboursent).
   - `tone` (l'« indice de bascule ») : mesuré par simulation sur 12 positions fixes (parties aléatoires, toujours les
     mêmes). Pour chaque position, on joue la meilleure cible légale et on cherche la position obtenue à 2 demi-coups.
     On retient le gain moyen, la part de positions où il change une partie (≥ 250 cp), la variation du nombre de coups de
     l'adversaire, et l'irréversibilité.
2. **La redondance.** `SkillDef::signature()` résume ce que fait la compétence en gros (effet, durée et pièces par
   catégories, règles autour). **Si une autre compétence du monde a déjà cette signature, la nouvelle est Commune,
   quel que soit son score.** Une compétence qui était unique devient donc banale quand sa combinaison se répète.

| Rareté | Condition |
|---|---|
| Commune | score sous le premier seuil, **ou signature déjà prise** |
| Peu commune, Rare, Épique | tranches du score |
| **Légendaire** | score au-dessus du dernier seuil **et** un atome de bascule |

Un Légendaire est une compétence **unique** (`SkillKind::Unique`) : un seul propriétaire dans le monde
(`unique_skill_owner`), elle ne compte pas parmi les trois choisies. Sans atome de bascule, le plafond est Épique.

Les seuils sont dans `crates/chessy-engine/src/forge/calibration.json`, calculés par :

```bash
cargo run --release -p chessy-forge -- calibrate --samples 600 --positions 12 --write
```

Ils sont des quantiles du score de 600 compétences tirées par le générateur (Peu commune = moitié haute, Rare = quart haut,
Épique = dixième haut, Légendaire = 3 % hauts). Ils sont **fixes à l'exécution**. Toute modification des effets, du
coût ou de la mesure impose de relancer le calibrage. `chessy-forge report` note les 27 compétences écrites à la main avec
la même mesure (`switch` et `evolve` sortent en tête, `mind` à zéro) ; `chessy-forge sample` forge quelques compétences.

## Identité, déduite de la définition

`forge::identity::identity(def)` donne le nom, la description, la famille, la spécification d'icône et celle du son.

- **Nom** : un nom commun lié à l'effet et un nom propre inventé de 2 ou 3 syllabes, tirés de l'empreinte (« Givre d'Alfen »).
- **Description** : assemblée par gabarits depuis l'arbre, elle ne peut donc pas dire autre chose que ce que fait la compétence.
- **Icône** (`IconSpec`) : glyphe central (un des 16 de `GLYPHS`), silhouette de la pièce concernée, badge de durée
  (`short`, `long`, `forever`). Le client dessine le tout (`web/src/ui/forgedGlyphs.tsx`) avec un cadre à la couleur de la rareté.
- **Son** (`SoundSpec`) : effet, note de la gamme, clarté, durée. Le client l'assemble avec les briques de
  `web/src/sound/bricks.ts` (`forgedRecipe.ts`), un geste sonore par effet, au niveau réglé comme les 27.

## Serveur

- Table `forged_skill` (migration 4) : `id`, `fingerprint` (unique), `signature`, `def_json`, `rarity`, `score`, `cost`,
  `tone`, `redundant`, `gen_version`, `retired`. Au démarrage, `Store::open` enregistre toutes les définitions dans le
  registre du moteur (`forge::registry`), avant que la moindre main soit lue.
- **Récompense** : seul `RewardChoice::Random` forge. `Steal`, `Skip`, le deck de départ et le remplissage d'un deck vide
  ne changent pas. La forge est lente (elle mesure les candidats) : `Hub::begin_forge` valide la récompense et la marque
  « en cours », puis `App` forge **hors du verrou du hub** (`spawn_blocking`) et appelle `Hub::finish_forge`.
  - Une rareté cible est tirée (55 / 25 / 13 / 6 / 1 % de Commune à Légendaire), puis des définitions sont tirées
    jusqu'à en trouver une de cette rareté (24 essais au plus ; sinon la plus proche, une fraîche plutôt qu'une redondante).
  - Le perdant perd toujours une compétence au hasard, comme avant.
- **Brouillard** : `hub/view.rs` masque les pièces ennemies hors de portée (deux cases) comme l'invisibilité, et un échec
  démasque toujours la pièce qui le donne. Les spectateurs ne sont pas dans le brouillard (ils ont déjà 30 s de retard).
  En Solo, le bot cherche sur **la vue de son camp** (`Hub::bot_job`) : il ne voit pas non plus ce que le brouillard ou
  l'invisibilité lui cachent, et un coup qui s'avère faux sur le vrai plateau est refusé (il joue alors un autre coup légal).
- `GET /api/skills/forged?ids=1,2,3` (64 au plus) : `{ skills: [SkillDefView] }` avec `id`, `name`, `description`, `family`,
  `rarity`, `unique`, `redundant`, `max_uses`, `icon`, `sound`. Les identifiants inconnus sont omis.

## Client

- `SkillId` = les 27 noms ou `forged_<n>`. Le client ne connaît pas les définitions à l'avance : `forged.ts` repère
  `forged_<n>` dans chaque message WebSocket et chaque réponse REST, charge ce qui manque (une requête par lot, sans doublon),
  et relance le rendu (`AppState.forged`). En attendant, une fiche d'attente s'affiche.
- La récompense « Forger une compétence » attend le serveur (« Le forgeron travaille… »), puis une notification donne le nom
  de la compétence gagnée. La page **Collection** est l'**historique** des compétences du joueur, pas un annuaire : tout ce qu'il a obtenu, forgé ou perdu,
du plus récent au plus ancien, groupé par jour, avec le résumé (forgées, obtenues, perdues, dans le deck), un filtre
(Tout / Forgées / Obtenues / Perdues) et, en dépliant une ligne, les règles et l'aperçu de la compétence.
- Nouveaux effets de partie entière : `truce`, `fog`, `silenced` (`ActiveEffect.piece = 65535`) et l'événement
  `global_effect`. Le bandeau de la partie indique ce qui est actif et pour combien de tours.

## Historique des compétences

- Table `skill_history` (migration 5) : `player_id`, `skill`, `change` (`gained` ou `lost`), `source`, `other` (l'autre joueur, s'il a
  un compte), `at`. Le journal est écrit là où un deck change : deck de départ (`starter`), deck vide rempli (`refill`),
  récompense (`apply_reward` : `forged`, `stolen`, `won` pour ce qui est gagné ; `taken` pour ce que le perdant se fait prendre ;
  `replaced` pour ce qu'on abandonne faute de place). Les compétences déjà possédées à la création du journal y sont
  entrées une fois (`earlier`), datées de la création du compte.
- `GET /api/me/skills` (Bearer, invités compris) : `{ entries: [{id, skill, change, source, other?, at}], deck: [skill_id] }`,
  les 500 plus récentes d'abord. `player_skills` reste ce que le joueur possède maintenant ; le journal n'en est pas la source.

## Garde-fous

- `simulate_skill` vérifie après chaque compétence forgée (`Position::board_is_sane`) : un roi par camp comme avant, aucune
  pièce en double, aucun pion en rangée de fond. Sinon l'action est refusée, la partie n'est pas touchée.
- Une définition qui ne passe plus `validate()` au chargement reste en base (des decks peuvent la contenir) mais se résout
  en une compétence sans cible : injouable.
- Tests : un module par famille d'effets (`tests/skills/forged.rs`), des parties aléatoires avec des compétences forgées
  aléatoires dans les decks (`CHESSY_RANDOM_GAMES=3000` pour pousser), la mesure et la forge sont déterministes.
