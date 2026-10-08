import type { CSSProperties } from "react";
import "./beam.css";

/** Faisceau de lumière qui tombe du haut : accueil, victoire, forge, nouveau palier. `glow` remplace la couleur d'accent. */
export function Beam({ width = 600, height = 520, glow, className }: { width?: number; height?: number; glow?: string; className?: string }) {
  const style = { "--beam-w": `${width}px`, "--beam-h": `${height}px`, ...(glow ? { "--beam-c": glow } : {}) } as CSSProperties;
  return <div className={`beam${className ? ` ${className}` : ""}`} style={style} aria-hidden="true" />;
}
