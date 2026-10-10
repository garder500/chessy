import { lazy, Suspense, useEffect } from "react";
import { useLang, t } from "./i18n";
import { navigate, useRoute } from "./router";
import { Auth, needsWelcome } from "./screens/Auth";
import { Campaign } from "./screens/Campaign";
import { Collection } from "./screens/Collection";
import { DeckSelect } from "./screens/DeckSelect";
import { Friends } from "./screens/Friends";
import { Games } from "./screens/Games";
import { Live } from "./screens/Live";
import { Lobby } from "./screens/Lobby";
import { Profile } from "./screens/Profile";
import { Ranking } from "./screens/Ranking";
import { RewardModal } from "./screens/RewardModal";
import { Settings } from "./screens/Settings";
import { installUiClicks } from "./sound/uiClicks";
import { store, useAppState } from "./store";
import { ChallengeModal, OutgoingChallenge } from "./ui/ChallengeModal";
import { NavBar } from "./ui/NavBar";
import { SkillSprite } from "./ui/SkillArt";
import { ForgeReveal } from "./ui/ForgeReveal";
import { Toasts } from "./ui/Toasts";

// Phaser is large; only load it once a game starts.
const Game = lazy(() => import("./screens/Game").then((m) => ({ default: m.Game })));
// Replay et spectateur affichent aussi le plateau : même découpage.
const Replay = lazy(() => import("./screens/Replay").then((m) => ({ default: m.Replay })));
const Watch = lazy(() => import("./screens/Watch").then((m) => ({ default: m.Watch })));

export function App() {
  const state = useAppState();
  const route = useRoute();
  useLang(); // re-render de toute l'application au changement de langue

  useEffect(() => {
    store.connect();
    return () => store.disconnect();
  }, []);
  useEffect(() => installUiClicks(), []);
  // Première visite sans compte ni invité : l'écran de bienvenue ouvre le parcours.
  useEffect(() => {
    if (route.name === "home" && needsWelcome()) navigate({ name: "auth" });
    // Une seule fois au chargement.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  // Un nouvel écran s'ouvre en haut de page (sinon un onglet de la barre du bas garde le défilement du précédent).
  useEffect(() => {
    window.scrollTo(0, 0); // renvoie une Promise sur les Chrome récents : ne pas la retourner comme nettoyage
  }, [route.name, route.param]);

  // Une récompense gagnée à l'instant s'ouvre depuis l'écran de victoire ; une récompense en attente (reconnexion) s'ouvre tout de suite.
  const reward = state.over ? (state.rewardOpen ? state.over.reward : null) : state.pendingReward;
  // Une partie en cours (ou son choix de compétences) prend la place de n'importe quelle page.
  const inGame = state.game !== null || state.deckSelect !== null;

  let screen;
  if (state.connection === "replaced") {
    screen = (
      <main className="page-center">
        <h1 className="page-title">Chessy</h1>
        <p className="muted">{t("app.replaced")}</p>
      </main>
    );
  } else if (state.game) {
    screen = (
      <Suspense fallback={<p className="page-center muted">{t("app.loadingGame")}</p>}>
        <Game view={state.game} />
      </Suspense>
    );
  } else if (state.deckSelect) {
    screen = <DeckSelect info={state.deckSelect} />;
  } else {
    switch (route.name) {
      case "auth":
        screen = <Auth />;
        break;
      case "ranking":
        screen = <Ranking />;
        break;
      case "friends":
        screen = <Friends />;
        break;
      case "profile":
        screen = <Profile username={route.param ?? ""} />;
        break;
      case "collection":
        screen = <Collection />;
        break;
      case "settings":
        // Un compte retrouve ses réglages dans son profil ; l'invité garde la page seule.
        screen = state.account && !state.account.guest && state.account.username ? <Profile username={state.account.username} /> : <Settings />;
        break;
      case "live":
        screen = <Live />;
        break;
      case "games":
        screen = <Games />;
        break;
      case "campaign":
        screen = <Campaign state={state} level={route.param} />;
        break;
      case "watch":
        screen = (
          <Suspense fallback={<p className="page-center muted">{t("app.loadingGame")}</p>}>
            <Watch gameId={route.param ?? ""} />
          </Suspense>
        );
        break;
      case "replay":
        screen = (
          <Suspense fallback={<p className="page-center muted">{t("app.loadingReplay")}</p>}>
            <Replay gameId={route.param ?? ""} autoAnalyse={route.sub === "analyse"} />
          </Suspense>
        );
        break;
      default:
        screen = <Lobby state={state} />;
    }
  }

  return (
    <>
      <SkillSprite />
      {state.connection === "closed" && (
        <div className="conn-banner" role="status">
          {t("app.connectionLost")}
        </div>
      )}
      {!inGame && state.connection !== "replaced" && route.name !== "auth" && <NavBar state={state} route={route.name} />}
      {screen}
      {reward && <RewardModal offer={reward} />}
      {state.incomingChallenge && <ChallengeModal challenge={state.incomingChallenge} />}
      {state.outgoingChallenge && !inGame && <OutgoingChallenge username={state.outgoingChallenge} />}
      {state.reveal && <ForgeReveal skill={state.reveal} />}
      <Toasts toasts={state.toasts} />
    </>
  );
}
