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
        component: () => import('./views/PlaceholderView.vue'),
        props: {
          eyebrow: '学习报告',
          title: '学习报告',
          description: '从节奏和记住率中看见真实进步，而不是制造焦虑。',
        },
      },
      {
        path: '/settings',
        name: 'settings',
        component: () => import('./views/PlaceholderView.vue'),
        props: {
          eyebrow: '设置',
          title: '设置',
          description: '管理档案、存储、同步、备份与可信设备。',
        },
      },
    ],
  })
}

export const router = createAppRouter()
