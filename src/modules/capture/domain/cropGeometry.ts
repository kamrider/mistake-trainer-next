export interface CropRegion {
  x: number
  y: number
  width: number
  height: number
}

export type CropResizeHandle = 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'nw'

const DEFAULT_MINIMUM_SIZE = 0.015

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(maximum, Math.max(minimum, value))
}

function normalized(value: number) {
  return Math.round(value * 1_000_000_000_000) / 1_000_000_000_000
}

function withEdges<T extends CropRegion>(
  region: T,
  left: number,
  top: number,
  right: number,
  bottom: number,
): T {
  return {
    ...region,
    x: normalized(left),
    y: normalized(top),
    width: normalized(right - left),
    height: normalized(bottom - top),
  }
}

export function moveCropRegion<T extends CropRegion>(region: T, deltaX: number, deltaY: number): T {
  if (!Number.isFinite(deltaX) || !Number.isFinite(deltaY)) return { ...region }
  return {
    ...region,
    x: normalized(clamp(region.x + deltaX, 0, 1 - region.width)),
    y: normalized(clamp(region.y + deltaY, 0, 1 - region.height)),
  }
}

export function resizeCropRegion<T extends CropRegion>(
  region: T,
  handle: CropResizeHandle,
  deltaX: number,
  deltaY: number,
  minimumSize = DEFAULT_MINIMUM_SIZE,
): T {
  if (!Number.isFinite(deltaX) || !Number.isFinite(deltaY)) return { ...region }
  const minimum = clamp(Number.isFinite(minimumSize) ? minimumSize : DEFAULT_MINIMUM_SIZE, 0.001, 1)
  let left = region.x
  let top = region.y
  let right = region.x + region.width
  let bottom = region.y + region.height

  if (handle.includes('w')) left = clamp(left + deltaX, 0, right - minimum)
  if (handle.includes('e')) right = clamp(right + deltaX, left + minimum, 1)
  if (handle.includes('n')) top = clamp(top + deltaY, 0, bottom - minimum)
  if (handle.includes('s')) bottom = clamp(bottom + deltaY, top + minimum, 1)

  return withEdges(region, left, top, right, bottom)
}

export function fitImageWithin(
  naturalWidth: number,
  naturalHeight: number,
  viewportWidth: number,
  viewportHeight: number,
  maximumWidth = Number.POSITIVE_INFINITY,
) {
  if (
    ![naturalWidth, naturalHeight, viewportWidth, viewportHeight].every(value => Number.isFinite(value) && value > 0)
  ) return { width: 1, height: 1 }

  const widthLimit = Math.max(1, Math.min(viewportWidth, maximumWidth))
  const heightLimit = Math.max(1, viewportHeight)
  const scale = Math.min(widthLimit / naturalWidth, heightLimit / naturalHeight, 1)
  return {
    width: Math.max(1, Math.round(naturalWidth * scale)),
    height: Math.max(1, Math.round(naturalHeight * scale)),
  }
}
