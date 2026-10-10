# Plan : une forge générative, pour des pouvoirs le plus « uniques » possible

## Constat

La forge assemble une **action** (17 effets) avec une durée, des contraintes, un nombre d'usages et l'action
gratuite : environ 2 800 signatures distinctes. Au-delà, deux pouvoirs « se ressemblent » et le second tombe en
Commune. Pour avoir beaucoup plus de pouvoirs réellement différents, il faut de **nouvelles dimensions
orthogonales** à l'effet : chacune multiplie l'espace au lieu de l'additionner.

## Principes (valables pour toutes les phases)

1. **Briques, pas effets sur mesure.** Un pouvoir = une action + des briques qui la précisent. Chaque brique a une
   valeur neutre qui ne change rien : les définitions déjà stockées gardent leur sens, leur empreinte et leur signature
   (les champs neutres ne sont pas sérialisés ; la signature n'ajoute un suffixe que si la brique est utilisée).
2. **Tout se déduit de la définition** : description, coût, signature, structure lisible par les icônes
   (`SkillDef::bricks()`). Aucune brique n'est décrite ailleurs.
3. **Une combinaison injouable est rejetée par la mesure** : un pouvoir muni d'une brique restrictive qui n'est
   utilisable que sur moins de 15 % des positions mesurées n'est gardé qu'en dernier recours.
4. **La signature lit chaque nouvelle dimension**, sinon des pouvoirs différents seraient « redondants » et tomberaient en
   Commune à tort. Le coût aussi (une restriction rembourse, une puissance coûte).
5. **Recalibrer après chaque phase** (`chessy-forge calibrate --samples 600 --positions 12 --write`) : le générateur
   change de distribution.
6. Chaque phase est livrable seule, avec ses tests, sans toucher au rendu des icônes (autre thread).

## Phases

### Phase 1 — Sélecteur et condition (livrée dans cette PR)

- **Sélecteur** : quelles pièces (`kinds`) et quelles cases (`zone` : moitié à soi, moitié adverse, centre, ailes, bord,
  cases claires, cases sombres) l'action peut viser. Relatif au camp du lanceur.
- **Condition d'usage** : derrière / devant au matériel, début de partie, fin de partie, plus de dame, trois pièces perdues.
- `SkillDef::bricks()` : vue plate et sérialisable (`action`, `side`, `kinds`, `zone`, `plies`, `permanent`, `condition`,
  contraintes, usages, action gratuite) pour les icônes et les outils.
- Description, coût, signature, générateur et rejet des injouables à jour ; recalibrage.
- Effet : environ 55 000 signatures distinctes atteintes en 2 millions de tirages (contre ~2 650 sans les nouvelles briques),
  et la courbe monte encore.

### Phase 2 — Durée par événement et portée

- Remplacer « pendant N demi-coups » par `Duration` : `Plies(n)` (inchangé) ou jusqu'à un événement (« jusqu'à la prochaine
  capture », « jusqu'à ce que la pièce bouge », « jusqu'au prochain échec »). Demande des durées d'effet pilotées par
  événement dans `Position::end_turn_events`.
- **Portée** : une brique `Range` (une case, voisines, ligne, colonne, diagonale) qui étend l'effet autour de la cible
  (geler une pièce et ses voisines). Le sélecteur de cible reste celui de la phase 1.

### Phase 3 — Modificateurs

- `Double` (l'effet touche deux cibles), `Fantôme` (la pièce visée devient invisible pour l'adversaire en plus),
  `Contagieux` (l'effet saute à une pièce voisine à la fin de sa durée), `Retardé` (l'effet démarre N demi-coups plus tard).
- Interactions à verrouiller par des tests de non-régression (cumul avec Bouclier, Brouillard, Trêve…).
- Plafond de modificateurs par pouvoir, comptés dans le coût.

### Phase 4 — Coût et camp

- **Coût** : sacrifier un pion, perdre un tempo, passer un tour de plus que prévu, ou laisser l'adversaire jouer deux fois ;
  un coût rembourse la puissance.
- **Camp / retournement** : un pouvoir qui s'applique à l'adversaire (ou aux deux) au lieu de soi, avec un coût inversé.

### Phase 5 — Duo

- Deux actions enchaînées (« gèle une pièce **puis** protège la tienne »), avec une règle de compatibilité des cibles
  et un coût combiné. C'est ici que l'espace explose : chaque action de la phase 1 à 4 se combine avec chaque autre.
- La mesure et le rejet des injouables passent au premier plan (le Duo est la brique la plus facile à rendre
  dégénérée).

### Phase 6 — Qualité du catalogue

- Mesure de **distinctivité** réelle : au lieu d'une signature en seaux grossiers, comparer le comportement mesuré
  (vecteur de gains par position) et rejeter les quasi-doublons mêmes de signatures différentes.
- Noms et descriptions plus variés quand les briques s'accumulent ; son et icône lisent `bricks()`.
- Outil `chessy-forge space` pour estimer la taille de l'espace et la répartition des raretés.

## Ce que lit le thread des icônes

`SkillDef::bricks()` (`crates/chessy-engine/src/forge/bricks.rs`) est la structure à lire : `action` est le nom de
l'effet, `side` (`own`, `enemy`, `any`, `none`), `kinds` (pièces concernées), `zone`, `plies` (durée), `permanent`,
`condition`, `constraints`, `max_uses`, `free_action`. Les phases suivantes ajoutent des champs sans retirer les
précédents.
