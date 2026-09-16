import * as THREE from 'three';
import { GLTFLoader, type GLTF } from 'three/addons/loaders/GLTFLoader.js';
import type { Clip } from './playback';
import { createTargets, type TargetLayout } from './targets';

const stillTime: Record<Clip, number> = {
  idle: 0,
  capture: 1.1,
  send: 3.55,
  receive_text: 6.96,
  error: 1.55,
  paused: 1,
  offline: 1.2,
};

function disposeTree(root: THREE.Object3D) {
  const geometries = new Set<THREE.BufferGeometry>();
  const materials = new Set<THREE.Material>();
  const textures = new Set<THREE.Texture>();
  root.traverse((object) => {
    if (object instanceof THREE.Mesh) {
      geometries.add(object.geometry);
      for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
        materials.add(material);
        Object.values(material).forEach((value) => {
          if (value instanceof THREE.Texture) textures.add(value);
        });
      }
    }
  });
  geometries.forEach((value) => value.dispose());
  materials.forEach((value) => value.dispose());
  textures.forEach((value) => value.dispose());
}

export async function createScene(
  element: HTMLDivElement,
  complete: (serial: number) => void,
  signal: AbortSignal,
  layout: (targets: TargetLayout) => void,
) {
  const response = await fetch('/mascot/nooboard.glb', { signal });
  if (!response.ok) throw new Error('Mascot asset unavailable');
  const buffer = await response.arrayBuffer();
  if (signal.aborted) throw new DOMException('Aborted', 'AbortError');
  const gltf = await new GLTFLoader().parseAsync(buffer, '');
  if (signal.aborted) {
    disposeTree(gltf.scene);
    throw new DOMException('Aborted', 'AbortError');
  }
  try {
    return new BirdScene(element, gltf, complete, layout);
  } catch (error) {
    disposeTree(gltf.scene);
    throw error;
  }
}

/** Owns WebGL resources and clip playback, with no knowledge of app data. */
export class BirdScene {
  private renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera = new THREE.OrthographicCamera(-4.1, 4.1, 2.3, -2.3, 0.1, 100);
  private mixer: THREE.AnimationMixer;
  private actions = new Map<string, THREE.AnimationAction>();
  private current?: THREE.AnimationAction;
  private clip: Clip = 'idle';
  private serial: number | null = null;
  private observer: ResizeObserver;
  private raf = 0;
  private last = 0;
  private reduced = false;
  private timer?: ReturnType<typeof setTimeout>;
  private bird: THREE.Object3D | undefined;
  private position = new THREE.Vector3();
  private floor: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>;
  private birdShadow: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>;
  private targets: ReturnType<typeof createTargets>;

