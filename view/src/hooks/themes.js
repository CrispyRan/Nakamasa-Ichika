import { computed } from 'vue'
import useAppStore from '@/store/modules/app'

/**
 * 主题 Hook
 */
export default function useThemes() {
  const appStore = useAppStore()
  
  const isDark = computed(() => appStore.mode === 'dark')
  
  return {
    isDark
  }
}
