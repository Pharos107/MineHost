import { useId, useMemo } from "react";

import styles from "./BlockWordmark.module.css";
import {
  cellsPath,
  crackPaths,
  edgePath,
  EXTRUDE_DEPTH,
  extrusionLayers,
  noiseTile,
  vanishingPoint,
  WORDMARK_ROWS,
  wordmarkCells,
} from "./wordmark";

interface BlockWordmarkProps {
  /** Các từ của logo (mặc định liền một khối "MineHost" giống tiêu đề MINECRAFT). */
  words?: string[];
  /** Nhãn cho trình đọc màn hình. */
  label: string;
}

// Màu đá là hình minh họa theo texture Minecraft, không phải màu giao diện nên không dùng token.
const OUTLINE = "#0e0d0c";
const TILE = 24;
const TILE_PX = 0.5;
const FACE_TILE = noiseTile(TILE, TILE_PX, ["#97958f", "#a8a6a0", "#b5b3ad", "#c0beb8", "#c9c7c2", "#d3d1cc", "#e0ded9"], 11);
const SIDE_TILE = noiseTile(TILE, TILE_PX, ["#383734", "#42413e", "#4c4b48", "#565551", "#605f5b"], 29);

function buildArt(words: string[]) {
  const { cells, columns, letterStarts } = wordmarkCells(words);
  return {
    columns,
    face: cellsPath(cells),
    highlightTop: edgePath(cells, "topEdge", 0.5),
    highlightLeft: edgePath(cells, "leftEdge", 0.35),
    shadeBottom: edgePath(cells, "bottomEdge", 0.45),
    shadeRight: edgePath(cells, "rightEdge", 0.3),
    cracks: crackPaths(cells, letterStarts),
  };
}

function NoisePattern({ id, tile }: { id: string; tile: { fill: string; d: string }[] }) {
  const size = TILE * TILE_PX;
  return (
    <pattern id={id} width={size} height={size} patternUnits="userSpaceOnUse">
      {tile.map(({ fill, d }) => (
        <path key={fill} d={d} fill={fill} />
      ))}
    </pattern>
  );
}

/**
 * Logo chữ đá như tiêu đề MINECRAFT: mặt đá texture pixel sáng ở đỉnh tối dần về chân, gờ vát,
 * vết nứt, khối 3D đổ xuống hội tụ về giữa (vẽ bằng nhiều lớp co dần) và viền đen dày bao ngoài.
 */
export function BlockWordmark({ words = ["MineHost"], label }: BlockWordmarkProps) {
  const uid = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const key = words.join(" ");
  const art = useMemo(() => buildArt(key.split(" ")), [key]);
  const [vx, vy] = vanishingPoint(art.columns);
  const layers = extrusionLayers();
  const at = (scale: number) => `matrix(${scale} 0 0 ${scale} ${vx * (1 - scale)} ${vy * (1 - scale)})`;
  const ids = { face: `${uid}-face`, side: `${uid}-side`, light: `${uid}-light`, glyph: `${uid}-glyph` };
  const pad = 1.6;

  return (
    <svg
      className={styles.root}
      viewBox={`${-pad} ${-pad} ${art.columns + pad * 2} ${WORDMARK_ROWS + EXTRUDE_DEPTH + pad * 2}`}
      role="img"
      aria-label={label}
      shapeRendering="crispEdges"
    >
      <defs>
        <path id={ids.glyph} d={art.face} />
        <NoisePattern id={ids.face} tile={FACE_TILE} />
        <NoisePattern id={ids.side} tile={SIDE_TILE} />
        <linearGradient id={ids.light} x1="0" y1="0" x2="0" y2={WORDMARK_ROWS} gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#ffffff" stopOpacity={0.5} />
          <stop offset="0.38" stopColor="#ffffff" stopOpacity={0} />
          <stop offset="0.62" stopColor="#000000" stopOpacity={0} />
          <stop offset="1" stopColor="#000000" stopOpacity={0.3} />
        </linearGradient>
      </defs>

      {/* Viền đen dày: tô nét mọi lớp trước, các lớp tô sau che phần nét bên trong chỉ để lộ mép ngoài. */}
      <g stroke={OUTLINE} strokeWidth={2} strokeLinejoin="miter" fill={OUTLINE} shapeRendering="geometricPrecision">
        {layers.map(({ scale }) => (
          <use key={`o-${scale}`} href={`#${ids.glyph}`} transform={at(scale)} />
        ))}
        <use href={`#${ids.glyph}`} />
      </g>

      {/* Khối 3D từ lớp xa nhất tới gần nhất, xa thì tối hơn, sọc xen kẽ như từng hàng pixel. */}
      {layers.map(({ scale, shade }) => (
        <g key={`l-${scale}`} transform={at(scale)}>
          <use href={`#${ids.glyph}`} fill={`url(#${ids.side})`} />
          <use href={`#${ids.glyph}`} fill="#000000" opacity={shade} />
        </g>
      ))}

      {/* Mặt chữ: nét tối mảnh tách khỏi khối 3D, texture đá, ánh sáng từ trên, gờ vát và vết nứt. */}
      <use href={`#${ids.glyph}`} fill="none" stroke="#1d1c1a" strokeWidth={0.5} />
      <use href={`#${ids.glyph}`} fill={`url(#${ids.face})`} />
      <use href={`#${ids.glyph}`} fill={`url(#${ids.light})`} />
      <path d={art.highlightTop} fill="#f7f6f3" opacity={0.85} />
      <path d={art.highlightLeft} fill="#efeeea" opacity={0.45} />
      <path d={art.shadeBottom} fill="#4a4946" opacity={0.75} />
      <path d={art.shadeRight} fill="#55534f" opacity={0.45} />
      <path d={art.cracks.dark} fill="#4b4946" />
      <path d={art.cracks.light} fill="#eeede9" opacity={0.7} />
    </svg>
  );
}
