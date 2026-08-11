import { describe, expect, it } from 'vitest'
import {
  fitImageWithin,
  identityPerspectiveQuad,
  isValidPerspectiveQuad,
  movePerspectiveCorner,
  moveCropRegion,
  resizeCropRegion,
  type CropRegion,
  type CropResizeHandle,
} from './cropGeometry'

const region: CropRegion = { x: 0.2, y: 0.3, width: 0.4, height: 0.3 }

describe('cropGeometry', () => {
  it('moves a normalized region without changing its size or leaving the source', () => {
    expect(moveCropRegion(region, -1, 1)).toEqual({ x: 0, y: 0.7, width: 0.4, height: 0.3 })
    expect(moveCropRegion(region, 0.15, -0.1)).toEqual({ x: 0.35, y: 0.2, width: 0.4, height: 0.3 })
    expect(moveCropRegion(region, Number.NaN, 0)).toEqual(region)
  })

  it.each<[CropResizeHandle, number, number, CropRegion]>([
    ['n', 0, -0.1, { x: 0.2, y: 0.2, width: 0.4, height: 0.4 }],
    ['ne', 0.1, -0.1, { x: 0.2, y: 0.2, width: 0.5, height: 0.4 }],
    ['e', 0.1, 0, { x: 0.2, y: 0.3, width: 0.5, height: 0.3 }],
    ['se', 0.1, 0.1, { x: 0.2, y: 0.3, width: 0.5, height: 0.4 }],
    ['s', 0, 0.1, { x: 0.2, y: 0.3, width: 0.4, height: 0.4 }],
    ['sw', -0.1, 0.1, { x: 0.1, y: 0.3, width: 0.5, height: 0.4 }],
    ['w', -0.1, 0, { x: 0.1, y: 0.3, width: 0.5, height: 0.3 }],
    ['nw', -0.1, -0.1, { x: 0.1, y: 0.2, width: 0.5, height: 0.4 }],
  ])('resizes from the %s handle', (handle, dx, dy, expected) => {
    expect(resizeCropRegion(region, handle, dx, dy)).toEqual(expected)
  })

  it('clamps resize edges and preserves a usable minimum region', () => {
    expect(resizeCropRegion(region, 'nw', 1, 1, 0.05)).toEqual({
      x: 0.55,
      y: 0.55,
      width: 0.05,
      height: 0.05,
    })
    expect(resizeCropRegion(region, 'se', 1, 1)).toEqual({ x: 0.2, y: 0.3, width: 0.8, height: 0.7 })
    expect(resizeCropRegion(region, 'e', Number.POSITIVE_INFINITY, 0)).toEqual(region)
  })

  it('fits portrait and landscape images into a viewport without changing aspect ratio', () => {
    expect(fitImageWithin(4000, 2000, 900, 700)).toEqual({ width: 900, height: 450 })
    expect(fitImageWithin(2000, 4000, 900, 700)).toEqual({ width: 350, height: 700 })
    expect(fitImageWithin(3000, 2000, 2000, 2000, 1200)).toEqual({ width: 1200, height: 800 })
    expect(fitImageWithin(0, 2000, 900, 700)).toEqual({ width: 1, height: 1 })
  })

  it('moves perspective corners inside a convex clockwise quadrilateral', () => {
    const identity = identityPerspectiveQuad()
    expect(movePerspectiveCorner(identity, 'topLeft', 0.12, 0.08)).toEqual({
      ...identity,
      topLeft: { x: 0.12, y: 0.08 },
    })
    expect(movePerspectiveCorner(identity, 'topRight', 2, -2).topRight).toEqual({ x: 1, y: 0 })
  })

  it('rejects crossed and undersized perspective quadrilaterals', () => {
    expect(isValidPerspectiveQuad({
      topLeft: { x: 0.1, y: 0.1 }, topRight: { x: 0.9, y: 0.9 },
      bottomRight: { x: 0.9, y: 0.1 }, bottomLeft: { x: 0.1, y: 0.9 },
    })).toBe(false)
    expect(isValidPerspectiveQuad({
      topLeft: { x: 0.1, y: 0.1 }, topRight: { x: 0.15, y: 0.1 },
      bottomRight: { x: 0.15, y: 0.15 }, bottomLeft: { x: 0.1, y: 0.15 },
    })).toBe(false)
  })
})
