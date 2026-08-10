import { ref } from 'vue'

/**
 * 加载状态 Hook
 */
export default function useLoading(initLoading = false) {
  const loading = ref(initLoading)
  
  const setLoading = (value) => {
    loading.value = value
  }
  
  return {
    loading,
    setLoading
  }
}
