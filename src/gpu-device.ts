let prefetched: Promise<GPUDevice> | undefined;

/** Set by the inline script in index.html, which starts the request during HTML parsing. */
interface PageGpuDevice {
  vectorDefenceGpuDevice?: Promise<GPUDevice>;
}

function takePageDevice(): Promise<GPUDevice> | undefined {
  const page = window as PageGpuDevice;
  const device = page.vectorDefenceGpuDevice;
  page.vectorDefenceGpuDevice = undefined;
  return device;
}

async function requestDevice(): Promise<GPUDevice> {
  const gpu = navigator.gpu;
  if (!gpu) {
    throw new Error("WebGPU is not available in this browser.");
  }
  const adapter = await gpu.requestAdapter({ powerPreference: "high-performance" });
  if (!adapter) {
    throw new Error("No WebGPU adapter is available.");
  }
  return adapter.requestDevice();
}

/**
 * Starts acquiring the GPU device at page startup (or adopts the request index.html already
 * started) so the adapter/device round trip overlaps the renderer chunk download. Failures
 * surface when the renderer takes the device.
 */
export function prefetchGpuDevice(): void {
  if (!prefetched) {
    prefetched = takePageDevice() ?? requestDevice();
    prefetched.catch(() => undefined);
  }
}

/** Hands out the prefetched device once; later renderers (remounts, device loss) request anew. */
export function takeGpuDevice(): Promise<GPUDevice> {
  const device = prefetched ?? takePageDevice() ?? requestDevice();
  prefetched = undefined;
  return device;
}
