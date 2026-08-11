export type MistakeReasonTag = {
  tag: `错因·${string}`
  label: string
  description: string
}

export const MISTAKE_REASON_TAGS = [
  { tag: '错因·概念混淆', label: '概念混淆', description: '定义、性质或适用条件没有分清' },
  { tag: '错因·审题遗漏', label: '审题遗漏', description: '漏看条件、单位、范围或题目要求' },
  { tag: '错因·计算失误', label: '计算失误', description: '方法正确，但运算、符号或抄写出错' },
  { tag: '错因·方法不会', label: '方法不会', description: '没有找到可执行的解题方法' },
  { tag: '错因·步骤不完整', label: '步骤不完整', description: '推理、证明或表达缺少关键步骤' },
  { tag: '错因·时间不足', label: '时间不足', description: '会做但没有在目标时间内完成' },
] as const satisfies readonly MistakeReasonTag[]

const knownReasonTags = new Set<string>(MISTAKE_REASON_TAGS.map(reason => reason.tag))

export function isMistakeReasonTag(tag: string): tag is MistakeReasonTag['tag'] {
  return knownReasonTags.has(tag)
}

export function toggleMistakeReason(tags: string[], tag: string): string[] {
  if (!isMistakeReasonTag(tag)) return [...tags]

  const selectedReasons = new Set(tags.filter(isMistakeReasonTag))
  if (selectedReasons.has(tag)) selectedReasons.delete(tag)
  else selectedReasons.add(tag)

  const freeTags = tags.filter(value => !isMistakeReasonTag(value))
  const orderedReasons = MISTAKE_REASON_TAGS
    .map(reason => reason.tag)
    .filter(value => selectedReasons.has(value))
  return [...freeTags, ...orderedReasons]
}
