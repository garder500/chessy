# Chessy v3 — les 20 compétences restantes et le mode Solo

Complète `docs/spec-v2.md`. Les noms de champs JSON sont en `snake_case`. Le texte d'origine des
compétences est dans `docs/skills.md` (source de vérité quand ce document ne tranche pas).

## 1. Règles communes

- Une compétence s'utilise **une fois par partie** (Mind Reading : 3 fois) et **consomme le tour**,
  sauf Mind Reading et Mind Control (voir plus bas, `ends_turn = false`).
- Une compétence est refusée si, après elle, **le roi de l'utilisateur est en échec**. Les compétences marquées
  `forbids_mate` sont aussi refusées si elles mettent l'adversaire échec et mat ; `forbids_check`, si l'un des deux
  rois est en échec après elles.
- Toutes les durées sont en *plies* (actions), `expires_at = ply_de_lancement + n` (voir `Position::add_effect`).
  Un effet « actif » pendant `ply < expires_at`.
- Un pion ne peut jamais se retrouver sur la 1re ou la 8e rangée par une compétence, sauf Tornado (promotion en dame).
- **Mat avec compétences** : un camp n'est mat que s'il n'a aucune action qui le sauve. Les compétences qui ne
  consomment pas le tour (Mind Reading, Mind Control) ne comptent comme « sauvantes » que si, après les avoir jouées,
  il existe au moins un coup légal ; Mind Reading ne sauve jamais.
- Tout nouveau type respecte la pureté du moteur (aucune E/S, déterminisme : le « hasard » est une fonction du `ply` et
  de la position, jamais de `rand`).

## 2. Les 20 compétences

Cibles : `Piece{square}`, `Square{square}` (case vide), `Pair{a,b}`, `Spawn{square, kind}`, `None`.
Familles : A attaque, D défense, M mobilité, C contrôle, X création.

### Uniques
| Id | Fam. | Cible | Règle |
|---|---|---|---|
| `wall` | D | `None` | Ramène jusqu'à 3 pions du camp (autant que de pions capturés : `Position.captured_pawns[color]`, décrémenté) sur des cases vides, par priorité la 3e rangée du camp (6e pour les noirs) puis la 2e, colonnes les plus proches du roi d'abord. Chaque pion est `wall = true` (un pion mur qui meurt n'est pas compté dans `captured_pawns`) et reçoit un effet `locked` jusqu'au prochain tour du joueur (`expires_at = ply + 2`) : il ne peut pas bouger. Refusée s'il n'y a aucun pion capturé ou aucune case. |
| `mirage` | X | `Spawn{square, kind}` | `square` vide, `kind` ∈ {knight, bishop, rook, queen}. Crée une pièce `mirage = true` : elle **ne capture jamais** et **n'attaque pas** (ne donne pas échec, n'est pas comptée par `is_attacked`), et **disparaît** quand elle est prise (pas de cimetière). Dure jusqu'à capture. |
| `evolve` | X | `Piece{square}` | Une pièce alliée qui n'est ni roi ni dame devient une dame (même id). |
| `switch` | A | `Piece{square}` | Pièce adverse (pas le roi) qui n'attaque **aucune** pièce du joueur et ne donne pas échec ; elle change de camp définitivement (ses effets sont retirés). `forbids_mate`. |
| `mind` | X | `None` | Ne consomme pas le tour. 3 usages. Émet `Event::BestMove{from,to,promo?}` = meilleur coup du camp au trait calculé par `search::best_move(pos, 3)`. Ne change pas la position. |
| `control` | C | `Piece{square}` | Ne consomme pas le tour. Pièce adverse (pas le roi) : elle **change de couleur jusqu'à la fin du tour du joueur** (effet `color_loan` avec `orig_color`, `expires_at = ply + 1`), le joueur peut donc la déplacer comme la sienne ; elle reprend sa couleur ensuite. |

