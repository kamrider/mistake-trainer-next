export interface CropRegion {
  x: number
  y: number
  width: number
  height: number
}

export type CropResizeHandle = 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'nw'

export interface PerspectivePoint {
  x: number
  y: number
}

export interface PerspectiveQuad {
  topLeft: PerspectivePoint
  topRight: PerspectivePoint
  bottomRight: PerspectivePoint
  bottomLeft: PerspectivePoint
}

export type PerspectiveCorner = keyof PerspectiveQuad

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

export function identityPerspectiveQuad(): PerspectiveQuad {
  return {
    topLeft: { x: 0, y: 0 },
    topRight: { x: 1, y: 0 },
    bottomRight: { x: 1, y: 1 },
    bottomLeft: { x: 0, y: 1 },
  }
}

export function clonePerspectiveQuad(quad: PerspectiveQuad): PerspectiveQuad {
  return {
    topLeft: { ...quad.topLeft },
    topRight: { ...quad.topRight },
    bottomRight: { ...quad.bottomRight },
    bottomLeft: { ...quad.bottomLeft },
  }
}

export function isValidPerspectiveQuad(quad: PerspectiveQuad, minimumArea = 0.02) {
  const points = [quad.topLeft, quad.topRight, quad.bottomRight, quad.bottomLeft]
  if (!points.every(point =>
    Number.isFinite(point.x)
    && Number.isFinite(point.y)
    && point.x >= 0
    && point.x <= 1
    && point.y >= 0
    && point.y <= 1,
  )) return false
  for (let index = 0; index < points.length; index += 1) {
    const a = points[index]!
    const b = points[(index + 1) % points.length]!
    const c = points[(index + 2) % points.length]!
    const cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x)
    if (cross <= 1e-6) return false
  }
  const twiceArea = Math.abs(points.reduce((sum, point, index) => {
    const next = points[(index + 1) % points.length]!
    return sum + point.x * next.y - next.x * point.y
  }, 0))
  return twiceArea >= Math.max(0.001, minimumArea * 2)
}

export function movePerspectiveCorner(
  quad: PerspectiveQuad,
  corner: PerspectiveCorner,
  deltaX: number,
  deltaY: number,
): PerspectiveQuad {
  if (!Number.isFinite(deltaX) || !Number.isFinite(deltaY)) return clonePerspectiveQuad(quad)
  const next = clonePerspectiveQuad(quad)
  next[corner] = {
    x: normalized(clamp(next[corner].x + deltaX, 0, 1)),
    y: normalized(clamp(next[corner].y + deltaY, 0, 1)),
  }
  return isValidPerspectiveQuad(next) ? next : clonePerspectiveQuad(quad)
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
