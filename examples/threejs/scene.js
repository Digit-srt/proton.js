import {
  AmbientLight,
  BufferGeometry,
  Color,
  DirectionalLight,
  Group,
  LineBasicMaterial,
  LineLoop,
  Mesh,
  MeshStandardMaterial,
  PerspectiveCamera,
  REVISION,
  Scene,
  TorusKnotGeometry,
  Vector3,
  WebGLRenderTarget,
  WebGLRenderer,
} from "./vendor/three.module.js";

const stage = document.getElementById("stage");
const canvas = document.getElementById("view");
const status = document.getElementById("status");
const toggle = document.getElementById("toggle");

start().catch((error) => {
  status.textContent = error.message;
  status.dataset.state = "error";
  document.title = "Proton.js · Three.js error: " + error.message;
});

async function start() {
  const context = canvas.getContext("webgl2", { antialias: true, preserveDrawingBuffer: true });
  if (!context) throw new Error("Could not create a WebGL 2 context.");

  const renderer = new WebGLRenderer({ canvas, context });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5));
  const scene = new Scene();
  scene.background = new Color(0x101725);
  const camera = new PerspectiveCamera(37, 1, 0.1, 100);
  camera.position.set(3.8, 3.1, 8.4);
  camera.lookAt(0, 0, 0);

  const knot = new Mesh(
    new TorusKnotGeometry(1.08, 0.31, 160, 24),
    new MeshStandardMaterial({ color: 0xa8f765, roughness: 0.37, metalness: 0.28 }),
  );
  knot.rotation.x = 0.38;
  scene.add(knot, new AmbientLight(0xc4d5ff, 1.1));
  addLight(0xecffe0, 3.1, 3, 6, 4);
  addLight(0x739bff, 3.7, -4, -1, 2);
  addLight(0xcd8fff, 2.9, -2, 3, -5);

  const rings = new Group();
  rings.add(orbit(2.08, 0x617bb0), orbit(2.40, 0x7663a3));
  rings.rotation.set(0.68, 0.3, 0.08);
  scene.add(rings);

  let shaderError;
  renderer.debug.onShaderError = (gl, program, vertex, fragment) => {
    shaderError = [gl.getProgramInfoLog(program), gl.getShaderInfoLog(vertex), gl.getShaderInfoLog(fragment)]
      .find((message) => message?.trim()) || "Could not compile the Three.js shader.";
  };
  const draw = () => renderer.render(scene, camera);

  function resize() {
    const { width, height } = stage.getBoundingClientRect();
    if (!width || !height) return;
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    renderer.setSize(width, height, false);
    draw();
  }

  resize();
  await new Promise(requestAnimationFrame);
  const size = 96;
  const probe = new WebGLRenderTarget(size, size);
  const background = new Uint8Array(size * size * 4);
  const pixels = new Uint8Array(size * size * 4);
  try {
    renderer.setRenderTarget(probe);
    knot.visible = rings.visible = false;
    draw();
    renderer.readRenderTargetPixels(probe, 0, 0, size, size, background);
    knot.visible = rings.visible = true;
    draw();
    renderer.readRenderTargetPixels(probe, 0, 0, size, size, pixels);
  } finally {
    renderer.setRenderTarget(null);
    probe.dispose();
  }
  const objectVisible = pixels.some((value, index) =>
    index % 4 !== 3 && Math.abs(value - background[index]) > 24
  );
  if (shaderError) throw new Error(shaderError);
  if (!objectVisible) {
    throw new Error("Could not verify the rendered Three.js frame.");
  }
  draw();
  status.textContent = `Three.js r${REVISION} · First frame verified`;
  status.dataset.state = "ready";
  document.title = `Proton.js · Three.js r${REVISION} · WebGL 2 ready`;

  let frame;
  let previous;
  function animate(now) {
    if (previous !== undefined) {
      const elapsed = Math.min((now - previous) / 1000, 0.05);
      knot.rotation.y += elapsed * 0.54;
      knot.rotation.z += elapsed * 0.19;
      rings.rotation.z -= elapsed * 0.08;
    }
    previous = now;
    draw();
    frame = requestAnimationFrame(animate);
  }

  toggle.disabled = false;
  toggle.addEventListener("click", () => {
    if (frame === undefined) {
      frame = requestAnimationFrame(animate);
      toggle.textContent = "Pause";
    } else {
      cancelAnimationFrame(frame);
      frame = previous = undefined;
      toggle.textContent = "Play";
    }
  });
  window.addEventListener("resize", resize);
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    toggle.textContent = "Play";
  } else {
    frame = requestAnimationFrame(animate);
  }

  function addLight(color, intensity, x, y, z) {
    const light = new DirectionalLight(color, intensity);
    light.position.set(x, y, z);
    scene.add(light);
  }
}

function orbit(radius, color) {
  const segments = 128;
  const points = Array.from({ length: segments }, (_, index) => {
    const angle = index * 2 * Math.PI / segments;
    return new Vector3(Math.cos(angle) * radius, Math.sin(angle) * radius, 0);
  });
  return new LineLoop(
    new BufferGeometry().setFromPoints(points),
    new LineBasicMaterial({ color }),
  );
}
