# Chessy v2 — comptes, classement, amis, design B

Contrat entre le serveur (`crates/`) et le client (`web/`). Les messages existants
(voir `crates/chessy-server/src/protocol.rs` et `web/src/protocol.ts`) restent valides ;
ce document décrit **ce qui change ou s'ajoute**. Tout est en JSON, tags `snake_case`.

## 1. Comptes

### Modèle
- `players` gagne : `username TEXT NULL`, `username_lower TEXT NULL UNIQUE`, `password_hash TEXT NULL`
  (argon2id, chaîne PHC), `elo INTEGER NOT NULL DEFAULT 1200`, `peak_elo`, `games`, `wins`, `losses`,
  `draws` (compteurs de parties **classées**), `last_seen TEXT`.
- Nouvelle table `sessions(token TEXT PRIMARY KEY, player_id, created_at, last_used)` (`last_used` : voir spec-v4, « Sessions »). Un jeton de session
  identifie un joueur. Les anciens `players.token` sont migrés vers `sessions`.
- **Invité** = joueur sans `username`. Créé comme aujourd'hui quand `hello` n'a pas de jeton valide.
- Migrations : `PRAGMA user_version`, idempotentes, sans casser une base existante.

### REST (JSON, préfixe `/api`)
Authentification : en-tête `Authorization: Bearer <token>`.

