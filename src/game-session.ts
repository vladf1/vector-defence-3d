import { AUDIO_CUE_ORDER, AudioCue } from "./audio-manifest";
import { loadEngine, type WebGame } from "./engine";
import { type GameProfile } from "./game-profile";
import { GameAudio } from "./game-audio";
import { takeGpuDevice } from "./gpu-device";
import { INITIAL_HUD_SNAPSHOT, INITIAL_RUNTIME_HUD_STATS, type RuntimeHudStats } from "./initial-hud";
import { RendererStatus } from "./renderer-status";
import { type HudSnapshot, type ModalAction, type ModalView, type Point, type TowerCatalogEntry, type TowerKind } from "./types";
import { get, readonly, writable } from "svelte/store";

const NERD_STATS_SAMPLE_MS = 500;
const TOWER_DRAG_THRESHOLD_PX = 6;
const KEYBOARD_INPUT_SELECTOR = "input, select, textarea";
const KEYBOARD_ACTIVATION_SELECTOR = "a[href], button, summary, [role='button'], [role='link']";
// Board camera controls (desktop): dragging pans, the wheel or a pinch zooms toward the
// cursor, Shift+wheel and the up/down arrows tilt (up leans toward the horizon), = and -
// zoom around the center, and 0 resets the view.
const ZOOM_PER_WHEEL_PIXEL = 0.0015;
const ZOOM_PER_PINCH_PIXEL = 0.01;
const ZOOM_PER_KEY_PRESS = 1.25;
const TILT_RADIANS_PER_WHEEL_PIXEL = 0.0006;
const TILT_RADIANS_PER_KEY_PRESS = 0.04;
const WHEEL_LINE_PIXELS = 16;
const BOARD_PAN_THRESHOLD_PX = 6;
const MIDDLE_BUTTON = 1;
const RIGHT_BUTTON = 2;

type ViewKeyAction =
  | { readonly kind: "tilt"; readonly radians: number }
  | { readonly kind: "zoom"; readonly factor: number }
  | { readonly kind: "reset" };

interface CanvasGeometry {
  rect: DOMRect;
}

/** Dev-only `?shaderSalt=N` defeats GPU shader caches so benchmarks can measure first-visit compiles. */
const SHADER_SALT = import.meta.env.DEV ? Number(new URLSearchParams(window.location.search).get("shaderSalt") ?? 0) : 0;

export interface BoardSurface {
  canvas: HTMLCanvasElement;
  overlay: HTMLCanvasElement;
}

function eventPathMatches(event: KeyboardEvent, selector: string): boolean {
  return event.composedPath().some((target) => target instanceof HTMLElement && target.matches(selector));
}

function isTextEntryEvent(event: KeyboardEvent): boolean {
  return eventPathMatches(event, KEYBOARD_INPUT_SELECTOR)
    || event.composedPath().some((target) => target instanceof HTMLElement && target.isContentEditable);
}

function shouldIgnoreGameShortcut(event: KeyboardEvent): boolean {
  if (
    event.defaultPrevented
    || event.repeat
    || event.isComposing
    || event.altKey
    || event.ctrlKey
    || event.metaKey
    || event.shiftKey
  ) {
    return true;
  }

  if (isTextEntryEvent(event)) {
    return true;
  }

  const isNativeActivationKey = event.code === "Space" || event.key === "Enter";
  return isNativeActivationKey && eventPathMatches(event, KEYBOARD_ACTIVATION_SELECTOR);
}

/** View keys repeat while held and accept Shift (so `+` works). */
function shouldIgnoreViewKey(event: KeyboardEvent): boolean {
  return event.defaultPrevented
    || event.isComposing
    || event.altKey
    || event.ctrlKey
    || event.metaKey
    || isTextEntryEvent(event);
}

