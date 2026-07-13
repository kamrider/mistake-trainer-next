import {
  createRouter,
  createWebHashHistory,
  type RouterHistory,
} from 'vue-router'
import DashboardView from './views/DashboardView.vue'
import CaptureView from './views/CaptureView.vue'
import LibraryView from './views/LibraryView.vue'
import ReviewView from './views/ReviewView.vue'

export function createAppRouter(history: RouterHistory = createWebHashHistory()) {
  return createRouter({
    history,
    routes: [
      { path: '/', name: 'dashboard', component: DashboardView },
      {
        path: '/inbox',
        name: 'inbox',
        component: CaptureView,
      },
      {
        path: '/library',
        name: 'library',
        component: LibraryView,
      },
      { path: '/review', name: 'review', component: ReviewView },
      {
        path: '/report',
        name: 'report',
        component: () => import('./views/ReportView.vue'),
      },
      {
        path: '/settings',
        name: 'settings',
        component: () => import('./views/SettingsView.vue'),
      },
    ],
  })
}

export const router = createAppRouter()
