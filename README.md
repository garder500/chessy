# Chessy

Jeu d'échecs en ligne où chaque joueur utilise des **compétences** pour modifier les règles.
Les règles des compétences sont décrites dans [docs/skills.md](docs/skills.md).

## Fonctionnalités

- **Comptes** : inscription / connexion (mots de passe argon2id, sessions), mode invité, promotion d'un invité en compte
  sans perdre son deck.
- **Classement Elo** : parties classées (file d'attente appariée par Elo, plage qui s'élargit avec l'attente),
  classement général, profils publics avec courbe d'Elo et historique, paliers Novice → Maître.
- **Social** : amis avec présence en temps réel, demandes, défis amicaux, recherche de joueurs.
- **En partie** : horloges serveur (10 min + 3 s par action), proposition de nulle, chat avec phrases rapides,
  revanche, abandon, reprise de partie après déconnexion (60 s de grâce).
- **Compétences** : les 27 sont jouables et illustrées (7 uniques, 20 classiques) ; règles dans [docs/skills.md](docs/skills.md) et [docs/spec-v3.md](docs/spec-v3.md).
- **Mode Solo** : partie d'entraînement contre l'IA « Sage », niveau d'Elo réglable de 400 à 2800 (profondeur, erreurs et usage des compétences varient avec le niveau), avec ou sans compte, sans horloge ni Elo en jeu.
- **Sons et couleurs** : une cinquantaine de sons synthétisés (Web Audio, aucun fichier) pour les coups, captures, compétences, « à vous de jouer », fin de partie, etc. ; page Réglages (volumes, thèmes de plateau, jeux de pièces, couleur d'accent).
- **Jouer à la souris** : glisser-déposer des pièces et **premoves** multiples (jusqu'à 10 coups empilés à l'avance, annulables d'un clic droit, Échap ou Retour arrière) comme sur chess.com.
- **Jouer sur téléphone** : plateau pleine largeur (cases de ~44 px) avec compétences et actions sous la main, mise en page dédiée au paysage (plateau fixe à gauche, panneaux à droite), journal et chat repliables, fenêtres de promotion / résultat plein écran, zone de toucher élargie sur le cadre et seuil de glisser adapté au doigt.
- **Parties en direct** : onglet « En direct » pour regarder les parties en cours (contre l'IA ou entre joueurs, avec 30 s de décalage pour les duels) ; informations cachées (pièces invisibles, pièges) jamais révélées.
- **Collection** : l'historique de vos compétences, obtenues, forgées ou perdues (d'où elles viennent, à qui vous les avez prises, qui vous les a prises).
- **La forge** : en choisissant une compétence aléatoire après une victoire classée, on reçoit une compétence **inventée** (nom, description, icône, son et rareté déduits de sa définition). Toutes sont uniques à la naissance, mais une combinaison qui en répète une autre devient Commune ; seules celles qui bouleversent une partie sont Légendaires. Spécification dans [docs/spec-forge.md](docs/spec-forge.md).
- **Replays et analyse** : toutes les parties sont enregistrées (« Mes parties ») ; replay pas à pas, analyse du moteur (précision, étiquettes meilleur/erreur/gaffe, meilleur coup en flèche) et exploration de variantes ; spécification dans [docs/spec-v4.md](docs/spec-v4.md).
- **Design « Jade »** : coins coupés, accent jade, titres condensés, en clair ou en sombre (suit le système, réglable dans Réglages) ; écran Jouer avec onglets de mode (Classée, Amicale, Salle privée, Contre l’IA), groupe, deck et amis en ligne autour d’une pièce éclairée.

Le contrat serveur/client est décrit dans [docs/spec-v2.md](docs/spec-v2.md).

## Structure

- `crates/chessy-engine` — moteur de règles (échecs standard + compétences), sans I/O
- `crates/chessy-server` — serveur WebSocket autoritatif (axum/tokio) + SQLite
- `web/` — client Vite + TypeScript (Phaser pour le plateau, React pour l'interface)

Le serveur est la seule source de vérité : le client reçoit l'état complet et la liste des
actions légales, et n'exécute aucune règle.

## Lancer en développement

Deux terminaux :

```bash
make dev-server   # serveur sur :3000 (base SQLite : ./chessy.sqlite)
make dev-web      # Vite sur :5173, proxy /ws vers le serveur
```

Ouvrez <http://localhost:5173>. Pour jouer contre vous-même, utilisez deux origines
différentes (le jeton d'identité est dans le `localStorage`), par exemple
`http://localhost:5173` et `http://[::1]:5173`.

Pour jouer seul, ouvrez deux navigateurs (ou profils) : le jeton de session est dans le `localStorage`.
Les routes de l'API REST (`/api/...`) et la WebSocket (`/ws`) sont proxifiées par Vite vers le serveur.

Variables d'environnement du serveur : `CHESSY_ADDR` (défaut `127.0.0.1:3000`),
`CHESSY_DB` (défaut `chessy.sqlite`), `CHESSY_WEB_DIR` (défaut `web/dist`, servi s'il existe).

## Lancer avec Docker

L'image `ghcr.io/garder500/chessy` est construite par GitHub Actions
([.github/workflows/docker.yml](.github/workflows/docker.yml)) : un build natif **amd64** et un build natif
**arm64** (runner ARM de GitHub), fusionnés en une seule image multi-architecture. Le même tag fonctionne donc
sur un PC, un serveur x86, un Raspberry Pi ou un Mac Apple Silicon. Tags : `latest` (branche `main`), `main`,
`sha-<commit>` et `X.Y.Z` / `X.Y` pour les tags `vX.Y.Z`.

```bash
docker run -d --name chessy -p 3000:3000 -v chessy-data:/data ghcr.io/garder500/chessy:latest
```

Ouvrez <http://localhost:3000>. Les comptes, parties et classements sont dans la base SQLite du volume
`chessy-data` (`/data/chessy.sqlite`) : ils survivent aux mises à jour de l'image.

Avec Docker Compose ([docker-compose.yml](docker-compose.yml)) :

```bash
docker compose up -d          # télécharge l'image depuis ghcr.io
docker compose up -d --build  # ou la construit localement
docker compose pull && docker compose up -d   # mise à jour
```

Construire l'image soi-même : `docker build -t chessy .`

Variables d'environnement de l'image : `CHESSY_ADDR` (défaut `0.0.0.0:3000`), `CHESSY_DB`
(défaut `/data/chessy.sqlite`), `CHESSY_WEB_DIR` (défaut `/app/web`), `RUST_LOG` (défaut `chessy_server=info`).
Pour exposer le jeu sur Internet, placez-le derrière un reverse proxy HTTPS qui laisse passer les WebSocket
(`/ws`), ou utilisez un tunnel (`make tunnel`).

Premier push : le paquet GHCR est créé privé. Pour le télécharger sans `docker login`, passez-le en public
(GitHub → Packages → chessy → Package settings → Change visibility). Les runners ARM gratuits ne sont
disponibles que pour les dépôts publics.

## Tests

```bash
make check                                   # fmt, clippy, tests Rust, build + tests du client
cargo test --release -p chessy-engine -- --ignored   # perft profond (4,8 M de nœuds)
```

## Règles de jeu retenues pour le MVP

Le cahier des charges ne tranche pas tout ; voici les choix faits (faciles à changer dans le moteur) :

- Chaque compétence est utilisable **une fois par partie** (Mind Reading : 3 fois) et **consomme le tour**, sauf
  Mind Reading et Mind Control, après lesquelles on joue encore.
- Un camp n'est **mat** que s'il ne peut ni jouer un coup légal ni utiliser une compétence
  (une compétence peut donc sauver d'un mat).
- Les compétences ne ciblent pas les rois (sauf Transposition et Destiny Swapper). Une pièce gelée ne donne pas échec.
- Un nouveau joueur reçoit 3 compétences classiques au hasard ; un joueur qui n'en a plus reçoit
  une compétence classique au hasard.
- **Seules les parties classées** (file classée entre deux comptes, au moins 4 plies, pas plus de 3 parties classées
  contre le même adversaire en une heure) donnent une récompense de compétence : ni les parties amicales (invités,
  salles, défis, Solo) ni un abandon à zéro coup n'en donnent, ce qui évite de « farmer » des compétences.
- Récompense « aléatoire » : le gagnant reçoit une compétence tirée au hasard dans le pool global
  (classiques qu'il n'a pas + uniques sans propriétaire) et le perdant en perd une au hasard parmi celles qu'il
  possédait à la fin de la partie. Une récompense non réclamée ne peut pas voler une compétence acquise par le
  perdant après la partie, ni une qu'il a perdue entre-temps ; elle expire au bout de 6 h.
- Sécurité : une déconnexion (`/api/auth/logout`) ferme aussi la WebSocket ouverte avec cette session ; chaque
  connexion a un quota de messages (rafale de 40, 20 par seconde, une recherche d'utilisateur ou une demande d'ami
  coûte 4) et une file d'envoi bornée. Détails dans [docs/spec-v4.md](docs/spec-v4.md) (« Durcissement »).
- Une compétence unique n'a qu'un seul propriétaire dans le monde (contrainte en base).
