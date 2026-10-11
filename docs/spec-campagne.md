# Mode campagne (spec)

Contenu des 5 chapitres, règles des étoiles, récompense de boss et contrat de protocole.
Le contenu vit dans `crates/chessy-server/src/campaign.rs`, `campaign/levels/` et `campaign/forge_table.rs`.
Les décisions de fond viennent du relevé de cohérence de la campagne ; les écarts connus entre cette spec et le code
sont listés en fin de document.

## Règles

- Une partie de campagne est une partie Solo contre Sage (`Solo.campaign`), sans nouveau `GameKind`, sans Elo ni pendule.
- **La campagne exige un compte.** Un invité ne peut ni la jouer ni y progresser : le serveur refuse `campaign_start`
  (`account_required`) et `GET /api/campaign` ; le client montre la carte grisée aux invités.
  La progression est stockée côté serveur sur l'identifiant du compte.
- 5 chapitres (0..4 sur le fil, 1..5 dans ce document) = familles attaque, défense, mobilité, contrôle, création.
  Chacun a 6 niveaux (0..5) et un boss (niveau 6 sur le fil).
- Sage ne voit que ce qu'un joueur verrait à sa place. Il joue sur la position filtrée (`view_position`) : Invisibility,
  Trap Card, Mirage et les forgées Cloak et Fog marchent donc contre lui.
- Sage n'abandonne jamais en v1. Un Sage à plus de 2 000 Elo se mate comme les autres : il n'y a pas de pendule.
- Abandon du joueur = défaite : aucune étoile acquise n'est retirée et rien n'est pénalisé.
  La proposition de nulle est toujours refusée en campagne (`solo_answer_draw`).

### Tableau des chapitres

