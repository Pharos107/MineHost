import { useEffect, useRef } from "react";
import { SkinViewer as Viewer } from "skinview3d";

interface SkinViewerProps {
  url: string;
  label: string;
  /** Không tải được skin hoặc máy không có WebGL: trang hiện đầu skin phẳng thay vào. */
  onError: () => void;
}

/**
 * Skin 3D kéo để xoay. File này chỉ được nạp bằng `import()` từ hộp thoại chi tiết, để thư viện 3D
 * (three.js) không nằm trong bundle chính.
 */
export default function SkinViewer({ url, label, onError }: SkinViewerProps) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (!canvas.current) return;
    let viewer: Viewer;
    try {
      viewer = new Viewer({ canvas: canvas.current, width: 180, height: 240 });
    } catch {
      onError();
      return;
    }
    viewer.loadSkin(url).catch(onError);
    return () => viewer.dispose();
  }, [url, onError]);

  return <canvas ref={canvas} role="img" aria-label={label} />;
}
