import Phaser from "phaser";
import { useEffect, useRef } from "react";
import type { Highlights } from "../interaction";
import type { Square, StateView } from "../protocol";
import { useT } from "../i18n";
import { getTheme, useTheme } from "../theme";
import { BOARD_SIZE, BoardScene, type PremoveMark } from "./BoardScene";

interface Props {
  view: StateView;
  highlights: Highlights;
  onSquare: (square: Square) => void;
  /** Faux : plateau en lecture seule (replay, spectateur) ; les clics ne sont pas transmis. */
  interactive?: boolean;
  /** Glisser-déposer (optionnel ; voir `dragStart`/`dropOn` dans interaction.ts). */
  canDrag?: (square: Square) => boolean;
  onDragStart?: (square: Square) => void;
  onDrop?: (from: Square, to: Square | null) => "snap" | "return";
  /** File de premoves, dessinée sur le plateau (numérotée dans l'ordre). */
  premove?: PremoveMark[] | null;
  onCancelPremove?: () => void;
  /** Plateau retourné (vue de l'adversaire). */
  flipped?: boolean;
}

/** Phaser ouvre un contexte WebGL1 et exige ANGLE_instanced_arrays ; sans cela on bascule en Canvas. */
function webglUsable(): boolean {
  try {
    const c = document.createElement("canvas");
    const gl = c.getContext("webgl") ?? (c.getContext("experimental-webgl") as WebGLRenderingContext | null);
    return !!gl && !!gl.getExtension("ANGLE_instanced_arrays");
  } catch {
    return false;
  }
}

export function PhaserBoard({ view, highlights, onSquare, interactive = true, canDrag, onDragStart, onDrop, premove = null, onCancelPremove, flipped = false }: Props) {
  const t = useT();
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<BoardScene | null>(null);
  // Latest props, readable from the deferred game setup and from scene callbacks.
  const latest = useRef({ view, highlights, onSquare, interactive, canDrag, onDragStart, onDrop, premove, onCancelPremove, flipped });
  latest.current = { view, highlights, onSquare, interactive, canDrag, onDragStart, onDrop, premove, onCancelPremove, flipped };
  const theme = useTheme();

  useEffect(() => {
    let game: Phaser.Game | null = null;
    let observer: ResizeObserver | null = null;
    // Phaser tears a game down on its next frame, which never comes if it has not
    // booted yet. React StrictMode mounts, unmounts and remounts straight away, so
    // creating the game synchronously would leave a second canvas behind. Deferring
    // by a tick lets the throwaway mount be cancelled before anything is built.
    const timer = setTimeout(() => {
      const boardScene = new BoardScene();
      boardScene.onSquare = (square) => {
        if (latest.current.interactive) latest.current.onSquare(square);
      };
      boardScene.canDrag = (square) => !!latest.current.interactive && !!latest.current.canDrag?.(square);
      boardScene.onDragStart = (square) => latest.current.onDragStart?.(square);
      boardScene.onDrop = (from, to) => (latest.current.interactive ? latest.current.onDrop?.(from, to) ?? "return" : "return");
      boardScene.onCancelPremove = () => latest.current.onCancelPremove?.();
      boardScene.setTheme(getTheme());
      boardScene.setPremove(latest.current.premove);
      boardScene.setFlipped(latest.current.flipped);
      boardScene.setView(latest.current.view);
      boardScene.setHighlights(latest.current.highlights);
      scene.current = boardScene;
      game = new Phaser.Game({
        type: webglUsable() ? Phaser.AUTO : Phaser.CANVAS,
        parent: host.current!,
        width: BOARD_SIZE,
        height: BOARD_SIZE,
        backgroundColor: "#15171b",
        scene: boardScene,
        scale: { mode: Phaser.Scale.FIT, autoCenter: Phaser.Scale.CENTER_BOTH },
      });
      // Phaser n'écoute que le redimensionnement de la fenêtre : on le prévient aussi quand la mise en page
      // change la taille du conteneur (rotation, barre d'adresse mobile, panneaux repliés).
      if (typeof ResizeObserver !== "undefined" && host.current) {
        observer = new ResizeObserver(() => game?.scale.refresh());
        observer.observe(host.current);
      }
      // Lets tests and the dev console inspect the canvas, which is not in the accessibility tree.
      if (import.meta.env.DEV) (window as unknown as { __chessy: unknown }).__chessy = { game, scene: boardScene };
    }, 0);

    return () => {
      clearTimeout(timer);
      observer?.disconnect();
      scene.current = null;
      if (game) {
        game.destroy(true);
        game.canvas.remove(); // do not wait for the game's next frame
      }
    };
  }, []);

  useEffect(() => {
    scene.current?.setFlipped(flipped);
  }, [flipped]);
  useEffect(() => {
    scene.current?.setView(view);
  }, [view]);
  useEffect(() => {
    scene.current?.setHighlights(highlights);
  }, [highlights]);
  useEffect(() => {
    scene.current?.setTheme(theme);
  }, [theme]);
  useEffect(() => {
    scene.current?.setPremove(premove);
  }, [premove]);

  return <div className="gm-board-host" ref={host} role="img" aria-label={t("game.board_aria")} />;
}