### Classiques
| Id | Fam. | Cible | Règle |
|---|---|---|---|
| `morph` | C | `Spawn{square, kind}` | `square` porte une pièce non-roi ; `kind` ∈ {pawn, knight, bishop, rook, queen} différent du type actuel (pas pion si la pièce est sur une rangée de fond). La pièce change de type : effet `morphed` avec `orig_kind`, `expires_at = ply + 2` si ennemie, `ply + 4` si alliée ; le type d'origine est rétabli à l'expiration. |
| `canceller` | C | `None` | Utilisable seulement si la **dernière action de l'adversaire était une compétence**. Restaure la position d'avant cette compétence (`Position.last_skill_snapshot`, voir §3) en décalant les expirations d'effets ; la compétence adverse reste consommée ; le tour passe. Événement `Event::Cancelled{skill}`. |
| `tornado` | M | `None` | Toutes les pièces sauf les rois tournent : on trie les pièces non-rois par angle autour du centre (puis par case) et chacune prend la case de la suivante (cycle). Un pion qui atterrit sur une rangée de fond est promu en dame. Événement `Event::Rotated{moves}`. |
| `invisibility` | D | `Piece{square}` | Pièce alliée non-roi : effet `invisible`, `expires_at = ply + 4`. Le serveur la cache à l'adversaire. |
| `terminator` | A | `Piece{square}` | Pièce adverse non-roi dont la case miroir (même colonne, rangée `7 − r`) est vide : une copie (`temp = true`, couleur du joueur, même type) apparaît sur la case miroir, effet `vanish` `expires_at = ply + 3` (la copie sert pendant le prochain tour du joueur puis disparaît). |
| `destiny_swapper` | M | `Pair{a,b}` | (déjà fait) |
| `trap` | C | `Square{square}` | Case vide : pose un piège `{square, owner}` (≤ 2 par joueur). Quand une pièce **adverse** parcourt une case piégée (tout le trajet d'un coup de glisseur, ou la case d'arrivée), elle **s'arrête sur la case piégée** (le coup est tronqué, pas de capture au-delà), reçoit `frozen` 2 tours (`expires_at = ply + 4`) et le piège est consommé. Les pièces ne peuvent pas être posées sur une case piégée par une compétence. Piège caché à l'adversaire. |
| `bench` | D | `Piece{square}` | Pièce alliée non-roi retirée de l'échiquier ; elle revient à `ply + 2` sur la case libre la plus proche de son ancienne case (distance de roi, puis ordre des cases), hors terrain bloqué. Stockée dans `Position.benched`. |
| `forcefield` | D | `Piece{square}` | Pièce alliée non-roi : effet `forcefield` sans expiration. Quand elle est capturée, elle va au cimetière et le capteur est **repoussé** de 2 cases au plus dans la direction d'où il vient (s'arrête devant un obstacle ou le bord). `Event::Pushed{piece, from, to}`. |
| `transposition` | M | `Pair{a,b}` | Deux pièces quelconques (n'importe quel camp) échangent leur case. `forbids_check` (aucun roi en échec après). |
| `queensac` | A | `None` | Utilisable quand le roi du joueur **est en échec** et qu'il a une dame : dame et roi échangent leur case, puis la dame **meurt** sur l'ancienne case du roi (comptée comme capturée) ; le roi doit être en sécurité à l'arrivée. |
| `temporal` | M | `Piece{square}` | Pièce alliée non-roi ayant un `prev` : elle **rejoue son dernier déplacement** (même vecteur `sq − prev`) depuis sa case actuelle si la case d'arrivée est dans l'échiquier et vide ou occupée par une pièce adverse non-roi non immune (capture), **en respectant les règles d'un vrai coup** : un glisseur (ou le double pas d'un pion) ne saute aucune pièce ni terrain adverse, un pion ne prend qu'en diagonale et n'avance tout droit que sur une case vide, et un piège adverse sur le trajet l'arrête et le gèle. Remplace le coup du tour. |
| `geomancy` | X | `Square{square}` | Case vide `s` : `s` et ses voisines de même rangée qui sont vides deviennent des **terrains** (effet `terrain` avec `square`, `owner` = joueur, `expires_at = ply + 6`). Les pièces **adverses** ne peuvent ni s'y arrêter ni les traverser (les cavaliers sautent mais ne s'y posent pas). |
| `celestial` | D | `Piece{square}` | Pièce alliée non-roi : effet `celestial` (une fois). Si elle est capturée : le capteur occupe bien la case, mais la pièce est **replacée sur sa case de départ** (`home`, ou la plus proche libre) et l'effet est consommé. `Event::Saved{piece, from, to}`. |
| `godhelp` | X | `None` | Une pièce (`temp = true`) du joueur apparaît sur une case vide choisie de façon déterministe parmi les rangées 3 à 6 (index 2..=5), de type knight/bishop/rook/queen déterministe ; effet `vanish` `expires_at = ply + 7`. |

