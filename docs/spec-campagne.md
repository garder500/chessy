# Mode campagne (spec)

Contenu des 5 chapitres et contrat de protocole (titres, choix de deck, positions de départ des boss).
Le contenu vit dans `crates/chessy-server/src/campaign.rs` et `campaign/levels.rs`.

## Règles

- Une partie de campagne est une partie Solo contre Sage (`Solo.campaign`), sans nouveau `GameKind`, sans Elo ni pendule.
  Les decks sont imposés par le niveau, sauf aux niveaux à choix de deck (chapitres 3 à 5, voir plus bas).
- 5 chapitres (0..4) = familles attaque, défense, mobilité, contrôle, création. Chacun a 6 niveaux (0..5) et un boss (niveau 6 sur le fil).
- Elo de Sage : `400 + 400 × chapitre + 50 × niveau` ; le boss vaut le dernier niveau + 100 (chapitre 0 : 400..650, boss 750).
- Étoiles d'un niveau : victoire, objectif, défi. L'objectif et le défi ne comptent qu'en cas de victoire.
  Les meilleures étoiles sont cumulées (OU de bits) par `player_id`, invités compris.
- Les niveaux 0..5 sont ouverts d'emblée. Le boss n'a que l'étoile de victoire ; il s'ouvre à 12 étoiles sur les 18 des niveaux (`boss_locked` sinon).
- « Gagner en N coups » compte les tours du joueur (une compétence qui termine le tour compte pour un coup).
  Toutes les positions de départ ont les Blancs au trait et `Position::from_fen` donne `ply = 0` : le décompte est identique à la position standard.
- Les decks de Sage ne contiennent jamais Mind Reading ni Mind Control (le bot les ignore) et ont 3 compétences au plus.
  Le deck de Sage grossit de 1 à 3 compétences au fil d'un chapitre.

## Récompense du boss

Première victoire d'un compte (pas d'un invité) sur le boss : une forge garantie, proposée via le `RewardOffer` habituel (`steal_options` vide).
La rareté est tirée dans une plage par chapitre, avec les poids de drop renormalisés : chapitre 0 Peu commune..=Épique, chapitres 1-2 Rare..=Légendaire, chapitres 3-4 Épique..=Légendaire.
Un drapeau `rewarded` persisté empêche de la farmer. Le boss ne retire rien au joueur. Un invité joue et progresse mais ne reçoit rien.
Le drapeau n'est posé qu'à la résolution de la récompense (choix, forge réussie ou « passer ») : une offre perdue (déconnexion, redémarrage, expiration) est reproposée à la prochaine victoire contre le boss. Une forge hors fourchette de rareté échoue et laisse l'offre ouverte.

Point d'attention : la plage des chapitres 1-4 inclut Légendaire, qui crée une compétence unique (`unique_skill_owner`).
Les ids forgés sont neufs, donc pas de collision, mais une unique rejoint le deck et peut être volée en classé.

## Titres

Battre le boss d'un chapitre (étoile de victoire) donne un titre ; le titre affiché est celui du chapitre le plus élevé dont le boss est battu.
Il est dérivé des lignes de progression (donc stocké aussi pour les invités) et affiché sur les profils de comptes.

| Chapitre | Titre |
|---|---|
| 1 Attaque | Fer de Lance |
| 2 Défense | Briseur de Muraille |
| 3 Mobilité | Marcheur du Vide |
| 4 Contrôle | Maître du Tempo |
| 5 Création | Grand Architecte |

## Choix de deck (chapitres 3 à 5 : mobilité, contrôle, création)

