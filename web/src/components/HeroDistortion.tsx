import { useEffect, useRef } from "react";

// The reference hero's signature interaction: a WebGL grid mouse-trail that
// ripples the image as the cursor moves and relaxes back to rest. Faithful port
// of the DICH shader (grid 50, mouse 0.14, relaxation 0.9, strength 1) in raw
// WebGL2 — no three.js. The plain <img> underneath is the fail-safe fallback if
// WebGL is unavailable, the context is lost, or the image hasn't decoded yet.

interface Props {
  src: string;
  alt: string;
}

const GRID = 50;
const MOUSE = 0.14;
const RELAX = 0.9;
const STRENGTH = 1;

// Screenshot / verification mode: skip the WebGL + parallax loops (the <img>
// fallback still shows the figure) so the page goes idle for stable captures.
const STATIC =
  typeof location !== "undefined" &&
  (new URLSearchParams(location.search).has("static") ||
    new URLSearchParams(location.search).has("nolenis"));

const VERT = `#version 300 es
in vec2 position;
out vec2 vUv;
void main() {
  vUv = position * 0.5 + 0.5;
  gl_Position = vec4(position, 0.0, 1.0);
}`;

const FRAG = `#version 300 es
precision highp float;
in vec2 vUv;
uniform sampler2D uTexture;
uniform sampler2D uDataTexture;
uniform vec2 uCover;
out vec4 frag;
void main() {
  vec2 newUV = (vUv - 0.5) * uCover + 0.5;
  vec4 offset = texture(uDataTexture, vUv);
  frag = texture(uTexture, newUV - 0.02 * offset.rg);
}`;

function compile(gl: WebGL2RenderingContext, type: number, src: string) {
  const sh = gl.createShader(type)!;
  gl.shaderSource(sh, src);
  gl.compileShader(sh);
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
    throw new Error(gl.getShaderInfoLog(sh) || "shader compile failed");
  }
  return sh;
}

