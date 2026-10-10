# Mode campagne (spec)

Première tranche : mécanique côté serveur et contenu du chapitre 1 (Attaque).

## Règles

- Une partie de campagne est une partie Solo contre Sage (`Solo.campaign`), sans nouveau `GameKind`, sans Elo ni pendule.
  Les decks des deux camps sont imposés par le niveau : pas de choix de deck.
- 5 chapitres (0..4) = familles attaque, défense, mobilité, contrôle, création. Chacun a 6 niveaux (0..5) et un boss (niveau 6 sur le fil).
  Seul le chapitre 0 a du contenu : lancer un niveau d'un autre chapitre est refusé (`unknown_level`).
- Elo de Sage : `400 + 400 × chapitre + 50 × niveau` ; le boss vaut le dernier niveau + 100 (chapitre 0 : 400..650, boss 750).
- Étoiles d'un niveau : victoire, objectif, défi. L'objectif et le défi ne comptent qu'en cas de victoire.
  Les meilleures étoiles sont cumulées (OU de bits) par `player_id`, invités compris.
- Les niveaux 0..5 sont ouverts d'emblée. Le boss n'a que l'étoile de victoire ; il s'ouvre à 12 étoiles sur les 18 des niveaux (`boss_locked` sinon).
- « Gagner en N coups » compte les actions des deux camps.

## Récompense du boss

Première victoire d'un compte (pas d'un invité) sur le boss : une forge garantie, proposée via le `RewardOffer` habituel (`steal_options` vide).
La rareté est tirée dans une plage par chapitre, avec les poids de drop renormalisés : chapitre 0 Peu commune..=Épique, chapitres 1-2 Rare..=Légendaire, chapitres 3-4 Épique..=Légendaire.
Un drapeau `rewarded` persisté empêche de la farmer. Le boss ne retire rien au joueur. Un invité joue et progresse mais ne reçoit rien.
La réclamation est enregistrée à la fin de la partie : se déconnecter avant de choisir la consomme.

Point d'attention : la plage des chapitres 1-4 inclut Légendaire, qui crée une compétence unique (`unique_skill_owner`).
Les ids forgés sont neufs, donc pas de collision, mais une unique rejoint le deck et peut être volée en classé.

## Protocole

- Client : `{"type":"campaign_start","chapter":0,"level":6}`.
- `game_over` gagne `campaign: {chapter, level, stars:[3 bool], best:[3 bool], chapter_stars, boss_unlocked} | null`.
- `GET /api/campaign` (authentifié) : `{chapters:[{chapter, family, name, available, stars, boss_stars_required, boss_unlocked, levels:[{level, name, elo, boss, player_deck, bot_deck, objective, challenge, best, rewarded}]}]}`.
  `objective` et `challenge` sont des textes français (ou `null`).

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

## Reporté

Titres, choix de 3 compétences du deck, positions de départ spéciales, chapitres 2 à 5, interface web.
