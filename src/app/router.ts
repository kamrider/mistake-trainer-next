import {
  createRouter,
  createWebHashHistory,
  type RouterHistory,
} from 'vue-router'
import DashboardView from './views/DashboardView.vue'
import ReviewView from './views/ReviewView.vue'

export function createAppRouter(history: RouterHistory = createWebHashHistory()) {
  return createRouter({
    history,
    routes: [
      { path: '/', name: 'dashboard', component: DashboardView },
      {
        path: '/inbox',
        name: 'inbox',
        component: () => import('./views/PlaceholderView.vue'),
        props: {
          eyebrow: '采集整理',
          title: '采集箱',
          description: '题图与答案图会先在这里安全暂存、去重和配对。',
        },
      },
      {
        path: '/library',
        name: 'library',
        component: () => import('./views/PlaceholderView.vue'),
        props: {
          eyebrow: '题库',
          title: '题库',
          description: '搜索、筛选、编辑和归档你的错题。',
        },
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