export function HeroDistortion({ src, alt }: Props) {
  const imgRef = useRef<HTMLImageElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);

  // Parallax: the whole figure leans toward the cursor (works with or without
  // WebGL). Lerped in its own rAF so it glides rather than snaps.
  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    if (STATIC || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    let raf = 0;
    let tx = 0, ty = 0, x = 0, y = 0;
    const onMove = (e: MouseEvent) => {
      tx = (e.clientX / window.innerWidth - 0.5) * 30;
      ty = (e.clientY / window.innerHeight - 0.5) * 22;
    };
    const tick = () => {
      x += (tx - x) * 0.08;
      y += (ty - y) * 0.08;
      wrap.style.transform = `translate3d(${x.toFixed(2)}px, ${y.toFixed(2)}px, 0) scale(1.06)`;
      raf = requestAnimationFrame(tick);
    };
    window.addEventListener("mousemove", onMove, { passive: true });
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("mousemove", onMove);
    };
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    const img = imgRef.current;
    if (!canvas || !img) return;
    if (STATIC || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    const gl = canvas.getContext("webgl2", { premultipliedAlpha: false, antialias: true });
    if (!gl) return; // no WebGL2 → <img> fallback stays visible

    let disposed = false;
    const cleanups: (() => void)[] = [];

    // All GL work waits until the image has actually decoded, otherwise the
    // texture would be empty and the canvas would paint nothing over the figure.
    const init = () => {
      if (disposed || !img.naturalWidth) return;

      let raf = 0;
      const mouse = { x: 0.5, y: 0.5, prevX: 0.5, prevY: 0.5, vX: 0, vY: 0 };
      // Zero-initialised: the figure is undistorted at rest (no startup-noise
      // flash, and robust if rAF is throttled); only the cursor displaces it.
      const data = new Float32Array(3 * GRID * GRID);

      let cover: [number, number] = [1, 1];
      const computeCover = () => {
        const imgAspect = img.naturalHeight / img.naturalWidth || 0.5;
        const cw = canvas.clientWidth || 1;
        const ch = canvas.clientHeight || 1;
        if (ch / cw > imgAspect) cover = [(cw / ch) * imgAspect, 1];
        else cover = [1, ch / cw / imgAspect];
      };
      const resize = () => {
        const dpr = Math.min(window.devicePixelRatio || 1, 2);
        const w = Math.max(1, Math.round(canvas.clientWidth * dpr));
        const h = Math.max(1, Math.round(canvas.clientHeight * dpr));
        if (canvas.width !== w || canvas.height !== h) {
          canvas.width = w;
          canvas.height = h;
        }
        gl.viewport(0, 0, canvas.width, canvas.height);
        computeCover();
      };

      let program: WebGLProgram;
      try {
        program = gl.createProgram()!;
        gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, VERT));
        gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, FRAG));
        gl.linkProgram(program);
        if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
          throw new Error(gl.getProgramInfoLog(program) || "link failed");
        }
      } catch {
        return; // shader failure → <img> fallback
      }
      gl.useProgram(program);

      const quad = new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]);
      const vbo = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
      gl.bufferData(gl.ARRAY_BUFFER, quad, gl.STATIC_DRAW);
      const loc = gl.getAttribLocation(program, "position");
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);

      const imgTex = gl.createTexture();
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, imgTex);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
      try {
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
      } catch {
        return; // tainted/unloadable image → <img> fallback
      }
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

      const dataTex = gl.createTexture();
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, dataTex);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB32F, GRID, GRID, 0, gl.RGB, gl.FLOAT, data);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

      gl.uniform1i(gl.getUniformLocation(program, "uTexture"), 0);
      gl.uniform1i(gl.getUniformLocation(program, "uDataTexture"), 1);
      const uCover = gl.getUniformLocation(program, "uCover");
      gl.clearColor(0, 0, 0, 0);
      resize();

      const onMove = (e: MouseEvent) => {
        const r = canvas.getBoundingClientRect();
        mouse.x = (e.clientX - r.left) / r.width;
        mouse.y = 1 - (e.clientY - r.top) / r.height;
        mouse.vX = mouse.x - mouse.prevX;
        mouse.vY = mouse.y - mouse.prevY;
        mouse.prevX = mouse.x;
        mouse.prevY = mouse.y;
      };
      window.addEventListener("mousemove", onMove, { passive: true });
      window.addEventListener("resize", resize);
      cleanups.push(() => {
        cancelAnimationFrame(raf);
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("resize", resize);
      });

      const updateData = () => {
        for (let i = 0; i < data.length; i += 3) {
          data[i] *= RELAX;
          data[i + 1] *= RELAX;
        }
        const gmx = GRID * mouse.x;
        const gmy = GRID * mouse.y;
        const maxDist = GRID * MOUSE;
        const maxDistSq = maxDist * maxDist;
        const aspect = (canvas.clientHeight || 1) / (canvas.clientWidth || 1);
        for (let i = 0; i < GRID; i++) {
          for (let j = 0; j < GRID; j++) {
            const dx = gmx - i;
            const dy = gmy - j;
            const distance = (dx * dx) / aspect + dy * dy;
            if (distance < maxDistSq && distance > 0.0001) {
              const index = 3 * (i + GRID * j);
              const power = Math.min(Math.max(maxDist / Math.sqrt(distance), 0), 10);
              data[index] += STRENGTH * 100 * mouse.vX * power;
              data[index + 1] -= STRENGTH * 100 * mouse.vY * power;
            }
          }
        }
        mouse.vX *= 0.9;
        mouse.vY *= 0.9;
      };

      const draw = () => {
        updateData();
        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, dataTex);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB32F, GRID, GRID, 0, gl.RGB, gl.FLOAT, data);
        gl.uniform2f(uCover, cover[0], cover[1]);
        gl.clear(gl.COLOR_BUFFER_BIT);
        gl.drawArrays(gl.TRIANGLES, 0, 6);
      };

      const loop = () => {
        if (disposed) return;
        draw();
        raf = requestAnimationFrame(loop);
      };
      draw(); // immediate first frame (figure shows even if rAF is throttled)
      raf = requestAnimationFrame(loop);
    };

    if (img.complete && img.naturalWidth) init();
    else img.addEventListener("load", init, { once: true });

    return () => {
      // NB: do NOT loseContext() here — React reuses the same <canvas> across
      // StrictMode's mount→cleanup→remount, so killing the context would leave
      // the second mount with a dead (lost) context and a blank canvas.
      disposed = true;
      img.removeEventListener("load", init);
      cleanups.forEach((fn) => fn());
    };
  }, [src]);

  return (
    <div ref={wrapRef} className="hero-figure-wrap">
      <img ref={imgRef} className="hero-photo" src={src} alt={alt} />
      <canvas ref={canvasRef} className="hero-canvas" aria-hidden="true" />
    </div>
  );
}
