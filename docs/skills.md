# Chessy — compétences

Source de vérité des règles de compétences.

## C'est quoi ?

Chessy est un jeu d'échecs sur un site web permettant aux joueurs de jouer de façon amusante en utilisant un système de compétences.

## Une compétence, c'est quoi ?

Une compétence est une capacité donnant un avantage sur le jeu sans pour autant assurer une victoire. Uniquement trois compétences sont sélectionnables pour la partie, MAIS les **compétences uniques** ne comptent pas dans les trois sélectionnées.

## Comment obtenir les compétences ?

Les compétences s'obtiennent en gagnant une partie **classée** (voir `docs/spec-v2.md` §2 : les parties amicales, contre l'IA ou abandonnées à zéro coup n'en donnent pas). En cas de victoire, une proposition est faite : prendre une compétence choisie à l'adversaire OU prendre une compétence aléatoire. Si vous choisissez une compétence dans le deck adverse, celui-ci la perd et vous la gagnez. Si vous choisissez l'aléatoire, l'adversaire perd une compétence aléatoire de son deck. Vous ne disposez que de 7 slots maximum : si vous avez déjà 7 compétences, vous devez en remplacer une OU passer l'obtention.

## Les compétences uniques

Une compétence unique ne peut être disponible que dans un seul deck (global). Elle est UNIQUE AU MONDE et ne peut pas être en double. Ces compétences sont beaucoup plus puissantes que les compétences *de base*.

### Liste des compétences uniques

- **Remover** — Retire un pion parmi les pions adverses. Impossible si la suppression engendre un mat.
- **Wall** — Ramène à la vie des pions (uniquement des **PIONS**) sous forme de mur de protection. Jouables à partir du tour suivant. S'ils meurent à nouveau, ils ne peuvent pas être ramenés avec une autre compétence.
- **Mirage** — Simule une pièce sur l'échiquier (pas le roi). Si elle meurt, elle disparaît. Elle ne peut ni engendrer de mat ni manger de pions. Elle empêche également le déplacement si ce dernier n'est pas un déplacement du roi.
- **Evolve** — Upgrade un pion comme s'il était passé sur une case « evolve » d'un terrain adverse. Utilisable sur toutes les pièces, sauf le roi.
- **Switch Sides** — Fait changer de camp une pièce adverse sans portée ni ligne de vue. Impossible sur une pièce engendrant un mat.
- **Mind Reading** — Donne le meilleur coup possible, 3 fois dans la partie, quand le joueur le veut.
- **Mind Control** — Prend le contrôle d'une pièce adverse pour un tour complet, comme s'il s'agissait d'une de ses pièces. Ne fonctionne pas sur le roi ennemi.

## Compétences classiques

- **Teleportation** — Déplace une pièce (pas le roi) vers n'importe quelle case de l'échiquier, sans tenir compte des obstacles.
- **Imune** — Rend une pièce invulnérable aux attaques ennemies pour un tour.
- **Rollback** — Rollback le mouvement réel d'une pièce. Pas sur le roi.
- **Clone** — Crée une copie d'une pièce sur une case vide adjacente. Possible uniquement si la cible dispose d'une case adjacente vide.
- **Morph** — Transforme une pièce en une autre : un tour si la cible est ennemie, 2 tours sinon.
- **Canceller** — Annule l'effet d'une compétence ennemie utilisée au tour précédent. La liste des compétences ennemies du tour précédent est proposée pour choisir celle à annuler.
- **Tornado** — Déplace toutes les pièces de l'échiquier dans un sens de rotation, sauf les rois.
- **Invisibility** — Rend une pièce invisible pendant 2 tours.
- **Freeze** — Empêche une pièce ennemie de se déplacer pendant deux tours.
- **Terminator** — Crée une copie d'une pièce ennemie aléatoire avec toutes ses capacités pour un tour, placée au même endroit mais de votre côté du plateau. Si la case n'est pas disponible, l'opération est impossible.
- **Destiny Swapper** — Échange les positions de deux pièces alliées.
- **Trap Card** — Place un piège sur une case vide, qui immobilise la première pièce ennemie qui marche dessus pendant deux tours. Chaque mouvement est une suite de mouvements case par case : un piège sur le chemin immobilise la pièce sur la case du piège. Seuls vos propres pièges excluent une case : un piège adverse (secret) ne change pas le choix, et deux pièges de camps opposés peuvent partager une case.
- **The Bench** — Pendant un tour, la pièce est mise sur le banc (plus sur l'échiquier). Au retour, elle est placée sur la case libre la plus proche de son ancienne case.
- **Force Field** — Appliqué sur une pièce : la pièce peut être prise, mais celui qui la prend est repoussé de deux cases au plus ; la protection dure jusqu'à cette prise. La pièce mangée va au cimetière (pas invincible).
- **Transposition** — Échange la position de deux pièces sur l'échiquier, sans engendrer d'échec.
- **Queen Sacrifice** — En cas de mat, transpose la reine avec le roi. La reine MEURT sur la position du roi.
- **Temporal Distortion** — Permet à une pièce alliée de revenir dans le temps et de refaire son dernier mouvement. Pas sur le roi.
- **Geomancy** — Modifie la disposition de l'échiquier en déplaçant les cases pour créer des murs qui bloquent l'adversaire pendant 3 tours.
- **Celestial Intervention** — Une fois par partie, empêche une pièce alliée d'être capturée et la replace à sa position de départ. Pas sur le roi.
- **God Help** — Une pièce apparaît aléatoirement sur le terrain avec des capacités aléatoires pendant 3 tours. Cette pièce n'appartient pas à l'échiquier de base ; ni la case d'apparition ni le type ne sont choisissables.
