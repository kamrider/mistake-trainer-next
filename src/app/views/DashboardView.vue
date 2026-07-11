<script setup lang="ts">
import { useRouter } from 'vue-router'
import TrainingDashboard from '@/modules/dashboard/components/TrainingDashboard.vue'

const router = useRouter()
</script>

<template>
  <TrainingDashboard
    learner-name="小树"
    :due-count="18"
    :streak-days="7"
    :retention="0.91"
    @start-review="router.push({ name: 'review' })"
    @open-inbox="router.push({ name: 'inbox' })"
  />
</template>
