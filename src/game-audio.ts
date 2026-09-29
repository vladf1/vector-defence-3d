import { AudioCue, type AudioCue as AudioCueValue } from "./audio-manifest";
import { FIELD_WIDTH } from "./constants";
import { clamp } from "./utils";

interface AudioCueOptions {
  intensity?: number;
  panX?: number;
}

type AudioContextConstructor = new () => AudioContext;

interface AudioWindow extends Window {
  webkitAudioContext?: AudioContextConstructor;
}

const MASTER_VOLUME = 0.34;
const DEFAULT_INTENSITY = 1;

export class GameAudio {
  private context?: AudioContext;
  private masterGain?: GainNode;
  private output?: AudioNode;
  private recentCueTimes = new Map<AudioCueValue, number>();
  private bufferPromises = new Map<AudioCueValue, Promise<AudioBuffer>>();
  private buffers = new Map<AudioCueValue, AudioBuffer>();
  private pendingPlaybackCues = new Set<AudioCueValue>();
  private reportedLoadFailures = new Set<AudioCueValue>();
  private enabled = true;

  constructor(private readonly fieldWidth = FIELD_WIDTH) {
  }

  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
    if (this.masterGain) {
      this.masterGain.gain.setTargetAtTime(enabled ? MASTER_VOLUME : 0, this.context?.currentTime ?? 0, 0.015);
    }
  }

  toggle(): boolean {
    this.setEnabled(!this.enabled);
    return this.enabled;
  }

  preload(): void {
    const context = this.getContext();
    if (!context) {
      return;
    }

    for (const cue of Object.values(AudioCue)) {
      void this.loadBuffer(cue, context).catch(() => undefined);
    }
  }

  unlock(): void {
    if (!this.enabled) {
      return;
    }

    const context = this.getContext();
    if (!context) {
      return;
    }

    if (context.state === "suspended") {
      void context.resume();
    }
    this.preload();
  }

  play(cue: AudioCueValue, options: AudioCueOptions = {}): void {
    if (!this.enabled) {
      return;
    }

    const context = this.getContext();
    if (!context || !this.output) {
      return;
    }

    if (context.state === "suspended") {
      void context.resume();
    }

    const now = context.currentTime;
    const previousTime = this.recentCueTimes.get(cue) ?? Number.NEGATIVE_INFINITY;
    if (now - previousTime < cue.cooldownSeconds) {
      return;
    }

    const intensity = clamp(options.intensity ?? DEFAULT_INTENSITY, 0.25, 2.2);
    const cachedBuffer = this.buffers.get(cue);
    if (cachedBuffer) {
      this.recentCueTimes.set(cue, now);
      this.playBuffer(cue, cachedBuffer, options.panX, intensity);
      return;
    }

    if (this.pendingPlaybackCues.has(cue)) {
      return;
    }

    this.recentCueTimes.set(cue, now);
    this.pendingPlaybackCues.add(cue);
    void this.loadBuffer(cue, context)
      .then((buffer) => {
        if (this.enabled) {
          this.recentCueTimes.set(cue, context.currentTime);
          this.playBuffer(cue, buffer, options.panX, intensity);
        }
      })
      .catch(() => undefined)
      .finally(() => {
        this.pendingPlaybackCues.delete(cue);
      });
  }

  private getContext(): AudioContext | undefined {
    if (this.context) {
      return this.context;
    }

    const AudioContextClass = window.AudioContext ?? (window as AudioWindow).webkitAudioContext;
    if (!AudioContextClass) {
      return undefined;
    }

    const context = new AudioContextClass();
    const compressor = context.createDynamicsCompressor();
    compressor.threshold.value = -18;
    compressor.knee.value = 20;
    compressor.ratio.value = 7;
    compressor.attack.value = 0.004;
    compressor.release.value = 0.12;

    const masterGain = context.createGain();
    masterGain.gain.value = this.enabled ? MASTER_VOLUME : 0;
    compressor.connect(masterGain);
    masterGain.connect(context.destination);

    this.context = context;
    this.masterGain = masterGain;
    this.output = compressor;
    return context;
  }

  private loadBuffer(cue: AudioCueValue, context: AudioContext): Promise<AudioBuffer> {
    const cachedBuffer = this.buffers.get(cue);
    if (cachedBuffer) {
      return Promise.resolve(cachedBuffer);
    }

    const cachedPromise = this.bufferPromises.get(cue);
    if (cachedPromise) {
      return cachedPromise;
    }

    const bufferPromise = fetch(cue.url)
      .then((response) => {
        if (!response.ok) {
          throw new Error(`Unable to load audio asset ${cue.id}: ${response.status}`);
        }
        return response.arrayBuffer();
      })
      .then((data) => context.decodeAudioData(data))
      .then((buffer) => {
        this.buffers.set(cue, buffer);
        this.bufferPromises.delete(cue);
        return buffer;
      }, (error: unknown) => {
        this.bufferPromises.delete(cue);
        if (!this.reportedLoadFailures.has(cue)) {
          this.reportedLoadFailures.add(cue);
          console.warn(`Unable to load audio asset "${cue.id}". Playback will retry on the next request.`, error);
        }
        throw error;
      });

    this.bufferPromises.set(cue, bufferPromise);
    return bufferPromise;
  }

  private playBuffer(cue: AudioCueValue, buffer: AudioBuffer, panX: number | undefined, intensity: number): void {
    const context = this.context;
    if (!context || !this.output) {
      return;
    }

    const source = context.createBufferSource();
    const gain = context.createGain();
    source.buffer = buffer;
    source.playbackRate.value = 1 + ((Math.random() * 2 - 1) * cue.rateVariation);
    gain.gain.value = clamp(cue.gain * intensity, 0.01, 1.4);

    source.connect(gain);

    let panner: StereoPannerNode | undefined;
    if (panX !== undefined && "createStereoPanner" in context) {
      panner = context.createStereoPanner();
      panner.pan.value = clamp((panX / this.fieldWidth) * 1.5 - 0.75, -0.75, 0.75);
      gain.connect(panner);
      panner.connect(this.output);
    } else {
      gain.connect(this.output);
    }

    let cleanedUp = false;
    const cleanup = (): void => {
      if (cleanedUp) {
        return;
      }
      cleanedUp = true;
      source.onended = null;
      source.disconnect();
      gain.disconnect();
      panner?.disconnect();
    };

    source.onended = cleanup;
    try {
      source.start();
    } catch (error) {
      cleanup();
      throw error;
    }
  }
}
