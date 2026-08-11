import { describe, expect, it } from 'vitest'
import {
  MISTAKE_REASON_TAGS,
  isMistakeReasonTag,
  toggleMistakeReason,
} from './mistakeReasons'

describe('mistake reason catalog', () => {
  it('keeps a stable, intentional set of canonical reason tags', () => {
    expect(MISTAKE_REASON_TAGS.map(reason => reason.tag)).toEqual([
      '错因·概念混淆',
      '错因·审题遗漏',
      '错因·计算失误',
      '错因·方法不会',
      '错因·步骤不完整',
      '错因·时间不足',
    ])
    expect(MISTAKE_REASON_TAGS.every(reason => reason.label && reason.description)).toBe(true)
  })

  it('recognizes only canonical reason tags', () => {
    expect(isMistakeReasonTag('错因·计算失误')).toBe(true)
    expect(isMistakeReasonTag('错因·其他')).toBe(false)
    expect(isMistakeReasonTag('计算失误')).toBe(false)
  })

  it('adds reasons in catalog order while preserving unrelated tags', () => {
    const original = ['函数', '错因·时间不足']
    const result = toggleMistakeReason(original, '错因·概念混淆')

    expect(result).toEqual(['函数', '错因·概念混淆', '错因·时间不足'])
    expect(original).toEqual(['函数', '错因·时间不足'])
  })

  it('removes an active reason and ignores an unknown reason', () => {
    expect(toggleMistakeReason(
      ['力学', '错因·审题遗漏', '错因·计算失误'],
      '错因·审题遗漏',
    )).toEqual(['力学', '错因·计算失误'])
    expect(toggleMistakeReason(['力学'], '错因·其他')).toEqual(['力学'])
  })
})