Sur ces niveaux (boss compris), `deck_choice` vaut `true` et `player_deck` est vide : le joueur choisit 1 à 3 compétences distinctes dans son deck actuel.
Les objectifs y sont limités à « Gagner en N coups », « Garder une pièce », « Sans compétence » et « Utiliser une compétence » (n'importe laquelle) : jamais une compétence précise.
La famille du chapitre se reconnaît dans le deck de Sage.

## Positions de départ des boss

Les boss des chapitres 2 à 5 ont une position de départ (FEN, Blancs au trait) conçue pour un camp fixe, `human_color`. Le boss du chapitre 0 garde la position standard.

| Chapitre | Boss | Joueur | Position |
|---|---|---|---|
| 2 Défense | Le Gardien | Blancs | Sage a deux pions de plus (c6, f6), un mur de pions |
| 3 Mobilité | Le Passeur | Blancs | Sage a un fou de plus (e6) |
| 4 Contrôle | Le Métronome | Blancs | Sage a un cavalier de plus (f6) |
| 5 Création | L'Architecte | Noirs | Sage (Blancs) a deux pions de plus (c3, f3) |

## Protocole (contrat)

- Client : `{"type":"campaign_start","chapter":2,"level":0,"deck":["freeze","clone"]}`.
  `deck` est facultatif, et obligatoire si le niveau a `deck_choice` (erreur `bad_deck` s'il manque ou s'il est invalide) :
  1 à 3 compétences distinctes du deck actuel du joueur.
- `game_over` gagne `campaign: {chapter, level, stars:[3 bool], best:[3 bool], chapter_stars, boss_unlocked, boss_stars_required, boss_just_unlocked, title} | null`.
  `boss_just_unlocked` est vrai quand cette partie ouvre le boss ; `title` (texte ou `null`) est renseigné sur une victoire contre un boss.
- `GET /api/campaign` (authentifié) : `{chapters:[{chapter, family, name, title, title_earned, available, stars, boss_stars_required, boss_unlocked, levels:[{level, name, elo, boss, player_deck, bot_deck, deck_choice, start_fen, human_color, objective, challenge, best, rewarded}]}]}`.
  `objective` et `challenge` sont des textes français (ou `null`) ; `start_fen` est une chaîne ou `null` ; `human_color` vaut `"white"`, `"black"` ou `null`.
- Le profil public gagne `title` (texte ou `null`) : le meilleur titre du joueur.

## Chapitre 1 : Attaque

| Niveau | Nom | Joueur | Sage | Objectif | Défi |
|---|---|---|---|---|---|
| 0 | Première piste | Trap | (aucune) | Utiliser Trap | Garder la dame |
| 1 | Appât | Trap, Terminator | Trap | Utiliser Terminator | Gagner en 40 coups |
| 2 | Sacrifice | Queensac, Trap | Terminator | Garder une tour | Sans compétence |
| 3 | Retrait | Remover, Trap | Trap, Queensac | Utiliser Remover | Gagner en 35 coups |
| 4 | Renversement | Switch, Terminator | Remover, Trap | Utiliser Switch | Garder la dame |
| 5 | Tempête d'acier | Remover, Switch, Terminator | Terminator, Trap, Queensac | Utiliser Terminator | Gagner en 30 coups |
| Boss | Le Stratège | Remover, Switch, Trap | Terminator, Trap, Queensac | Victoire | (aucun) |

## Chapitre 2 : Défense

| Niveau | Nom | Joueur | Sage | Objectif | Défi |
|---|---|---|---|---|---|
| 0 | Premier rempart | Imune | Trap | Utiliser Imune | Garder la dame |
| 1 | Voile d'ombre | Invisibility, Imune | Terminator | Utiliser Invisibility | Gagner en 45 coups |
| 2 | Bouclier d'énergie | Forcefield, Imune | Trap, Remover | Garder une tour | Sans compétence |
| 3 | Grâce céleste | Celestial, Forcefield | Queensac, Trap | Utiliser Celestial | Gagner en 40 coups |
| 4 | Mur de brume | Invisibility, Forcefield, Imune | Remover, Trap, Switch | Utiliser Forcefield | Garder la dame |
| 5 | Citadelle | Celestial, Imune, Invisibility | Terminator, Trap, Queensac | Utiliser Celestial | Gagner en 30 coups |
| Boss | Le Gardien | Celestial, Forcefield, Invisibility | Terminator, Remover, Switch | Victoire | (aucun) |

## Chapitre 3 : Mobilité (deck au choix)

| Niveau | Nom | Sage | Objectif | Défi |
|---|---|---|---|---|
| 0 | Premier pas | Teleportation | Utiliser une compétence | Garder la dame |
| 1 | Détour | Rollback | Garder une tour | Gagner en 45 coups |
| 2 | Faille | Transposition, Bench | Utiliser une compétence | Sans compétence |
| 3 | Écho | Destiny Swapper, Rollback | Garder une tour | Gagner en 40 coups |
| 4 | Sables du temps | Temporal, Teleportation, Bench | Utiliser une compétence | Garder la dame |
| 5 | Au-delà du voile | Teleportation, Rollback, Temporal | Sans compétence | Gagner en 30 coups |
| Boss | Le Passeur | Teleportation, Destiny Swapper, Transposition | Victoire | (aucun) |

## Chapitre 4 : Contrôle (deck au choix)

| Niveau | Nom | Sage | Objectif | Défi |
|---|---|---|---|---|
| 0 | Premier signe | Canceller | Utiliser une compétence | Garder la dame |
| 1 | Vent contraire | Tornado | Garder une tour | Gagner en 45 coups |
| 2 | Givre | Freeze, Canceller | Utiliser une compétence | Sans compétence |
| 3 | Fracture | Tornado, Geomancy | Garder une tour | Gagner en 40 coups |
| 4 | Œil du cyclone | Freeze, Tornado, Canceller | Utiliser une compétence | Garder la dame |
| 5 | Maîtrise du terrain | Geomancy, Freeze, Tornado | Sans compétence | Gagner en 30 coups |
| Boss | Le Métronome | Canceller, Tornado, Freeze | Victoire | (aucun) |

## Chapitre 5 : Création (deck au choix)

| Niveau | Nom | Sage | Objectif | Défi |
|---|---|---|---|---|
| 0 | Première pierre | Wall | Utiliser une compétence | Garder la dame |
| 1 | Double | Clone | Garder une tour | Gagner en 45 coups |
| 2 | Mirage | Mirage, Wall | Utiliser une compétence | Sans compétence |
| 3 | Métamorphose | Morph, Evolve | Garder une tour | Gagner en 40 coups |
| 4 | Main divine | Clone, Godhelp, Wall | Utiliser une compétence | Garder la dame |
| 5 | Chef-d'œuvre | Evolve, Mirage, Morph | Sans compétence | Gagner en 30 coups |
| Boss | L'Architecte | Clone, Godhelp, Morph | Victoire | (aucun) |
