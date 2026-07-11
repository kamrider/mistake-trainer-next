import { createApp } from 'vue'
import { VueQueryPlugin } from '@tanstack/vue-query'
import App from './app/App.vue'
import { router } from './app/router'
import './shared/styles/tokens.css'

createApp(App).use(router).use(VueQueryPlugin).mount('#app')