| Route | Corps / réponse |
|---|---|
| `POST /api/auth/register` | `{username, password, guest_token?}` → `200 {token, player: Me, recovery_code}` (le code de récupération, montré **une seule fois** : voir « Récupération de compte »). Avec `guest_token` valide, l'invité est **promu** (même id, même deck). Erreurs `400 {error}` : `invalid_username`, `weak_password` ; `409 {error:"username_taken"}`. |
| `POST /api/auth/login` | `{username, password}` → `200 {token, player: Me}` (nouvelle session). `401 {error:"bad_credentials"}` (même message pour pseudo inconnu ou mauvais mot de passe). |
| `POST /api/auth/logout` | Bearer → `204`, supprime la session. |
| `POST /api/auth/logout-all` | Bearer → `204`, supprime **toutes** les sessions du compte (autres appareils compris) et coupe les WebSocket ouvertes avec l'une d'elles (`session_revoked`). `401` sans Bearer valide. |
| `POST /api/auth/recover` | `{username, recovery_code, new_password}` → `200 {token, player: Me, recovery_code}` : nouveau mot de passe, **toutes** les anciennes sessions révoquées, code remplacé (le nouveau est dans la réponse, une seule fois) et une session neuve (`token`). `400 weak_password` (8 à 128 caractères, vérifié avant le code : le code n'est pas consommé) ; `401 bad_recovery` (même réponse pseudo inconnu, invité, compte sans code ou mauvais code) ; `429 too_many_attempts` (verrouillage, voir plus bas). |
| `POST /api/me/recovery-code` | Bearer + `{password}` (mot de passe actuel) → `200 {recovery_code}` : génère ou **remplace** le code du compte (c'est ainsi qu'un compte antérieur aux codes en obtient un). `401 unauthorized` (Bearer), `401 bad_credentials` (mauvais mot de passe, ou invité), `429 too_many_attempts`. |
| `GET /api/me` | Bearer → `Me`. `401` sinon. |
| `GET /api/leaderboard?limit=50&offset=0` | `{total, entries:[{rank, username, elo, games, wins, draws, losses}]}` — comptes enregistrés uniquement, tri `elo DESC, wins DESC, username`. `limit` ≤ 100. |
| `GET /api/players/{username}` | `PublicProfile` (insensible à la casse) ou `404`. |

- `username` : `^[A-Za-z0-9_]{3,16}$`, unique sans distinction de casse. `password` : 8 à 128 caractères.
- `Me = {player_id, username|null, guest: bool, elo, rank: int|null, games, wins, draws, losses}`
  (`rank` = rang parmi les comptes enregistrés, `null` pour un invité).
- `PublicProfile = {username, elo, peak_elo, rank, games, wins, draws, losses, streak: int (positif = victoires de suite, négatif = défaites), created_at, history: [{elo, at}] (≤ 30 derniers points, ordre chronologique, commence par 1200), recent: [{game_id, opponent: string|null, result: "win"|"loss"|"draw", color: "white"|"black", rated: bool, elo_delta: int|null, reason: string, at}] (≤ 10)}`.

## 2. Elo et parties classées

- Départ 1200. `E = 1 / (1 + 10^((Rb - Ra)/400))`. `K = 40` pendant les 30 premières parties classées du joueur, sinon `20`.
  Plancher 100. Arrondi à l'entier le plus proche, le gain d'un joueur n'est pas forcément l'opposé de l'autre.
- Une partie est **classée** si : issue de la file *classée*, les deux joueurs ont un compte, et elle a duré au moins 4 plies
  (sinon abandon trop précoce = non classée). Les salles privées, défis d'amis, parties d'invités sont **amicales** (aucun Elo).
- **Seule une partie classée donne une récompense de compétence** (`game_over.reward` est `null` pour une partie amicale — invités, salle, défi, revanche d'une amicale —, pour le Solo, pour un abandon ou une déconnexion avant 4 plies, et pour une partie qui a dépassé le plafond de parties classées entre les deux mêmes comptes, voir `docs/spec-v4.md` « Durcissement »). Sans récompense, `reward_choice` répond `no_reward`.
- La récompense est liée à **ce que le perdant possédait à la fin de la partie** (instantané dans `PendingReward`) : `steal_options` = ces compétences, que le perdant possède **encore** au moment de la réclamation et que le gagnant n'a pas ; un vol d'une autre compétence (acquise après coup ou déjà perdue) est refusé par `invalid_reward` (la réclamation reste ouverte). Le tirage aléatoire retire aussi au perdant une compétence tirée dans cet ensemble. Une récompense se réclame une fois, est abandonnée dès que le gagnant lance une autre partie/file/salle, et expire après `reward_ttl` (6 h ; ensuite `no_reward`, y compris dans le `welcome`).
- Les compteurs/Elo/`rating_history(player_id, game_id, elo, at)` sont mis à jour dans la **même transaction** que l'enregistrement de la partie.
- `games` gagne : `rated`, `reason`, `plies`, `white_elo_before/after`, `black_elo_before/after`, `started_at`.

## 3. Matchmaking

- `queue_join {ranked?: bool}` : `ranked` vaut `true` par défaut pour un compte, forcé `false` pour un invité.
  Deux files : classée (comptes seulement) et amicale (tous).
- Appariement dans la file classée : le candidat d'Elo le plus proche dont l'écart ≤ `100 + 25 × s` où `s` = secondes d'attente du plus ancien des deux
  (plafond 800). Un timer `QueueSweep` (toutes les 3 s tant qu'une file est non vide) relance l'appariement. File amicale : premier arrivé.
- Les messages `Lobby` gagnent `status: {type:"queued", ranked: bool}`.

## 4. Amis, présence, défis (WebSocket)

Table `friendships(user_a, user_b, requester, status, created_at)` avec `user_a < user_b`, `status ∈ {pending, accepted}`.

Client → serveur (comptes uniquement ; un invité reçoit `error {code:"account_required"}`) :
- `friend_request {username}` · `friend_respond {username, accept: bool}` · `friend_remove {username}` · `friends_list {}`
- `user_search {query}` (≥ 2 caractères, 10 résultats max, préfixe insensible à la casse)
- `challenge {username}` (ami en ligne et pas en partie) · `challenge_respond {username, accept: bool}` · `challenge_cancel {}`

Serveur → client :
- `friends {friends: [FriendInfo], incoming: [{username, elo}], outgoing: [{username}]}` — envoyé après `welcome` et à chaque changement qui concerne le joueur.
  `FriendInfo = {username, elo, presence: "online"|"in_game"|"offline", last_seen: string|null}`.
- `user_results {query, users: [{username, elo, relation: "none"|"friend"|"incoming"|"outgoing"|"self"}]}`
- `notice {code, username?}` pour les toasts : `friend_request_received`, `friend_accepted`, `friend_removed`, `challenge_declined`,
  `challenge_expired`, `challenge_cancelled`, `user_not_found`, `already_friends`, `friend_offline`, `friend_busy`.
- `challenge_received {from: {username, elo}}` (expire après 60 s) · `challenge_sent {username}`.
- Une acceptation de défi crée la partie par le flux normal (`deck_select`), amicale.
- La présence est poussée aux amis (`friends` mis à jour) à la connexion, déconnexion, début et fin de partie.

## 5. Fonctions de jeu

### Horloges (serveur autoritaire)
10 min par joueur + 3 s par action (coup ou compétence), configurables dans `HubConfig` (`clock_initial`, `clock_increment`).
Le temps ne tourne qu'en phase `Playing`. Un `Timer::Flag{game_id,color,ply}` est planifié au début de chaque tour ;
s'il se déclenche encore au même `ply`, la partie se termine par `Outcome::Timeout {winner}`.

### Nulle, chat, revanche
- `offer_draw {}` → l'adversaire reçoit `draw_offered {}` ; `respond_draw {accept}` ; accepté → `Outcome::DrawAgreed`. Refus → `draw_declined {}` à l'offrant.
  Une offre expire si un coup est joué. Une seule offre active par joueur et par partie.
- `chat {text}` (≤ 140 caractères, nettoyé, 1 message/seconde max) → l'adversaire reçoit `chat {text, mine: false}` ; l'émetteur reçoit son écho `chat {text, mine: true}`.
- Après la fin d'une partie : `rematch_request {}` → l'adversaire reçoit `rematch_offered {}` ; `rematch_respond {accept}` ;
  accepté → nouvelle partie (couleurs inversées, nouveau `deck_select`, même classement/amical). Refus ou départ → `rematch_declined {}`.
  Disponible tant que les deux joueurs sont connectés et libres.

### Moteur
`Outcome` gagne `Timeout {winner}` et `DrawAgreed`. `winner()` couvre `Timeout`.

### Messages enrichis
- `welcome` gagne `account: Me`.
- `deck_select` gagne `opponent: OpponentInfo`, `rated: bool`.
- `state` (StateView) gagne :
  `clock: {white_ms, black_ms, running: "white"|"black"|null}` (temps restant au moment de l'envoi — le client interpole),
  `rated: bool`, `opponent: OpponentInfo`, `draw_offer: "none"|"you"|"them"`, `ply_count`.
- `OpponentInfo = {username: string|null, elo: int|null, guest: bool}`.
- `game_over` gagne `rated: bool`, `elo: {you_before, you_after, opp_before, opp_after}|null`, `reason: string`
  (`checkmate`, `resignation`, `timeout`, `agreed_draw`, `stalemate`, `fifty_moves`, `repetition`, `insufficient_material`, `disconnect`).
- `lobby.status` voir §3.

## 6. Client (design B « Graphite »)

Référence visuelle : les maquettes `B-*` du canvas (voir aussi l'historique de la conversation).
- Jetons : fond `#0e0f12`, surfaces `#15171b`/`#1b1e23`, filets `#262a31`/`#2f343c`/`#3b414b`, texte `#e9ebef`, atténué `#8b919c`,
  accent unique `#8fb4ff` (focus) ; bouton principal blanc cassé plein ; **aucune pastille verte** ; sélection = bord `#e9ebef` + liseré de la couleur de famille.
  Familles : attaque `#ee8272`, défense `#5fd0a0`, mobilité `#7aa2ff`, contrôle `#b79cff`, création `#eec06a`.
- Polices : Geist + Geist Mono, **auto-hébergées** via `@fontsource-variable/geist` et `@fontsource-variable/geist-mono`.
- Illustrations : un sprite SVG de 27 symboles `sk-<id>` (source : maquette B-Arena), composant `<SkillArt id="…"/>`.
- Routage par hash : `#/` accueil · `#/auth` · `#/ranking` · `#/friends` · `#/profile/<pseudo>` · `#/collection`.
  Les écrans de deck et de partie remplacent la page courante tant qu'une partie est en cours.
- Textes en français.

## 7. Notes d'implémentation (serveur)

Précisions et écarts par rapport au contrat ci-dessus, tels que livrés dans `crates/`.

### Comptes et sessions
- `register` avec un `guest_token` valide **réutilise ce même jeton** (la session de l'invité devient celle du compte) ; sans lui, une nouvelle session est créée. Un `guest_token` inconnu, ou déjà rattaché à un compte, est ignoré (un nouveau compte est créé, rien n'est promu). Un échec (`username_taken`…) laisse l'invité intact.
- Après une promotion, le serveur pousse `friends` à la connexion WS déjà ouverte de l'invité, mais **ne renvoie pas `welcome`** : le client rafraîchit `account` via `GET /api/me`. Les messages sociaux sont vérifiés en base à chaque message, donc ils fonctionnent aussitôt.
- `logout` supprime la session mais ne coupe pas une WebSocket déjà ouverte avec ce jeton (corrigé en v4 : voir spec-v4, « Sessions »).
- Erreurs REST : toutes en `{error}` ; en plus du contrat, `400 bad_request` (JSON invalide, paramètre de requête invalide), `413 payload_too_large` (corps > 4 Kio), `401 unauthorized` (Bearer absent/inconnu), `404 not_found` (profil). Mot de passe : 8 à 128 **caractères**. Hash argon2id (crate `argon2` 0.6, paramètres par défaut), calculé hors des workers async ; un pseudo inconnu vérifie un hash factice (même durée, même réponse).
- Horodatages : ISO 8601 UTC avec `Z` (`2026-10-06T20:12:07Z`) partout (`created_at`, `history[].at`, `recent[].at`, `last_seen`).
- Classement : départage `elo DESC, wins DESC`, puis pseudo en minuscules. `streak` ne compte que les parties **classées** ; une nulle le remet à 0. `history` : les 30 derniers points ; le point initial 1200 n'est présent que s'il y a moins de 30 points. `recent` contient toutes les parties (classées ou non), `elo_delta` est `null` hors classé, `opponent` est `null` si l'adversaire était invité.
- Migrations : `PRAGMA user_version` (1 = schéma d'origine, 2 = comptes/sessions/Elo/amis). `players` est reconstruite pour supprimer la colonne `token` ; les anciens jetons deviennent des sessions.

### Récupération de compte
Pas de SMTP, donc pas d'e-mail : un mot de passe oublié se récupère avec un **code de récupération**, montré une seule fois.
- **Code** : 20 symboles tirés au hasard (CSPRNG) dans un alphabet de 32 sans `0 O 1 I` (5 bits chacun, donc 100 bits), écrits en 4 groupes de 5 : `K7QF2-M9XWB-3HNRA-TD8LC`. À la vérification, tirets et espaces sont ignorés et les minuscules acceptées. Seul son **hash argon2id** (mêmes paramètres et mêmes fonctions que les mots de passe) est gardé, dans la table `recovery_codes(player_id PK → players, code_hash, created_at)` ; la dernière entrée de `MIGRATIONS` la crée (`CREATE TABLE IF NOT EXISTS`, jamais d'`ALTER TABLE ADD COLUMN`). Un compte antérieur n'a pas de ligne tant qu'il n'a pas demandé de code.
- **Où il est donné** : `register` (écrit dans la même transaction que le compte, y compris pour un invité promu), `recover` (le code est remplacé) et `POST /api/me/recovery-code`. Il n'est ni relu ni renvoyé ailleurs, ni journalisé (les corps de requête ne sont jamais loggués et les requêtes qui le portent n'implémentent pas `Debug`).
- **`recover`** : dans l'ordre, verrouillage (429) → validation du nouveau mot de passe (400, avant toute lecture du code : la réponse ne dépend pas du code et un refus ne le consomme pas) → une vérification argon2 (contre le hash factice si le pseudo est inconnu, si c'est un invité ou si le compte n'a pas de code : même durée, même réponse `401 bad_recovery`) → une seule transaction : remplacement du hash du code **seulement s'il est toujours celui qui a été vérifié** (deux requêtes simultanées ne peuvent pas consommer le même code), nouveau hash du mot de passe, suppression de toutes les sessions. Ensuite `App::session_revoked` pour chacun des jetons supprimés (les WebSocket ouvertes reçoivent `session_revoked`, comme `logout-all`), puis création d'une session neuve. Décision : cette session est renvoyée (`token`), parce que détenir le code vaut déjà une connexion et que cela évite une seconde saisie ; elle est créée **après** la révocation. Les échecs de connexion du pseudo sont effacés (un compte verrouillé en connexion doit pouvoir se récupérer).
- **Limitation des essais** (REST n'a pas de limiteur : `limits::FailureWindow`, fenêtre fixe qui s'ouvre au premier échec) : `recover` tient un compteur **par pseudo** (5 échecs / 15 min, les pseudos inconnus comptent comme les autres) et, **si on le demande**, un compteur **par adresse** (`HubConfig::recovery_max_failures_per_ip` / `CHESSY_RECOVERY_MAX_FAILURES_PER_IP`, entier, `0` = désactivé, **valeur par défaut** ; valeur illisible ignorée avec un avertissement ; fenêtre de 15 min, IPv6 par /64 ; adresse = pair TCP, ou entrée la plus à droite de `X-Forwarded-For` avec `CHESSY_TRUST_PROXY`, comme pour les connexions WebSocket). Même raison que `max_connections_per_ip` (spec-v4, « Connexions simultanées ») : derrière un reverse proxy (Docker) le pair TCP est le proxy, donc un plafond actif par défaut bloquerait la récupération de **tous** les comptes dès 20 échecs de n'importe qui ; à n'activer qu'avec `CHESSY_TRUST_PROXY=1` (ou en direct). Sans adresse connue (tests d'intégration sans `ConnectInfo`, sans en-tête de proxy) ce compteur ne joue pas ; les tests l'exercent via `X-Forwarded-For` avec `trust_proxy`. Un compteur à part de celui de la connexion : celui qui a oublié son mot de passe a sans doute verrouillé sa connexion. Un succès efface le compteur du pseudo, jamais celui de l'adresse. À 100 bits, 5 essais par quart d'heure rendent une devinette sans espoir ; le prix est qu'un tiers peut bloquer la récupération d'un pseudo pendant 15 minutes (il ne peut rien d'autre).
- **`POST /api/me/recovery-code`** exige le mot de passe actuel, et un mauvais mot de passe compte dans le **verrouillage de connexion** du compte (8 / 5 min), sinon un jeton de session volé servirait d'oracle pour deviner le mot de passe. Un invité n'a pas de mot de passe : `401 bad_credentials`.
- Client : le code est montré à l'inscription et après une récupération dans le panneau de `Auth` (copie, case « J'ai noté mon code de récupération » obligatoire ; la session n'est appliquée qu'après), « Mot de passe oublié ? » sous le mot de passe de la connexion, et « Générer un code de récupération » dans Profil → Réglages.
- Tests : `crates/chessy-server/tests/recovery.rs` et le test unitaire de `limits.rs`.

### Matchmaking
- `queue_join {ranked?}` : `ranked` par défaut `true` pour un compte, forcé à `false` pour un invité (il entre alors dans la file amicale ; `lobby.status.ranked` vaut `false`).
- Appariement classé : parmi toutes les paires dont l'écart est ≤ `100 + 25 × s` (s = attente du plus ancien des deux, plafond 800), le serveur apparie **la paire d'écart minimal** (égalité : la plus ancienne). Il s'exécute à chaque arrivée et à chaque `QueueSweep`. Tous les seuils sont dans `HubConfig` (`ranked_range_base`, `ranked_range_per_second`, `ranked_range_max`, `queue_sweep_interval`).
- Quitter la file, se déconnecter ou lancer une autre recherche retire le joueur de sa file.

### Horloges, nulle, chat, revanche
- `HubConfig` : `clock_initial` (10 min), `clock_increment` (3 s), `challenge_ttl` (60 s), `chat_interval` (1 s). L'incrément s'ajoute après chaque action, y compris la première. Un coup reçu après expiration du temps (timer en retard) termine aussi la partie par `Timeout`. `state.clock` est à 0/0, `running: null` une fois la partie finie.
- Nulle : une offre par joueur **et par ply** (`draw_already_offered` si l'on re-propose sans qu'un coup ait été joué) ; proposer alors que l'adversaire a une offre ouverte vaut acceptation ; `draw_pending` si sa propre offre est déjà ouverte. Une offre disparaît dès qu'une action est jouée (`state.draw_offer` repasse à `none`). Les offres ne déclenchent pas de `state`, seulement `draw_offered` / `draw_declined` ; `state.draw_offer` sert à la reprise après reconnexion.
- Chat : autorisé pour tout joueur (invités compris) **pendant une partie** (y compris le choix de deck), sinon `not_in_game`. Texte nettoyé (retours ligne/tabulations → espace, autres caractères de contrôle supprimés, trim) ; vide ou > 140 caractères (pas octets) → `invalid_message` (refusé, pas tronqué) ; trop rapide → `rate_limited`.
- Revanche : disponible pour les deux joueurs après `game_over` (invités compris). Si l'adversaire a déjà demandé, `rematch_request` vaut acceptation. Elle disparaît (et l'autre reçoit `rematch_declined`) si l'un se déconnecte, lance une file/salle ou une autre partie. Même type (classé/amical) que la partie précédente. Comme toute nouvelle partie, elle **abandonne la récompense non réclamée** du gagnant. Une revanche d'une partie classée reste classée tant que le plafond de parties classées entre les deux comptes n'est pas atteint (sinon `rated:false`, voir `docs/spec-v4.md`).
- `game_over.reason` : `checkmate`, `resignation`, `timeout`, `agreed_draw`, `stalemate`, `fifty_moves`, `repetition`, `insufficient_material`, `disconnect` (forfait après le délai de reconnexion ; l'`outcome` reste `resignation`). `game_over.rated` est `false` si la partie a duré moins de 4 plies même issue de la file classée ; `state.rated` / `deck_select.rated` indiquent la file d'origine.

### Social (WebSocket)
- `friends` n'est envoyé qu'aux comptes. `last_seen` n'est renseigné que pour `presence: "offline"` (sinon `null`). `friends` est aussi renvoyé à la fin d'une partie (Elo mis à jour).
- Cibles introuvables ou soi-même : `notice user_not_found` (pour `friend_request`, `friend_remove`, `challenge`). `friend_respond` sans demande en attente : `error no_such_request`. `friend_request` croisées : amitié immédiate (l'autre reçoit `friend_accepted`). Retirer une demande sortante en attente est possible via `friend_remove` (pas de notice pour l'autre).
- Défis : cible non amie → `error not_friends` ; hors ligne → `notice friend_offline` ; en partie → `notice friend_busy`. Un nouveau défi remplace l'ancien (`challenge_cancelled` à l'ancienne cible) ; défier celui qui vous a défié vaut acceptation. Réponse sans défi (expiré, annulé, déjà consommé) → `error no_challenge`. À l'expiration, les deux reçoivent `notice challenge_expired` (`username` = l'autre partie) ; si le défié part en partie ou se déconnecte, le défieur reçoit `challenge_expired` ; si le défieur part, le défié reçoit `challenge_cancelled`.
- Autres codes d'`error` : `account_required`, `already_in_game`, `not_in_game`, `wrong_phase`, `no_draw_offer`, `no_rematch`.
