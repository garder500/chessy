import type { CSSProperties } from "react";

const COLORS = ["#ffd36b", "#ff4d5e", "#1fb89a", "#ffffff"];

/** Pluie de confettis (décor, décalée à l'ouverture de l'écran). */
export function Confetti({ count = 32, delay = 0.5 }: { count?: number; delay?: number }) {
  return (
    <div className="confetti" aria-hidden="true">
      {Array.from({ length: count }, (_, i) => (
        <i
          key={i}
          style={
            {
              left: `${(i * 37) % 100}%`,
              width: 6 + (i % 3) * 3,
              height: 10 + (i % 4) * 3,
              background: COLORS[i % 4],
              "--t": `${3 + (i % 5) * 0.5}s`,
              "--dl": `${delay + (i % 7) * 0.18}s`,
              "--dx": `${(i % 2 ? 1 : -1) * (20 + (i % 5) * 14)}px`,
              "--rot": `${(i % 2 ? 1 : -1) * (360 + (i % 4) * 120)}deg`,
            } as CSSProperties
          }
        />
      ))}
    </div>
  );
}
