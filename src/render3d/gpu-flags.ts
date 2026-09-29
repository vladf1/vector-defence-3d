// WebGPU usage/visibility bit flags (fixed by the spec). Local copies keep the TypeScript
// DOM lib sufficient and let the minifier inline plain numbers.

export const BufferUsage = {
  COPY_DST: 0x0008,
  INDEX: 0x0010,
  VERTEX: 0x0020,
  UNIFORM: 0x0040,
} as const;

export const TextureUsage = {
  COPY_DST: 0x02,
  TEXTURE_BINDING: 0x04,
  RENDER_ATTACHMENT: 0x10,
} as const;

export const ShaderStage = {
  VERTEX: 0x1,
  FRAGMENT: 0x2,
} as const;
