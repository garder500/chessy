// Pièce 3D de l'écran Jouer (three.js) : modèle « Chess Pieces 3D Model » (low poly) teinté par l'accent, éclairé
// par le haut comme sous le faisceau, qui tourne lentement. Chargée à la demande : three.js n'alourdit pas l'accueil.
import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import { getTheme, subscribeTheme } from "../theme";
import { HeroPiece, type HeroKind } from "./HeroPiece";
import kingUrl from "../assets/models/king.bin?url";
import knightUrl from "../assets/models/knight.bin?url";
import pawnUrl from "../assets/models/pawn.bin?url";
import rookUrl from "../assets/models/rook.bin?url";

const MODEL_URL: Record<HeroKind, string> = { king: kingUrl, pawn: pawnUrl, rook: rookUrl, knight: knightUrl };

/** Lit un .bin produit par scripts/obj-to-pieces.py (positions, normales, indices). */
export function parsePieceBin(buf: ArrayBuffer): THREE.BufferGeometry {
  const head = new Uint32Array(buf, 0, 2);
  const [count, indices] = [head[0], head[1]];
  const pos = new Float32Array(buf, 8, count * 3);
  const nor = new Float32Array(buf, 8 + count * 12, count * 3);
  const idx = new Uint32Array(buf, 8 + count * 24, indices);
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  geo.setAttribute("normal", new THREE.BufferAttribute(nor, 3));
  geo.setIndex(new THREE.BufferAttribute(idx, 1));
  return geo;
}

const cache = new Map<HeroKind, Promise<THREE.BufferGeometry>>();
function loadPiece(kind: HeroKind): Promise<THREE.BufferGeometry> {
  let p = cache.get(kind);
  if (!p) {
    p = fetch(MODEL_URL[kind])
      .then((r) => {
        if (!r.ok) throw new Error(`modèle ${kind} : ${r.status}`);
        return r.arrayBuffer();
      })
      .then(parsePieceBin);
    p.catch(() => cache.delete(kind));
    cache.set(kind, p);
  }
  return p;
}

function cssAccent(): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim();
  return v || "#1fb89a";
}

function reducedMotion(): boolean {
  return getTheme().reduceMotion || matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export default function HeroPiece3D({ kind, className, fast }: { kind: HeroKind; className?: string; fast?: boolean }) {
  const host = useRef<HTMLDivElement>(null);
  const [failed, setFailed] = useState(false);
  const [ready, setReady] = useState(false);
  // Le rendu lit la pièce et la vitesse courantes sans recréer la scène.
  const want = useRef({ kind, fast: !!fast });
  want.current = { kind, fast: !!fast };
  const swap = useRef<((k: HeroKind) => void) | null>(null);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let renderer: THREE.WebGLRenderer;
    try {
      renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, powerPreference: "low-power" });
    } catch {
      setFailed(true);
      return;
    }
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.toneMappingExposure = 0.9;
    el.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    const pmrem = new THREE.PMREMGenerator(renderer);
    const env = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
    scene.environment = env;

    const camera = new THREE.PerspectiveCamera(26, 1, 0.1, 20);
    camera.position.set(0, 0.6, 2.75);
    camera.lookAt(0, 0.47, 0);

    // Faisceau du dessus (le cône CSS), contre-jour pour détacher la silhouette, ambiance douce.
    const spot = new THREE.SpotLight(0xffffff, 18, 8, Math.PI / 7, 0.6, 1.2);
    spot.position.set(0, 4, 0.6);
    spot.target.position.set(0, 0.4, 0);
    scene.add(spot, spot.target);
    const rim = new THREE.DirectionalLight(0xffffff, 1.6);
    rim.position.set(-2, 1.5, -2);
    scene.add(rim);
    scene.add(new THREE.HemisphereLight(0xffffff, 0x0a1412, 0.25));

    const material = new THREE.MeshPhysicalMaterial({
      color: cssAccent(),
      metalness: 0.1,
      roughness: 0.32,
      clearcoat: 0.5,
      clearcoatRoughness: 0.3,
      envMapIntensity: 0.45,
    });
    const mesh = new THREE.Mesh(new THREE.BufferGeometry(), material);
    scene.add(mesh);

    let alive = true;
    let pop = 1;
    swap.current = (k: HeroKind) => {
      loadPiece(k)
        .then((geo) => {
          if (!alive || want.current.kind !== k) return;
          mesh.geometry = geo;
          pop = 0;
          setReady(true);
        })
        .catch(() => alive && setFailed(true));
    };
    swap.current(want.current.kind);

    const resize = () => {
      const w = el.clientWidth || 1;
      const h = el.clientHeight || 1;
      renderer.setSize(w, h, false);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
    };
    const ro = new ResizeObserver(resize);
    ro.observe(el);
    resize();

    const unsubTheme = subscribeTheme(() => material.color.set(cssAccent()));
    // Le mode clair/sombre et l'accent passent par des variables CSS : on relit l'accent quand <html> change.
    const mo = new MutationObserver(() => material.color.set(cssAccent()));
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["style", "data-mode"] });

    let angle = 0.5;
    let last = performance.now();
    let frame = 0;
    const tick = (now: number) => {
      frame = requestAnimationFrame(tick);
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      const still = reducedMotion();
      if (!still) angle += dt * (want.current.fast ? 2.6 : 0.9);
      pop = Math.min(1, pop + dt * 3.5);
      const ease = 1 - Math.pow(1 - pop, 3);
      mesh.rotation.y = angle;
      mesh.position.y = still ? 0 : Math.sin(now / 900) * 0.02 + (1 - ease) * -0.08;
      mesh.scale.setScalar(0.94 + ease * 0.06);
      renderer.render(scene, camera);
    };
    frame = requestAnimationFrame(tick);

    return () => {
      alive = false;
      swap.current = null;
      cancelAnimationFrame(frame);
      ro.disconnect();
      mo.disconnect();
      unsubTheme();
      material.dispose();
      env.dispose();
      pmrem.dispose();
      renderer.dispose();
      renderer.domElement.remove();
    };
  }, []);

  useEffect(() => {
    swap.current?.(kind);
  }, [kind]);

  if (failed) return <HeroPiece kind={kind} className={className} />;
  return (
    <div ref={host} className={`${className ?? ""} pl-piece-3d${ready ? " ready" : ""}`} role="presentation">
      {!ready && <HeroPiece kind={kind} className="pl-piece-svg" />}
    </div>
  );
}