  constructor(
    private element: HTMLDivElement,
    private gltf: GLTF,
    private complete: (serial: number) => void,
    private layout: (targets: TargetLayout) => void,
  ) {
    this.renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: true,
      powerPreference: 'low-power',
    });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 1.75));
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.05;
    this.renderer.domElement.setAttribute('aria-hidden', 'true');
    element.append(this.renderer.domElement);
    this.scene.add(gltf.scene);
    this.mixer = new THREE.AnimationMixer(gltf.scene);
    gltf.animations.forEach((clip) => this.actions.set(clip.name, this.mixer.clipAction(clip)));
    this.actions.get('idle')?.play();
    this.mixer.update(0);
    gltf.scene.updateMatrixWorld(true);
    this.targets = createTargets(gltf.scene);
    this.mixer.addEventListener('finished', this.finished);
    this.camera.position.set(3, 5, 12.6);
    this.camera.lookAt(0, 1.25, 0.1);
    this.scene.add(new THREE.HemisphereLight(0xfff1db, 0xa4aba1, 2.2));
    [
      [0xfff0d7, 2.5, -3, 7, 6],
      [0xe3efff, 1.1, 5, 4, -2],
      [0xfff7e8, 1.2, 0, 5, -5],
    ].forEach(([color, intensity, x, y, z]) => {
      const light = new THREE.DirectionalLight(color, intensity);
      light.position.set(x, y, z);
      this.scene.add(light);
    });
    this.floor = new THREE.Mesh(
      new THREE.PlaneGeometry(200, 200),
      new THREE.MeshBasicMaterial({ toneMapped: false }),
    );
    this.floor.rotation.x = -Math.PI / 2;
    this.floor.position.y = 0.014;
    this.scene.add(this.floor);
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 128;
    const context = canvas.getContext('2d')!;
    const gradient = context.createRadialGradient(64, 64, 5, 64, 64, 64);
    gradient.addColorStop(0, 'rgba(0,0,0,.27)');
    gradient.addColorStop(0.45, 'rgba(0,0,0,.12)');
    gradient.addColorStop(1, 'rgba(0,0,0,0)');
    context.fillStyle = gradient;
    context.fillRect(0, 0, 128, 128);
    const texture = new THREE.CanvasTexture(canvas);
    const shadow = (w: number, h: number) => {
      const mesh = new THREE.Mesh(
        new THREE.PlaneGeometry(w, h),
        new THREE.MeshBasicMaterial({
          map: texture,
          transparent: true,
          depthWrite: false,
          toneMapped: false,
        }),
      );
      mesh.rotation.x = -Math.PI / 2;
      mesh.position.y = 0.022;
      this.scene.add(mesh);
      return mesh;
    };
    this.birdShadow = shadow(3.2, 2.5);
    shadow(2.2, 1.8).position.set(2.25, 0.022, -0.1);
    this.bird = gltf.scene.getObjectByName('Bird_Root');
    this.observer = new ResizeObserver(() => this.resize());
    this.observer.observe(element);
    this.theme();
    this.resize();
    this.play('idle', null);
  }
  private finished = (event: THREE.AnimationMixerEventMap['finished']) => {
    if (event.action !== this.current) return;
    if (this.serial !== null) {
      const serial = this.serial;
      this.serial = null;
      this.complete(serial);
    } else if (this.clip === 'paused') this.sleep();
  };
  theme() {
    const color = new THREE.Color(getComputedStyle(this.element).backgroundColor);
    this.scene.background = color;
    this.floor.material.color.copy(color);
    this.render();
  }
  setReduced(value: boolean) {
    if (this.reduced !== value) {
      this.reduced = value;
      this.play(this.clip, this.serial);
    }
  }
  private resize() {
    const { clientWidth: width, clientHeight: height } = this.element;
    if (!width || !height) return;
    const aspect = width / height;
    const half = Math.max(4.1, 1.5 * aspect);
    this.camera.left = -half;
    this.camera.right = half;
    this.camera.top = half / aspect;
    this.camera.bottom = -half / aspect;
    this.camera.updateProjectionMatrix();
    this.layout(this.targets(this.camera));
    this.renderer.setSize(width, height, false);
    this.render();
  }
  private render() {
    if (this.bird) {
      this.scene.updateMatrixWorld(true);
      this.bird.getWorldPosition(this.position);
      const height = Math.max(0, this.position.y);
      this.birdShadow.position.set(this.position.x, 0.023, this.position.z);
      this.birdShadow.scale.setScalar(1 + height * 0.25);
      this.birdShadow.material.opacity = 1 / (1 + height * 2.5);
    }
    this.renderer.render(this.scene, this.camera);
  }
  play(clip: Clip, serial: number | null) {
    clearTimeout(this.timer);
    this.clip = clip;
    this.serial = serial;
    const action = this.actions.get(clip);
    if (!action) return;
    const previous = this.current;
    if (this.reduced) this.mixer.stopAllAction();
    else if (previous && previous !== action) previous.fadeOut(0.16);
    action.reset().setEffectiveWeight(1).setEffectiveTimeScale(1);
    action.setLoop(
      clip === 'idle' || clip === 'offline' ? THREE.LoopRepeat : THREE.LoopOnce,
      Infinity,
    );
    action.clampWhenFinished = true;
    action.play();
    if (previous && previous !== action && !this.reduced) action.fadeIn(0.16);
    this.current = action;
    if (this.reduced) {
      this.sleep();
      action.time = stillTime[clip];
      this.mixer.update(0);
      this.render();
      if (serial !== null)
        this.timer = setTimeout(() => {
          if (this.serial === serial) {
            this.serial = null;
            this.complete(serial);
          }
        }, 450);
    } else this.wake();
  }
  wake() {
    if (this.raf || document.hidden || this.reduced) return;
    this.last = performance.now();
    this.raf = requestAnimationFrame(this.tick);
  }
  sleep() {
    cancelAnimationFrame(this.raf);
    this.raf = 0;
  }
  private tick = (time: number) => {
    this.raf = 0;
    const delta = (time - this.last) / 1000;
    if (delta >= 1 / 30) {
      this.last = time;
      this.mixer.update(Math.min(delta, 0.1));
      this.render();
    }
    if (
      !this.raf &&
      !document.hidden &&
      !this.reduced &&
      !(this.clip === 'paused' && this.current?.paused)
    )
      this.raf = requestAnimationFrame(this.tick);
  };
  dispose() {
    clearTimeout(this.timer);
    this.sleep();
    this.observer.disconnect();
    this.mixer.removeEventListener('finished', this.finished);
    this.mixer.stopAllAction();
    this.mixer.uncacheRoot(this.gltf.scene);
    disposeTree(this.scene);
    this.renderer.dispose();
    this.renderer.forceContextLoss();
    this.renderer.domElement.remove();
  }
}
