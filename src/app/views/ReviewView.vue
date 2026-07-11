<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import ReviewRoom from '@/modules/review/components/ReviewRoom.vue'

const router = useRouter()
const current = ref(1)
</script>

<template>
  <ReviewRoom
    subject="数学"
    prompt="已知函数 f(x) 为奇函数，求 f(0)。"
    answer="由奇函数定义可知 f(0) = 0。"
    :current="current"
    :total="18"
    @exit="router.push({ name: 'dashboard' })"
    @rate="current = Math.min(18, current + 1)"
  />
</template>