| Chapitre | Famille | Sage, niveaux 1 à 6 | Boss (Elo) | Rareté minimale | Tirage de forge | Compétences enseignées |
|---|---|---|---|---|---|---|
| 1 · Première lame | Attaque | 400 à 650 | Le Bélier, 800 (Terminator, Trap Card + Queen Sacrifice) | Peu commune | Peu commune 57 % / Rare 29 % / Épique 14 % (famille Attaque) | Queen Sacrifice, Terminator, Trap Card (imposées) |
| 2 · Le Rempart | Défense | 800 à 1 050 | La Muraille, 1 200 (Force Field, Imune + Celestial Intervention) | Rare | Rare 68 % / Épique 32 % (famille Défense) | Imune, Force Field, Invisibility (imposées) |
| 3 · Les Routes | Mobilité | 1 200 à 1 450 | Le Passeur, 1 600 (Teleportation, Destiny Swapper + Rollback) | Rare | Rare 68 % / Épique 32 % (famille Mobilité) | Teleportation, Transposition, The Bench (prêtées, 3 au choix avec le deck) |
| 4 · La Main invisible | Contrôle | 1 600 à 1 850 | L'Illusionniste, 2 000 (Invisibility, Geomancy + Tornado) | Épique | Épique 100 % (famille Contrôle) | Freeze, Tornado, Canceller (prêtées, 3 au choix avec le deck) |
| 5 · La Forge | Création | 2 000 à 2 250 | Le Forgeron, 2 400 (Clone, God Help + Morph) | Épique | Épique 86 % / Légendaire 14 % (famille libre, Légendaire seulement s'il reste une signature libre) | Clone, Morph, God Help (prêtées, 3 au choix avec le deck) |

Formules : niveau n (1 à 6) du chapitre c (1 à 5) = 400·c + 50·(n−1) ; boss = 400·(c+1).
Dans le code (indices à partir de 0) : `400 × (chapitre + 1) + 50 × niveau`, boss `400 × (chapitre + 2)`.
Sage change de profondeur à 800, 1 200, 1 600, 2 000 et 2 400 : le boss est placé au début de la marche suivante, pour que le vrai saut
de difficulté soit sur lui. L'écran affiche l'Elo de Sage seul (« Sage · 1 000 »), sans nom de palier.

### Les mains

- **Une fois par partie.** Chaque compétence imposée ou prêtée s'utilise une fois par partie, comme en classée
  (Mind Reading : 3 fois).
- **Main de Sage fixe et affichée.** Chaque niveau fixe la main de Sage dans ses données et la montre au briefing ; rien n'est tiré au hasard.
  Niveaux 1 et 2 : 1 compétence ; niveaux 3 et 4 : 2 ; niveaux 5 et 6 : 3. Elles viennent de la famille du chapitre et des
  cinq classiques hors chapitre (Celestial Intervention, Rollback, Destiny Swapper, Temporal Distortion, Geomancy).
  Sage n'a jamais Mind Reading ni Mind Control.
- **Boss.** Main fixe de 3 classiques (2 signatures et 1 classique hors chapitre), montrée au briefing. Pour un boss,
  `skill_permille` est forcé à 1000 : Sage envisage ses compétences à chaque tour (sinon, à 800 Elo, environ une fois sur trois).
  Le Bélier : Terminator, Trap Card + Queen Sacrifice. La Muraille : Force Field, Imune + Celestial Intervention.
  Le Passeur : Teleportation, Destiny Swapper + Rollback. L'Illusionniste : Invisibility, Geomancy + Tornado.
  Le Forgeron : Clone, God Help + Morph.
- **Chapitres 1 et 2 : main imposée.** Le joueur joue la main du niveau, sans aucune unique.
- **Chapitres 3 à 5 : 3 compétences prêtées.** Le joueur choisit 1 à 3 classiques distinctes parmi son deck et les 3 compétences
  du chapitre, prêtées pour la partie. Un prêt n'entre jamais dans le deck, n'apparaît pas dans la Collection et ne peut pas être volé.
  Ses uniques sont ajoutées automatiquement, comme en classée, et le briefing les affiche « en plus ».

## Étoiles

- Trois étoiles par niveau : **victoire**, **objectif**, **défi**. Les deux dernières ne comptent que dans une partie gagnée.
- **Victoire = mat, et rien d'autre.** Toute nulle (pat, 50 coups, répétition, matériel insuffisant) est un niveau non réussi,
  sans étoile et sans pénalité. L'écran l'explique : « Pat : aux échecs, c'est une nulle. »
- Chaque étoile est acquise pour de bon et se cumule d'un essai à l'autre (OU de bits d'un masque de 3 bits par niveau et par compte) :
  il n'est pas nécessaire de réussir les trois dans la même partie. Une défaite, une nulle ou un abandon ne retire jamais d'étoile.
  L'écran « Échoué » propose « Réessayer » et « Carte ».
- Le boss s'ouvre à 12 étoiles sur les 18 des six niveaux (`boss_locked` sinon). Le boss a lui aussi 3 étoiles ; elles comptent
  dans le total sur 105 (5 chapitres × 7 niveaux × 3) et donnent le titre.
- Les objectifs possibles : « Mater avant le coup N », « Terminer avec une pièce », « Utiliser une compétence (précise ou
  n'importe laquelle) », « Utiliser toutes ses compétences », « Gagner sans utiliser de compétence » et « Ne perdre aucune pièce
  avant le coup N ». Chaque niveau vérifie que ses défis sont compatibles avec sa main (pas de « Garder la dame » avec Queen Sacrifice,
  pas de défi « sans compétence » aux chapitres 1 et 2, qui servent à apprendre les compétences).
- **Comptage des coups.** Un coup est un numéro de coup de la notation (une paire blanc + noir) du côté du joueur ; une compétence qui
  coûte le tour compte pour un coup. Mind Reading et Mind Control, actions gratuites, ne font pas avancer le compteur.
  « Avant le coup 30 » = mat donné au coup 29 au plus tard. Le bandeau de partie affiche « Coup 18 / 30 ».
  Toutes les positions de départ ont les Blancs au trait et `Position::from_fen` donne `ply = 0` : le décompte est identique à la position standard.
- **Joueur bloqué.** Après 3 défaites d'affilée sur un même niveau ou un même boss (`HINT_AFTER_DEFEATS`), l'écran « Échoué » propose de relire
  le briefing avec un conseil écrit propre au niveau (`Level.hint`, renvoyé par l'API seulement à partir de ce seuil). Il n'y a pas d'aide
  pendant la partie en v1 et l'étoile de victoire ne change pas. Une victoire remet la série à zéro.

## Récompense du boss

La forge de boss a **son propre chemin**, distinct de la récompense classée : le joueur gagne une compétence et personne n'en perd.
La forge de campagne a son propre déclencheur : la première victoire d'un compte sur un boss (voir aussi `docs/spec-forge.md`).

- **États** (table `campaign_boss_forges`, une ligne par compte et par chapitre, 5 au plus) : aucune ligne → `forging` à la première victoire →
  `pending` quand la compétence est enregistrée → `placed` quand elle est dans le deck. Un claim n'est accepté qu'en `forging` sans tâche en vol ;
  un placement n'est accepté qu'en `pending`.
- **Pas de deuxième forge.** Rejouer un boss ne relance jamais la forge : le serveur refuse une deuxième forge pour le même compte et le même boss.
- **Révélation rejouable.** Le serveur enregistre la compétence avant l'animation ; si la révélation est interrompue (déconnexion, appli fermée),
  elle est rejouée au retour du joueur depuis la carte de campagne.
- **Deck plein (7).** Si le deck a de la place, la compétence y entre. Sinon le joueur remplace une compétence (elle part dans l'historique comme
  « remplacée ») ou choisit « Plus tard » : la forgée reste `pending` sans expiration et se place depuis la carte de campagne ou la Collection.
  Il n'y a pas de réserve générale.
- **Tirage.** On prend `DROP_WEIGHTS` [55, 25, 13, 6, 1], on garde les raretés au-dessus du plancher du chapitre, on retire celles que la famille
  ne peut pas atteindre, et on renormalise (plus grand reste). La table du chapitre est affichée sur l'écran du boss.
  Les effets Légendaires n'existent qu'en Contrôle et en Création : la Légendaire est donc **réservée au chapitre 5**.
- **Famille.** Aux chapitres 1 à 4 la forge ne produit que des effets de la famille du chapitre ; au chapitre 5 la famille est libre.
  Exemple au chapitre 2 : « Égide d'Orvane · Rare · Défense : une de vos pièces (pas le roi) ne peut pas être prise pendant 3 tours ».
- **Chapitre 5.** Épique ou mieux, 14 % de Légendaire tant qu'il reste une signature de bascule libre. Quand il n'en reste plus, l'écran le dit
  (« plus aucune Légendaire disponible : Épique garantie ») au lieu de baisser la rareté sans prévenir. Exemple :
  « Armistice d'Alfen · Légendaire · Contrôle · Unique : pendant 4 demi-coups, rien n'attaque rien, ni prise ni échec. »
  Une Légendaire est unique au monde et ne compte pas dans les 3 ; le titre de fin de campagne est garanti.
- **Garanties.** La forge ne rend jamais une compétence redondante, sous le plancher ou hors famille (`forge_at_least`, voir `docs/spec-forge.md`).
  Si aucune candidate n'est acceptable dans le budget, la ligne reste `forging` et sera reprise.
- **Perte en classée.** Les forgées de campagne sont des compétences comme les autres : elles peuvent être volées ou perdues en classée.
  C'est annoncé sur l'écran de forge du boss (« Elle vous suit en classée, où elle peut être perdue ») et avant la première partie classée.
- Le boss ne retire rien au joueur.

### Rappel : la récompense classée

La forge est proposée au gagnant d'une partie classée (choix Forger) ; jamais en Solo. La campagne a son propre déclencheur sur la victoire contre un boss.
Passer, ou laisser expirer la récompense (6 h), épargne le perdant ; un joueur dont le deck est vide reçoit une classique au hasard.
Les récompenses en attente sont stockées en base (`pending_rewards`) et ne sont plus abandonnées quand une nouvelle partie commence.

## Titres

Un titre par boss, obtenu avec ses 3 étoiles. Il s'affiche sous le pseudo ; le joueur choisit son titre actif (`POST /api/profile/title`,
champ `title_active`), par défaut le plus élevé.

| Chapitre | Boss | Titre |
|---|---|---|
| 1 Attaque | Le Bélier | Tombeur du Bélier |
| 2 Défense | La Muraille | Briseur de Muraille |
| 3 Mobilité | Le Passeur | Maître des Routes |
| 4 Contrôle | L'Illusionniste | Démasqueur |
| 5 Création | Le Forgeron | Maître de forge |

## Positions de départ des boss

Les boss des chapitres 2 à 5 ont une position de départ (FEN, Blancs au trait) conçue pour un camp fixe, `human_color`. Le boss du chapitre 1 garde la position standard.

| Chapitre | Boss | Joueur | Position |
|---|---|---|---|
| 2 Défense | La Muraille | Blancs | Sage a deux pions de plus (c6, f6), un mur de pions |
| 3 Mobilité | Le Passeur | Blancs | Sage a un fou de plus (e6) |
| 4 Contrôle | L'Illusionniste | Blancs | Sage a un cavalier de plus (f6) |
| 5 Création | Le Forgeron | Noirs | Sage (Blancs) a deux pions de plus (c3, f3) |

Conseil écrit contre La Muraille : « Sa Force Field protège une seule pièce et repousse celui qui la prend : prenez-la avec un pion, ou attaquez une autre cible. »
Force Field : la pièce peut être prise, mais celui qui la prend est repoussé de deux cases au plus ; la protection dure jusqu'à cette prise.

## Protocole (contrat)

- Client : `{"type":"campaign_start","chapter":2,"level":0,"deck":["freeze","clone"]}`.
  `deck` est facultatif, et utile seulement si le niveau a `deck_choice` (erreur `bad_deck` s'il est invalide) :
  1 à 3 classiques distinctes du deck du joueur ou prêtées par le niveau. Un invité reçoit `account_required`.
- `game_over` gagne `campaign: {chapter, level, stars:[3 bool], best:[3 bool], chapter_stars, boss_unlocked, boss_stars_required, boss_just_unlocked, title, total_stars, hint_available, boss_forge} | null`.
  `boss_just_unlocked` est vrai quand cette partie ouvre le boss ; `title` (texte ou `null`) est renseigné quand cette partie donne un titre.
- Contexte de partie : `campaign: {chapter, level, move_limit, objective, challenge} | null`.
- Boss : `boss_forge { info }` (serveur), `boss_forge_claim { chapter }` et `boss_forge_place { chapter, replace }` (client) ;
  `BossForgeInfo { chapter, state: forging|pending|placed, skill, deck_full, legendary_unavailable }`.
- Client (builds de debug uniquement, erreur `dev_only` sinon) : `{"type":"dev_finish","result":"win"}` termine la partie de campagne en cours par une victoire (`win`), une défaite (`loss`) ou une victoire qui enregistre toutes les étoiles du niveau (`all_stars`) ; erreur `not_campaign` hors partie de campagne.
- `GET /api/campaign` (compte authentifié) : `{total_stars, max_stars, chapters:[{chapter, family, name, title, title_earned, titles, available, stars, boss_stars_required, boss_unlocked, forge_table:[{rarity, percent}], boss_forge, levels:[{level, name, elo, boss, player_deck, bot_deck, lent, deck_choice, start_fen, human_color, move_limit, objective, challenge, hint, best, rewarded}]}]}`.
  `best` est le masque d'étoiles sous forme de 3 booléens ; `hint` n'est présent qu'après 3 défaites d'affilée ; `objective` et `challenge` sont des textes français (ou `null`).
- `POST /api/profile/title { chapter }` (400 si le titre n'est pas gagné ; `null` retire le titre choisi). Le profil public gagne `title` (texte ou `null`) et, pour soi, `titles [{chapter, name}]`.

## Chapitre 1 : Attaque (main imposée)

| Niveau | Nom | Joueur | Sage | Objectif | Défi |
|---|---|---|---|---|---|
| 0 | Première piste | Trap Card | Remover | Utiliser Trap Card | Mater avant le coup 25 |
| 1 | Appât | Trap Card, Terminator | Trap Card | Utiliser Terminator | Mater avant le coup 30 |
| 2 | Sacrifice | Queen Sacrifice, Trap Card | Terminator, Remover | Utiliser Queen Sacrifice | Terminer avec une tour |
| 3 | Tenaille | Terminator, Queen Sacrifice | Trap Card, Switch Sides | Utiliser toutes ses compétences | Terminer avec une tour |
| 4 | Triple lame | Queen Sacrifice, Terminator, Trap Card | Remover, Trap Card, Terminator | Utiliser toutes ses compétences | Mater avant le coup 28 |
| 5 | Tempête d'acier | Queen Sacrifice, Terminator, Trap Card | Terminator, Trap Card, Queen Sacrifice | Utiliser Terminator | Terminer avec un cavalier |
| Boss | Le Bélier | Queen Sacrifice, Terminator, Trap Card | Terminator, Trap Card, Queen Sacrifice | Utiliser toutes ses compétences | Mater avant le coup 35 |

## Chapitre 2 : Défense (main imposée)

| Niveau | Nom | Joueur | Sage | Objectif | Défi |
|---|---|---|---|---|---|
| 0 | Premier rempart | Imune | Celestial Intervention | Utiliser Imune | Terminer avec sa dame |
| 1 | Voile d'ombre | Imune, Invisibility | Imune | Utiliser Invisibility | Mater avant le coup 35 |
| 2 | Bouclier d'énergie | Force Field, Imune | Celestial Intervention, Invisibility | Utiliser Force Field | Terminer avec une tour |
| 3 | Grâce céleste | Invisibility, Force Field | Imune, Force Field | Utiliser toutes ses compétences | Ne perdre aucune pièce avant le coup 20 |
| 4 | Mur de brume | Invisibility, Force Field, Imune | Force Field, Celestial Intervention, Invisibility | Utiliser Imune | Mater avant le coup 32 |
| 5 | Citadelle | Invisibility, Force Field, Imune | Imune, Celestial Intervention, Force Field | Utiliser toutes ses compétences | Terminer avec sa dame |
| Boss | La Muraille | Force Field, Imune, Invisibility | Force Field, Imune, Celestial Intervention | Utiliser toutes ses compétences | Ne perdre aucune pièce avant le coup 20 |

## Chapitre 3 : Mobilité (compétences prêtées : Teleportation, Transposition, The Bench)

| Niveau | Nom | Sage | Objectif | Défi |
|---|---|---|---|---|
| 0 | Premier pas | Rollback | Utiliser une compétence | Terminer avec sa dame |
| 1 | Détour | Teleportation | Utiliser une compétence | Mater avant le coup 36 |
| 2 | Faille | Temporal Distortion, Destiny Swapper | Utiliser une compétence | Terminer avec une tour |
| 3 | Écho | Rollback, The Bench | Utiliser toutes ses compétences | Mater avant le coup 34 |
| 4 | Sables du temps | Teleportation, Rollback, Temporal Distortion | Utiliser toutes ses compétences | Terminer avec sa dame |
| 5 | Au-delà du voile | Destiny Swapper, Transposition, Temporal Distortion | Mater avant le coup 30 | Gagner sans utiliser de compétence |
| Boss | Le Passeur | Teleportation, Destiny Swapper, Rollback | Utiliser toutes ses compétences | Mater avant le coup 36 |

## Chapitre 4 : Contrôle (compétences prêtées : Freeze, Tornado, Canceller)

| Niveau | Nom | Sage | Objectif | Défi |
|---|---|---|---|---|
| 0 | Premier signe | Canceller | Utiliser une compétence | Terminer avec sa dame |
| 1 | Vent contraire | Tornado | Utiliser une compétence | Mater avant le coup 38 |
| 2 | Givre | Freeze, Geomancy | Utiliser une compétence | Terminer avec une tour |
| 3 | Fracture | Tornado, Canceller | Utiliser toutes ses compétences | Mater avant le coup 36 |
| 4 | Œil du cyclone | Freeze, Geomancy, Tornado | Utiliser toutes ses compétences | Terminer avec sa dame |
| 5 | Maîtrise du terrain | Canceller, Tornado, Freeze | Mater avant le coup 32 | Gagner sans utiliser de compétence |
| Boss | L'Illusionniste | Invisibility, Geomancy, Tornado | Utiliser toutes ses compétences | Ne perdre aucune pièce avant le coup 20 |

## Chapitre 5 : Création (compétences prêtées : Clone, Morph, God Help)

| Niveau | Nom | Sage | Objectif | Défi |
|---|---|---|---|---|
| 0 | Première pierre | Wall | Utiliser une compétence | Terminer avec sa dame |
| 1 | Double | Mirage | Utiliser une compétence | Mater avant le coup 38 |
| 2 | Mirage | Evolve, Wall | Utiliser une compétence | Terminer avec une tour |
| 3 | Métamorphose | Mirage, Morph | Utiliser toutes ses compétences | Mater avant le coup 36 |
| 4 | Main divine | Clone, Evolve, Wall | Utiliser toutes ses compétences | Terminer avec sa dame |
| 5 | Chef-d'œuvre | God Help, Mirage, Evolve | Mater avant le coup 32 | Gagner sans utiliser de compétence |
| Boss | Le Forgeron | Clone, God Help, Morph | Utiliser toutes ses compétences | Terminer avec sa dame |

## Questions ouvertes

Chaque question a reçu une valeur par défaut, appliquée dans cette spec ; le porteur du projet peut la renverser.

1. **Chapitre 5 : la Légendaire est-elle garantie au Forgeron, ou seulement possible, et à quel taux ?**
   Défaut appliqué : Épique garantie, avec 14 % de Légendaire tant qu'il reste une signature libre ; le taux est affiché et le titre de fin est garanti.
   Autres options : Épique garantie et 30 % de Légendaire ; Légendaire garantie, au risque de vider le stock d'uniques.
2. **Cadences : une seule (10+3), ou les Courte / Moyenne / Longue dessinées sur le canevas ?**
   Défaut appliqué : une seule cadence de 10 min + 3 s en v1 ; le sélecteur est retiré du canevas.
   Autres options : cadences libres en Amicale et en salle privée ; 5+3 / 10+3 / 30+3 partout avec une file par cadence et un seul Elo.
3. **Après une victoire classée, la perte du perdant doit-elle dépendre du gagnant ?**
   Défaut appliqué : Passer ou laisser expirer la récompense épargne le perdant ; les récompenses sont stockées en base et ne sont plus perdues quand on lance une nouvelle partie.
   Autres options : Forger d'office à l'expiration ; le perdant perd toujours une compétence.
4. **Les compétences gagnées en campagne peuvent-elles être volées ou perdues en classée ?**
   Défaut appliqué : oui, comme toutes les autres, et c'est annoncé sur l'écran de forge du boss et avant la première classée.
   Autres options : protégées en classée ; réservées à la campagne et à l'amicale.
5. **Deck plein au moment d'une forge de boss : que proposer ?**
   Défaut appliqué : remplacer une compétence, ou « Plus tard » (la forgée reste en attente sans expiration, au plus une par boss) ; pas de réserve générale.
   Autres options : remplacer ou renoncer (la forgée est perdue) ; créer une vraie réserve hors deck.
6. **Noms des titres de profil obtenus avec 3 étoiles sur un boss.**
   Défaut appliqué : Tombeur du Bélier, Briseur de Muraille, Maître des Routes, Démasqueur, Maître de forge ; le joueur choisit le titre affiché sous son pseudo.
   Autres options : un modèle neutre « Vainqueur de <Boss> » ; un seul titre à la fin de la campagne.
