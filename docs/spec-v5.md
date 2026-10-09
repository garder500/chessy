# Spec v5 — parties d'évaluation (placement)

## But

Un compte neuf (ou existant, d'avant cette mise à jour) démarre à 1200 Elo, ce qui est trop haut pour la plupart
des joueurs face aux faux joueurs. Cinq **parties d'évaluation** estiment un Elo réel.

## Déroulement

- Message client `placement_start { color? }` (comptes seulement, pas les invités). Le serveur tire au hasard un niveau
  parmi **400, 800, 1200, 1600, 2000** que le joueur n'a pas encore rencontré et lance une partie Solo (l'IA « Sage »,
  sans horloge) à ce niveau. L'Elo de l'adversaire n'est jamais envoyé (`opponent.elo: null`), ni stocké dans la
  partie enregistrée (`solo_elo` nul), donc absent du replay.
- Une partie d'évaluation ne touche ni `games`/`wins`/`losses`, ni l'Elo, ni les récompenses ; pas de revanche.
  Abandonner une partie lancée compte comme une défaite à ce niveau ; quitter avant le choix du deck n'enregistre rien.
- `game_over.placement` : `{ done, total, elo?, before? }` ; `elo`/`before` à la cinquième partie.
- `Me.placement` et `PublicProfile.placed` : `{ placed, done, total }`. Un joueur sans placement est « non évalué »
  (badge dans le lobby et le profil) et garde son Elo en attendant. Les comptes de faux joueurs comptent comme évalués.

## Estimation

L'Elo estimé est l'Elo de performance : le niveau `R` pour lequel la somme des scores attendus contre les cinq
adversaires égale le score réel (recherche par dichotomie), borné à **200..2200** (un score parfait ou nul ne dit
qu'« au moins / au plus »). Il **remplace** l'Elo (le pic repart de là, un point est ajouté à la courbe) ; les parties
classées suivantes utilisent K = 40 / 20 comme avant. L'évaluation ne se rejoue pas.

## Stockage

Migration : `placement_results (player_id, level, score, game_id)` et `placements (player_id, elo)`.
