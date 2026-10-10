# Spec v6 — Mode Campagne

Un mode solo en cinq chapitres contre Sage, le bot de la maison. Chaque chapitre enseigne une famille de compétences
(attaque, défense, mobilité, contrôle, création), compte six niveaux et un boss. Design Jade, tout est décidé par le
serveur ; le client lit.

## Niveaux et difficulté

- Identifiant d'un niveau : `chapitre × 10 + index` (1 à 6), le boss porte l'index 7 (ex. 27).
- Elo de Sage : `400 + 400 × (chapitre − 1) + 50 × (index − 1)` ; le boss joue à `départ + 350`.
- Chapitres 1 et 2 : mains imposées (pas de choix de deck). Chapitres 3 à 5 : le joueur choisit jusqu'à trois
  compétences de son deck, validées côté serveur.
- Le joueur est toujours Blanc. Partie solo ordinaire : pas d'horloge, pas d'Elo, pas de récompense, pas de revanche,
  démarrage immédiat sans choix de deck.

## Étoiles

Trois par niveau, en masque de bits (1 victoire, 2 objectif, 4 défi). Le meilleur résultat est conservé. Une étoile
n'est donnée que si la partie est gagnée. L'objectif et le défi sont jugés en fin de partie à partir des évènements
du moteur (captures, promotions, pertes).

## Ouverture

- Un niveau s'ouvre quand le précédent a été gagné.
- Le boss demande 12 étoiles sur les six niveaux ordinaires de son chapitre.
- Le chapitre suivant s'ouvre quand le boss est vaincu.

## Forge de boss

Vaincre un boss donne une forge due, réclamée une seule fois (`forge_pending`, messages `campaign_forge`). La rareté
est tirée avec un plancher (`roll_rarity_at_least`) puis la compétence forgée avec `forge_at_least` (jusqu'à six
tirages). Le joueur choisit quelle compétence remplacer, comme pour une récompense classée. Les cotes sont affichées
dans le briefing du boss.

## Choix par défaut là où le design est ambigu

- L'objectif « Survivre » du document n'est pas retenu : il se confondait avec la victoire.
- Les étoiles ne s'obtiennent qu'en cas de victoire.
- La progression suit l'identifiant du joueur (invité ou compte).
- Le boss du chapitre 1 (Sacrifice de dame) échappe au mat du berger : c'est voulu.

## Protocole

Client : `campaign_get`, `campaign_start { level, skills }`, `campaign_forge { replace }`.
Serveur : `campaign { levels }`, `game_over.campaign`, `state.campaign` (bandeau en partie).
Base : table `campaign_levels` (migration ordonnée).