## 3. Modèle de données du moteur

- `Piece` gagne `home: Square`, `mirage: bool`, `wall: bool`, `temp: bool` (les trois booléens sont sérialisés seulement s'ils sont vrais ; `#[serde(default)]`).
- `EffectKind` gagne : `invisible`, `forcefield`, `celestial`, `locked`, `morphed`, `color_loan`, `vanish`, `terrain`.
  `ActiveEffect` gagne `#[serde(skip_serializing_if = "Option::is_none")]` `square`, `owner` (terrain), `orig_kind`, `orig_color` (morph / color_loan).
- `Position` gagne : `traps: Vec<Trap{square, owner}>`, `benched: Vec<BenchedPiece{piece, square, back_at}>`,
  `captured_pawns: [u8; 2]`, `last_skill_snapshot: Option<Box<Snapshot>>` (clone de `Position` sans snapshot, pris juste avant une compétence, effacé à toute autre action).
- `SkillTarget` gagne `Square{square}` et `Spawn{square, kind}`.
- `Skill` gagne `ends_turn() -> bool` (défaut `true`), `forbids_check() -> bool`, `max_uses() -> u8` (défaut 1) ; `SkillSlot` gagne `uses: u8` (`used` reste vrai quand `uses >= max_uses`).
- Nouveaux événements (`type` snake_case) : `spawned{square, piece}`, `transformed{square, kind}`, `switched{square, piece}`, `rotated{moves:[{from,to}]}`,
  `trap_set{square}`, `trap_sprung{square, piece}`, `benched{square, piece}`, `unbenched{square, piece}`, `pushed{piece, from, to}`,
  `saved{piece, from, to}`, `best_move{from, to, promo?}`, `cancelled{skill}`, `terrain{squares}`, `vanished{square, piece}`, `loan_ended{square, piece}`.
- Le comportement des anciennes compétences ne change pas ; les perft restent identiques (aucune compétence active).
- `search.rs` (déjà présent) : `best_move`, `score_moves`, `evaluate`. Le moteur reste sans dépendance à `rand`.

## 4. Serveur : informations cachées et vues

- `state.board` (vue de l'adversaire) : les pièces `invisible` du joueur adverse sont `null`. Le propriétaire les voit et ses `effects` incluent leurs effets ; la vue adverse retire les effets `invisible`.
- Nouveaux champs de `state` : `traps: Square[]` (uniquement les pièges **du joueur qui regarde**), `benched: Piece[]` (les siennes), `terrain: {square, owner, expires_at}[]`,
  `skill_options` inclut déjà les cibles ; `mind_uses_left` n'est pas nécessaire (`my_skills[].uses` suffit : ajoute `uses` et `max_uses` à `SkillSlot` dans la vue).
- Les événements qui révèlent une information cachée (apparition d'une case piégée, pièce invisible qui bouge) sont filtrés pour l'adversaire : un coup d'une pièce invisible produit pour l'adversaire un événement sans `piece`/`from` (il voit seulement l'arrivée si la case est visible).
  Simplification acceptée : l'adversaire voit « un coup a été joué » via le changement de `ply`.
- `Welcome`/`catalog` : aucune autre modification.

## 4 bis. Notes d'implémentation (compétences : moteur et serveur)

Précisions et écarts par rapport aux sections 1 à 4, tels que livrés dans `crates/`.

### Formats pour le client
- **Cibles** (`SkillTarget`, tag `kind`) : `{"kind":"none"}`, `{"kind":"piece","square":n}`, `{"kind":"square","square":n}`, `{"kind":"pair","a":n,"b":n}` (a < b), `{"kind":"piece_to","from":n,"to":n}` et **`{"kind":"spawn","square":n,"piece":"knight"}`** : le type de pièce s'appelle `piece` dans le JSON (et non `kind`, déjà pris par le tag ; côté Rust le champ reste `kind`). Cibles par compétence : `wall`, `mind`, `canceller`, `tornado`, `queensac`, `godhelp` → `none` ; `mirage` → `spawn` (cases vides × {knight, bishop, rook, queen}) ; `morph` → `spawn` (case d'une pièce non-roi × type différent) ; `trap`, `geomancy` → `square` ; `transposition` → `pair` ; toutes les autres → `piece`. `skill_options[].targets` ne contient que des actions légales (échec, mat, case libre... déjà vérifiés).
- **Vue `state`** : `my_skills[] = {skill, used, uses, max_uses}` ; `traps: number[]` (les siens) ; `benched: Piece[]` (les siennes) ; `terrain: [{square, owner, expires_at}]` ; `effects` ne contient **pas** les terrains (ils sont dans `terrain`) ni les effets des pièces cachées. `Piece` JSON : `{id, kind, color, home, mirage?, wall?, temp?}` (`home` toujours présent, les booléens seulement s'ils sont vrais). `ActiveEffect` : `{kind, piece, expires_at, square?, owner?, orig_kind?, orig_color?}` ; les effets sans fin (`forcefield`, `celestial`) ont `expires_at = 4294967295`.
- **Événements** : `piece` est un `Piece` complet pour `spawned`, `switched`, `benched`, `unbenched`, `vanished`, `loan_ended`, `captured` ; c'est un `PieceId` (nombre) pour `moved`, `trap_sprung`, `pushed`, `saved`, `effect_added`. `rotated.moves = [{from, to}]` dans l'ordre de rotation (ordre de lecture des cases en sens antihoraire). Un `captured` n'est **pas** émis quand Celestial sauve la pièce (seulement `saved`). Quand un piège arrête un glisseur, `moved.to` est la case piégée (le coup joué peut donc finir avant la case demandée), suivi de `trap_sprung` puis `effect_added{frozen}`.
- **Tours** : `mind` et `control` laissent le trait au même joueur (`state.to_move`/`ply` inchangés, un nouveau `state` est envoyé, l'horloge continue sans incrément, aucune offre de nulle annulée). Mind n'apparaît pas dans `opponent_skills.used` tant qu'il n'est pas épuisé (3 usages).

### Règles précisées
- **Compétences sans fin de tour** : refusées si le joueur n'a ensuite aucun coup légal ; Control est aussi refusée si le roi adverse se retrouve en échec (la pièce prêtée pourrait le prendre). Mind exige au moins un coup légal (jamais utilisable en mat). Pas d'enregistrement de position (répétition) pour une action qui ne change pas le `ply`.
- **Durées** : toutes selon le spec, sauf le gel d'un piège : le glisseur piégé joue au `ply` p, l'effet `frozen` expire à `p + 5` (soit deux de **ses** tours, `p + 2` et `p + 4` ; `p + 4` n'en aurait gelé qu'un). Un effet `locked` (Wall) fige comme `frozen` : `Position::is_frozen` couvre les deux (la pièce ne bouge pas, n'attaque rien, n'est pas cible de Rollback/Teleportation/Destiny Swapper/Temporal).
- **Pièges** : le coup est tronqué dans `make_move` (donc dans la légalité, pour les humains comme pour la recherche) ; sont concernés tout le trajet d'un fou/tour/dame et d'un double pas de pion, sinon seulement la case d'arrivée (cavalier, roi, roque, pas simple, prise). Une promotion est annulée si le coup est tronqué. Les pièges (et le terrain adverse) interdisent la pose de pièces par compétence (`Position::can_place`), y compris Teleportation, Clone, Rollback, retour du banc, Celestial, Wall, Mirage, Terminator, Godhelp.
- **Terrain (Geomancy)** : cases `s`, `s-1`, `s+1` (même rangée) vides et pas déjà du terrain ; effet `{kind: terrain, piece: 65535, square, owner}`. Une pièce adverse ne peut ni s'y arrêter ni les traverser (cavaliers : seulement s'y poser) ; elles ne sont pas non plus « attaquables » par l'adversaire (un roi sur son propre terrain ne peut pas être mis en échec) et les rayons d'attaque s'arrêtent devant. Un terrain peut donc annuler un échec (légal).
- **Capture** (`begin_capture`/`finish_capture`) : Celestial d'abord (pas de `captured`, pas de cimetière, ni Force Field), puis Force Field ; le cimetière compte le type d'origine d'une pièce morphée ; un pion `wall`, `mirage` ou `temp` n'est jamais compté ; `Remover` ne compte pas. Les effets d'une pièce morte sont supprimés. Un Force Field qui repousse refuse la capture si le roi du capturant est alors en échec (la légalité est évaluée après la poussée).
- **Morph** : sur une pièce déjà morphée on garde le type d'origine ; si le type d'origine est pion et que la case est une rangée de fond à l'expiration, la pièce devient dame. Evolve annule un Morph en cours. **Bench** : refusé pour une pièce portant `vanish`, `morphed` ou `color_loan` ; la pièce revient à la fin de l'action qui atteint `back_at` (`ply + 2`), sur la case libre la plus proche (distance de roi puis ordre des cases) hors pièges/terrain ; une pièce sur le banc compte comme matériel (pas de nulle « roi contre roi »).
- **Switch** : retire tous les effets de la pièce sauf `vanish` ; `forbids_mate`. **Terminator** : la copie hérite de `mirage` mais pas des effets ; **Godhelp** : graine FNV-1a de (`ply`, trait, pièces), case parmi les cases libres des rangées 3 à 6 (index 2..=5), type parmi {cavalier, fou, tour, dame}.
- **Canceller** : `Snapshot {position, skill}` (le `skill` annulé donne `cancelled.skill`) ; il est posé par toute compétence qui passe le tour, effacé par un coup et par Mind Control, conservé par Mind Reading. Les expirations sont décalées du nombre de plies écoulés depuis le snapshot (durées restantes préservées) ; les prêts de Control encore présents dans le snapshot sont refermés. Annuler un Canceller est possible (le snapshot du Canceller est celui d'avant lui).
- **Queen Sacrifice** : s'il y a plusieurs dames, la première (par case) dont le sacrifice met le roi en sécurité ; sinon la première. **Temporal** : `prev` est mis à jour comme un vrai coup ; la prise passe par le même chemin que `make_move` (Force Field, Celestial, cimetière).
- **Mat avec compétences** : calculé par `Game` : aucun coup légal **et** aucune action de compétence légale (`Position::skill_is_legal`).
- **Perf** : les `Vec` ajoutés à `Position` sont vides hors compétences ; tous les chemins chauds testent `effects`/`traps`/`benched` vides. `Cargo.toml` (racine) compile le crate `chessy-engine` en `opt-level = 3` même en profil dev/test (checks d'overflow conservés), car chaque compétence est simulée pour lister les actions légales.

### Serveur
- `state_view` délègue le filtrage à `crates/chessy-server/src/hub/view.rs` : cases des pièces invisibles adverses à `null` ; leurs effets (tous) retirés ; `skill_used` de `trap`/`invisibility` (ou visant une case cachée) passe à `target: {"kind":"none"}` pour l'adversaire ; `best_move`, `trap_set` et `effect_added{invisible}` ne sont envoyés qu'à l'auteur ; les événements portant sur une pièce cachée (`moved`, `pushed`, `saved`, `spawned`, `benched`, `teleported`, `swapped`... ) sont supprimés, les `rotated.moves` qui l'arrivent sont filtrés. Les pièges qui se déclenchent (`trap_sprung`) sont publics.
- La pioche de départ d'un nouveau joueur reste « 3 classiques distinctes tirées au hasard » ; le pool classique compte désormais **20** compétences (6 existantes + 14 nouvelles), pas 14 ; 7 uniques (`remover`, `wall`, `mirage`, `evolve`, `switch`, `mind`, `control`). Le tirage aléatoire de récompense parcourt `SkillId::ALL` (uniques seulement s'ils n'ont pas de propriétaire).

## 5. Mode Solo (IA)

### Protocole
- Client → serveur : `solo_start {elo: int (400..=2800), color: "white"|"black"|"random"}`. Autorisé aux invités et aux comptes. Refusé si le joueur est déjà en partie (`already_in_game`).
- Le flux est ensuite le même qu'un duel : `deck_select` (le bot a déjà choisi : 3 compétences classiques jouables au hasard + aucune unique), puis `state`, `game_over`.
  `OpponentInfo` gagne `bot: bool` (`username` = `"Sage"`, `elo` = niveau choisi, `guest = true`, `bot = true`). Les parties solo sont **amicales** (`rated=false`), **sans horloge** (`state.clock_enabled = false`, `clock.running = null`), **sans récompense** de compétence.
- Revanche contre le bot : `rematch_request` est accepté automatiquement (nouveau `solo` avec les mêmes réglages, couleurs inversées). Offre de nulle : le bot accepte seulement si l'évaluation est entre −30 et +30 centipions et que `ply_count ≥ 40`, sinon `draw_declined`. Le chat est ignoré.
- Le bot joue avec un délai de 600–1400 ms (plus long si la recherche a pris du temps) ; la recherche tourne dans `tokio::task::spawn_blocking` (le `Hub` ne bloque jamais). Si la partie a changé entre-temps (`ply` différent), le résultat est jeté. Le bot abandonne jamais ; il se retire si le joueur est déconnecté plus de 60 s (comme un duel).

### Niveaux
Le niveau `elo` règle : profondeur de recherche, bruit d'évaluation, probabilité de coup raté et d'usage des compétences.

| Elo | Profondeur | Comportement |
|---|---|---|
| 400–799 | 1 | choisit parmi les 8 meilleurs coups avec très forte dispersion, 55–30 % de coups quasi aléatoires |
| 800–1199 | 2 | dispersion moyenne, 10–4 % de coups ratés |
| 1200–1599 | 3 | faible dispersion, 3–1 % |
| 1600–1999 | 4 | quasi déterministe |
| 2000–2399 | 5 | déterministe, budget ≈ 1,5 s |
| 2400–2800 | 6 | déterministe, budget ≈ 3 s, évaluation complète |

Interpoler linéairement à l'intérieur d'une tranche. L'IA est **déterministe à graine donnée** (graine = id de partie + ply) pour pouvoir tester.
Usage des compétences : à chaque tour l'IA évalue chaque action `Skill` légale (résultat simulé, évalué après un coup de réponse adverse peu profond) et la joue si elle améliore l'évaluation d'au moins `T` centipions
(`T` décroît avec l'Elo : 250 à 400, 60 à 2800), avec une probabilité d'usage croissante avec l'Elo ; elle ne joue jamais Mind Reading/Mind Control (inutiles pour elle).

### Notes d'implémentation (Solo)

Précisions et écarts, tels que livrés dans `crates/`.

**Protocole**
- `solo_start {elo, color}` : `elo` est lu comme entier signé ; hors 400..=2800 → `error invalid_elo`. `color` est facultatif (défaut `"random"`). `already_in_game` si le joueur est en partie **ou** en file/salle d'attente (le client doit d'abord faire `leave_lobby`). Un éventuel défi ouvert, une revanche ou une récompense non réclamée sont abandonnés comme pour toute nouvelle partie.
- `OpponentInfo.bot` n'est sérialisé que lorsqu'il vaut `true` (absent = `false`), pour ne pas changer la forme des messages face à un humain. Face au bot : `{"username":"Sage","elo":<niveau>,"guest":true,"bot":true}` dans `deck_select.opponent` et `state.opponent`.
- `StateView.clock_enabled: bool` (toujours présent) : `false` en solo, avec `clock = {white_ms:0, black_ms:0, running:null}`. Aucun timer `Flag` n'est créé.
- `deck_select` est envoyé comme pour un duel ; le bot a déjà choisi (`opponent_skills.total = 3` dès le premier `state`, aucune unique). Ses 3 compétences sont tirées au hasard parmi `SkillId::ALL` filtrées sur `kind() == Classic` (donc les 6 classiques actuelles, les 15 futures aussi), sans doublon.
- `game_over` : `rated:false`, `elo:null`, `reward:null`. Pas d'effet sur l'Elo, les compteurs, `rating_history`, le classement.

**Structure serveur**
- Pas d'`enum Seat` : `Session.players` reste `[PlayerId; 2]` ; le siège du bot porte un identifiant synthétique `bot:<game_id>` qui n'est jamais dans `conns`, `player_game` ni en base (les `send` vers lui sont des no-ops, aucune requête ne le trouve). Ce qui rend la partie « solo » est `Session.solo: Option<Solo{bot: Color, elo}>` (`hub/solo.rs`). `Hub::start_session` délègue à `open_session(.., solo)`.
- `HubConfig` gagne `bot_delay_min/max` (600/1400 ms), `bot_think_max` (3 s), `bot_draw_min_plies` (40), `bot_draw_window` (30 cp).
- **Les parties solo ne sont pas enregistrées** : aucune ligne dans `games` (la table référence des joueurs, et l'historique public `recent` n'est pas pollué). La revanche et l'état de partie vivent uniquement en mémoire, comme le reste du hub.

**Tour du bot**
- Après chaque action qui lui donne la main (et au démarrage s'il a les blancs), le hub planifie `Timer::BotMove{game_id, ply}` avec un délai aléatoire dans `[bot_delay_min, bot_delay_max]`. `App::fire` l'intercepte : instantané du `Game` sous le verrou (`Hub::bot_job`), recherche dans `spawn_blocking` (`bot::think`), puis `Hub::apply_bot_move` qui jette le résultat si la partie a disparu, est finie, ou si `ply` / le trait ont changé. Le délai de 600–1400 ms précède la recherche : le temps total d'une réponse est donc « délai + recherche ».
- Plafond de réflexion : `min(Strength.think_ms, bot_think_max)` en temps réel (`stop` interrogé tous les 2 048 nœuds) en plus du budget de nœuds du niveau ; la première itération (profondeur 1) n'est jamais interrompue, donc une action légale est toujours trouvée. Un résultat illégal (ne devrait pas arriver) est remplacé par la première action légale.
- Graine de l'IA : hachage FNV de l'identifiant de partie combiné au `ply` (`bot::seed_for`) ; avec la même graine, le moteur répond toujours la même chose (hors plafond de temps réel).
- Le bot ne propose ni n'accepte de chat, ne propose pas de nulle, ne demande pas de revanche, n'abandonne jamais ; un joueur déconnecté forfait après `reconnect_grace` (60 s) comme dans un duel, y compris à l'écran de choix des compétences (annulation).

**Nulle et revanche**
- `offer_draw` est évaluée tout de suite par le bot (`bot::accepts_draw`, recherche de profondeur 2) : acceptée seulement si `ply ≥ bot_draw_min_plies` et si l'évaluation de son point de vue est dans `[−bot_draw_window, +bot_draw_window]` → `game_over` (`agreed_draw`, `outcome: {"type":"draw_agreed"}`), sinon `draw_declined`. Les règles « une offre par ply » du duel s'appliquent.
- `rematch_request` après une partie solo démarre aussitôt une nouvelle partie (même niveau, couleurs inversées) avec un nouveau `deck_select` ; pas de `rematch_offered`. La possibilité disparaît dès que le joueur lance autre chose, se déconnecte ou démarre la revanche. `rematch_respond` n'a pas de sens face au bot (`no_rematch`).

**Moteur et IA** (`chessy_engine::{search, ai}`)
- `search.rs` : approfondissement itératif, négamax alpha-bêta avec PVS, table de transposition (clé `search::position_key` : cases avec identité de pièce, camp, roques, prise en passant, effets actifs), tri TT/MVV-LVA/tueurs/historique, réductions tardives légères, extension d'échec, recherche de quiétude sur les captures, nulles (50 coups, matériel insuffisant, répétition sur le chemin + historique optionnel). L'évaluation (`evaluate`) combine matériel, tables pièce-case (roi milieu/fin de partie), et en version complète structure de pions, paire de fous, tours sur colonnes ouvertes, abri du roi, mobilité. Budgets : `Limits{depth, nodes, margin, full_eval}` et une closure `stop` ; le moteur ne lit aucune horloge.
- `score_moves` ne donne des scores exacts que pour les coups à moins de 100 cp du meilleur (`SCORE_MOVES_MARGIN`) ; les autres sont des bornes supérieures, pour rester sous 150 ms en profondeur 3 sur des positions tactiques. `best_move(pos, 3)` (Mind Reading) prend quelques millisecondes.
- `ai::Strength::from_elo` : profondeur 1/2/3/4/5/6 par tranche, budget de nœuds et de temps croissants (0,3 s à 3 s), dispersion (400→150, 60→30, 25→10, 8→0, 0 cp), coups ratés (55→30 %, 10→4 %, 3→1 %, 1→0 %), seuil de compétence `T` de 250 (400) à 60 (2 800) cp, probabilité d'envisager une compétence de 25 % à 100 %, évaluation complète à partir de 2 400. Interpolation linéaire dans chaque tranche. Un mat forcé trouvé est toujours joué à partir de 800.
- Compétences : l'IA simule chaque cible légale (`Position::try_skill`), échantillonne au plus 96 cibles, évalue d'abord à profondeur 1 puis re-cherche les 3 meilleures à `profondeur − 1`, et la joue si elle dépasse le meilleur coup d'au moins `T`. `mind` et `control` sont exclues par identifiant (`ai::IGNORED_SKILLS`), de même que toute compétence qui ne passe pas la main. Elle ne connaît pas l'historique de répétition de la partie.

## 6. Client

- `catalog.ts` : les 20 compétences passent `implemented: true` une fois le serveur prêt.
- Ciblage : `Square` (clic sur case vide), `Spawn` (clic sur la case puis choix du type dans une petite fenêtre, comme la promotion), `None` (lancement immédiat), `Pair` (déjà géré).
  Mind Reading/Mind Control ne passent pas la main : l'interface reste active ; `best_move` s'affiche en flèche sur le plateau jusqu'au prochain coup.
- Rendu des états : pièces `mirage` translucides, `wall` en pierre, `temp` avec liseré pointillé, `invisible` (pour son propriétaire) fantôme, effets `forcefield`/`celestial`/`locked`/`morphed`/`color_loan`, pièges (propriétaire seulement), terrains, banc (pièces mises de côté), animations des nouveaux événements.
- Solo : carte « Jouer contre l'IA » dans le lobby avec curseur d'Elo (400–2800, pas de 50), paliers nommés (Débutant, Amateur, Club, Expert, Maître, Grand Maître), choix de la couleur, bouton Commencer ; plaque adversaire affiche « Sage · 1400 » ; pas d'horloge quand `clock_enabled` est faux ; pas de récompense ; revanche immédiate.

### Notes d'implémentation (client, compétences)

- **Identifiants** : le catalogue client (`catalog.ts`, sprite `sk-<id>`, `collectionData.RULES`) utilise désormais les identifiants du serveur (`switch`, `mind`, `control`, `trap`, `bench`, `forcefield`, `queensac`, `temporal`, `celestial`, `godhelp`) ; les 27 compétences sont `implemented`.
- **Interaction** (`interaction.ts`) : `activateSkill` lance tout de suite une cible `none` ; `square` se joue en un clic sur une case vide ; `spawn` demande la case puis le type de pièce (`PendingSpawn`, `chooseSpawn`, `SpawnPicker`), sans fenêtre s'il ne reste qu'un type. Les cases vides ciblables sont repérées par un petit rond.
- **Nouvelle action détectée par empreinte** (`actionKey` = ply + usages + événements) et non par `ply` : Mind Reading/Control laissent le `ply` et le trait inchangés. L'empreinte réinitialise l'interaction, ajoute une ligne au journal (clé `key`), déclenche la carte de lancement et les animations du plateau ; une même position renvoyée après reconnexion ne rejoue rien.
- **Plateau** : `BoardScene.ts` (pièces, terrains, pièges, marqueurs, animations par événement, effet de lancement par compétence), `fx.ts` (primitives de dessin), `textures.ts` (pion de pierre, pilier de roc). Les pièces invisibles adverses ne sont jamais reçues. Le banc est un panneau latéral (`BenchPanel`) ; les mirages et copies temporaires ne comptent pas dans la barre de matériel, le banc oui.
- **Mind Reading** : `best_move` s'affiche en flèche tant que le `ply` ne change pas (y compris après une reconnexion si l'événement est renvoyé) ; le libellé « 2/3 usages » vient de `my_skills[].uses/max_uses`.