function getViewKeyAction(key: string): ViewKeyAction | null {
  switch (key) {
    case "ArrowUp":
      return { kind: "tilt", radians: TILT_RADIANS_PER_KEY_PRESS };
    case "ArrowDown":
      return { kind: "tilt", radians: -TILT_RADIANS_PER_KEY_PRESS };
    case "=":
    case "+":
      return { kind: "zoom", factor: ZOOM_PER_KEY_PRESS };
    case "-":
    case "_":
      return { kind: "zoom", factor: 1 / ZOOM_PER_KEY_PRESS };
    case "0":
      return { kind: "reset" };
    default:
      return null;
  }
}

function clientToField(currentGame: WebGame, clientX: number, clientY: number, rect: DOMRect): Point | null {
  const point = currentGame.clientToField(clientX, clientY, rect.left, rect.top, rect.width, rect.height);
  return point ? { x: point[0], y: point[1] } : null;
}

function setPointer(currentGame: WebGame, point: Point | null): void {
  if (point) {
    currentGame.setPointer(point.x, point.y);
  } else {
    currentGame.clearPointer();
  }
}

function currentPointer(currentGame: WebGame): Point | null {
  const point = currentGame.pointer();
  return point ? { x: point[0], y: point[1] } : null;
}

export function createGameSession(profile: GameProfile) {
  const hudStore = writable(INITIAL_HUD_SNAPSHOT);
  const modalStore = writable<ModalView | null>(null);
  const soundEnabledStore = writable(true);
  const rendererStatusStore = writable<RendererStatus>(RendererStatus.Loading);
  const startupTimingsStore = writable<Record<string, number> | null>(null);
  const towerCatalogStore = writable<readonly TowerCatalogEntry[]>([]);
  const audio = new GameAudio(profile.fieldWidth);
  let canvas: HTMLCanvasElement | null = null;
  let game: WebGame | null = null;
  let destroyed = false;
  let mountToken = 0;
  let boardReady = false;
  let windowListenersAttached = false;
  let soundEnabled = true;
  let frameId = 0;
  let previousFrameTime = 0;
  let pendingSimulationSeconds = 0;
  let runtimeStats: RuntimeHudStats = { ...INITIAL_RUNTIME_HUD_STATS };
  let sampledFrameCount = 0;
  let sampledFrameDurationMs = 0;
  let sampledUpdateDurationMs = 0;
  let sampledDrawDurationMs = 0;
  let lastNerdStatsSampleTime = 0;
  let nerdStatsEnabled = false;
  let canvasResizeObserver: ResizeObserver | null = null;
  let canvasGeometry: CanvasGeometry | null = null;
  let lastPointerClient: { x: number; y: number } | null = null;
  let boardPan:
    | {
      pointerId: number;
      startClientX: number;
      startClientY: number;
      lastClientX: number;
      lastClientY: number;
      active: boolean;
    }
    | null = null;
  let viewRedrawId = 0;
  let towerDrag:
    | {
      kind: TowerKind;
      pointerId: number;
      startClientX: number;
      startClientY: number;
      active: boolean;
    }
    | null = null;

  function resetFrameClock(): void {
    previousFrameTime = 0;
    pendingSimulationSeconds = 0;
  }

  function handleVisibilityChange(): void {
    resetFrameClock();
  }

  function requestGameFrame(): void {
    if (!game?.needsAnimationFrame() || frameId !== 0) {
      return;
    }

    resetFrameClock();
    frameId = window.requestAnimationFrame(frame);
  }

  function syncAnimationLoop(): void {
    if (game?.needsAnimationFrame()) {
      requestGameFrame();
      return;
    }

    if (frameId !== 0) {
      window.cancelAnimationFrame(frameId);
      frameId = 0;
    }
    resetFrameClock();
    game?.draw();
  }

  /** Plays the cues the engine queued (cue index, pan x or NaN, intensity or NaN per entry). */
  const playQueuedSounds = (currentGame: WebGame): void => {
    const sounds = currentGame.takeSounds();
    for (let index = 0; index + 2 < sounds.length; index += 3) {
      const cue = AUDIO_CUE_ORDER[sounds[index]];
      const panX = sounds[index + 1];
      const intensity = sounds[index + 2];
      if (cue) {
        audio.play(cue, {
          panX: Number.isNaN(panX) ? undefined : panX,
          intensity: Number.isNaN(intensity) ? undefined : intensity,
        });
      }
    }
  };

  const publish = (forceHud = false, forceModal = false): void => {
    if (!game) {
      return;
    }

    playQueuedSounds(game);
    const hud = game.takeHud(forceHud, runtimeStats.fps, runtimeStats.frameTimeMs, runtimeStats.updateTimeMs, runtimeStats.drawTimeMs);
    if (hud !== undefined) {
      hudStore.set(JSON.parse(hud) as HudSnapshot);
    }

    const modal = game.takeModal(forceModal);
    if (modal !== undefined) {
      modalStore.set(JSON.parse(modal) as ModalView | null);
    }
  };

  const withGame = (action: (currentGame: WebGame) => void, force = false): void => {
    if (!game) {
      return;
    }

    audio.unlock();
    action(game);
    publish(force, force);
    syncAnimationLoop();
  };

  const refreshCanvasGeometry = (): void => {
    if (!canvas) {
      canvasGeometry = null;
      return;
    }

    canvasGeometry = { rect: canvas.getBoundingClientRect() };
  };

  const toCanvasPoint = (event: PointerEvent): Point | null => {
    const geometry = canvasGeometry;
    if (!geometry || !game) {
      return null;
    }

    return clientToField(game, event.clientX, event.clientY, geometry.rect);
  };

  const isPointerInsideCanvas = (event: PointerEvent): boolean => {
    if (!canvasGeometry) {
      return false;
    }

    const { rect } = canvasGeometry;
    return event.clientX >= rect.left
      && event.clientX <= rect.right
      && event.clientY >= rect.top
      && event.clientY <= rect.bottom;
  };

  const resetNerdStatsSamples = (): void => {
    sampledFrameCount = 0;
    sampledFrameDurationMs = 0;
    sampledUpdateDurationMs = 0;
    sampledDrawDurationMs = 0;
    lastNerdStatsSampleTime = 0;
  };

  function frame(timestamp: number): void {
    frameId = 0;
    const activeGame = game;
    // Hold the simulation while no board is visible (async 3D load) so play never runs unseen;
    // attaching the renderer restarts the loop.
    if (!activeGame?.needsAnimationFrame() || !boardReady) {
      resetFrameClock();
      return;
    }

    const hasPreviousFrame = previousFrameTime !== 0;
    const elapsedSeconds = hasPreviousFrame ? (timestamp - previousFrameTime) / 1000 : 0;

    if (nerdStatsEnabled && hasPreviousFrame) {
      sampledFrameCount += 1;
      sampledFrameDurationMs += timestamp - previousFrameTime;

      if (lastNerdStatsSampleTime === 0) {
        lastNerdStatsSampleTime = timestamp;
      }
    }

    const updateStart = nerdStatsEnabled ? performance.now() : 0;
    if (!hasPreviousFrame) {
      activeGame.updateSimulation(0);
    } else {
      // Bounded substeps (at most 1/60 s each, capped catch-up) run inside the engine.
      const remainingSeconds = activeGame.advance(pendingSimulationSeconds + elapsedSeconds);
      pendingSimulationSeconds = activeGame.needsAnimationFrame() ? remainingSeconds : 0;
    }
    const drawStart = nerdStatsEnabled ? performance.now() : 0;
    activeGame.draw();

    if (nerdStatsEnabled && hasPreviousFrame) {
      sampledUpdateDurationMs += drawStart - updateStart;
      sampledDrawDurationMs += performance.now() - drawStart;

      if (timestamp - lastNerdStatsSampleTime >= NERD_STATS_SAMPLE_MS && sampledFrameDurationMs > 0) {
        runtimeStats = {
          fps: (sampledFrameCount * 1000) / sampledFrameDurationMs,
          frameTimeMs: sampledFrameDurationMs / sampledFrameCount,
          updateTimeMs: sampledUpdateDurationMs / sampledFrameCount,
          drawTimeMs: sampledDrawDurationMs / sampledFrameCount,
        };
        sampledFrameCount = 0;
        sampledFrameDurationMs = 0;
        sampledUpdateDurationMs = 0;
        sampledDrawDurationMs = 0;
        lastNerdStatsSampleTime = timestamp;
        activeGame.requestHudSync();
      }
    }
    publish();
    previousFrameTime = timestamp;
    if (activeGame.needsAnimationFrame()) {
      frameId = window.requestAnimationFrame(frame);
    } else {
      resetFrameClock();
    }
  }

  const ensureGame = async (): Promise<WebGame | null> => {
    const engine = await loadEngine();
    if (game || destroyed) {
      return game;
    }

    game = new engine.WebGame(profile.mode === "mobile", Math.random() * 2 ** 32);
    towerCatalogStore.set(JSON.parse(game.towerCatalog()) as TowerCatalogEntry[]);
    if (import.meta.env.DEV) {
      // Dev-only handle for render/benchmark scripts: mutate the game, then call sync().
      (window as unknown as { __vectorDefence?: unknown }).__vectorDefence = {
        game,
        sync: () => withGame(() => {}, true),
      };
    }
    runtimeStats = { ...INITIAL_RUNTIME_HUD_STATS };
    resetNerdStatsSamples();
    publish(true, true);
    return game;
  };

  const attachWindowListeners = (): void => {
    if (windowListenersAttached) {
      return;
    }

    windowListenersAttached = true;
    window.addEventListener("resize", refreshCanvasGeometry);
    window.addEventListener("scroll", refreshCanvasGeometry, true);
    window.visualViewport?.addEventListener("resize", refreshCanvasGeometry);
    window.visualViewport?.addEventListener("scroll", refreshCanvasGeometry);
    document.addEventListener("visibilitychange", handleVisibilityChange);
  };

  const detachWindowListeners = (): void => {
    if (!windowListenersAttached) {
      return;
    }

    windowListenersAttached = false;
    window.removeEventListener("resize", refreshCanvasGeometry);
    window.removeEventListener("scroll", refreshCanvasGeometry, true);
    window.visualViewport?.removeEventListener("resize", refreshCanvasGeometry);
    window.visualViewport?.removeEventListener("scroll", refreshCanvasGeometry);
    document.removeEventListener("visibilitychange", handleVisibilityChange);
  };

  const attachRenderer = (activeGame: WebGame, renderer: import("./engine").BoardRenderer): void => {
    activeGame.attachRenderer(renderer);
    boardReady = true;
    refreshCanvasGeometry();
    activeGame.draw();
    rendererStatusStore.set(RendererStatus.Ready);
    publish(true, false);
    resetFrameClock();
    requestGameFrame();
  };

  const reportRendererFailure = (error: unknown): void => {
    console.error("WebGPU board renderer unavailable.", error);
    rendererStatusStore.set(RendererStatus.Failed);
  };

  const mountBoardRenderer = (activeGame: WebGame, surface: BoardSurface, token: number): void => {
    rendererStatusStore.set(RendererStatus.Loading);
    const startedAt = performance.now();
    let deviceMs = 0;
    void Promise.all([loadEngine(), takeGpuDevice()])
      .then(([engine, device]) => {
        deviceMs = performance.now() - startedAt;
        void device.lost.then(() => {
          if (token === mountToken && game === activeGame) {
            // A lost device (driver reset, GPU process crash) gets a fresh renderer.
            boardReady = false;
            activeGame.detachRenderer();
            mountBoardRenderer(activeGame, surface, token);
          }
        });
        return engine.createBoardRenderer(device, surface.canvas, surface.overlay, profile.mode === "mobile", SHADER_SALT);
      })
      .then((renderer) => {
        if (token !== mountToken || game !== activeGame) {
          renderer.free();
          return;
        }
        const timings = { deviceMs, ...(JSON.parse(renderer.startupTimings()) as Record<string, number>), pageReadyMs: performance.now() };
        startupTimingsStore.set(timings);
        if (import.meta.env.DEV) {
          console.info("3D board startup (ms)", timings);
          (window as unknown as { __vectorDefenceStartup?: unknown }).__vectorDefenceStartup = timings;
        }
        attachRenderer(activeGame, renderer);
      })
      .catch((error: unknown) => {
        if (token === mountToken) {
          reportRendererFailure(error);
        }
      });
  };

  const mount = (surface: BoardSurface): void => {
    unmount();

    const token = mountToken;
    canvas = surface.overlay;
    refreshCanvasGeometry();
    canvasResizeObserver = new ResizeObserver(() => {
      if (!game) {
        return;
      }

      game.resize();
      refreshCanvasGeometry();
      game.draw();
    });
    canvasResizeObserver.observe(canvas);
    if (profile.ui.allowViewControls) {
      canvas.addEventListener("wheel", handleBoardWheel, { passive: false });
    }
    attachWindowListeners();

    void ensureGame()
      .then((activeGame) => {
        if (!activeGame || token !== mountToken) {
          return;
        }
        mountBoardRenderer(activeGame, surface, token);
        resetFrameClock();
        requestGameFrame();
      })
      .catch((error: unknown) => {
        if (token === mountToken) {
          reportRendererFailure(error);
        }
      });
  };

  const unmount = (): void => {
    mountToken += 1;
    boardReady = false;
    endTowerDrag();
    canvasResizeObserver?.disconnect();
    canvasResizeObserver = null;
    canvas?.removeEventListener("wheel", handleBoardWheel);
    endBoardPan();
    lastPointerClient = null;
    game?.clearPointer();
    game?.detachRenderer();
    canvasGeometry = null;
    canvas = null;
  };

  const destroy = (): void => {
    unmount();

    if (frameId !== 0) {
      window.cancelAnimationFrame(frameId);
      frameId = 0;
    }
    if (viewRedrawId !== 0) {
      window.cancelAnimationFrame(viewRedrawId);
      viewRedrawId = 0;
    }

    detachWindowListeners();
    resetFrameClock();
    runtimeStats = { ...INITIAL_RUNTIME_HUD_STATS };
    resetNerdStatsSamples();
    game?.free();
    game = null;
    destroyed = true;
  };

  const setNerdStatsEnabled = (enabled: boolean): void => {
    nerdStatsEnabled = enabled;

    if (!enabled) {
      runtimeStats = { ...INITIAL_RUNTIME_HUD_STATS };
      resetNerdStatsSamples();
      publish(true, false);
      return;
    }

    resetNerdStatsSamples();
  };

  const toggleSound = (): void => {
    if (soundEnabled) {
      audio.play(AudioCue.SoundToggle);
    }
    soundEnabled = audio.toggle();
    if (soundEnabled) {
      audio.unlock();
      audio.play(AudioCue.SoundToggle);
    }
    soundEnabledStore.set(soundEnabled);
  };

  const togglePause = (): void => {
    withGame((currentGame) => {
      currentGame.togglePause();
      if (!currentGame.canPerformBattleAction()) {
        endTowerDrag();
      }
    }, true);
  };

  const skipBreak = (): void => {
    withGame((currentGame) => {
      currentGame.skipBuildBreak();
    });
  };

  const openMenu = (): void => {
    withGame((currentGame) => {
      currentGame.openMenu();
    }, true);
  };

  const restart = (): void => {
    withGame((currentGame) => {
      audio.play(AudioCue.UiClick);
      currentGame.restart();
    }, true);
  };

  const upgradeSelectedTower = (): void => {
    withGame((currentGame) => {
      currentGame.upgradeSelectedTower();
    });
  };

  const toggleSelectedLaserLock = (): void => {
    withGame((currentGame) => {
      currentGame.toggleSelectedLaserLock();
    });
  };

  const sellSelectedTower = (): void => {
    withGame((currentGame) => {
      currentGame.sellSelectedTower();
    });
  };

  const cancelBuild = (): void => {
    withGame((currentGame) => {
      currentGame.cancelTowerPlacement();
    });
  };

  const toggleTowerPlacement = (kind: TowerKind): void => {
    withGame((currentGame) => {
      currentGame.toggleTowerPlacement(kind);
    });
  };

  const handleModalAction = (action: ModalAction): void => {
    withGame((currentGame) => {
      audio.play(AudioCue.UiConfirm);
      currentGame.performModalAction(action);
    }, true);
  };

  const selectLevel = (levelIndex: number): void => {
    withGame((currentGame) => {
      currentGame.startLevelByIndex(levelIndex);
    }, true);
  };

  /** After a camera change: keeps the build pointer under the cursor and redraws if idle. */
  const handleViewChanged = (): void => {
    if (!game) {
      return;
    }

    if (lastPointerClient && canvasGeometry) {
      setPointer(game, clientToField(game, lastPointerClient.x, lastPointerClient.y, canvasGeometry.rect));
    }
    if (frameId !== 0 || viewRedrawId !== 0) {
      return;
    }
    // One redraw per display frame, however many drag or wheel events arrive.
    viewRedrawId = window.requestAnimationFrame(() => {
      viewRedrawId = 0;
      if (frameId === 0) {
        game?.draw();
      }
    });
  };

  const changeView = (change: (currentGame: WebGame, rect: DOMRect) => boolean): void => {
    if (!game || !boardReady || !canvasGeometry) {
      return;
    }

    if (change(game, canvasGeometry.rect)) {
      handleViewChanged();
    }
  };

  const performViewKeyAction = (action: ViewKeyAction): void => {
    switch (action.kind) {
      case "tilt":
        changeView((currentGame) => currentGame.tiltBy(action.radians));
        break;
      case "zoom":
        changeView((currentGame, rect) => currentGame.zoomAt(action.factor, rect.left + (rect.width / 2), rect.top + (rect.height / 2), rect.left, rect.top, rect.width, rect.height));
        break;
      case "reset":
        changeView((currentGame) => currentGame.resetView());
        break;
    }
  };

  function handleBoardWheel(event: WheelEvent): void {
    if (event.metaKey) {
      return;
    }

    // Browsers may turn Shift+wheel into horizontal scrolling, so take whichever axis moved.
    const delta = event.shiftKey ? (event.deltaY || event.deltaX) : event.deltaY;
    if (delta === 0 || (!event.shiftKey && Math.abs(event.deltaX) > Math.abs(event.deltaY))) {
      return;
    }

    event.preventDefault();
    refreshCanvasGeometry();
    const scale = event.deltaMode === WheelEvent.DOM_DELTA_LINE
      ? WHEEL_LINE_PIXELS
      : event.deltaMode === WheelEvent.DOM_DELTA_PAGE
        ? (canvasGeometry?.rect.height ?? 0)
        : 1;
    const pixels = delta * scale;
    if (event.shiftKey) {
      changeView((currentGame) => currentGame.tiltBy(-pixels * TILT_RADIANS_PER_WHEEL_PIXEL));
      return;
    }

    // Trackpad pinches arrive as ctrl+wheel with small deltas.
    const zoomPerPixel = event.ctrlKey ? ZOOM_PER_PINCH_PIXEL : ZOOM_PER_WHEEL_PIXEL;
    const { clientX, clientY } = event;
    changeView((currentGame, rect) => currentGame.zoomAt(Math.exp(-pixels * zoomPerPixel), clientX, clientY, rect.left, rect.top, rect.width, rect.height));
  }

  const activateBoardPan = (pointerId: number): void => {
    if (!boardPan) {
      return;
    }

    boardPan.active = true;
    canvas?.setPointerCapture(pointerId);
    canvas?.style.setProperty("cursor", "grabbing");
  };

  function endBoardPan(): void {
    if (boardPan?.active) {
      canvas?.style.removeProperty("cursor");
    }
    boardPan = null;
  }

  /** Pans with a held pointer; returns whether the move belonged to a board pan. */
  const updateBoardPan = (event: PointerEvent): boolean => {
    const pan = boardPan;
    if (!pan || event.pointerId !== pan.pointerId) {
      return false;
    }
    if (event.buttons === 0) {
      endBoardPan();
      return false;
    }
    if (!pan.active) {
      if (Math.hypot(event.clientX - pan.startClientX, event.clientY - pan.startClientY) < BOARD_PAN_THRESHOLD_PX) {
        return false;
      }
      activateBoardPan(event.pointerId);
    }

    // Grab the ground: the point under the cursor at the press stays under the cursor.
    const fromX = pan.lastClientX;
    const fromY = pan.lastClientY;
    pan.lastClientX = event.clientX;
    pan.lastClientY = event.clientY;
    changeView((currentGame, rect) => currentGame.panBetween(fromX, fromY, event.clientX, event.clientY, rect.left, rect.top, rect.width, rect.height));
    return true;
  };

  const startBoardPan = (event: PointerEvent): void => {
    boardPan = {
      pointerId: event.pointerId,
      startClientX: event.clientX,
      startClientY: event.clientY,
      lastClientX: event.clientX,
      lastClientY: event.clientY,
      active: false,
    };
  };

  const handleCanvasMove = (event: PointerEvent): void => {
    lastPointerClient = { x: event.clientX, y: event.clientY };
    if (updateBoardPan(event)) {
      return;
    }

    const point = toCanvasPoint(event);
    if (!game || !point) {
      return;
    }

    setPointer(game, point);
  };

  const handleCanvasDown = (event: PointerEvent): void => {
    const viewControls = profile.ui.allowViewControls;
    if (viewControls && (event.button === MIDDLE_BUTTON || event.button === RIGHT_BUTTON)) {
      event.preventDefault();
      refreshCanvasGeometry();
      startBoardPan(event);
      activateBoardPan(event.pointerId);
      return;
    }

    if (event.button !== 0 && event.pointerType !== "touch") {
      return;
    }

    refreshCanvasGeometry();
    const point = toCanvasPoint(event);
    if (!point) {
      return;
    }

    event.preventDefault();
    // Outside build mode, a left press that keeps moving past the threshold pans the board.
    if (viewControls && game?.placingTower() === undefined) {
      startBoardPan(event);
    }
    if (game) {
      setPointer(game, point);
    }
    withGame((currentGame) => {
      currentGame.handleBoardClick(point.x, point.y);
    });
  };

  const handleCanvasUp = (event: PointerEvent): void => {
    if (boardPan && event.pointerId === boardPan.pointerId) {
      endBoardPan();
    }
  };

  const handleCanvasContextMenu = (event: MouseEvent): void => {
    // Right-drag pans the board, so its release must not open the context menu.
    if (profile.ui.allowViewControls) {
      event.preventDefault();
    }
  };

  const handleCanvasLeave = (): void => {
    lastPointerClient = null;
    game?.clearPointer();
  };

  const endTowerDrag = (): void => {
    window.removeEventListener("pointermove", handleTowerDragMove);
    window.removeEventListener("pointerup", handleTowerDragEnd);
    window.removeEventListener("pointercancel", handleTowerDragCancel);
    towerDrag = null;
  };

  function handleTowerDragMove(event: PointerEvent): void {
    const drag = towerDrag;
    if (!drag || event.pointerId !== drag.pointerId) {
      return;
    }

    if (!game?.canPerformBattleAction()) {
      endTowerDrag();
      return;
    }

    const distance = Math.hypot(event.clientX - drag.startClientX, event.clientY - drag.startClientY);
    if (!drag.active) {
      if (distance < TOWER_DRAG_THRESHOLD_PX) {
        return;
      }

      drag.active = true;
      withGame((currentGame) => {
        currentGame.startTowerPlacement(drag.kind);
      }, true);
    }

    event.preventDefault();
    const point = toCanvasPoint(event);
    if (!game || !point || !isPointerInsideCanvas(event)) {
      game?.clearPointer();
      return;
    }

    setPointer(game, point);
  }

  function handleTowerDragEnd(event: PointerEvent): void {
    const drag = towerDrag;
    if (!drag || event.pointerId !== drag.pointerId) {
      return;
    }

    if (!game?.canPerformBattleAction()) {
      endTowerDrag();
      return;
    }

    const wasActive = drag.active;
    const point = toCanvasPoint(event);
    const isOnCanvas = isPointerInsideCanvas(event);
    const releasePoint = point && isOnCanvas
      ? point
      : currentPointer(game);

    if (wasActive) {
      event.preventDefault();
      if (releasePoint) {
        setPointer(game, releasePoint);
        withGame((currentGame) => {
          if (!currentGame.placeTower(drag.kind, releasePoint.x, releasePoint.y)) {
            currentGame.cancelTowerPlacement();
          }
        });
      } else {
        cancelBuild();
        game.clearPointer();
      }
    }

    endTowerDrag();
  }

  function handleTowerDragCancel(event: PointerEvent): void {
    if (!towerDrag || event.pointerId !== towerDrag.pointerId) {
      return;
    }

    if (towerDrag.active) {
      cancelBuild();
      game?.clearPointer();
    }
    endTowerDrag();
  }

  const handleTowerButtonPointerDown = (kind: TowerKind, event: PointerEvent): void => {
    if (event.button !== 0 && event.pointerType !== "touch") {
      return;
    }

    if (!game?.canPerformBattleAction()) {
      event.preventDefault();
      return;
    }
    if (!game.isTowerAvailable(kind)) {
      event.preventDefault();
      return;
    }

    refreshCanvasGeometry();
    towerDrag = {
      kind,
      pointerId: event.pointerId,
      startClientX: event.clientX,
      startClientY: event.clientY,
      active: false,
    };
    window.addEventListener("pointermove", handleTowerDragMove, { passive: false });
    window.addEventListener("pointerup", handleTowerDragEnd);
    window.addEventListener("pointercancel", handleTowerDragCancel);
  };

  const handleKeyDown = (event: KeyboardEvent): void => {
    // View keys stay with the page (and modal scrolling) while a modal covers the board.
    const viewAction = profile.ui.allowViewControls && get(modalStore) === null ? getViewKeyAction(event.key) : null;
    if (viewAction) {
      if (!shouldIgnoreViewKey(event)) {
        event.preventDefault();
        performViewKeyAction(viewAction);
      }
      return;
    }

    if (shouldIgnoreGameShortcut(event)) {
      return;
    }

    const key = event.key.toLowerCase();

    if (import.meta.env.DEV && key === "j") {
      event.preventDefault();
      withGame((currentGame) => {
        currentGame.finishLevel();
      }, true);
      return;
    }

    if (import.meta.env.DEV && key === "k") {
      event.preventDefault();
      withGame((currentGame) => {
        currentGame.loseLevel();
      }, true);
      return;
    }

    if (import.meta.env.DEV && key === "o") {
      event.preventDefault();
      withGame((currentGame) => {
        currentGame.unlockAllLevelsForDebug();
      }, true);
      return;
    }

    if (event.code === "Space") {
      event.preventDefault();
      togglePause();
      return;
    }

    if (!game?.canPerformBattleAction()) {
      return;
    }

    if (key === "u") {
      event.preventDefault();
      upgradeSelectedTower();
      return;
    }

    if (key === "escape") {
      event.preventDefault();
      cancelBuild();
      return;
    }

    const towerKind = game.towerForShortcut(key) as TowerKind | undefined;
    if (!towerKind) {
      return;
    }

    event.preventDefault();
    toggleTowerPlacement(towerKind);
  };

  return {
    profile,
    hud: readonly(hudStore),
    modal: readonly(modalStore),
    soundEnabled: readonly(soundEnabledStore),
    rendererStatus: readonly(rendererStatusStore),
    startupTimings: readonly(startupTimingsStore),
    towerCatalog: readonly(towerCatalogStore),
    toggleSound,
    setNerdStatsEnabled,
    mount,
    unmount,
    destroy,
    handleKeyDown,
    handleCanvasMove,
    handleCanvasDown,
    handleCanvasUp,
    handleCanvasContextMenu,
    handleCanvasLeave,
    handleTowerButtonPointerDown,
    togglePause,
    skipBreak,
    openMenu,
    restart,
    upgradeSelectedTower,
    toggleSelectedLaserLock,
    sellSelectedTower,
    cancelBuild,
    toggleTowerPlacement,
    handleModalAction,
    selectLevel,
  };
}

export type GameSession = ReturnType<typeof createGameSession>;
